use super::PROBE_TRADE_INPUT_AMOUNT;
use super::test_pool_builders::{
    DEEP_LIQUIDITY, test_pool_at_tick_with_one_empty_tick_array,
    test_pool_with_one_empty_tick_array,
};
use crate::step_3_store_latest_pool_state::{
    DexProgram, InitializedTickWithLiquidityChange, LatestPoolStateCache,
};
use crate::step_4_quote_swaps::WhySwapQuoteFailed;
use crate::step_5_find_best_arbitrage_size::{
    main_find_input_amount_that_maximizes_profit, main_quote_most_profitable_two_pool_round_trip,
    main_quote_round_trips_touching_pool,
};

/// Same pool on both sides means no price gap at all, so the best size is zero.
#[test]
fn no_arbitrage_when_both_pools_have_the_same_price() {
    let (pool, tick_array) =
        test_pool_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY);
    let tick_arrays = [&tick_array];
    let error =
        main_quote_most_profitable_two_pool_round_trip(&pool, &tick_arrays, &pool, &tick_arrays)
            .unwrap_err();
    assert!(matches!(
        error,
        WhySwapQuoteFailed::NoProfitablePriceGapAfterFees
    ));
    assert_eq!(
        main_find_input_amount_that_maximizes_profit(&pool, &tick_arrays, &pool, &tick_arrays)
            .unwrap(),
        0
    );
}

/// A 4-tick gap is about 4 bps; two 4 bps fees cost 8 bps, so nothing is left.
#[test]
fn no_arbitrage_when_fees_are_bigger_than_the_price_gap() {
    let (sell_pool, sell_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 20);
    let (buy_pool, buy_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 16);
    let error = main_quote_most_profitable_two_pool_round_trip(
        &sell_pool,
        &[&sell_tick_array],
        &buy_pool,
        &[&buy_tick_array],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WhySwapQuoteFailed::NoProfitablePriceGapAfterFees
    ));
}

/// A fee-adjusted gap smaller than 1 USDC must not be walked onward to the next tick.
///
/// The closed-form size truncates that gap to 0. The next cached tick is still
/// hundreds of USDC away, so treating 0 as "skip this event" would size a trade
/// that is already past the profitable point.
#[test]
fn sub_unit_price_gap_stops_the_walk_instead_of_continuing_to_the_next_tick() {
    let liquidity = 1_000_000;
    let (mut sell_pool, sell_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, liquidity, 0);
    let (buy_pool, buy_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, liquidity, 0);
    // Both fees are 0.04%, so sqrt(k_sell * k_buy) = 0.9996. Sit just past that
    // breakeven: the float gap is still open, but it is under 1 USDC.
    let fee_kept = 0.9996_f64;
    let breakeven_sqrt_price = (buy_pool.sqrt_price_q64_64 as f64) / fee_kept;
    sell_pool.sqrt_price_q64_64 = (breakeven_sqrt_price * (1.0 + 1e-7)) as u128;

    let size = main_find_input_amount_that_maximizes_profit(
        &sell_pool,
        &[&sell_tick_array],
        &buy_pool,
        &[&buy_tick_array],
    )
    .unwrap();
    assert_eq!(size, 0);
}

/// A wide gap in deep pools is profitable, and the best size is far bigger than the 0.1 SOL probe.
#[test]
fn wide_price_gap_gives_profitable_size_larger_than_probe() {
    let (sell_pool, sell_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 160);
    let (buy_pool, buy_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 16);
    let best = main_quote_most_profitable_two_pool_round_trip(
        &sell_pool,
        &[&sell_tick_array],
        &buy_pool,
        &[&buy_tick_array],
    )
    .unwrap();
    assert!(best.both_swaps_fully_filled);
    assert!(best.profit_in_start_token() > 0);
    assert!(best.start_token_amount_in > PROBE_TRADE_INPUT_AMOUNT);
}

/// Crossing a tick that switches off all liquidity must stop the walk early, giving a smaller size.
#[test]
fn trade_size_stops_where_liquidity_runs_out_at_a_tick() {
    let (sell_pool, mut sell_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 160);
    let (buy_pool, buy_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 16);
    let size_without_tick = main_find_input_amount_that_maximizes_profit(
        &sell_pool,
        &[&sell_tick_array],
        &buy_pool,
        &[&buy_tick_array],
    )
    .unwrap();

    // Moving down across tick 152 removes all of the pool's active liquidity.
    sell_tick_array
        .initialized_ticks
        .push(InitializedTickWithLiquidityChange {
            tick_index: 152,
            liquidity_added_when_price_crosses_upward: sell_pool.active_liquidity_at_current_price
                as i128,
        });
    let size_with_tick = main_find_input_amount_that_maximizes_profit(
        &sell_pool,
        &[&sell_tick_array],
        &buy_pool,
        &[&buy_tick_array],
    )
    .unwrap();
    assert!(size_with_tick > 0);
    assert!(size_with_tick < size_without_tick);
}

/// Sizing refuses to run without the tick array covering the current price.
#[test]
fn trade_sizing_fails_when_current_tick_array_is_missing() {
    let (pool, mut tick_array) =
        test_pool_with_one_empty_tick_array(DexProgram::RaydiumClmm, DEEP_LIQUIDITY);
    tick_array.start_tick_index += 10_000;
    tick_array
        .initialized_ticks
        .push(InitializedTickWithLiquidityChange {
            tick_index: tick_array.start_tick_index,
            liquidity_added_when_price_crosses_upward: 1,
        });
    let tick_arrays = [&tick_array];
    let error =
        main_quote_most_profitable_two_pool_round_trip(&pool, &tick_arrays, &pool, &tick_arrays)
            .unwrap_err();
    assert!(matches!(
        error,
        WhySwapQuoteFailed::TickArrayForCurrentPriceNotCachedYet
    ));
}

/// With one pool in the cache there is nothing to pair it with.
#[test]
fn no_round_trips_until_a_second_pool_with_the_same_mints_arrives() {
    let cache = LatestPoolStateCache::new();
    let (pool, _) = test_pool_with_one_empty_tick_array(DexProgram::RaydiumClmm, DEEP_LIQUIDITY);
    let pool_address = pool.pool_address;
    cache.save_pool_state(pool);
    assert!(
        main_quote_round_trips_touching_pool(&cache, &pool_address)
            .best_size_round_trips
            .is_empty()
    );
}

/// An update re-quotes the updated pool against every same-mint pool in the one
/// direction that can profit (sell the higher price), skips an equal price, and
/// ignores other mints.
#[test]
fn update_quotes_only_the_higher_price_sell_for_pairs_that_include_the_updated_pool() {
    let cache = LatestPoolStateCache::new();
    let pool_on_dex = |dex, address_byte: u8, tick: i32| {
        let (mut pool, _) = test_pool_at_tick_with_one_empty_tick_array(dex, DEEP_LIQUIDITY, tick);
        pool.pool_address = [address_byte; 32];
        pool
    };
    let updated = pool_on_dex(DexProgram::RaydiumClmm, 1, 100);
    cache.save_pool_state(updated.clone());
    cache.save_pool_state(pool_on_dex(DexProgram::RaydiumClmm, 2, 200));
    cache.save_pool_state(pool_on_dex(DexProgram::OrcaWhirlpool, 3, 50));
    cache.save_pool_state(pool_on_dex(DexProgram::OrcaWhirlpool, 5, 100));
    let mut other_mints = pool_on_dex(DexProgram::OrcaWhirlpool, 4, 300);
    other_mints.token_b_mint = [9; 32];
    cache.save_pool_state(other_mints);

    let quotes = main_quote_round_trips_touching_pool(&cache, &updated.pool_address);
    let directions: Vec<_> = quotes
        .best_size_round_trips
        .iter()
        .map(|quote| {
            (
                quote.sell_pool.pool_address[0],
                quote.buy_pool.pool_address[0],
            )
        })
        .collect();
    // Tick 200 is dearer than the update, so sell there. Tick 50 is cheaper, so
    // sell the update. Tick 100 matches, so that pair is skipped.
    assert_eq!(directions, vec![(2, 1), (1, 3)]);
}
