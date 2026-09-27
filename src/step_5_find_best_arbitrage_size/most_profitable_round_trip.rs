//! **Sub-step 5.1.** Put it together: find the best size, then quote that exact size.
//!
//! **Start here:** [`main_quote_most_profitable_two_pool_round_trip`].

use super::main_find_input_amount_that_maximizes_profit;
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks,
};
use crate::step_4_quote_swaps::{
    TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed, main_quote_two_pool_round_trip,
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
    main_quote_two_pool_round_trip(
        sell_pool,
        sell_pool_tick_arrays,
        buy_pool,
        buy_pool_tick_arrays,
        best_input_amount,
    )
}
