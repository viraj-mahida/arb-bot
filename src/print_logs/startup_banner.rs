//! What the bot prints once at startup and while connecting.

use crate::step_3_store_latest_pool_state::WatchedPools;

pub fn startup_banner(watched_pools: &WatchedPools) {
    log_line!(
        "arb-bot  (educational two-pool arbitrage bot; simulate-only unless SEND_TRANSACTIONS=true)"
    );
    log_line!();
    log_line!("pipeline");
    log_line!("  1. listen     Geyser (Yellowstone gRPC) streams live account writes");
    log_line!("  2. decode     raw account bytes -> pool state + tick arrays");
    log_line!("                RPC getMultipleAccounts loads each new tick array once");
    log_line!("                (Geyser only pushes future writes, never current state)");
    log_line!("  3. store      in-memory cache holds each pool + its nearby tick arrays");
    log_line!("  4. quote      local DEX math for one swap and a two-pool round trip");
    log_line!("  5. size       walk both tick books to find the profit-maximizing size");
    log_line!(
        "  6. trade      subtract all costs, build + sign the transaction, simulate, (optionally) send"
    );
    log_line!();
    match super::output::log_file_path() {
        Some(path) => log_line!(
            "writing a copy of every line to {}  (override with LOG_FILE, empty disables)",
            path.display()
        ),
        None => log_line!("log file disabled (LOG_FILE is empty)"),
    }
    log_line!();
    log_line!("watching");
    for pool_config in watched_pools.all_configs() {
        log_line!(
            "  {:<14}  {}  ({} bps fee, decimals token A {} / token B {})",
            pool_config.dex.name(),
            pool_config.pool_address_base58,
            pool_config.fee_in_basis_points,
            pool_config.token_a_decimals,
            pool_config.token_b_decimals
        );
    }
    log_line!();
    log_line!("how to read the log  (times are UTC)");
    log_line!("  [pool]               a pool's price changed (someone swapped)");
    log_line!(
        "  [spread]             both prices, the gap between them, and the fees it must beat"
    );
    log_line!(
        "                       1 bp = 0.01%; two 4 bp swaps cost 8 bps, so smaller gaps lose money"
    );
    log_line!(
        "  [best size]          only when the gap beats fees: the most profitable trade size"
    );
    log_line!("  [rpc/tick-array]     one-time load of liquidity data near a new price");
    log_line!("  [geyser/tick-array]  liquidity data updates (hidden unless LOG_VERBOSE=true)");
    log_line!(
        "  [decide]             after network fee + priority fee + tip + flash fee: trade or skip"
    );
    log_line!("  [simulate]           dry-run of the signed transaction on the RPC node");
    log_line!("  [send]               real submission (Jito bundle or RPC) and its confirmation");
    log_line!();
}

pub fn geyser_connecting() {
    log_line!("[geyser] connecting to Yellowstone gRPC…");
}

pub fn geyser_subscribed(pool_count: usize) {
    log_line!("[geyser] subscribed to {pool_count} pool account(s), commitment=processed");
    crate::dashboard_events::geyser(
        0,
        "",
        "status",
        &format!("subscribed to {pool_count} pool account(s)"),
    );
}

pub fn rpc_client_ready() {
    log_line!("[rpc] HTTP client ready  (10s timeout)");
}

pub fn waiting_for_account_updates() {
    log_line!("[geyser] waiting for account writes…");
    log_line!();
    crate::dashboard_events::geyser(0, "", "status", "waiting for account writes");
}

/// Printed when `DEMO_PRICE_SHIFT_BPS` is set. Sending is forced off in `main`.
pub fn demo_scenario(basis_points: u64) {
    log_line!(
        "[demo] Orca's quoted price is shifted up by {basis_points} bps so a round trip can be sized, built, and simulated. The cache stays real, and sending is forced off."
    );
    crate::dashboard_events::log_line(
        "demo",
        &format!("demo scenario: Orca +{basis_points} bps, simulate only"),
    );
}
