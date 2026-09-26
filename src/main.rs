//! # arb-bot — an educational Solana arbitrage watcher
//!
//! ## The story in plain words
//!
//! Two shops sell the same thing. Right now one shop pays 101 dollars for it
//! and the other sells it for 100. Buy from the cheap one, sell to the
//! expensive one, and keep the difference. That is **arbitrage**.
//!
//! On Solana the "shops" are **pools** on **DEXes** (decentralized exchanges)
//! — here, a SOL/USDC pool on Orca and one on Raydium. There is no shopkeeper:
//! a program (smart contract) sets each pool's price from how many tokens it
//! holds, and anyone can swap against it. When traders hit one pool and not
//! the other, their prices drift apart, and an arbitrage bot pulls them back
//! together, earning the gap minus fees.
//!
//! This bot watches both pools live, keeps their state in memory, and works
//! out — with the pools' own math — whether a round trip would be profitable
//! and how big it should be. It is **read-only**: it never sends a transaction.
//!
//! ## Solana basics you need
//!
//! - **Account:** everything on Solana is an account — a wallet, a token
//!   balance, a pool, a program. Each holds some bytes of data.
//! - **Public key / address:** the 32-byte name of an account, written in
//!   base58 (e.g. `Czfq3xZZ…44zE`).
//! - **Program:** an account holding executable code (a smart contract). A DEX
//!   is a program; its pools are data accounts the program owns.
//! - **PDA (Program Derived Address):** an address computed from seeds and a
//!   program address, so anyone can find it without asking the chain.
//! - **Slot:** Solana's clock tick, roughly every 400 ms; a block per slot.
//! - **Commitment:** how final data is. `processed` = seen by one validator,
//!   fastest; `confirmed` / `finalized` = voted on by the network, slower.
//! - **Lamports:** SOL's smallest unit; 1 SOL = 1,000,000,000 lamports. Every
//!   amount in the code is an integer in the token's smallest unit.
//! - **RPC vs Geyser:** RPC answers questions on request; Geyser pushes every
//!   account change as it happens. See `solana_connections`.
//!
//! See `GLOSSARY.md` for every term used in the code.
//!
//! ## Table of contents (read the folders in this order)
//!
//! 0. [`solana_connections`] — the two network links: RPC and Geyser gRPC
//! 1. [`step_1_listen_to_account_updates`] — loop over live account changes
//! 2. [`step_2_decode_account_bytes`] — raw bytes → pool / tick-array structs
//! 3. [`step_3_store_latest_pool_state`] — shared types + in-memory cache
//! 4. [`step_4_quote_swaps`] — AMM/CLMM math, one swap, a round trip
//! 5. [`step_5_find_best_arbitrage_size`] — the profit-maximizing trade size
//! 6. [`step_6_print_logs`] — everything printed to the terminal
//!
//! ## Roadmap — not built yet
//!
//! To trade for real, a future `step_7_build_and_send_transactions` would:
//!
//! - **Build swap instructions:** an *instruction* is one call into a program
//!   ("swap X on this pool"); a *transaction* bundles instructions that succeed
//!   or fail together, so both legs happen or neither does.
//! - **Set a minimum output (slippage guard):** if prices move before the
//!   transaction lands, it reverts instead of losing money.
//! - **Budget compute and pay a priority fee:** transactions pay per compute
//!   unit; a higher priority fee gets you processed earlier.
//! - **Use a flash loan (optional):** borrow the start amount and repay it in
//!   the same transaction, so the bot needs little capital of its own.
//! - **Send through Jito with a tip:** a Jito *bundle* is a group of
//!   transactions a block builder includes in order, all or nothing; the *tip*
//!   pays for that. This protects against MEV (other bots front-running you).
//! - **Simulate, then send:** dry-run with `simulateTransaction`, then send
//!   with `sendTransaction`, both through the same `SolanaRpcClient`.
//! - **Account for all costs:** only trade when profit exceeds pool fees plus
//!   network fee, priority fee, and tip.
//!
//! A production bot would also watch many DEXes, many pools, and many token
//! pairs, and search routes through three or more pools.

use crate::solana_connections::{SolanaRpcClient, connect_to_geyser_grpc};
use crate::step_1_listen_to_account_updates::process_account_updates_forever;
use crate::step_3_store_latest_pool_state::{LatestPoolStateCache, WatchedPools};

pub(crate) mod solana_connections;
pub(crate) mod step_1_listen_to_account_updates;
pub(crate) mod step_2_decode_account_bytes;
pub(crate) mod step_3_store_latest_pool_state;
pub(crate) mod step_4_quote_swaps;
pub(crate) mod step_5_find_best_arbitrage_size;
pub(crate) mod step_6_print_logs;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    // TLS needs a crypto backend chosen once per process before any HTTPS / gRPC connection.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    // Load RPC_URL, GRPC_URL, and X_TOKEN from a local `.env` file if present.
    dotenvy::dotenv().ok();

    let watched_pools = WatchedPools::sol_usdc_pools_on_orca_and_raydium();
    step_6_print_logs::startup_banner(&watched_pools);
    let cache = LatestPoolStateCache::new();
    let rpc_client = SolanaRpcClient::from_env();
    step_6_print_logs::rpc_client_ready();

    let (geyser_subscription_sender, geyser_account_update_stream) = connect_to_geyser_grpc(&watched_pools)
        .await
        .expect("failed to open Geyser stream");
    process_account_updates_forever(
        geyser_account_update_stream,
        geyser_subscription_sender,
        &watched_pools,
        &cache,
        &rpc_client,
    )
    .await;
}
