//! Check that we hold the tick array covering the pool's current price.
//!
//! A swap starts at the current price and walks outward. If the tick array
//! for the current price is missing, we do not know where the first liquidity
//! change is, and any quote would be a guess.

use crate::step_3_store_latest_pool_state::tick_array_pda_derivation::tick_array_start_index_containing_tick;
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks,
};

pub fn is_tick_array_for_current_price_cached(
    pool: &ConcentratedLiquidityPoolState,
    cached_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
) -> bool {
    let ticks_covered_by_one_array = pool.dex.ticks_per_tick_array() * i32::from(pool.tick_spacing);
    if ticks_covered_by_one_array == 0 {
        return false;
    }
    let current_window_start =
        tick_array_start_index_containing_tick(pool.current_tick_index, ticks_covered_by_one_array);
    cached_tick_arrays
        .iter()
        .any(|tick_array| tick_array.start_tick_index == current_window_start)
}
