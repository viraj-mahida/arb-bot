//! Lines printed while processing Geyser updates and RPC loads.

use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, PublicKeyBytes, TickArrayAccountWithInitializedTicks,
    encode_public_key_as_base58, shorten_public_key_for_logs,
};

pub fn pool_account_updated(
    pool: &ConcentratedLiquidityPoolState,
    loaded_tick_array_count: usize,
    watched_tick_array_count: usize,
) {
    println!(
        "[geyser/pool]  {:<14}  {}  slot={:<10} tick={:<7} spacing={}  liquidity={:<12.3e}  price={:.4} USDC/SOL  tick_arrays={}/{}",
        pool.dex.name(),
        shorten_public_key_for_logs(&pool.pool_address),
        pool.slot,
        pool.current_tick_index,
        pool.tick_spacing,
        pool.active_liquidity_at_current_price as f64,
        pool.human_readable_price_token_b_per_token_a(),
        loaded_tick_array_count,
        watched_tick_array_count,
    );
}

pub fn starting_to_watch_tick_arrays(dex: DexProgram, new_tick_array_count: usize) {
    println!(
        "               {new_tick_array_count} new tick array(s) near the {} price — subscribing on Geyser + loading once via RPC",
        dex.name()
    );
}

pub fn tick_array_account_updated(tick_array: &TickArrayAccountWithInitializedTicks) {
    println!(
        "[geyser/tick-array] {:<14}  pool={}  start_tick={:<8} slot={:<10} initialized_ticks={}",
        tick_array.dex.name(),
        shorten_public_key_for_logs(&tick_array.pool_address),
        tick_array.start_tick_index,
        tick_array.slot,
        tick_array.initialized_ticks.len(),
    );
}

pub fn rpc_tick_array_load_started(tick_array_count: usize) {
    println!(
        "[rpc/tick-array]  requesting {tick_array_count} tick-array account(s)  (current bytes, one time)"
    );
}

pub fn rpc_tick_array_load_finished(decoded: usize, requested: usize, missing_on_chain: usize, failed: usize) {
    println!(
        "[rpc/tick-array]  decoded {decoded}/{requested}  not_on_chain={missing_on_chain}  decode_failed={failed}"
    );
}

pub fn rpc_tick_array_load_failed(error: impl std::fmt::Display) {
    eprintln!("[error] RPC tick-array load failed: {error}");
}

pub fn tick_array_subscribe_failed(error: impl std::fmt::Display) {
    eprintln!("[error] failed to subscribe to tick-array accounts: {error}");
}

pub fn pool_decode_failed(dex: DexProgram, pool_address_base58: &str) {
    eprintln!("[error] could not decode {} pool {pool_address_base58}", dex.name());
}

pub fn tick_array_decode_failed(dex: DexProgram, tick_array_address: &PublicKeyBytes, came_from_rpc: bool) {
    let source = if came_from_rpc { " (from RPC)" } else { "" };
    eprintln!(
        "[error] could not decode {} tick array {}{source}",
        dex.name(),
        encode_public_key_as_base58(tick_array_address)
    );
}

pub fn geyser_stream_error(error: impl std::fmt::Display) {
    eprintln!("[error] Geyser stream: {error}");
}
