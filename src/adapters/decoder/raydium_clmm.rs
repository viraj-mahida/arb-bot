use crate::core::{ClmmPoolState, PoolSpec, Venue};

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
pub fn decode(spec: &PoolSpec, data: &[u8], slot: u64, write_version: u64) -> Option<ClmmPoolState> {
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
