use crate::core::quote::{
    QuoteError, PROBE_SOL_LAMPORTS, best_round_trip, max_pnl_sol_in, quote_exact_in, round_trip,
};
use crate::core::tick_array::start_index;
use crate::core::{ClmmPoolState, InitializedTick, TickArraySnapshot, Venue};

fn test_pool(venue: Venue, liquidity: u128) -> (ClmmPoolState, TickArraySnapshot) {
    let tick_spacing = match venue {
        Venue::OrcaWhirlpool => 4,
        Venue::RaydiumClmm => 1,
    };
    // Interior tick so a_to_b can walk down without leaving the current array.
    test_pool_at(venue, liquidity, tick_spacing as i32 * 4)
}

fn test_pool_at(venue: Venue, liquidity: u128, tick: i32) -> (ClmmPoolState, TickArraySnapshot) {
    let tick_spacing = match venue {
        Venue::OrcaWhirlpool => 4,
        Venue::RaydiumClmm => 1,
    };
    let width = venue.ticks_per_array() * i32::from(tick_spacing);
    let pool = ClmmPoolState {
        pubkey: [1u8; 32],
        venue,
        sqrt_price_x64: orca_whirlpools_core::tick_index_to_sqrt_price(tick),
        liquidity,
        tick,
        tick_spacing,
        fee_rate: 400,
        mint_a: [0u8; 32],
        mint_b: [0u8; 32],
        vault_a: [0u8; 32],
        vault_b: [0u8; 32],
        decimals_a: 9,
        decimals_b: 6,
        slot: 1,
        write_version: 1,
    };
    let array = TickArraySnapshot {
        pubkey: [2u8; 32],
        pool: pool.pubkey,
        venue,
        start_tick_index: start_index(tick, width),
        ticks: Vec::new(),
        slot: 1,
        write_version: 1,
    };
    (pool, array)
}

#[test]
fn orca_exact_in_fills_probe() {
    let (pool, array) = test_pool(Venue::OrcaWhirlpool, 1_000_000_000_000_000);
    let quote = quote_exact_in(&pool, &[array], PROBE_SOL_LAMPORTS, true).unwrap();
    assert_eq!(quote.amount_in, PROBE_SOL_LAMPORTS);
    assert!(quote.amount_out > 99_000_000);
    assert!(quote.amount_out < 101_000_000);
}

#[test]
fn raydium_exact_in_fills_probe() {
    let (pool, array) = test_pool(Venue::RaydiumClmm, 1_000_000_000_000_000);
    let quote = quote_exact_in(&pool, &[array], PROBE_SOL_LAMPORTS, true).unwrap();
    assert_eq!(quote.amount_in, PROBE_SOL_LAMPORTS);
    assert!(quote.amount_out > 99_000_000);
    assert!(quote.amount_out < 101_000_000);
}

#[test]
fn same_pool_round_trip_loses_two_fees() {
    let (pool, array) = test_pool(Venue::OrcaWhirlpool, 1_000_000_000_000_000);
    let arrays = [array];
    let rt = round_trip(&pool, &arrays, &pool, &arrays, PROBE_SOL_LAMPORTS).unwrap();
    assert!(rt.fully_filled);
    assert!(rt.sol_out < rt.sol_in);
    assert!(rt.pnl_lamports() > -200_000);
}

#[test]
fn missing_current_array_errors() {
    let (pool, mut array) = test_pool(Venue::RaydiumClmm, 1);
    array.start_tick_index += 10_000;
    array.ticks.push(InitializedTick {
        tick: array.start_tick_index,
        liquidity_net: 1,
    });
    let err = quote_exact_in(&pool, &[array], PROBE_SOL_LAMPORTS, true).unwrap_err();
    assert!(matches!(err, QuoteError::MissingCurrentArray));
}

#[test]
fn same_pool_size_search_has_no_edge() {
    let (pool, array) = test_pool(Venue::OrcaWhirlpool, 1_000_000_000_000_000);
    let arrays = [array];
    let err = best_round_trip(&pool, &arrays, &pool, &arrays).unwrap_err();
    assert!(matches!(err, QuoteError::NoEdge));
    assert_eq!(max_pnl_sol_in(&pool, &arrays, &pool, &arrays).unwrap(), 0);
}

#[test]
fn fees_eat_a_narrow_spread() {
    // 4 ticks ≈ 4 bps of price; two 4 bp fees = 8 bps, so no leftover edge.
    let (sell, sell_array) = test_pool_at(Venue::OrcaWhirlpool, 1_000_000_000_000_000, 20);
    let (buy, buy_array) = test_pool_at(Venue::OrcaWhirlpool, 1_000_000_000_000_000, 16);
    let err = best_round_trip(&sell, &[sell_array], &buy, &[buy_array]).unwrap_err();
    assert!(matches!(err, QuoteError::NoEdge));
}

#[test]
fn spread_size_comes_from_the_book() {
    let (sell, sell_array) = test_pool_at(Venue::OrcaWhirlpool, 1_000_000_000_000_000, 160);
    let (buy, buy_array) = test_pool_at(Venue::OrcaWhirlpool, 1_000_000_000_000_000, 16);
    let best = best_round_trip(&sell, &[sell_array], &buy, &[buy_array]).unwrap();
    assert!(best.fully_filled);
    assert!(best.pnl_lamports() > 0);
    assert!(best.sol_in > PROBE_SOL_LAMPORTS);
}

#[test]
fn initialized_tick_caps_size() {
    let (sell, mut sell_array) = test_pool_at(Venue::OrcaWhirlpool, 1_000_000_000_000_000, 160);
    let (buy, buy_array) = test_pool_at(Venue::OrcaWhirlpool, 1_000_000_000_000_000, 16);
    let open = max_pnl_sol_in(&sell, &[sell_array.clone()], &buy, &[buy_array.clone()]).unwrap();

    // Crossing this tick downward removes all active liquidity, so the walk must stop.
    sell_array.ticks.push(InitializedTick {
        tick: 152,
        liquidity_net: sell.liquidity as i128,
    });
    let capped = max_pnl_sol_in(&sell, &[sell_array], &buy, &[buy_array]).unwrap();
    assert!(capped > 0);
    assert!(capped < open);
}

#[test]
fn size_search_errors_without_current_array() {
    let (pool, mut array) = test_pool(Venue::RaydiumClmm, 1_000_000_000_000_000);
    array.start_tick_index += 10_000;
    array.ticks.push(InitializedTick {
        tick: array.start_tick_index,
        liquidity_net: 1,
    });
    let arrays = [array];
    let err = best_round_trip(&pool, &arrays, &pool, &arrays).unwrap_err();
    assert!(matches!(err, QuoteError::MissingCurrentArray));
}
