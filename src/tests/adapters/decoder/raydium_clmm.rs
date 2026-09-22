use crate::adapters::decoder::raydium_clmm::{
    DISCRIMINATOR, TICKS_OFF, TICK_ARRAY_MIN_LEN, decode_tick_array,
};
use crate::core::{TickArrayRef, Venue};

#[test]
fn decode_initialized_tick() {
    let mut data = vec![0u8; TICK_ARRAY_MIN_LEN];
    data[..8].copy_from_slice(&DISCRIMINATOR);
    let pool = [9u8; 32];
    data[8..40].copy_from_slice(&pool);
    data[40..44].copy_from_slice(&0i32.to_le_bytes());
    data[TICKS_OFF..TICKS_OFF + 4].copy_from_slice(&64i32.to_le_bytes());
    data[TICKS_OFF + 4..TICKS_OFF + 20].copy_from_slice(&(-500i128).to_le_bytes());
    data[TICKS_OFF + 20..TICKS_OFF + 36].copy_from_slice(&500u128.to_le_bytes());

    let id = TickArrayRef {
        pubkey: [2u8; 32],
        pool,
        venue: Venue::RaydiumClmm,
        start_tick_index: 0,
        tick_spacing: 1,
    };
    let snap = decode_tick_array(&id, &data, 1, 1).unwrap();
    assert_eq!(snap.ticks.len(), 1);
    assert_eq!(snap.ticks[0].tick, 64);
    assert_eq!(snap.ticks[0].liquidity_net, -500);
}
