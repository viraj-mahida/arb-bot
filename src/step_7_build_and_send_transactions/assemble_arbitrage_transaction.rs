//! Put every instruction in order, compile a v0 message, and sign it.
//!
//! Instruction order (each line runs only if every line above succeeded):
//!
//! ```text
//!  1. set compute-unit limit          ┐ compute budget
//!  2. set compute-unit price          ┘
//!  3. create wSOL account if missing  ┐ make sure the wallet can hold both tokens
//!  4. create USDC account if missing  ┘
//!  5. wallet mode: wrap SOL (transfer + sync_native)
//!     flash mode:  flash-borrow wSOL from Kamino
//!  6. leg 1: sell wSOL for USDC on the expensive pool
//!  7. leg 2: buy wSOL with USDC on the cheap pool (minimum out = start + costs + min profit)
//!  8. flash mode: repay the flash loan
//!  9. close the wSOL account → back to plain SOL (profit included)
//! 10. Jito tip (last, so it is only paid if everything worked)
//! ```
//!
//! **v0 message and address lookup tables:** a transaction may be at most
//! 1,232 bytes, and every account costs 32 bytes. Version-0 messages can point
//! at *address lookup tables* (on-chain lists of addresses) and name an account
//! by a 1-byte index instead. Flash-loan trades touch ~35 accounts, so they
//! usually need one; set `ADDRESS_LOOKUP_TABLES` to use yours.

use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};

use super::compute_budget_instructions::{set_compute_unit_limit, set_compute_unit_price};
use super::decide_if_trade_is_worth_it::ApprovedArbitrageTrade;
use super::flash_loan_instructions::FlashLoanProvider;
use super::jito_tip_instruction::jito_tip_instruction;
use super::swap_leg_instruction::{SwapLeg, build_swap_instruction};
use super::trading_wallet::{TradingWallet, close_token_account, create_associated_token_account_if_missing, wrap_sol};
use super::well_known_program_addresses::pubkey;
use crate::step_3_store_latest_pool_state::{PublicKeyBytes, public_key_from_byte_slice};
use crate::step_4_quote_swaps::SwapDirection;

/// Largest serialized transaction the network accepts (IPv6 MTU minus headers).
pub const MAX_TRANSACTION_SIZE_IN_BYTES: usize = 1232;

/// How leg 1's SOL is paid for, with whatever that funding source needs.
pub enum FundingSource<'a> {
    OwnWallet,
    FlashLoan(&'a FlashLoanProvider),
}

/// Knobs that shape the transaction but are not part of the trade itself.
pub struct TransactionFeeSettings {
    pub compute_unit_limit: u32,
    pub priority_fee_micro_lamports_per_compute_unit: u64,
    /// `(tip account, lamports)`, or `None` when not sending through Jito.
    pub jito_tip: Option<(PublicKeyBytes, u64)>,
}

/// Every instruction of the round trip, in execution order.
pub fn arbitrage_instructions(
    wallet: &TradingWallet,
    trade: &ApprovedArbitrageTrade,
    funding: &FundingSource,
    fees: &TransactionFeeSettings,
) -> Vec<Instruction> {
    let owner = wallet.address();
    let start_token_mint = trade.sell_pool.token_a_mint;
    let bridge_token_mint = trade.sell_pool.token_b_mint;
    let start_token_account = wallet.associated_token_account(&start_token_mint);
    let bridge_token_account = wallet.associated_token_account(&bridge_token_mint);

    let mut instructions = vec![
        set_compute_unit_limit(fees.compute_unit_limit),
        set_compute_unit_price(fees.priority_fee_micro_lamports_per_compute_unit),
        create_associated_token_account_if_missing(&owner, &owner, &start_token_mint),
        create_associated_token_account_if_missing(&owner, &owner, &bridge_token_mint),
    ];

    let flash_borrow_instruction_index = instructions.len();
    match funding {
        FundingSource::OwnWallet => instructions.extend(wrap_sol(&owner, &start_token_account, trade.start_token_amount_in)),
        FundingSource::FlashLoan(provider) => {
            instructions.push(provider.borrow_instruction(&owner, &start_token_account, trade.start_token_amount_in));
        }
    }

    instructions.push(build_swap_instruction(&SwapLeg {
        pool: &trade.sell_pool,
        cached_tick_arrays: &trade.sell_pool_tick_arrays,
        direction: SwapDirection::TokenAToTokenB,
        amount_in: trade.start_token_amount_in,
        minimum_amount_out: trade.leg_1_minimum_bridge_token_out,
        wallet: owner,
        wallet_token_a_account: start_token_account,
        wallet_token_b_account: bridge_token_account,
    }));
    instructions.push(build_swap_instruction(&SwapLeg {
        pool: &trade.buy_pool,
        cached_tick_arrays: &trade.buy_pool_tick_arrays,
        direction: SwapDirection::TokenBToTokenA,
        amount_in: trade.leg_2_bridge_token_amount_in,
        minimum_amount_out: trade.leg_2_minimum_start_token_out,
        wallet: owner,
        wallet_token_a_account: start_token_account,
        wallet_token_b_account: bridge_token_account,
    }));

    if let FundingSource::FlashLoan(provider) = funding {
        let borrow_index = u8::try_from(flash_borrow_instruction_index).expect("fewer than 256 instructions");
        instructions.push(provider.repay_instruction(&owner, &start_token_account, trade.start_token_amount_in, borrow_index));
    }
    instructions.push(close_token_account(&start_token_account, &owner));
    if let Some((tip_account, tip_lamports)) = fees.jito_tip {
        instructions.push(jito_tip_instruction(&owner, &tip_account, tip_lamports));
    }
    instructions
}

/// A signed transaction ready for `simulateTransaction` / `sendTransaction` / Jito.
pub struct SignedTransaction {
    /// Wire bytes: signature count, signatures, then the message.
    pub wire_bytes: Vec<u8>,
    /// The first signature in base58 — this *is* the transaction id on explorers.
    pub signature_base58: String,
}

/// Compile instructions into a v0 message, sign it, and check the size limit.
pub fn compile_and_sign_v0_transaction(
    wallet: &TradingWallet,
    instructions: &[Instruction],
    address_lookup_tables: &[AddressLookupTableAccount],
    recent_blockhash: Hash,
) -> Result<SignedTransaction, String> {
    let message = v0::Message::try_compile(&pubkey(wallet.address()), instructions, address_lookup_tables, recent_blockhash)
        .map_err(|error| format!("could not compile message: {error}"))?;
    let message_bytes = VersionedMessage::V0(message).serialize();
    let signature = wallet.sign_message_bytes(&message_bytes);

    // One signature → the "compact-u16" count is the single byte 1.
    let mut wire_bytes = Vec::with_capacity(1 + signature.len() + message_bytes.len());
    wire_bytes.push(1);
    wire_bytes.extend_from_slice(&signature);
    wire_bytes.extend_from_slice(&message_bytes);
    if wire_bytes.len() > MAX_TRANSACTION_SIZE_IN_BYTES {
        return Err(format!(
            "transaction is {} bytes (limit {MAX_TRANSACTION_SIZE_IN_BYTES}); add an address lookup table via ADDRESS_LOOKUP_TABLES",
            wire_bytes.len()
        ));
    }
    Ok(SignedTransaction { wire_bytes, signature_base58: bs58::encode(signature).into_string() })
}

/// Lookup-table accounts start with a 56-byte header (authority, slots, …),
/// followed by the stored addresses, 32 bytes each.
const LOOKUP_TABLE_HEADER_SIZE_IN_BYTES: usize = 56;

/// Decode an address lookup table account's bytes into the list of addresses it stores.
pub fn decode_address_lookup_table(table_address: PublicKeyBytes, account_data: &[u8]) -> Option<AddressLookupTableAccount> {
    let addresses = account_data
        .get(LOOKUP_TABLE_HEADER_SIZE_IN_BYTES..)?
        .chunks_exact(32)
        .filter_map(public_key_from_byte_slice)
        .map(pubkey)
        .collect();
    Some(AddressLookupTableAccount { key: pubkey(table_address), addresses })
}
