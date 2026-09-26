//! The Jito tip: a plain SOL transfer to one of Jito's tip accounts.
//!
//! **What Jito is:** most Solana validators run the Jito client. Its *block
//! engine* accepts *bundles* — up to 5 transactions executed in order, all or
//! nothing — and auctions block space to the highest tippers. Two wins for an
//! arbitrage bot: nobody can squeeze a transaction between our legs, and a
//! bundle that would fail is simply dropped, so a lost race costs nothing.
//!
//! The tip is just a transfer to one of eight published tip accounts. Picking
//! one at random spreads load (all bundles tipping the same account would
//! contend for it). Keep the tip as the *last* instruction so it is only paid
//! if everything before it succeeded.

use solana_instruction::Instruction;

use super::trading_wallet::transfer_sol;
use crate::step_3_store_latest_pool_state::{PublicKeyBytes, parse_base58_public_key};

/// Jito's published mainnet tip accounts. The client also asks the block
/// engine (`getTipAccounts`) at startup and prefers that live list.
pub const JITO_MAINNET_TIP_ACCOUNTS: [&str; 8] = [
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

pub fn default_jito_tip_accounts() -> Vec<PublicKeyBytes> {
    JITO_MAINNET_TIP_ACCOUNTS
        .iter()
        .map(|address| parse_base58_public_key(address))
        .collect()
}

/// Pick a tip account pseudo-randomly (clock nanoseconds are random enough to spread load).
pub fn pick_tip_account(tip_accounts: &[PublicKeyBytes]) -> Option<PublicKeyBytes> {
    if tip_accounts.is_empty() {
        return None;
    }
    let nanoseconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.subsec_nanos())
        .unwrap_or(0);
    Some(tip_accounts[nanoseconds as usize % tip_accounts.len()])
}

pub fn jito_tip_instruction(
    wallet: &PublicKeyBytes,
    tip_account: &PublicKeyBytes,
    tip_lamports: u64,
) -> Instruction {
    transfer_sol(wallet, tip_account, tip_lamports)
}
