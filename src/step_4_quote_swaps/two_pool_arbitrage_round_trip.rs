//! **Sub-step 4.2.** Chain two swap quotes into an arbitrage round trip.
//!
//! **Start here:** [`main_quote_two_pool_round_trip`].
//!
//! The idea in plain words: pool S pays 101 USDC per SOL, pool B sells SOL for
//! 100 USDC. Sell SOL on S, take the USDC to B, buy SOL back — you end with
//! more SOL than you started with, minus two small fees. Real bots do the two
//! legs inside one transaction so either both happen or neither does.
//!
//! Same cycle, other way round: "buy cheap on B, sell dear on S" is the same
//! trade started from USDC instead of SOL. We start from SOL (token A).

use std::sync::Arc;

use super::quote_swap_exact_input::main_quote_swap_exact_input;
use super::swap_quote_types::{
    DirectedRoundTripQuote, SwapDirection, TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed,
    pool_label,
};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, LatestPoolStateCache, TickArrayAccountWithInitializedTicks,
};

/// Quote selling `start_token_amount_in` of token A on `sell_pool`, then
/// swapping all the token B received back into token A on `buy_pool`.
pub fn main_quote_two_pool_round_trip(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
    start_token_amount_in: u64,
) -> Result<TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed> {
    let sell_leg = main_quote_swap_exact_input(
        sell_pool,
        sell_pool_tick_arrays,
        start_token_amount_in,
        SwapDirection::TokenAToTokenB,
    )?;
    let buy_leg = main_quote_swap_exact_input(
        buy_pool,
        buy_pool_tick_arrays,
        sell_leg.output_amount,
        SwapDirection::TokenBToTokenA,
    )?;
    Ok(TwoPoolArbitrageRoundTrip {
        start_token_amount_in,
        bridge_token_amount_between_legs: sell_leg.output_amount,
        start_token_amount_out: buy_leg.output_amount,
        both_swaps_fully_filled: sell_leg.input_amount_used == start_token_amount_in
            && buy_leg.input_amount_used == sell_leg.output_amount,
    })
}

/// One cached pool plus the tick arrays cached for it.
pub(crate) struct CachedPoolWithTickArrays {
    pub pool: Arc<ConcentratedLiquidityPoolState>,
    pub tick_arrays: Vec<Arc<TickArrayAccountWithInitializedTicks>>,
}

impl CachedPoolWithTickArrays {
    pub(crate) fn from_cache(
        cache: &LatestPoolStateCache,
        pool: Arc<ConcentratedLiquidityPoolState>,
    ) -> Self {
        // ignr: demo quote copy only (`DEMO_PRICE_SHIFT_BPS`). No-op otherwise; cache is not written.
        let pool = crate::dashboard_events::ignr_share_or_shift_pool(pool);
        Self {
            tick_arrays: cache.tick_arrays_for_pool(&pool.pool_address),
            pool,
        }
    }
}

fn tick_array_refs(
    tick_arrays: &[Arc<TickArrayAccountWithInitializedTicks>],
) -> Vec<&TickArrayAccountWithInitializedTicks> {
    tick_arrays.iter().map(Arc::as_ref).collect()
}

/// Quote selling token A on the more expensive pool and buying it back on the cheaper one.
///
/// Price is `sqrt_price` (token B per token A). Selling where that is higher and
/// buying where it is lower is the only direction that can profit. Fees can
/// close a small gap, but they cannot make the cheaper pool the right place to
/// sell, so the other direction is not quoted. Equal prices cannot profit
/// either way.
pub(crate) fn quote_sell_high_buy_low(
    updated_pool: &CachedPoolWithTickArrays,
    other_pool: &CachedPoolWithTickArrays,
    mut quote: impl FnMut(
        &ConcentratedLiquidityPoolState,
        &[&TickArrayAccountWithInitializedTicks],
        &ConcentratedLiquidityPoolState,
        &[&TickArrayAccountWithInitializedTicks],
    ) -> Result<TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed>,
) -> Option<DirectedRoundTripQuote> {
    let (sell, buy) = if updated_pool.pool.sqrt_price_q64_64 > other_pool.pool.sqrt_price_q64_64 {
        (updated_pool, other_pool)
    } else if other_pool.pool.sqrt_price_q64_64 > updated_pool.pool.sqrt_price_q64_64 {
        (other_pool, updated_pool)
    } else {
        return None;
    };
    let sell_tick_arrays = tick_array_refs(&sell.tick_arrays);
    let buy_tick_arrays = tick_array_refs(&buy.tick_arrays);
    Some(DirectedRoundTripQuote {
        sell_pool_label: pool_label(&sell.pool),
        buy_pool_label: pool_label(&buy.pool),
        result: quote(&sell.pool, &sell_tick_arrays, &buy.pool, &buy_tick_arrays),
        sell_pool: Arc::clone(&sell.pool),
        sell_pool_tick_arrays: sell.tick_arrays.clone(),
        buy_pool: Arc::clone(&buy.pool),
        buy_pool_tick_arrays: buy.tick_arrays.clone(),
    })
}
