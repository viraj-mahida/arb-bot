//! A scratch copy of one pool that we can move along its ticks while sizing.
//!
//! The real cached pool is never modified. The walker starts from the pool's
//! current price and liquidity, then pretends to trade: its price moves, and
//! whenever it crosses an initialized tick, its active liquidity changes.

use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, InitializedTickWithLiquidityChange, TickArrayAccountWithInitializedTicks,
};
use crate::step_4_quote_swaps::{SwapDirection, WhySwapQuoteFailed, is_tick_array_for_current_price_cached};

pub(super) struct PoolPriceWalkerAlongTicks {
    /// Simulated current price, as `sqrt(price) * 2^64`.
    pub sqrt_price_q64_64: u128,
    /// Simulated current tick index.
    pub current_tick_index: i32,
    /// Simulated active liquidity `L` at the current price.
    pub active_liquidity: u128,
    /// Pool fee in millionths of the input.
    pub fee_rate_in_millionths: u32,
    /// All initialized ticks from the cached tick arrays, sorted low to high.
    initialized_ticks_sorted: Vec<InitializedTickWithLiquidityChange>,
    /// Lowest tick covered by cached data. Below it, liquidity is unknown.
    lowest_cached_tick_index: i32,
    /// Highest tick covered by cached data. Above it, liquidity is unknown.
    highest_cached_tick_index: i32,
}

/// The next place the walker must stop: an initialized tick, or the edge of the cache.
pub(super) struct NextTickBoundary {
    pub tick_index: i32,
    /// `true` = a real initialized tick we can cross.
    /// `false` = only the edge of cached data; we must stop, not guess.
    pub is_initialized_tick: bool,
}

impl PoolPriceWalkerAlongTicks {
    pub(super) fn new(
        pool: &ConcentratedLiquidityPoolState,
        cached_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    ) -> Result<Self, WhySwapQuoteFailed> {
        if cached_tick_arrays.is_empty() {
            return Err(WhySwapQuoteFailed::NoTickArraysCachedYet);
        }
        if !is_tick_array_for_current_price_cached(pool, cached_tick_arrays) {
            return Err(WhySwapQuoteFailed::TickArrayForCurrentPriceNotCachedYet);
        }
        let ticks_covered_by_one_array = pool.dex.ticks_per_tick_array() * i32::from(pool.tick_spacing);
        if ticks_covered_by_one_array == 0 {
            return Err(WhySwapQuoteFailed::TickSpacingIsZero);
        }
        let lowest_cached_tick_index = cached_tick_arrays
            .iter()
            .map(|tick_array| tick_array.start_tick_index)
            .min()
            .ok_or(WhySwapQuoteFailed::NoTickArraysCachedYet)?;
        let highest_cached_tick_index = cached_tick_arrays
            .iter()
            .map(|tick_array| tick_array.start_tick_index)
            .max()
            .ok_or(WhySwapQuoteFailed::NoTickArraysCachedYet)?
            + ticks_covered_by_one_array
            - 1;

        let mut initialized_ticks_sorted: Vec<InitializedTickWithLiquidityChange> = cached_tick_arrays
            .iter()
            .flat_map(|tick_array| tick_array.initialized_ticks.iter().copied())
            .collect();
        initialized_ticks_sorted.sort_by_key(|tick| tick.tick_index);
        initialized_ticks_sorted.dedup_by_key(|tick| tick.tick_index);

        Ok(Self {
            sqrt_price_q64_64: pool.sqrt_price_q64_64,
            current_tick_index: pool.current_tick_index,
            active_liquidity: pool.active_liquidity_at_current_price,
            fee_rate_in_millionths: u32::from(pool.fee_rate_in_millionths),
            initialized_ticks_sorted,
            lowest_cached_tick_index,
            highest_cached_tick_index,
        })
    }

    /// Where the price stops next when it moves DOWN (we are selling token A here).
    ///
    /// "At or below" because a tick exactly at the current index is a boundary
    /// we have not crossed yet when moving down.
    pub(super) fn next_initialized_tick_below_price(&self) -> NextTickBoundary {
        match self
            .initialized_ticks_sorted
            .iter()
            .rev()
            .find(|tick| tick.tick_index <= self.current_tick_index)
        {
            Some(tick) => NextTickBoundary { tick_index: tick.tick_index, is_initialized_tick: true },
            None => NextTickBoundary { tick_index: self.lowest_cached_tick_index, is_initialized_tick: false },
        }
    }

    /// Where the price stops next when it moves UP (we are buying token A here).
    pub(super) fn next_initialized_tick_above_price(&self) -> NextTickBoundary {
        match self
            .initialized_ticks_sorted
            .iter()
            .find(|tick| tick.tick_index > self.current_tick_index)
        {
            Some(tick) => NextTickBoundary { tick_index: tick.tick_index, is_initialized_tick: true },
            None => NextTickBoundary { tick_index: self.highest_cached_tick_index, is_initialized_tick: false },
        }
    }

    /// Move the price onto `tick_index` and switch liquidity on or off there.
    ///
    /// Example: liquidity 800, tick says `+300` when crossing upward.
    /// - Price moving up across it → 800 + 300 = 1,100.
    /// - Price moving down across it → 800 − 300 = 500.
    ///
    /// After crossing downward we sit at `tick_index - 1`, i.e. in the range
    /// just below the tick, so the next search does not find the same tick again.
    pub(super) fn cross_tick_and_update_liquidity(&mut self, tick_index: i32, direction: SwapDirection) {
        let liquidity_added_when_crossing_upward = self
            .initialized_ticks_sorted
            .iter()
            .find(|tick| tick.tick_index == tick_index)
            .map(|tick| tick.liquidity_added_when_price_crosses_upward)
            .unwrap_or(0);
        let moving_price_down = direction == SwapDirection::TokenAToTokenB;
        self.active_liquidity =
            liquidity_after_crossing(self.active_liquidity, liquidity_added_when_crossing_upward, moving_price_down);
        self.sqrt_price_q64_64 = orca_whirlpools_core::tick_index_to_sqrt_price(tick_index);
        self.current_tick_index = if moving_price_down { tick_index - 1 } else { tick_index };
    }
}

/// Apply a tick's liquidity change in the direction the price is moving.
///
/// Saturating math: a malformed tick can never wrap liquidity around to a huge number.
fn liquidity_after_crossing(active_liquidity: u128, liquidity_added_when_crossing_upward: i128, moving_price_down: bool) -> u128 {
    let change_size = liquidity_added_when_crossing_upward.unsigned_abs();
    let liquidity_goes_down = if moving_price_down {
        liquidity_added_when_crossing_upward >= 0
    } else {
        liquidity_added_when_crossing_upward < 0
    };
    if liquidity_goes_down {
        active_liquidity.saturating_sub(change_size)
    } else {
        active_liquidity.saturating_add(change_size)
    }
}
