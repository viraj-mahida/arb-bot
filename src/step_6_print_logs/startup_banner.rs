//! What the bot prints once at startup and while connecting.

use crate::step_3_store_latest_pool_state::WatchedPools;

pub fn startup_banner(watched_pools: &WatchedPools) {
    println!("arb-bot  (educational two-pool arbitrage watcher, read-only)");
    println!();
    println!("pipeline");
    println!("  1. listen     Geyser (Yellowstone gRPC) streams live account writes");
    println!("  2. decode     raw account bytes -> pool state + tick arrays");
    println!("                RPC getMultipleAccounts loads each new tick array once");
    println!("                (Geyser only pushes future writes, never current state)");
    println!("  3. store      in-memory cache holds each pool + its nearby tick arrays");
    println!("  4. quote      local DEX math quotes a 0.1 SOL round trip");
    println!("  5. size       walk both tick books to find the profit-maximizing size");
    println!("  6. print      these logs (pool fees included; no network fee / tip; no transaction sent)");
    println!();
    println!("watching");
    for pool_config in watched_pools.all_configs() {
        println!(
            "  {:<14}  {}  ({} bps fee, decimals token A {} / token B {})",
            pool_config.dex.name(),
            pool_config.pool_address_base58,
            pool_config.fee_in_basis_points,
            pool_config.token_a_decimals,
            pool_config.token_b_decimals
        );
    }
    println!();
    println!("log tags");
    println!("  [geyser/pool]        a pool account changed (price / liquidity / current tick)");
    println!("  [geyser/tick-array]  a tick-array account changed (where liquidity steps are)");
    println!("  [rpc/tick-array]     one-time fetch of tick arrays we just started watching");
    println!("  [snapshot]           both pools as cached now; gap = raydium - orca (USDC per SOL)");
    println!("  [quote 0.1 SOL]      sell 0.1 SOL on one pool, buy SOL back on the other (math only)");
    println!("  [best size]          same round trip at the profit-maximizing size (math only)");
    println!();
}

pub fn geyser_connecting() {
    println!("[geyser] connecting to Yellowstone gRPC…");
}

pub fn geyser_subscribed(pool_count: usize) {
    println!("[geyser] subscribed to {pool_count} pool account(s), commitment=processed");
}

pub fn rpc_client_ready() {
    println!("[rpc] HTTP client ready  (getMultipleAccounts, 10s timeout)");
}

pub fn waiting_for_account_updates() {
    println!("[geyser] waiting for account writes…");
    println!();
}
