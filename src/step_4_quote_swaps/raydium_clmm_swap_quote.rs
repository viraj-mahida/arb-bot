//! Quote a swap on a Raydium CLMM pool using the `solana_clmm_raydium` library.
//!
//! Unlike Orca's library, this one wants a single flat, sorted list of
//! initialized ticks rather than per-account tick arrays.

use solana_clmm_raydium::{InitializedTick as RaydiumInitializedTick, SwapPool, compute_swap_full};

use super::swap_quote_types::{SwapDirection, SwapQuoteForExactInput, WhySwapQuoteFailed};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks,
};

pub(super) fn quote_raydium_clmm_swap(
    pool: &ConcentratedLiquidityPoolState,
    cached_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    input_amount: u64,
    direction: SwapDirection,
) -> Result<SwapQuoteForExactInput, WhySwapQuoteFailed> {
    let mut all_initialized_ticks: Vec<RaydiumInitializedTick> = cached_tick_arrays
        .iter()
        .flat_map(|tick_array| {
            tick_array
                .initialized_ticks
                .iter()
                .map(|tick| RaydiumInitializedTick {
                    tick: tick.tick_index,
                    liquidity_net: tick.liquidity_added_when_price_crosses_upward,
                })
        })
        .collect();
    all_initialized_ticks.sort_by_key(|tick| tick.tick);
    // Overlapping cache windows could list the same tick twice; keep one.
    all_initialized_ticks.dedup_by_key(|tick| tick.tick);

    let pool_for_raydium = SwapPool {
        sqrt_price_x64: pool.sqrt_price_q64_64,
        liquidity: pool.active_liquidity_at_current_price,
        tick_current: pool.current_tick_index,
        tick_spacing: pool.tick_spacing,
        fee_rate_pips: u32::from(pool.fee_rate_in_millionths),
    };
    // 0 = no price limit: let the swap run until the input is used up or liquidity ends.
    let no_price_limit = 0;
    let is_exact_input = true;
    let result = compute_swap_full(
        &pool_for_raydium,
        &all_initialized_ticks,
        input_amount,
        no_price_limit,
        is_exact_input,
        direction.is_a_to_b(),
    )
    .map_err(|error| WhySwapQuoteFailed::RaydiumQuoteLibraryError(error.reason()))?;
    Ok(SwapQuoteForExactInput {
        input_amount_used: result.amount_in,
        output_amount: result.amount_out,
        fee_amount: result.fee_amount,
    })
}
