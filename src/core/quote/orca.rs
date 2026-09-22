use super::super::types::{ClmmPoolState, TickArraySnapshot};
use super::types::{Quote, QuoteError};

use orca_whirlpools_core::{
    TICK_ARRAY_SIZE, TickArrayFacade, TickArrays, TickFacade, WhirlpoolFacade,
    swap_quote_by_input_token,
};

/// Orca Whirlpool quote via `orca_whirlpools_core`.
///
/// We convert our cached snapshots into Orca's `TickArrayFacade` / `WhirlpoolFacade`
/// types, then call their `swap_quote_by_input_token` — the same math Orca uses.
pub(crate) fn quote_orca( //TODO? (crate) really needed?
    pool: &ClmmPoolState,
    arrays: &[TickArraySnapshot],
    amount_in: u64,
    a_to_b: bool,
) -> Result<Quote, QuoteError> {
    let facades = orca_facades(pool, arrays)?;
    // Orca's API takes a fixed-size enum (1–6 arrays), not a `Vec`.
    let tick_arrays = into_tick_arrays(facades)?;
    // `WhirlpoolFacade` is a lightweight copy of on-chain pool fields needed for quoting.
    let whirlpool = WhirlpoolFacade {
        fee_tier_index_seed: pool.tick_spacing.to_le_bytes(),
        tick_spacing: pool.tick_spacing,
        fee_rate: pool.fee_rate,
        liquidity: pool.liquidity,
        sqrt_price: pool.sqrt_price_x64,
        tick_current_index: pool.tick,
        ..WhirlpoolFacade::default()
    };
    let quoted = swap_quote_by_input_token(
        amount_in,
        a_to_b,
        0,
        whirlpool,
        None,
        tick_arrays,
        0,
        None,
        None,
    )
    .map_err(QuoteError::Orca)?;
    Ok(Quote {
        amount_in: quoted.token_in,
        amount_out: quoted.token_est_out,
        fee: quoted.trade_fee,
    })
}

/// Build a contiguous sequence of Orca tick-array facades from cached snapshots.
///
/// Orca expects every array between `min` and `max` in the cache window, even
/// if some slots are empty (no liquidity initialized). Gaps are filled with
/// `empty_orca_facade`. Capped at 6 arrays — Orca's quote API limit.
fn orca_facades(
    pool: &ClmmPoolState,
    arrays: &[TickArraySnapshot],
) -> Result<Vec<TickArrayFacade>, QuoteError> {
    let width = (TICK_ARRAY_SIZE as i32).saturating_mul(i32::from(pool.tick_spacing));
    if width == 0 {
        return Err(QuoteError::BadTickSpacing);
    }

    // Index snapshots by their window start so we can walk min → max in order.
    let mut by_start = std::collections::HashMap::new();
    for array in arrays {
        by_start.insert(array.start_tick_index, array);
    }
    let mut starts: Vec<i32> = by_start.keys().copied().collect();
    starts.sort();
    let min = *starts.first().ok_or(QuoteError::NoTickArrays)?;
    let max = *starts.last().ok_or(QuoteError::NoTickArrays)?;

    let mut facades = Vec::new();
    let mut start = min;
    while start <= max {
        let facade = match by_start.get(&start) {
            Some(snap) => to_orca_facade(snap, pool.tick_spacing),
            // Missing window in the span: still pass an empty array so Orca's walk is contiguous.
            None => empty_orca_facade(start),
        };
        facades.push(facade);
        if facades.len() > 6 {
            return Err(QuoteError::TooManyArrays);
        }
        start = start.saturating_add(width);
    }
    Ok(facades)
}

/// Map our `TickArraySnapshot` into Orca's fixed-size `[TickFacade; 88]` layout.
///
/// Each initialized tick is placed at slot `(tick - start) / tick_spacing`
/// inside the array — matching how Whirlpool stores ticks on-chain.
fn to_orca_facade(array: &TickArraySnapshot, tick_spacing: u16) -> TickArrayFacade {
    let mut ticks = [TickFacade::default(); TICK_ARRAY_SIZE];
    let spacing = i32::from(tick_spacing);
    if spacing != 0 {
        for tick in &array.ticks {
            let idx = (tick.tick - array.start_tick_index) / spacing;
            if (0..TICK_ARRAY_SIZE as i32).contains(&idx) {
                ticks[idx as usize] = TickFacade {
                    initialized: true,
                    liquidity_net: tick.liquidity_net,
                    liquidity_gross: tick.liquidity_net.unsigned_abs(),
                    ..TickFacade::default()
                };
            }
        }
    }
    TickArrayFacade {
        start_tick_index: array.start_tick_index,
        ticks,
    }
}

/// Placeholder for a tick-array window we haven't cached (all ticks uninitialized).
fn empty_orca_facade(start_tick_index: i32) -> TickArrayFacade {
    TickArrayFacade {
        start_tick_index,
        ticks: [TickFacade::default(); TICK_ARRAY_SIZE],
    }
}

/// Convert `Vec` → Orca's `TickArrays` enum (supports exactly 1–6 arrays).
///
/// Rust enums with typed variants are used here instead of a slice because the
/// Orca quote function takes a fixed set of arrays by value.
fn into_tick_arrays(facades: Vec<TickArrayFacade>) -> Result<TickArrays, QuoteError> {
    Ok(match facades.as_slice() {
        [] => return Err(QuoteError::NoTickArrays),
        [a] => TickArrays::One(*a),
        [a, b] => TickArrays::Two(*a, *b),
        [a, b, c] => TickArrays::Three(*a, *b, *c),
        [a, b, c, d] => TickArrays::Four(*a, *b, *c, *d),
        [a, b, c, d, e] => TickArrays::Five(*a, *b, *c, *d, *e),
        [a, b, c, d, e, f] => TickArrays::Six(*a, *b, *c, *d, *e, *f),
        _ => return Err(QuoteError::TooManyArrays),
    })
}
