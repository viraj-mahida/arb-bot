mod bytes;
mod orca_whirlpool;
mod raydium_clmm;

use crate::core::{ClmmPoolState, PoolSpec, TickArrayRef, TickArraySnapshot, Venue};

pub fn decode_clmm_pool(
    spec: &PoolSpec,
    data: &[u8],
    slot: u64,
    write_version: u64,
) -> Option<ClmmPoolState> {
    match spec.venue {
        Venue::RaydiumClmm => raydium_clmm::decode(spec, data, slot, write_version),
        Venue::OrcaWhirlpool => orca_whirlpool::decode(spec, data, slot, write_version),
    }
}

pub fn decode_tick_array(
    id: &TickArrayRef,
    data: &[u8],
    slot: u64,
    write_version: u64,
) -> Option<TickArraySnapshot> {
    match id.venue {
        Venue::RaydiumClmm => raydium_clmm::decode_tick_array(id, data, slot, write_version),
        Venue::OrcaWhirlpool => orca_whirlpool::decode_tick_array(id, data, slot, write_version),
    }
}
