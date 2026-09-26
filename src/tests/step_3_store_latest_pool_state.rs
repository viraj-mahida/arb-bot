use crate::step_3_store_latest_pool_state::known_program_and_pool_addresses::ORCA_WHIRLPOOL_SOL_USDC_POOL_ADDRESS;
use crate::step_3_store_latest_pool_state::tick_array_pda_derivation::{
    derive_tick_array_pda_address, tick_array_start_index_containing_tick, tick_array_start_indices_near_tick,
};
use crate::step_3_store_latest_pool_state::{DexProgram, parse_base58_public_key};

/// Window starts use floor division, so negative ticks round toward minus infinity.
#[test]
fn tick_array_start_rounds_negative_ticks_down() {
    // tick 200, spacing 2, 88 slots → each array covers 176 ticks → window starts at 176.
    assert_eq!(tick_array_start_index_containing_tick(200, 2 * 88), 176);
    assert_eq!(tick_array_start_index_containing_tick(0, 88), 0);
    assert_eq!(tick_array_start_index_containing_tick(-1, 88), -88);
    assert_eq!(tick_array_start_index_containing_tick(-88, 88), -88);
    assert_eq!(tick_array_start_index_containing_tick(88, 88), 88);
}

/// We watch the current tick array plus two neighbours on each side.
#[test]
fn watched_tick_arrays_are_current_plus_two_on_each_side() {
    let window_starts = tick_array_start_indices_near_tick(200, 2, 88);
    assert_eq!(window_starts, vec![-176, 0, 176, 352, 528]);
}

/// Orca and Raydium encode the start tick differently in PDA seeds, so addresses differ.
#[test]
fn orca_and_raydium_derive_different_tick_array_addresses() {
    let pool_address = parse_base58_public_key(ORCA_WHIRLPOOL_SOL_USDC_POOL_ADDRESS);
    let orca_address = derive_tick_array_pda_address(DexProgram::OrcaWhirlpool, &pool_address, 0);
    let raydium_address = derive_tick_array_pda_address(DexProgram::RaydiumClmm, &pool_address, 0);
    assert_ne!(orca_address, raydium_address);
    assert_ne!(orca_address, [0u8; 32]);
}
