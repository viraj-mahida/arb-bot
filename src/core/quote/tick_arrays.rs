use super::super::tick_array::start_index;
use super::super::types::{ClmmPoolState, TickArraySnapshot};

/// Whether the cached tick arrays include the one covering the pool's current price.
///
/// Tick arrays are fixed-width windows on the price ladder. `start_index` maps
/// the pool's current tick to the window it lives in; we must have that window
/// loaded or the quote cannot start.
pub(crate) fn has_current_array(pool: &ClmmPoolState, arrays: &[TickArraySnapshot]) -> bool {
    // Width = number of price ticks per on-chain tick-array account.
    let width = pool
        .venue
        .ticks_per_array()
        .saturating_mul(i32::from(pool.tick_spacing));
    if width == 0 {
        return false;
    }
    let start = start_index(pool.tick, width);
    arrays.iter().any(|array| array.start_tick_index == start)
}