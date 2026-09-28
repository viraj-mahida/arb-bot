//! Lines printed while processing Geyser updates and RPC loads.

use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, PublicKeyBytes,
    TickArrayAccountWithInitializedTicks, encode_public_key_as_base58, shorten_public_key_for_logs,
};
use crate::step_4_quote_swaps::pool_label;

pub fn pool_account_updated(
    pool: &ConcentratedLiquidityPoolState,
    loaded_tick_array_count: usize,
    watched_tick_array_count: usize,
) {
    log_line!(
        "[pool]    {:<22}  price {:.4} USDC/SOL  tick {}  slot {}  tick arrays loaded {}/{}",
        pool_label(pool),
        pool.human_readable_price_token_b_per_token_a(),
        pool.current_tick_index,
        pool.slot,
        loaded_tick_array_count,
        watched_tick_array_count,
    );
    crate::dashboard_events::geyser(
        pool.slot,
        pool.dex.name(),
        "pool",
        &format!(
            "POOL {}  {:.0}bp  price {:.4}  tick {}  liq {}  slot {}",
            pool_label(pool),
            f64::from(pool.fee_rate_in_millionths) / 100.0,
            pool.human_readable_price_token_b_per_token_a(),
            pool.current_tick_index,
            pool.active_liquidity_at_current_price,
            pool.slot,
        ),
    );
}

pub fn starting_to_watch_tick_arrays(dex: DexProgram, new_tick_array_count: usize) {
    log_line!(
        "[pool]    {} price is near {new_tick_array_count} tick array(s) we were not watching; subscribing and loading them once via RPC",
        dex.name()
    );
}

/// Tick arrays are rewritten almost every slot, so these lines only appear with `LOG_VERBOSE=true`.
pub fn tick_array_account_updated(tick_array: &TickArrayAccountWithInitializedTicks) {
    let pool_address = encode_public_key_as_base58(&tick_array.pool_address);
    let pool = format!(
        "{}:{}",
        tick_array.dex.name(),
        &pool_address[..4.min(pool_address.len())]
    );
    crate::dashboard_events::liquidity_touch(
        tick_array.dex.name(),
        &pool,
        tick_array.slot,
        tick_array.initialized_ticks.len(),
        tick_array.start_tick_index,
    );
    crate::dashboard_events::geyser(
        tick_array.slot,
        tick_array.dex.name(),
        "tickArray",
        &format!(
            "TICKS {}  start {}  initialized {}  slot {}",
            tick_array.dex.name(),
            tick_array.start_tick_index,
            tick_array.initialized_ticks.len(),
            tick_array.slot,
        ),
    );
    if !super::output::verbose() {
        return;
    }
    log_line!(
        "[geyser/tick-array] {:<14}  pool={}  start_tick={:<8} slot={:<10} initialized_ticks={}",
        tick_array.dex.name(),
        shorten_public_key_for_logs(&tick_array.pool_address),
        tick_array.start_tick_index,
        tick_array.slot,
        tick_array.initialized_ticks.len(),
    );
}

pub fn rpc_tick_array_load_started(tick_array_count: usize) {
    log_line!(
        "[rpc/tick-array]  requesting {tick_array_count} tick-array account(s)  (current bytes, one time)"
    );
}

pub fn rpc_tick_array_load_finished(
    decoded: usize,
    requested: usize,
    missing_on_chain: usize,
    failed: usize,
) {
    log_line!(
        "[rpc/tick-array]  decoded {decoded}/{requested}  not_on_chain={missing_on_chain}  decode_failed={failed}"
    );
}

pub fn rpc_tick_array_load_failed(error: impl std::fmt::Display) {
    log_error!("[error] RPC tick-array load failed: {error}");
}

pub fn tick_array_subscribe_failed(error: impl std::fmt::Display) {
    log_error!("[error] failed to subscribe to tick-array accounts: {error}");
}

pub fn pool_decode_failed(dex: DexProgram, pool_address_base58: &str) {
    log_error!(
        "[error] could not decode {} pool {pool_address_base58}",
        dex.name()
    );
}

pub fn tick_array_decode_failed(
    dex: DexProgram,
    tick_array_address: &PublicKeyBytes,
    came_from_rpc: bool,
) {
    let source = if came_from_rpc { " (from RPC)" } else { "" };
    log_error!(
        "[error] could not decode {} tick array {}{source}",
        dex.name(),
        encode_public_key_as_base58(tick_array_address)
    );
}

pub fn raydium_fee_config_loaded(fee_rate_in_millionths: u16) {
    log_line!(
        "[rpc/fee-config]  raydium_clmm fee from amm_config: {fee_rate_in_millionths} millionths ({:.2} bps)",
        f64::from(fee_rate_in_millionths) / 100.0
    );
}

pub fn raydium_fee_config_load_failed() {
    log_error!("[error] could not load Raydium amm_config fee; using the configured fee");
}

pub fn geyser_stream_error(error: impl std::fmt::Display) {
    log_error!("[error] Geyser stream: {error}");
}
