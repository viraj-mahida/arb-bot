//! The table of cached pools printed after every pool update.

use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, LatestPoolStateCache, shorten_public_key_for_logs,
};

pub fn pool_snapshot(cache: &LatestPoolStateCache) {
    println!("[snapshot]  {} pool(s) cached", cache.pool_count());
    println!(
        "  {:<14}  {:<10}  {:>10}  {:>7}  {:>10}  {:>11}  {:>6}",
        "dex", "pool", "slot", "tick", "USDC/SOL", "tick_arrays", "ticks"
    );

    let mut pools: Vec<ConcentratedLiquidityPoolState> = cache.all_pool_states();
    pools.sort_by_key(|pool| match pool.dex {
        DexProgram::RaydiumClmm => 0u8,
        DexProgram::OrcaWhirlpool => 1,
    });

    for pool in &pools {
        let tick_arrays = cache.tick_arrays_for_pool(&pool.pool_address);
        let initialized_tick_count: usize = tick_arrays
            .iter()
            .map(|tick_array| tick_array.initialized_ticks.len())
            .sum();
        let watched_tick_array_count = cache.watched_tick_array_count_for_pool(&pool.pool_address);
        println!(
            "  {:<14}  {:<10}  {:>10}  {:>7}  {:>10.4}  {:>8}/{:<2}  {:>6}",
            pool.dex.name(),
            shorten_public_key_for_logs(&pool.pool_address),
            pool.slot,
            pool.current_tick_index,
            pool.human_readable_price_token_b_per_token_a(),
            tick_arrays.len(),
            watched_tick_array_count,
            initialized_tick_count,
        );
    }

    let raydium_pool = cache.first_pool_on_dex(DexProgram::RaydiumClmm);
    let orca_pool = cache.first_pool_on_dex(DexProgram::OrcaWhirlpool);
    match (raydium_pool, orca_pool) {
        (Some(raydium_pool), Some(orca_pool)) => {
            let raydium_price = raydium_pool.human_readable_price_token_b_per_token_a();
            let orca_price = orca_pool.human_readable_price_token_b_per_token_a();
            let price_gap = raydium_price - orca_price;
            let midpoint_price = (raydium_price + orca_price) / 2.0;
            // 1 basis point = 0.01%, the usual unit for small price differences.
            let price_gap_in_basis_points = if midpoint_price > 0.0 {
                price_gap / midpoint_price * 10_000.0
            } else {
                0.0
            };
            println!(
                "  gap raydium−orca = {price_gap:+.4} USDC/SOL  ({price_gap_in_basis_points:+.2} bps of mid)  — spot prices only, fees not included"
            );
        }
        _ => println!("  (need both pools in the cache before a price gap can be shown)"),
    }
    println!();
}
