use orca_whirlpools_core::tick_index_to_sqrt_price;

use super::super::tick_arrays;
use super::super::types::QuoteError;
use crate::core::types::{ClmmPoolState, InitializedTick, TickArraySnapshot};

/// One pool's live curve for the dual walk: price, active L, and nearby ticks.
pub(super) struct Side {
    pub sqrt: u128,
    pub tick: i32,
    pub liq: u128,
    pub fee: u32,
    ticks: Vec<InitializedTick>,
    window_lo: i32,
    window_hi: i32,
}

impl Side {
    pub(super) fn new(
        pool: &ClmmPoolState,
        arrays: &[TickArraySnapshot],
    ) -> Result<Self, QuoteError> {
        if arrays.is_empty() {
            return Err(QuoteError::NoTickArrays);
        }
        if !tick_arrays::has_current_array(pool, arrays) {
            return Err(QuoteError::MissingCurrentArray);
        }
        let width = pool
            .venue
            .ticks_per_array()
            .saturating_mul(i32::from(pool.tick_spacing));
        if width == 0 {
            return Err(QuoteError::BadTickSpacing);
        }
        let window_lo = arrays
            .iter()
            .map(|a| a.start_tick_index)
            .min()
            .ok_or(QuoteError::NoTickArrays)?;
        let window_hi = arrays
            .iter()
            .map(|a| a.start_tick_index)
            .max()
            .ok_or(QuoteError::NoTickArrays)?
            .saturating_add(width)
            .saturating_sub(1);

        let mut ticks: Vec<InitializedTick> = arrays
            .iter()
            .flat_map(|array| array.ticks.iter().copied())
            .collect();
        ticks.sort_by_key(|t| t.tick);
        ticks.dedup_by_key(|t| t.tick);

        Ok(Self {
            sqrt: pool.sqrt_price_x64,
            tick: pool.tick,
            liq: pool.liquidity,
            fee: u32::from(pool.fee_rate),
            ticks,
            window_lo,
            window_hi,
        })
    }

    /// Next initialized tick at or below price, else the cached window floor.
    pub(super) fn next_down(&self) -> (i32, bool) {
        match self.ticks.iter().rev().find(|t| t.tick <= self.tick) {
            Some(t) => (t.tick, true),
            None => (self.window_lo, false),
        }
    }

    /// Next initialized tick above price, else the cached window ceiling.
    pub(super) fn next_up(&self) -> (i32, bool) {
        match self.ticks.iter().find(|t| t.tick > self.tick) {
            Some(t) => (t.tick, true),
            None => (self.window_hi, false),
        }
    }

    fn net_at(&self, tick: i32) -> i128 {
        self.ticks
            .iter()
            .find(|t| t.tick == tick)
            .map(|t| t.liquidity_net)
            .unwrap_or(0)
    }

    /// Cross `tick` and apply `liquidity_net`. `a_to_b` is price-down (sell SOL).
    pub(super) fn cross(&mut self, tick: i32, a_to_b: bool) {
        self.liq = cross_liquidity(self.liq, self.net_at(tick), a_to_b);
        self.sqrt = tick_index_to_sqrt_price(tick);
        self.tick = if a_to_b { tick.saturating_sub(1) } else { tick };
    }
}

fn cross_liquidity(liq: u128, net: i128, a_to_b: bool) -> u128 {
    let mag = net.unsigned_abs();
    let subtract = if a_to_b { net >= 0 } else { net < 0 };
    if subtract {
        liq.saturating_sub(mag)
    } else {
        liq.saturating_add(mag)
    }
}
