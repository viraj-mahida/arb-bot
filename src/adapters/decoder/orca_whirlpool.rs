use crate::core::{
    ClmmPoolState, InitializedTick, PoolSpec, TickArrayRef, TickArraySnapshot, Venue,
};

use super::bytes;

/// Orca Whirlpool account (8-byte Anchor discriminator).
///
///   41  tick_spacing
///   45  fee_rate (hundredths of a bip)
///   49  liquidity
///   65  sqrt_price
///   81  tick_current_index
///   101 token_mint_a
///   133 token_vault_a
///   181 token_mint_b
///   213 token_vault_b
///
/// Decimals are not on the pool; they come from `PoolSpec`.
pub fn decode(
    spec: &PoolSpec,
    data: &[u8],
    slot: u64,
    write_version: u64,
) -> Option<ClmmPoolState> {
    if spec.venue != Venue::OrcaWhirlpool {
        return None;
    }

    Some(ClmmPoolState {
        pubkey: spec.address,
        venue: spec.venue,
        tick_spacing: bytes::u16_le(data, 41)?,
        fee_rate: bytes::u16_le(data, 45).unwrap_or_else(|| spec.fee_rate()),
        liquidity: bytes::u128_le(data, 49)?,
        sqrt_price_x64: bytes::u128_le(data, 65)?,
        tick: bytes::i32_le(data, 81)?,
        mint_a: bytes::pubkey(data, 101)?,
        vault_a: bytes::pubkey(data, 133)?,
        mint_b: bytes::pubkey(data, 181)?,
        vault_b: bytes::pubkey(data, 213)?,
        decimals_a: spec.decimals_a,
        decimals_b: spec.decimals_b,
        slot,
        write_version,
    })
}

/// FixedTickArray: 8 disc + i32 start + 88 * Tick(113) + whirlpool pubkey = 9988.
const TICK_ARRAY_LEN: usize = 9988;
// Orca Whirlpool Tick struct fields:
//  1  initialized (u8)
// 16  liquidity_net (i128)
// 16  liquidity_gross (u128)
// 16  fee_growth_outside_a (u128)
// 16  fee_growth_outside_b (u128)
// 16  reward_growths_outside (3 * u128 = 48 bytes)
const TICK_LEN: usize = 113;
const TICKS_OFF: usize = 12;
const WHIRLPOOL_OFF: usize = 9956;
const TICK_COUNT: usize = 88;
const DISCRIMINATOR: [u8; 8] = [69, 97, 189, 190, 110, 7, 66, 187];

pub fn decode_tick_array(
    id: &TickArrayRef,
    data: &[u8],
    slot: u64,
    write_version: u64,
) -> Option<TickArraySnapshot> {
    if id.venue != Venue::OrcaWhirlpool || data.len() < TICK_ARRAY_LEN {
        return None;
    }
    if data.get(..8)? != DISCRIMINATOR {
        return None;
    }
    if bytes::pubkey(data, WHIRLPOOL_OFF)? != id.pool {
        return None;
    }

    let start_tick_index = bytes::i32_le(data, 8)?;
    let spacing = i32::from(id.tick_spacing);
    let mut ticks = Vec::new();
    for i in 0..TICK_COUNT {
        let off = TICKS_OFF + i * TICK_LEN;
        if *data.get(off)? == 0 {
            continue;
        }
        ticks.push(InitializedTick {
            tick: start_tick_index + (i as i32) * spacing,
            liquidity_net: bytes::i128_le(data, off + 1)?,
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
}
