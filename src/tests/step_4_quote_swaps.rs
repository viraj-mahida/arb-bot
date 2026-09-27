use super::test_pool_builders::{DEEP_LIQUIDITY, test_pool_with_one_empty_tick_array};
use crate::step_3_store_latest_pool_state::{DexProgram, InitializedTickWithLiquidityChange};
use crate::step_4_quote_swaps::{
    PROBE_TRADE_INPUT_AMOUNT, SwapDirection, WhySwapQuoteFailed, main_quote_swap_exact_input,
    main_quote_two_pool_round_trip,
};

/// At price ≈ 1 raw USDC unit per lamport in a deep pool, 0.1 SOL in ≈ 100 USDC-units out.
#[test]
fn orca_quote_fills_small_trade_in_deep_pool() {
    let (pool, tick_array) =
        test_pool_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY);
    let quote = main_quote_swap_exact_input(
        &pool,
        &[tick_array],
        PROBE_TRADE_INPUT_AMOUNT,
        SwapDirection::TokenAToTokenB,
    )
    .unwrap();
    assert_eq!(quote.input_amount_used, PROBE_TRADE_INPUT_AMOUNT);
    assert!(quote.output_amount > 99_000_000);
    assert!(quote.output_amount < 101_000_000);
}

/// Same scenario through Raydium's library gives the same kind of answer.
#[test]
fn raydium_quote_fills_small_trade_in_deep_pool() {
    let (pool, tick_array) =
        test_pool_with_one_empty_tick_array(DexProgram::RaydiumClmm, DEEP_LIQUIDITY);
    let quote = main_quote_swap_exact_input(
        &pool,
        &[tick_array],
        PROBE_TRADE_INPUT_AMOUNT,
        SwapDirection::TokenAToTokenB,
    )
    .unwrap();
    assert_eq!(quote.input_amount_used, PROBE_TRADE_INPUT_AMOUNT);
    assert!(quote.output_amount > 99_000_000);
    assert!(quote.output_amount < 101_000_000);
}

/// Swapping out and back on the same pool can only lose: you pay the fee twice.
#[test]
fn round_trip_on_same_pool_loses_two_fees() {
    let (pool, tick_array) =
        test_pool_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY);
    let tick_arrays = [tick_array];
    let round_trip = main_quote_two_pool_round_trip(
        &pool,
        &tick_arrays,
        &pool,
        &tick_arrays,
        PROBE_TRADE_INPUT_AMOUNT,
    )
    .unwrap();
    assert!(round_trip.both_swaps_fully_filled);
    assert!(round_trip.start_token_amount_out < round_trip.start_token_amount_in);
    assert!(round_trip.profit_in_start_token() > -200_000);
}

/// Without the tick array for the current price, quoting must refuse instead of guessing.
#[test]
fn quote_fails_when_current_tick_array_is_missing() {
    let (pool, mut tick_array) = test_pool_with_one_empty_tick_array(DexProgram::RaydiumClmm, 1);
    tick_array.start_tick_index += 10_000;
    tick_array
        .initialized_ticks
        .push(InitializedTickWithLiquidityChange {
            tick_index: tick_array.start_tick_index,
            liquidity_added_when_price_crosses_upward: 1,
        });
    let error = main_quote_swap_exact_input(
        &pool,
        &[tick_array],
        PROBE_TRADE_INPUT_AMOUNT,
        SwapDirection::TokenAToTokenB,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WhySwapQuoteFailed::TickArrayForCurrentPriceNotCachedYet
    ));
}
