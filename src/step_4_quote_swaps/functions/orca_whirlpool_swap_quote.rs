//! Quote a swap on an Orca Whirlpool using Orca's official `orca_whirlpools_core` library.
//!
//! The library does not read our structs; it wants its own "facade" types that
//! mirror the on-chain accounts. Most of this file is translating our cached
//! state into those facades.

use orca_whirlpools_core::{
    TICK_ARRAY_SIZE, TickArrayFacade, TickArrays, TickFacade, WhirlpoolFacade,
    swap_quote_by_input_token,
};

use super::super::swap_quote_types::{SwapDirection, SwapQuoteForExactInput, WhySwapQuoteFailed};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks,
};

/// Orca's quote function accepts between 1 and 6 tick arrays.
const MAXIMUM_TICK_ARRAYS_ORCA_ACCEPTS: usize = 6;

pub(crate) fn quote_orca_whirlpool_swap(
    pool: &ConcentratedLiquidityPoolState,
    cached_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
    input_amount: u64,
    direction: SwapDirection,
) -> Result<SwapQuoteForExactInput, WhySwapQuoteFailed> {
    let tick_array_facades = contiguous_orca_tick_array_facades(pool, cached_tick_arrays)?;
    let tick_arrays_for_orca = into_orca_tick_arrays_enum(tick_array_facades)?;
    let whirlpool_facade = WhirlpoolFacade {
        fee_tier_index_seed: pool.tick_spacing.to_le_bytes(),
        tick_spacing: pool.tick_spacing,
        fee_rate: pool.fee_rate_in_millionths,
        liquidity: pool.active_liquidity_at_current_price,
        sqrt_price: pool.sqrt_price_q64_64,
        tick_current_index: pool.current_tick_index,
        ..WhirlpoolFacade::default()
    };
    // Slippage tolerance 0: we want the exact expected output, not a minimum.
    let no_slippage_tolerance = 0;
    let quote = swap_quote_by_input_token(
        input_amount,
        direction.is_a_to_b(),
        no_slippage_tolerance,
        whirlpool_facade,
        None, // oracle (only for adaptive-fee pools)
        tick_arrays_for_orca,
        0,    // current Unix timestamp (only for adaptive-fee pools)
        None, // token A transfer fee (Token-2022 only)
        None, // token B transfer fee (Token-2022 only)
    )
    .map_err(WhySwapQuoteFailed::OrcaQuoteLibraryError)?;
    Ok(SwapQuoteForExactInput {
        input_amount_used: quote.token_in,
        output_amount: quote.token_est_out,
        fee_amount: quote.trade_fee,
    })
}

/// Build one facade per tick-array window, from the lowest cached window to
/// the highest, with no gaps.
///
/// Orca's walker steps from one array to the next and expects them to be
/// neighbours. If a window in between is not cached (usually because it does
/// not exist on-chain — nobody placed liquidity there), we insert an empty one.
fn contiguous_orca_tick_array_facades(
    pool: &ConcentratedLiquidityPoolState,
    cached_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
) -> Result<Vec<TickArrayFacade>, WhySwapQuoteFailed> {
    let ticks_covered_by_one_array = (TICK_ARRAY_SIZE as i32) * i32::from(pool.tick_spacing);
    if ticks_covered_by_one_array == 0 {
        return Err(WhySwapQuoteFailed::TickSpacingIsZero);
    }

    let tick_array_by_start_tick: std::collections::HashMap<
        i32,
        &TickArrayAccountWithInitializedTicks,
    > = cached_tick_arrays
        .iter()
        .map(|tick_array| (tick_array.start_tick_index, *tick_array))
        .collect();
    let lowest_start = *tick_array_by_start_tick
        .keys()
        .min()
        .ok_or(WhySwapQuoteFailed::NoTickArraysCachedYet)?;
    let highest_start = *tick_array_by_start_tick
        .keys()
        .max()
        .ok_or(WhySwapQuoteFailed::NoTickArraysCachedYet)?;

    let mut facades = Vec::new();
    let mut window_start = lowest_start;
    while window_start <= highest_start {
        let facade = match tick_array_by_start_tick.get(&window_start) {
            Some(tick_array) => orca_facade_from_cached_tick_array(tick_array, pool.tick_spacing),
            None => empty_orca_tick_array_facade(window_start),
        };
        facades.push(facade);
        if facades.len() > MAXIMUM_TICK_ARRAYS_ORCA_ACCEPTS {
            return Err(WhySwapQuoteFailed::MoreThanSixTickArraysForOrca);
        }
        window_start += ticks_covered_by_one_array;
    }
    Ok(facades)
}

/// Put each initialized tick back into its fixed slot, the way Orca stores it
/// on-chain: slot `(tick_index - start_tick_index) / tick_spacing`.
fn orca_facade_from_cached_tick_array(
    tick_array: &TickArrayAccountWithInitializedTicks,
    tick_spacing: u16,
) -> TickArrayFacade {
    let mut tick_slots = [TickFacade::default(); TICK_ARRAY_SIZE];
    let tick_spacing = i32::from(tick_spacing);
    if tick_spacing != 0 {
        for tick in &tick_array.initialized_ticks {
            let slot_position = (tick.tick_index - tick_array.start_tick_index) / tick_spacing;
            if (0..TICK_ARRAY_SIZE as i32).contains(&slot_position) {
                tick_slots[slot_position as usize] = TickFacade {
                    initialized: true,
                    liquidity_net: tick.liquidity_added_when_price_crosses_upward,
                    // The swap math only reads `liquidity_net`. We do not decode
                    // `liquidity_gross`, so any non-zero value marks the tick as used.
                    liquidity_gross: tick
                        .liquidity_added_when_price_crosses_upward
                        .unsigned_abs(),
                    ..TickFacade::default()
                };
            }
        }
    }
    TickArrayFacade {
        start_tick_index: tick_array.start_tick_index,
        ticks: tick_slots,
    }
}

/// A window with no initialized ticks (liquidity stays constant across it).
fn empty_orca_tick_array_facade(start_tick_index: i32) -> TickArrayFacade {
    TickArrayFacade {
        start_tick_index,
        ticks: [TickFacade::default(); TICK_ARRAY_SIZE],
    }
}

/// Orca's API takes an enum with one variant per count (One, Two, … Six), not a list.
fn into_orca_tick_arrays_enum(
    facades: Vec<TickArrayFacade>,
) -> Result<TickArrays, WhySwapQuoteFailed> {
    Ok(match facades.as_slice() {
        [] => return Err(WhySwapQuoteFailed::NoTickArraysCachedYet),
        [a] => TickArrays::One(*a),
        [a, b] => TickArrays::Two(*a, *b),
        [a, b, c] => TickArrays::Three(*a, *b, *c),
        [a, b, c, d] => TickArrays::Four(*a, *b, *c, *d),
        [a, b, c, d, e] => TickArrays::Five(*a, *b, *c, *d, *e),
        [a, b, c, d, e, f] => TickArrays::Six(*a, *b, *c, *d, *e, *f),
        _ => return Err(WhySwapQuoteFailed::MoreThanSixTickArraysForOrca),
    })
}
