use crate::adapters::decoder::orca_whirlpool::{
    DISCRIMINATOR, TICKS_OFF, TICK_ARRAY_LEN, WHIRLPOOL_OFF, decode_tick_array,
};
use crate::core::{TickArrayRef, Venue};

#[test]
fn decode_initialized_tick() {
    let mut data = vec![0u8; TICK_ARRAY_LEN];
    data[..8].copy_from_slice(&DISCRIMINATOR);
    data[8..12].copy_from_slice(&176i32.to_le_bytes());
    data[TICKS_OFF] = 1;
    data[TICKS_OFF + 1..TICKS_OFF + 17].copy_from_slice(&1_000i128.to_le_bytes());
    let pool = [7u8; 32];
    data[WHIRLPOOL_OFF..].copy_from_slice(&pool);

    let id = TickArrayRef {
        pubkey: [1u8; 32],
        pool,
        venue: Venue::OrcaWhirlpool,
        start_tick_index: 176,
        tick_spacing: 2,
    };
    let snap = decode_tick_array(&id, &data, 1, 1).unwrap();
    assert_eq!(snap.start_tick_index, 176);
    assert_eq!(snap.ticks.len(), 1);
    assert_eq!(snap.ticks[0].tick, 176);
    assert_eq!(snap.ticks[0].liquidity_net, 1_000);
}
