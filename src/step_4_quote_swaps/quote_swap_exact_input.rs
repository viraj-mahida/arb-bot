//! **Sub-step 4.1.** Quote one exact-input swap on one pool, whichever DEX it is on.
//!
//! **Start here:** [`main_quote_swap_exact_input`].
//!
//! We replay the DEX's own integer math locally (walking ticks, applying
//! fees), using each DEX's official Rust library. This is much faster than
//! asking an RPC node to simulate a transaction, and gives the same answer as
//! long as our cached state is current.

use super::functions::{
    is_tick_array_for_current_price_cached, quote_orca_whirlpool_swap, quote_raydium_clmm_swap,
};
use super::swap_quote_types::{SwapDirection, SwapQuoteForExactInput, WhySwapQuoteFailed};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, TickArrayAccountWithInitializedTicks,
};

/// How much output a swap of `input_amount` would return on `pool`.
pub fn main_quote_swap_exact_input(
    pool: &ConcentratedLiquidityPoolState,
    cached_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
    input_amount: u64,
    direction: SwapDirection,
) -> Result<SwapQuoteForExactInput, WhySwapQuoteFailed> {
    if input_amount == 0 {
        return Ok(SwapQuoteForExactInput {
            input_amount_used: 0,
            output_amount: 0,
            fee_amount: 0,
        });
    }
    if !is_tick_array_for_current_price_cached(pool, cached_tick_arrays) {
        return Err(if cached_tick_arrays.is_empty() {
            WhySwapQuoteFailed::NoTickArraysCachedYet
        } else {
            WhySwapQuoteFailed::TickArrayForCurrentPriceNotCachedYet
        });
    }
    match pool.dex {
        DexProgram::OrcaWhirlpool => {
            quote_orca_whirlpool_swap(pool, cached_tick_arrays, input_amount, direction)
        }
        DexProgram::RaydiumClmm => {
            quote_raydium_clmm_swap(pool, cached_tick_arrays, input_amount, direction)
        }
    }
}
