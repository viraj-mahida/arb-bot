use crate::core::{ClmmPoolState, PoolSpec, Venue};

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
pub fn decode(spec: &PoolSpec, data: &[u8], slot: u64, write_version: u64) -> Option<ClmmPoolState> {
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
