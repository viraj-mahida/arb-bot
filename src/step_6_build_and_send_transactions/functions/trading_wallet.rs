//! The bot's wallet: its signing key, its token accounts, and wrapping SOL.
//!
//! **Keypair:** a wallet is a secret key plus the public key (address) derived
//! from it. Signing a transaction with the secret key proves the wallet owner
//! approved it. Anyone who reads the key file can spend the funds — keep it
//! private and put only trading money in it.
//!
//! **Token accounts and ATAs:** a wallet does not hold tokens directly. Each
//! token (USDC, wrapped SOL, …) sits in a separate *token account* owned by the
//! wallet. The *associated token account* (ATA) is the one standard token
//! account per (wallet, mint) pair; its address is a PDA, so anyone can compute it.
//!
//! **Wrapped SOL:** DEX pools trade SPL tokens, but SOL itself is not one. Wrapped
//! SOL (wSOL) is an SPL token backed 1:1 by lamports sitting in a token account.
//! To wrap: send lamports to the wSOL token account, then call `sync_native` so
//! the token balance catches up. To unwrap: close the account, which returns
//! every lamport (balance + rent) to the wallet.

use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

use super::well_known_program_addresses::{
    ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS, SYSTEM_PROGRAM_ADDRESS, TOKEN_PROGRAM_ADDRESS,
    program, pubkey, read_only, read_only_program, writable,
};
use crate::step_3_store_latest_pool_state::PublicKeyBytes;

/// The signing wallet the bot trades from.
pub struct TradingWallet {
    keypair: Keypair,
}

impl TradingWallet {
    /// Load a keypair file in the `solana-keygen` JSON format (`[12, 34, …]`, 64 numbers).
    pub fn load_from_keypair_file(path: &str) -> Result<Self, String> {
        let keypair = solana_keypair::read_keypair_file(path)
            .map_err(|error| format!("could not read wallet keypair '{path}': {error}"))?;
        Ok(Self { keypair })
    }

    /// Test-only. The bot loads [`Self::load_from_keypair_file`].
    #[cfg(test)]
    pub fn from_keypair(keypair: Keypair) -> Self {
        Self { keypair }
    }

    /// The wallet's public address (it pays fees and signs the transaction).
    pub fn address(&self) -> PublicKeyBytes {
        self.keypair.pubkey().to_bytes()
    }

    /// Sign raw message bytes with the wallet's secret key → 64-byte signature.
    pub fn sign_message_bytes(&self, message_bytes: &[u8]) -> [u8; 64] {
        self.keypair.sign_message(message_bytes).into()
    }

    /// This wallet's ATA for `mint` (classic token program).
    pub fn associated_token_account(&self, mint: &PublicKeyBytes) -> PublicKeyBytes {
        associated_token_account_address(&self.address(), mint)
    }
}

/// Address of the ATA for (`owner`, `mint`), for mints on the classic token program.
///
/// Seeds are `[owner, token_program, mint]` under the ATA program.
pub fn associated_token_account_address(
    owner: &PublicKeyBytes,
    mint: &PublicKeyBytes,
) -> PublicKeyBytes {
    let token_program = program(TOKEN_PROGRAM_ADDRESS);
    let (address, _bump) = Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &program(ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS),
    );
    address.to_bytes()
}

/// "Create this ATA unless it already exists" (the ATA program's `CreateIdempotent`).
///
/// Idempotent means running it twice is harmless, so every trade can include
/// it without first asking the chain whether the account exists. Creating an
/// account costs a refundable rent deposit (~0.002 SOL) the first time.
pub fn create_associated_token_account_if_missing(
    payer: &PublicKeyBytes,
    owner: &PublicKeyBytes,
    mint: &PublicKeyBytes,
) -> Instruction {
    const CREATE_IDEMPOTENT: u8 = 1;
    Instruction::new_with_bytes(
        program(ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS),
        &[CREATE_IDEMPOTENT],
        vec![
            AccountMeta::new(pubkey(*payer), true),
            writable(associated_token_account_address(owner, mint)),
            read_only(*owner),
            read_only(*mint),
            read_only_program(SYSTEM_PROGRAM_ADDRESS),
            read_only_program(TOKEN_PROGRAM_ADDRESS),
        ],
    )
}

/// Move `lamports` of plain SOL from `from` to `to` (system program `Transfer`).
pub fn transfer_sol(from: &PublicKeyBytes, to: &PublicKeyBytes, lamports: u64) -> Instruction {
    const SYSTEM_TRANSFER: u32 = 2;
    let mut data = SYSTEM_TRANSFER.to_le_bytes().to_vec();
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction::new_with_bytes(
        program(SYSTEM_PROGRAM_ADDRESS),
        &data,
        vec![AccountMeta::new(pubkey(*from), true), writable(*to)],
    )
}

/// Wrap SOL: send lamports into the wSOL token account, then `sync_native`
/// so its token balance counts them.
pub fn wrap_sol(
    owner: &PublicKeyBytes,
    wrapped_sol_token_account: &PublicKeyBytes,
    lamports: u64,
) -> [Instruction; 2] {
    const SYNC_NATIVE: u8 = 17;
    [
        transfer_sol(owner, wrapped_sol_token_account, lamports),
        Instruction::new_with_bytes(
            program(TOKEN_PROGRAM_ADDRESS),
            &[SYNC_NATIVE],
            vec![writable(*wrapped_sol_token_account)],
        ),
    ]
}

/// Unwrap SOL: close the wSOL token account; every lamport inside returns to `owner`.
pub fn close_token_account(token_account: &PublicKeyBytes, owner: &PublicKeyBytes) -> Instruction {
    const CLOSE_ACCOUNT: u8 = 9;
    Instruction::new_with_bytes(
        program(TOKEN_PROGRAM_ADDRESS),
        &[CLOSE_ACCOUNT],
        vec![
            writable(*token_account),
            writable(*owner),
            AccountMeta::new_readonly(pubkey(*owner), true),
        ],
    )
}
