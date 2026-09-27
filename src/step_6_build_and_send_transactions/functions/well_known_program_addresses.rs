//! Addresses of the Solana system programs a trade transaction calls, plus a
//! tiny helper to build "account X, writable or not, signer or not" entries.
//!
//! **Why programs need addresses in a transaction:** Solana runs transactions in
//! parallel. To do that safely, every transaction must list up front *every*
//! account it will read or write — including the programs it calls. Two
//! transactions that write the same account cannot run at the same time.

use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;

use crate::step_3_store_latest_pool_state::{PublicKeyBytes, parse_base58_public_key};

/// Creates accounts and moves SOL (lamports) between wallets.
pub const SYSTEM_PROGRAM_ADDRESS: &str = "11111111111111111111111111111111";
/// The classic SPL Token program: owns every token account for SOL (wrapped) and USDC.
pub const TOKEN_PROGRAM_ADDRESS: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// The newer Token-2022 program. Raydium's `swap_v2` asks for it even when both tokens use the classic one.
pub const TOKEN_2022_PROGRAM_ADDRESS: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
/// Computes and creates each wallet's default token account for a mint.
pub const ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS: &str =
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
/// Memo program; `swap_v2` on both DEXes lists it (used for Token-2022 transfer memos).
pub const MEMO_PROGRAM_ADDRESS: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
/// Lets a transaction request more compute units and set its priority fee.
pub const COMPUTE_BUDGET_PROGRAM_ADDRESS: &str = "ComputeBudget111111111111111111111111111111";
/// A read-only "sysvar" account listing every instruction in the current
/// transaction. Kamino reads it to check that a flash borrow is repaid later.
pub const INSTRUCTIONS_SYSVAR_ADDRESS: &str = "Sysvar1nstructions1111111111111111111111111";
/// The wrapped-SOL mint: SOL dressed up as an SPL token so DEXes can treat it like any other token.
pub const WRAPPED_SOL_MINT_ADDRESS: &str = "So11111111111111111111111111111111111111112";
/// Kamino Lend program (flash loans).
pub const KAMINO_LEND_PROGRAM_ADDRESS: &str = "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";

/// Turn raw address bytes into the `Pubkey` type the instruction library uses.
pub fn pubkey(address: PublicKeyBytes) -> Pubkey {
    Pubkey::from(address)
}

/// Parse one of the constants above into a `Pubkey`.
pub fn program(base58_address: &str) -> Pubkey {
    Pubkey::from(parse_base58_public_key(base58_address))
}

/// An account the instruction will change (debit, credit, overwrite).
pub fn writable(address: PublicKeyBytes) -> AccountMeta {
    AccountMeta::new(pubkey(address), false)
}

/// An account the instruction only reads.
pub fn read_only(address: PublicKeyBytes) -> AccountMeta {
    AccountMeta::new_readonly(pubkey(address), false)
}

/// A read-only program or sysvar given by its base58 constant.
pub fn read_only_program(base58_address: &str) -> AccountMeta {
    AccountMeta::new_readonly(program(base58_address), false)
}
