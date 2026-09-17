use crate::core::{
    ClmmPoolState, InitializedTick, PoolSpec, TickArrayRef, TickArraySnapshot, Venue,
};

use super::bytes;

/// Raydium CLMM `PoolState` (Anchor discriminator + packed account).
///
/// Offsets are from the start of account data:
///   8   bump
///   9   amm_config
///   41  owner
///   73  mint_0
///   105 mint_1
///   137 vault_0
///   169 vault_1
///   233 mint_decimals_0
///   234 mint_decimals_1
///   235 tick_spacing
///   237 liquidity
///   253 sqrt_price_x64
///   269 tick_current
pub fn decode(
    spec: &PoolSpec,
    data: &[u8],
    slot: u64,
    write_version: u64,
) -> Option<ClmmPoolState> {
    if spec.venue != Venue::RaydiumClmm {
        return None;
    }

    Some(ClmmPoolState {
        pubkey: spec.address,
        venue: spec.venue,
        mint_a: bytes::pubkey(data, 73)?,
        mint_b: bytes::pubkey(data, 105)?,
        vault_a: bytes::pubkey(data, 137)?,
        vault_b: bytes::pubkey(data, 169)?,
        decimals_a: *data.get(233)?,
        decimals_b: *data.get(234)?,
        tick_spacing: bytes::u16_le(data, 235)?,
        liquidity: bytes::u128_le(data, 237)?,
        sqrt_price_x64: bytes::u128_le(data, 253)?,
        tick: bytes::i32_le(data, 269)?,
        fee_rate: spec.fee_rate(),
        slot,
        write_version,
    })
}

/// TickArrayState: 8 disc + pool + i32 start + 60 * TickState(168) + tail.
// [0..4) tick (i32, 4 bytes)
// [4..20) liquidity_net (i128, 16 bytes)
// [20..36) liquidity_gross (u128, 16 bytes)
//          ...
const TICK_LEN: usize = 168; // Size of one TickState in bytes
const TICKS_OFF: usize = 44; // Byte offset where that array starts
const TICK_COUNT: usize = 60; // How many TickState slots are packed in one array
const TICK_ARRAY_MIN_LEN: usize = TICKS_OFF + TICK_LEN * TICK_COUNT;
const DISCRIMINATOR: [u8; 8] = [192, 155, 85, 205, 49, 249, 129, 42];

pub fn decode_tick_array(
    id: &TickArrayRef,
    data: &[u8],
    slot: u64,
    write_version: u64,
) -> Option<TickArraySnapshot> {
    if id.venue != Venue::RaydiumClmm || data.len() < TICK_ARRAY_MIN_LEN {
        return None;
    }
    if data.get(..8)? != DISCRIMINATOR {
        return None;
    }
    if bytes::pubkey(data, 8)? != id.pool {
        return None;
    }

    let start_tick_index = bytes::i32_le(data, 40)?;
    let mut ticks = Vec::new();
    for i in 0..TICK_COUNT {
        let off = TICKS_OFF + i * TICK_LEN;
        let liquidity_gross = bytes::u128_le(data, off + 20)?; // tick(i32) 4 bytes + liquidity_net(i128) 16 bytes
        if liquidity_gross == 0 { // uninitialized tick
            continue;
        }
        ticks.push(InitializedTick {
            tick: bytes::i32_le(data, off)?,
            liquidity_net: bytes::i128_le(data, off + 4)?,
        });
    }

    Some(TickArraySnapshot {
        pubkey: id.pubkey,
        pool: id.pool,
        venue: id.venue,
        start_tick_index,
        ticks,
        slot,
        write_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
