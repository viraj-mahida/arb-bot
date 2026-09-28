//! Flash loans: borrow at the start of a transaction, repay at the end.
//!
//! **What a flash loan is:** a loan that must be repaid *inside the same
//! transaction*. The lending program checks (through the instructions sysvar)
//! that a matching repay instruction comes later. If the repay fails — say the
//! arbitrage lost money — the whole transaction reverts as if nothing happened,
//! so the lender risks nothing and needs no collateral from us.
//!
//! **Why use one:** the trade size is no longer limited by the wallet balance.
//! The cost is a small fee on the borrowed amount.
//!
//! **Providers:** Jupiter Lend (fee set on-chain, currently 0 bps) and Kamino
//! Lend (fee per reserve). Other lenders would become new variants of
//! [`FlashLoanProvider`] with their own builders.

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::trading_wallet::associated_token_account_address;
use super::well_known_program_addresses::{
    ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS, INSTRUCTIONS_SYSVAR_ADDRESS,
    JUPITER_FLASHLOAN_PROGRAM_ADDRESS, JUPITER_LIQUIDITY_PROGRAM_ADDRESS,
    KAMINO_LEND_PROGRAM_ADDRESS, SYSTEM_PROGRAM_ADDRESS, TOKEN_PROGRAM_ADDRESS,
    WRAPPED_SOL_MINT_ADDRESS, program, pubkey, read_only, read_only_program, writable,
};
use crate::step_3_store_latest_pool_state::{
    PublicKeyBytes, encode_public_key_as_base58, parse_base58_public_key,
};

/// `sha256("global:flash_borrow_reserve_liquidity")[..8]`
pub const KAMINO_FLASH_BORROW_DISCRIMINATOR: [u8; 8] =
    [0x87, 0xe7, 0x34, 0xa7, 0x07, 0x34, 0xd4, 0xc1];
/// `sha256("global:flash_repay_reserve_liquidity")[..8]`
pub const KAMINO_FLASH_REPAY_DISCRIMINATOR: [u8; 8] =
    [0xb9, 0x75, 0x00, 0xcb, 0x60, 0xf5, 0xb4, 0xba];

/// Which lender a flash loan comes from.
#[derive(Debug, Clone)]
pub enum FlashLoanProvider {
    Kamino(KaminoFlashLoanAccounts),
    Jupiter(JupiterFlashLoanAccounts),
}

impl FlashLoanProvider {
    pub fn borrow_instruction(
        &self,
        wallet: &PublicKeyBytes,
        destination: &PublicKeyBytes,
        amount: u64,
    ) -> Instruction {
        match self {
            Self::Kamino(kamino) => kamino.flash_borrow_instruction(wallet, destination, amount),
            Self::Jupiter(jupiter) => {
                jupiter.flash_instruction(JUPITER_BORROW_DISCRIMINATOR, wallet, destination, amount)
            }
        }
    }

    /// `borrow_instruction_index` = position of the borrow instruction in the transaction.
    pub fn repay_instruction(
        &self,
        wallet: &PublicKeyBytes,
        source: &PublicKeyBytes,
        amount: u64,
        borrow_instruction_index: u8,
    ) -> Instruction {
        match self {
            Self::Kamino(kamino) => {
                kamino.flash_repay_instruction(wallet, source, amount, borrow_instruction_index)
            }
            // Jupiter finds the borrow itself; it only needs the amount owed.
            Self::Jupiter(jupiter) => jupiter.flash_instruction(
                JUPITER_PAYBACK_DISCRIMINATOR,
                wallet,
                source,
                jupiter.amount_to_repay(amount),
            ),
        }
    }
}

// ── Jupiter Lend flash loan ───────────────────────────────────────────────
//
// Two top-level instructions on the flash-loan program (no CPI allowed):
//   flashloan_borrow(amount)   — data = discriminator ++ amount (u64 LE)
//   flashloan_payback(amount)  — same layout; amount = borrowed + fee
// The program scans the instructions sysvar for a matching payback.

/// `flashloan_borrow` Anchor discriminator (from the program IDL).
pub const JUPITER_BORROW_DISCRIMINATOR: [u8; 8] = [103, 19, 78, 24, 240, 9, 135, 63];
/// `flashloan_payback` Anchor discriminator (from the program IDL).
pub const JUPITER_PAYBACK_DISCRIMINATOR: [u8; 8] = [213, 47, 153, 137, 84, 243, 94, 232];

// FlashloanAdmin account: 8 discriminator, 32 authority, 32 liquidity_program,
// 1 status (bool), 2 flashloan_fee (u16 LE, basis points).
const JUPITER_ADMIN_STATUS_AT_BYTE: usize = 72;
const JUPITER_ADMIN_FEE_AT_BYTE: usize = 73;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JupiterFlashLoanAccounts {
    /// PDA `["flashloan_admin"]`; also the borrower on the liquidity layer.
    pub flashloan_admin: PublicKeyBytes,
    pub mint: PublicKeyBytes,
    pub token_reserve: PublicKeyBytes,
    pub borrow_position: PublicKeyBytes,
    pub rate_model: PublicKeyBytes,
    pub liquidity: PublicKeyBytes,
    /// The liquidity layer's token account holding the lendable SOL.
    pub vault: PublicKeyBytes,
    /// Fee read from the admin account at startup.
    pub fee_in_basis_points: u16,
}

impl JupiterFlashLoanAccounts {
    /// Address of the on-chain admin account (read at startup for the fee).
    pub fn flashloan_admin_address() -> PublicKeyBytes {
        Pubkey::find_program_address(
            &[b"flashloan_admin"],
            &program(JUPITER_FLASHLOAN_PROGRAM_ADDRESS),
        )
        .0
        .to_bytes()
    }

    /// Derive every wrapped-SOL account from PDA seeds and read the fee (and
    /// paused flag) from the admin account bytes.
    pub fn from_admin_account_bytes(admin_account_data: &[u8]) -> Result<Self, String> {
        let status = *admin_account_data
            .get(JUPITER_ADMIN_STATUS_AT_BYTE)
            .ok_or("jupiter flashloan admin account too short")?;
        if status == 0 {
            return Err("jupiter flash loans are paused".to_string());
        }
        let fee_in_basis_points = u16::from_le_bytes(
            admin_account_data
                .get(JUPITER_ADMIN_FEE_AT_BYTE..JUPITER_ADMIN_FEE_AT_BYTE + 2)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or("jupiter flashloan admin account too short")?,
        );

        let liquidity_program = program(JUPITER_LIQUIDITY_PROGRAM_ADDRESS);
        let mint = parse_base58_public_key(WRAPPED_SOL_MINT_ADDRESS);
        let flashloan_admin = Self::flashloan_admin_address();
        let derive = |seeds: &[&[u8]]| {
            Pubkey::find_program_address(seeds, &liquidity_program)
                .0
                .to_bytes()
        };
        let liquidity = derive(&[b"liquidity"]);
        Ok(Self {
            flashloan_admin,
            mint,
            token_reserve: derive(&[b"reserve", &mint]),
            borrow_position: derive(&[b"user_borrow_position", &mint, &flashloan_admin]),
            rate_model: derive(&[b"rate_model", &mint]),
            liquidity,
            vault: associated_token_account_address(&liquidity, &mint),
            fee_in_basis_points,
        })
    }

    /// Borrowed amount plus the on-chain fee, rounded up (as the program does).
    pub fn amount_to_repay(&self, borrowed: u64) -> u64 {
        let fee = (u128::from(borrowed) * u128::from(self.fee_in_basis_points)).div_ceil(10_000);
        borrowed.saturating_add(u64::try_from(fee).unwrap_or(u64::MAX))
    }

    /// Borrow and payback share one account list.
    fn flash_instruction(
        &self,
        discriminator: [u8; 8],
        wallet: &PublicKeyBytes,
        wallet_token_account: &PublicKeyBytes,
        amount: u64,
    ) -> Instruction {
        let mut data = discriminator.to_vec();
        data.extend_from_slice(&amount.to_le_bytes());
        Instruction::new_with_bytes(
            program(JUPITER_FLASHLOAN_PROGRAM_ADDRESS),
            &data,
            vec![
                AccountMeta::new(pubkey(*wallet), true),
                writable(self.flashloan_admin),
                writable(*wallet_token_account),
                read_only(self.mint),
                writable(self.token_reserve),
                writable(self.borrow_position),
                read_only(self.rate_model),
                writable(self.vault),
                read_only(self.liquidity),
                read_only_program(JUPITER_LIQUIDITY_PROGRAM_ADDRESS),
                read_only_program(TOKEN_PROGRAM_ADDRESS),
                read_only_program(ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS),
                read_only_program(SYSTEM_PROGRAM_ADDRESS),
                read_only_program(INSTRUCTIONS_SYSVAR_ADDRESS),
            ],
        )
    }
}

// ── Kamino Reserve account (start of it) ──────────────────────────────────
//
//   byte   size  field
//   0      8     Anchor discriminator
//   8      8     version
//   16     16    last_update
//   32     32    lending_market
//   64     32    farm_collateral
//   96     32    farm_debt
//   128    32    liquidity.mint_pubkey
//   160    32    liquidity.supply_vault   (where lent tokens sit)
//   192    32    liquidity.fee_vault      (where flash-loan fees go)
const KAMINO_RESERVE_LENDING_MARKET_AT_BYTE: usize = 32;
const KAMINO_RESERVE_LIQUIDITY_MINT_AT_BYTE: usize = 128;
const KAMINO_RESERVE_SUPPLY_VAULT_AT_BYTE: usize = 160;
const KAMINO_RESERVE_FEE_VAULT_AT_BYTE: usize = 192;

/// All accounts a Kamino flash borrow/repay needs, resolved once at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KaminoFlashLoanAccounts {
    pub lending_market: PublicKeyBytes,
    /// PDA `["lma", lending_market]` that signs token transfers out of the reserve.
    pub lending_market_authority: PublicKeyBytes,
    pub reserve: PublicKeyBytes,
    pub reserve_liquidity_mint: PublicKeyBytes,
    pub reserve_supply_vault: PublicKeyBytes,
    pub reserve_fee_vault: PublicKeyBytes,
}

impl KaminoFlashLoanAccounts {
    /// Read vault addresses from the reserve account's bytes, and check that the
    /// reserve really belongs to `lending_market` and lends wrapped SOL.
    pub fn from_reserve_account_bytes(
        lending_market: PublicKeyBytes,
        reserve: PublicKeyBytes,
        reserve_account_data: &[u8],
    ) -> Result<Self, String> {
        let read = |at: usize| -> Result<PublicKeyBytes, String> {
            reserve_account_data
                .get(at..at + 32)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or_else(|| "kamino reserve account too short".to_string())
        };
        let reserve_lending_market = read(KAMINO_RESERVE_LENDING_MARKET_AT_BYTE)?;
        if reserve_lending_market != lending_market {
            return Err(format!(
                "reserve belongs to market {}, not KAMINO_LENDING_MARKET",
                encode_public_key_as_base58(&reserve_lending_market)
            ));
        }
        let reserve_liquidity_mint = read(KAMINO_RESERVE_LIQUIDITY_MINT_AT_BYTE)?;
        if reserve_liquidity_mint != parse_base58_public_key(WRAPPED_SOL_MINT_ADDRESS) {
            return Err("KAMINO_SOL_RESERVE does not lend wrapped SOL".to_string());
        }
        Ok(Self {
            lending_market,
            lending_market_authority: kamino_lending_market_authority(&lending_market),
            reserve,
            reserve_liquidity_mint,
            reserve_supply_vault: read(KAMINO_RESERVE_SUPPLY_VAULT_AT_BYTE)?,
            reserve_fee_vault: read(KAMINO_RESERVE_FEE_VAULT_AT_BYTE)?,
        })
    }

    pub fn flash_borrow_instruction(
        &self,
        wallet: &PublicKeyBytes,
        destination: &PublicKeyBytes,
        amount: u64,
    ) -> Instruction {
        let mut data = KAMINO_FLASH_BORROW_DISCRIMINATOR.to_vec();
        data.extend_from_slice(&amount.to_le_bytes());
        Instruction::new_with_bytes(
            program(KAMINO_LEND_PROGRAM_ADDRESS),
            &data,
            self.accounts(wallet, self.reserve_supply_vault, *destination),
        )
    }

    pub fn flash_repay_instruction(
        &self,
        wallet: &PublicKeyBytes,
        source: &PublicKeyBytes,
        amount: u64,
        borrow_instruction_index: u8,
    ) -> Instruction {
        let mut data = KAMINO_FLASH_REPAY_DISCRIMINATOR.to_vec();
        data.extend_from_slice(&amount.to_le_bytes());
        data.push(borrow_instruction_index);
        Instruction::new_with_bytes(
            program(KAMINO_LEND_PROGRAM_ADDRESS),
            &data,
            self.accounts(wallet, *source, self.reserve_supply_vault),
        )
    }

    /// Borrow and repay share one account layout; only the token flow direction differs.
    /// Unused optional accounts (referrer) are filled with the program's own address,
    /// which is how Anchor encodes "None".
    fn accounts(
        &self,
        wallet: &PublicKeyBytes,
        from: PublicKeyBytes,
        to: PublicKeyBytes,
    ) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(pubkey(*wallet), true),
            read_only(self.lending_market_authority),
            read_only(self.lending_market),
            writable(self.reserve),
            read_only(self.reserve_liquidity_mint),
            writable(from),
            writable(to),
            writable(self.reserve_fee_vault),
            read_only_program(KAMINO_LEND_PROGRAM_ADDRESS),
            read_only_program(KAMINO_LEND_PROGRAM_ADDRESS),
            read_only_program(INSTRUCTIONS_SYSVAR_ADDRESS),
            read_only_program(TOKEN_PROGRAM_ADDRESS),
        ]
    }
}

/// PDA `["lma", lending_market]` under the Kamino program.
pub fn kamino_lending_market_authority(lending_market: &PublicKeyBytes) -> PublicKeyBytes {
    let (address, _bump) = Pubkey::find_program_address(
        &[b"lma", lending_market.as_ref()],
        &program(KAMINO_LEND_PROGRAM_ADDRESS),
    );
    address.to_bytes()
}
