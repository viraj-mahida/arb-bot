//! Fake pools and tick arrays for tests: known price, known liquidity, no network.

use crate::step_3_store_latest_pool_state::tick_array_pda_derivation::tick_array_start_index_containing_tick;
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, TickArrayAccountWithInitializedTicks,
};

/// Very deep liquidity, so a 0.1 SOL trade barely moves the price.
pub const DEEP_LIQUIDITY: u128 = 1_000_000_000_000_000;

/// Fee used by every test pool: 400 millionths = 0.04%.
const TEST_FEE_RATE_IN_MILLIONTHS: u16 = 400;

fn tick_spacing_for(dex: DexProgram) -> u16 {
    match dex {
        DexProgram::OrcaWhirlpool => 4,
        DexProgram::RaydiumClmm => 1,
    }
}

/// A pool whose price sits a few ticks inside its tick array, so a small
/// price-down swap does not leave the cached window.
pub fn test_pool_with_one_empty_tick_array(
    dex: DexProgram,
    liquidity: u128,
) -> (ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks) {
    let tick_a_few_steps_inside_the_array = i32::from(tick_spacing_for(dex)) * 4;
    test_pool_at_tick_with_one_empty_tick_array(dex, liquidity, tick_a_few_steps_inside_the_array)
}

/// A pool at `current_tick_index` (price = 1.0001^tick), with one tick array
/// covering that price and no initialized ticks (constant liquidity).
pub fn test_pool_at_tick_with_one_empty_tick_array(
    dex: DexProgram,
    liquidity: u128,
    current_tick_index: i32,
) -> (ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks) {
    let tick_spacing = tick_spacing_for(dex);
    let ticks_covered_by_one_array = dex.ticks_per_tick_array() * i32::from(tick_spacing);
    let pool = ConcentratedLiquidityPoolState {
        pool_address: [1u8; 32],
        dex,
        sqrt_price_q64_64: orca_whirlpools_core::tick_index_to_sqrt_price(current_tick_index),
        active_liquidity_at_current_price: liquidity,
        current_tick_index,
        tick_spacing,
        fee_rate_in_millionths: TEST_FEE_RATE_IN_MILLIONTHS,
        token_a_mint: [0u8; 32],
        token_b_mint: [0u8; 32],
        token_a_vault: [0u8; 32],
        token_b_vault: [0u8; 32],
        token_a_decimals: 9,
        token_b_decimals: 6,
        slot: 1,
        geyser_write_version_for_ordering: 1,
    };
    let tick_array = TickArrayAccountWithInitializedTicks {
        tick_array_address: [2u8; 32],
        pool_address: pool.pool_address,
        dex,
        start_tick_index: tick_array_start_index_containing_tick(current_tick_index, ticks_covered_by_one_array),
        initialized_ticks: Vec::new(),
        slot: 1,
        geyser_write_version_for_ordering: 1,
    };
    (pool, tick_array)
}
