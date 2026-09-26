//! Chain two swap quotes into an arbitrage round trip.
//!
//! The idea in plain words: pool S pays 101 USDC per SOL, pool B sells SOL for
//! 100 USDC. Sell SOL on S, take the USDC to B, buy SOL back — you end with
//! more SOL than you started with, minus two small fees. Real bots do the two
//! legs inside one transaction so either both happen or neither does.
//!
//! Same cycle, other way round: "buy cheap on B, sell dear on S" is the same
//! trade started from USDC instead of SOL. We start from SOL (token A).

use super::quote_swap_exact_input::quote_swap_exact_input;
use super::swap_quote_types::{SwapDirection, TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks,
};

/// Quote selling `start_token_amount_in` of token A on `sell_pool`, then
/// swapping all the token B received back into token A on `buy_pool`.
pub fn quote_two_pool_round_trip(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    start_token_amount_in: u64,
) -> Result<TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed> {
    let sell_leg = quote_swap_exact_input(
        sell_pool,
        sell_pool_tick_arrays,
        start_token_amount_in,
        SwapDirection::TokenAToTokenB,
    )?;
    let buy_leg = quote_swap_exact_input(
        buy_pool,
        buy_pool_tick_arrays,
        sell_leg.output_amount,
        SwapDirection::TokenBToTokenA,
    )?;
    Ok(TwoPoolArbitrageRoundTrip {
        sell_pool_dex: sell_pool.dex,
        buy_pool_dex: buy_pool.dex,
        start_token_amount_in,
        bridge_token_amount_between_legs: sell_leg.output_amount,
        start_token_amount_out: buy_leg.output_amount,
        both_swaps_fully_filled: sell_leg.input_amount_used == start_token_amount_in
            && buy_leg.input_amount_used == sell_leg.output_amount,
    })
}
