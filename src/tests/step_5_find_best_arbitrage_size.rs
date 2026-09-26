use super::test_pool_builders::{
    DEEP_LIQUIDITY, test_pool_at_tick_with_one_empty_tick_array, test_pool_with_one_empty_tick_array,
};
use crate::step_3_store_latest_pool_state::{DexProgram, InitializedTickWithLiquidityChange};
use crate::step_4_quote_swaps::{PROBE_TRADE_INPUT_AMOUNT, WhySwapQuoteFailed};
use crate::step_5_find_best_arbitrage_size::{
    find_input_amount_that_maximizes_profit, quote_most_profitable_two_pool_round_trip,
};

/// Same pool on both sides means no price gap at all, so the best size is zero.
#[test]
fn no_arbitrage_when_both_pools_have_the_same_price() {
    let (pool, tick_array) = test_pool_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY);
    let tick_arrays = [tick_array];
    let error = quote_most_profitable_two_pool_round_trip(&pool, &tick_arrays, &pool, &tick_arrays).unwrap_err();
    assert!(matches!(error, WhySwapQuoteFailed::NoProfitablePriceGapAfterFees));
    assert_eq!(find_input_amount_that_maximizes_profit(&pool, &tick_arrays, &pool, &tick_arrays).unwrap(), 0);
}

/// A 4-tick gap is about 4 bps; two 4 bps fees cost 8 bps, so nothing is left.
#[test]
fn no_arbitrage_when_fees_are_bigger_than_the_price_gap() {
    let (sell_pool, sell_tick_array) = test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 20);
    let (buy_pool, buy_tick_array) = test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 16);
    let error =
        quote_most_profitable_two_pool_round_trip(&sell_pool, &[sell_tick_array], &buy_pool, &[buy_tick_array]).unwrap_err();
    assert!(matches!(error, WhySwapQuoteFailed::NoProfitablePriceGapAfterFees));
}

/// A wide gap in deep pools is profitable, and the best size is far bigger than the 0.1 SOL probe.
#[test]
fn wide_price_gap_gives_profitable_size_larger_than_probe() {
    let (sell_pool, sell_tick_array) = test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 160);
    let (buy_pool, buy_tick_array) = test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 16);
    let best =
        quote_most_profitable_two_pool_round_trip(&sell_pool, &[sell_tick_array], &buy_pool, &[buy_tick_array]).unwrap();
    assert!(best.both_swaps_fully_filled);
    assert!(best.profit_in_start_token() > 0);
    assert!(best.start_token_amount_in > PROBE_TRADE_INPUT_AMOUNT);
}

/// Crossing a tick that switches off all liquidity must stop the walk early, giving a smaller size.
#[test]
fn trade_size_stops_where_liquidity_runs_out_at_a_tick() {
    let (sell_pool, mut sell_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 160);
    let (buy_pool, buy_tick_array) = test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 16);
    let size_without_tick = find_input_amount_that_maximizes_profit(
        &sell_pool,
        std::slice::from_ref(&sell_tick_array),
        &buy_pool,
        std::slice::from_ref(&buy_tick_array),
    )
    .unwrap();

    // Moving down across tick 152 removes all of the pool's active liquidity.
    sell_tick_array.initialized_ticks.push(InitializedTickWithLiquidityChange {
        tick_index: 152,
        liquidity_added_when_price_crosses_upward: sell_pool.active_liquidity_at_current_price as i128,
    });
    let size_with_tick =
        find_input_amount_that_maximizes_profit(&sell_pool, &[sell_tick_array], &buy_pool, &[buy_tick_array]).unwrap();
    assert!(size_with_tick > 0);
    assert!(size_with_tick < size_without_tick);
}

/// Sizing refuses to run without the tick array covering the current price.
#[test]
fn trade_sizing_fails_when_current_tick_array_is_missing() {
    let (pool, mut tick_array) = test_pool_with_one_empty_tick_array(DexProgram::RaydiumClmm, DEEP_LIQUIDITY);
    tick_array.start_tick_index += 10_000;
    tick_array.initialized_ticks.push(InitializedTickWithLiquidityChange {
        tick_index: tick_array.start_tick_index,
        liquidity_added_when_price_crosses_upward: 1,
    });
    let tick_arrays = [tick_array];
    let error = quote_most_profitable_two_pool_round_trip(&pool, &tick_arrays, &pool, &tick_arrays).unwrap_err();
    assert!(matches!(error, WhySwapQuoteFailed::TickArrayForCurrentPriceNotCachedYet));
}
