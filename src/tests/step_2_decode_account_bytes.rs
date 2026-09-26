use crate::step_2_decode_account_bytes::orca_whirlpool_account_decoder::{
    ORCA_TICK_ARRAY_ACCOUNT_SIZE_IN_BYTES, ORCA_TICK_ARRAY_DISCRIMINATOR, ORCA_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE,
    ORCA_TICK_ARRAY_POOL_ADDRESS_AT_BYTE, decode_orca_tick_array_account,
};
use crate::step_2_decode_account_bytes::raydium_clmm_account_decoder::{
    RAYDIUM_TICK_ARRAY_DISCRIMINATOR, RAYDIUM_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE,
    RAYDIUM_TICK_ARRAY_MINIMUM_SIZE_IN_BYTES, decode_raydium_tick_array_account,
};
use crate::step_3_store_latest_pool_state::{DexProgram, TickArrayPdaToWatch};

/// Orca slots carry no tick index: slot 0 of an array starting at 176 must decode as tick 176.
#[test]
fn orca_tick_array_with_one_initialized_slot_decodes_that_tick() {
    let mut account_data = vec![0u8; ORCA_TICK_ARRAY_ACCOUNT_SIZE_IN_BYTES];
    account_data[..8].copy_from_slice(&ORCA_TICK_ARRAY_DISCRIMINATOR);
    account_data[8..12].copy_from_slice(&176i32.to_le_bytes());
    let first_slot = ORCA_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE;
    account_data[first_slot] = 1; // initialized flag
    account_data[first_slot + 1..first_slot + 17].copy_from_slice(&1_000i128.to_le_bytes());
    let pool_address = [7u8; 32];
    account_data[ORCA_TICK_ARRAY_POOL_ADDRESS_AT_BYTE..].copy_from_slice(&pool_address);

    let watched_tick_array = TickArrayPdaToWatch {
        tick_array_address: [1u8; 32],
        pool_address,
        dex: DexProgram::OrcaWhirlpool,
        start_tick_index: 176,
        tick_spacing: 2,
    };
    let tick_array = decode_orca_tick_array_account(&watched_tick_array, &account_data, 1, 1).unwrap();
    assert_eq!(tick_array.start_tick_index, 176);
    assert_eq!(tick_array.initialized_ticks.len(), 1);
    assert_eq!(tick_array.initialized_ticks[0].tick_index, 176);
    assert_eq!(tick_array.initialized_ticks[0].liquidity_added_when_price_crosses_upward, 1_000);
}

/// Raydium slots are initialized when `liquidity_gross > 0`, and store their own tick index.
#[test]
fn raydium_tick_array_with_one_initialized_slot_decodes_that_tick() {
    let mut account_data = vec![0u8; RAYDIUM_TICK_ARRAY_MINIMUM_SIZE_IN_BYTES];
    account_data[..8].copy_from_slice(&RAYDIUM_TICK_ARRAY_DISCRIMINATOR);
    let pool_address = [9u8; 32];
    account_data[8..40].copy_from_slice(&pool_address);
    account_data[40..44].copy_from_slice(&0i32.to_le_bytes());
    let first_slot = RAYDIUM_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE;
    account_data[first_slot..first_slot + 4].copy_from_slice(&64i32.to_le_bytes());
    account_data[first_slot + 4..first_slot + 20].copy_from_slice(&(-500i128).to_le_bytes());
    account_data[first_slot + 20..first_slot + 36].copy_from_slice(&500u128.to_le_bytes());

    let watched_tick_array = TickArrayPdaToWatch {
        tick_array_address: [2u8; 32],
        pool_address,
        dex: DexProgram::RaydiumClmm,
        start_tick_index: 0,
        tick_spacing: 1,
    };
    let tick_array = decode_raydium_tick_array_account(&watched_tick_array, &account_data, 1, 1).unwrap();
    assert_eq!(tick_array.initialized_ticks.len(), 1);
    assert_eq!(tick_array.initialized_ticks[0].tick_index, 64);
    assert_eq!(tick_array.initialized_ticks[0].liquidity_added_when_price_crosses_upward, -500);
}
