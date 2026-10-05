//! # arb-bot — a Solana arbitrage bot
//!
//! Built to make money if it is deployed correctly: live pool state, each
//! pool's own swap math, and a signed trade that is sent when
//! `SEND_TRANSACTIONS=true`.
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
//! and how big it should be. With a wallet configured it builds, signs, and
//! **simulates** the trade; it only **sends** it when `SEND_TRANSACTIONS=true`.
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
//! 6. [`step_6_build_and_send_transactions`] — costs, swap instructions, flash
//!    loan, Jito tip, sign, simulate, send
//!
//! Terminal output is formatted in [`print_logs`]. It is not a numbered step:
//! the steps (and the connections) call it whenever they have something to
//! show. Trading settings are in [`bot_settings`]; see `.env.example`.
//!
//! ## Still to add
//!
//! More DEXes, more pools, and more token pairs; routes through three or more
//! pools; a bot-owned address lookup table; the flash-loan fee read from the
//! reserve; and Token-2022 mints.

use crate::bot_settings::BotSettingsFromEnvironment;
use crate::solana_connections::{SolanaRpcClient, connect_to_geyser_grpc};
use crate::step_1_listen_to_account_updates::main_process_account_updates_forever;
use crate::step_3_store_latest_pool_state::{LatestPoolStateCache, WatchedPools};
use crate::step_6_build_and_send_transactions::ArbitrageTradeExecutor;

pub(crate) mod bot_settings;
pub(crate) mod dashboard_events;
pub(crate) mod print_logs;
pub(crate) mod solana_connections;
pub(crate) mod step_1_listen_to_account_updates;
pub(crate) mod step_2_decode_account_bytes;
pub(crate) mod step_3_store_latest_pool_state;
pub(crate) mod step_4_quote_swaps;
pub(crate) mod step_5_find_best_arbitrage_size;
pub(crate) mod step_6_build_and_send_transactions;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    // TLS needs a crypto backend chosen once per process before any HTTPS / gRPC connection.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    // Load environment variables from a local `.env` file if present.
    dotenvy::dotenv().ok();
    // ignr: log file + visualizer. Not part of the trading pipeline.
    print_logs::ignr_start();

    if std::env::args().nth(1).as_deref() == Some("create-lookup-table") {
        let settings = BotSettingsFromEnvironment::from_env();
        let rpc_client = SolanaRpcClient::from_env();
        if let Err(error) =
            step_6_build_and_send_transactions::create_lookup_table::main_create_lookup_table(
                &settings,
                &rpc_client,
            )
            .await
        {
            eprintln!("create-lookup-table failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    let watched_pools = WatchedPools::sol_usdc_pools();
    print_logs::startup_banner(&watched_pools);
    let cache = LatestPoolStateCache::new();
    let rpc_client = SolanaRpcClient::from_env();
    print_logs::rpc_client_ready();

    let mut settings = BotSettingsFromEnvironment::from_env();
    // ignr: demo price is not on-chain, so this turns sending off. Cache stays real.
    print_logs::ignr_block_sends_during_demo_price_shift(&mut settings);
    // ignr: visualizer fee line. The approve/skip check computes its own costs later.
    print_logs::ignr_remember_dashboard_route_costs(&settings);

    let trade_executor = match ArbitrageTradeExecutor::prepare(settings, &rpc_client).await {
        Ok(Some(executor)) => Some(executor),
        Ok(None) => {
            print_logs::trading_disabled("WALLET_KEYPAIR_PATH not set");
            None
        }
        Err(error) => panic!("trading is configured but could not start: {error}"),
    };

    let (geyser_subscription_sender, geyser_account_update_stream) = connect_to_geyser_grpc(
        &watched_pools,
        trade_executor
            .as_deref()
            .map(ArbitrageTradeExecutor::blockhash_cache),
    )
    .await
    .expect("failed to open Geyser stream");
    main_process_account_updates_forever(
        geyser_account_update_stream,
        geyser_subscription_sender,
        &watched_pools,
        &cache,
        &rpc_client,
        trade_executor,
    )
    .await;
}
