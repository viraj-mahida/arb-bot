//! **Sub-step 5.1.** Put it together: find the best size, then quote that exact size.
//!
//! **Start here:** [`main_quote_most_profitable_two_pool_round_trip`].

use super::find_input_amount_that_maximizes_profit::main_find_input_amount_that_maximizes_profit;
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks,
};
use crate::step_4_quote_swaps::{
    TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed, main_quote_two_pool_round_trip, pool_label,
};

/// Size the round trip from both tick books, then quote it with each DEX's own math.
///
/// The walk gives the size where profit peaks; Step 4's quote gives the exact
/// integer amounts the chain would produce, which is what a transaction's
/// `minimum_amount_out` would be based on.
pub fn main_quote_most_profitable_two_pool_round_trip(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
) -> Result<TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed> {
    let best_input_amount = main_find_input_amount_that_maximizes_profit(
        sell_pool,
        sell_pool_tick_arrays,
        buy_pool,
        buy_pool_tick_arrays,
    )?;
    if best_input_amount == 0 {
        return Err(WhySwapQuoteFailed::NoProfitablePriceGapAfterFees);
    }
    if crate::dashboard_events::curve_sample_due() {
        publish_profit_curve_samples(
            sell_pool,
            sell_pool_tick_arrays,
            buy_pool,
            buy_pool_tick_arrays,
            best_input_amount,
        );
    }
    main_quote_two_pool_round_trip(
        sell_pool,
        sell_pool_tick_arrays,
        buy_pool,
        buy_pool_tick_arrays,
        best_input_amount,
    )
}

/// A few exact quotes around the best size, so the visualizer can draw the
/// profit curve. Throttled by the caller.
fn publish_profit_curve_samples(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    best_input_amount: u64,
) {
    const LAMPORTS_PER_SOL: f64 = 1e9;
    const FRACTIONS: [f64; 6] = [0.2, 0.4, 0.6, 0.8, 1.0, 1.35];
    let mut points = Vec::with_capacity(FRACTIONS.len());
    for fraction in FRACTIONS {
        let size = ((best_input_amount as f64) * fraction) as u64;
        if size == 0 {
            continue;
        }
        let Ok(round_trip) = main_quote_two_pool_round_trip(
            sell_pool,
            sell_pool_tick_arrays,
            buy_pool,
            buy_pool_tick_arrays,
            size,
        ) else {
            continue;
        };
        points.push((
            round_trip.start_token_amount_in as f64 / LAMPORTS_PER_SOL,
            round_trip.profit_in_start_token() as f64 / LAMPORTS_PER_SOL,
        ));
    }
    if !points.is_empty() {
        crate::dashboard_events::profit_curve(
            &points,
            &pool_label(sell_pool),
            &pool_label(buy_pool),
        );
    }
}
