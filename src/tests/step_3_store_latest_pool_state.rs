use super::test_pool_builders::test_pool_with_one_empty_tick_array;
use crate::step_3_store_latest_pool_state::known_program_and_pool_addresses::ORCA_WHIRLPOOL_SOL_USDC_POOL_ADDRESS;
use crate::step_3_store_latest_pool_state::tick_array_pda_derivation::{
    derive_tick_array_pda_address, tick_array_start_index_containing_tick,
    tick_array_start_indices_near_tick,
};
use crate::step_3_store_latest_pool_state::{
    DexProgram, LatestPoolStateCache, parse_base58_public_key,
};

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

/// A later write replaces the same tick array in place, and another pool's arrays stay separate.
#[test]
fn tick_arrays_stay_one_entry_per_pool_after_an_update() {
    let cache = LatestPoolStateCache::new();
    let (pool_a, mut ticks_a) =
        test_pool_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, 1_000);
    let (mut pool_b, mut ticks_b) =
        test_pool_with_one_empty_tick_array(DexProgram::RaydiumClmm, 1_000);
    pool_b.pool_address = [3u8; 32];
    ticks_a.pool_address = pool_a.pool_address;
    ticks_b.pool_address = pool_b.pool_address;
    ticks_b.tick_array_address = [4u8; 32];

    cache.save_tick_array(ticks_a.clone());
    cache.save_tick_array(ticks_b.clone());

    ticks_a.geyser_write_version_for_ordering = 2;
    ticks_a.slot = 8;
    cache.save_tick_array(ticks_a.clone());

    let stored_for_a = cache.tick_arrays_for_pool(&pool_a.pool_address);
    assert_eq!(stored_for_a.len(), 1);
    assert_eq!(stored_for_a[0].slot, 8);
    assert_eq!(
        stored_for_a[0].tick_array_address,
        ticks_a.tick_array_address
    );
    let stored_for_b = cache.tick_arrays_for_pool(&pool_b.pool_address);
    assert_eq!(stored_for_b.len(), 1);
    assert_eq!(
        stored_for_b[0].tick_array_address,
        ticks_b.tick_array_address
    );

    let mut older = ticks_a.clone();
    older.geyser_write_version_for_ordering = 1;
    older.slot = 99;
    cache.save_tick_array(older);
    assert_eq!(cache.tick_arrays_for_pool(&pool_a.pool_address)[0].slot, 8);
    assert_eq!(
        cache.loaded_tick_array_count_for_pool(&pool_a.pool_address),
        1
    );
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
