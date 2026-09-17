use solana_pubkey::Pubkey;

use super::constants::{ORCA_WHIRLPOOL_PROGRAM, RAYDIUM_CLMM_PROGRAM};
use super::pubkey::parse_pubkey;
use super::types::{ClmmPoolState, TickArrayRef, Venue};

/// Same bounds as Orca / Raydium CLMM (`±443636`).
const MIN_TICK_INDEX: i32 = -443636;
const MAX_TICK_INDEX: i32 = 443636;
const TICK_ARRAY_RADIUS: i32 = 2;

/// Current array plus `TICK_ARRAY_RADIUS` neighbors on each side.
pub fn tick_arrays_around(pool: &ClmmPoolState) -> Vec<TickArrayRef> {
    if pool.tick_spacing == 0 {
        return Vec::new();
    }
    nearby_start_indices(pool.tick, pool.tick_spacing, pool.venue.ticks_per_array())
        .into_iter()
        .map(|start_tick_index| TickArrayRef {
            pubkey: derive_tick_array_pda(pool.venue, &pool.pubkey, start_tick_index),
            pool: pool.pubkey,
            venue: pool.venue,
            start_tick_index,
            tick_spacing: pool.tick_spacing,
        })
        .collect()
}

fn nearby_start_indices(tick: i32, tick_spacing: u16, ticks_per_array: i32) -> Vec<i32> {
    let ticks_in_array = ticks_per_array.saturating_mul(i32::from(tick_spacing)); // TODO? normal * can do
    if ticks_in_array == 0 {
        return Vec::new();
    }
    let current = start_index(tick, ticks_in_array);
    let min_start = start_index(MIN_TICK_INDEX, ticks_in_array);
    let max_start = start_index(MAX_TICK_INDEX, ticks_in_array);
    (-TICK_ARRAY_RADIUS..=TICK_ARRAY_RADIUS)
        .map(|i| current.saturating_add(i.saturating_mul(ticks_in_array)))
        .filter(|&start| start >= min_start && start <= max_start) //TODO? no need of this check ig
        .collect()
}

/// Floor-div start of the array that contains `tick` (toward −∞).
pub fn start_index(tick: i32, ticks_in_array: i32) -> i32 {
    let mut start = tick / ticks_in_array;
    if tick < 0 && tick % ticks_in_array != 0 {
        start -= 1;
    }
    start * ticks_in_array
}

fn derive_tick_array_pda(venue: Venue, pool: &[u8; 32], start_tick_index: i32) -> [u8; 32] {
    let program = Pubkey::from(parse_pubkey(program_id(venue)));
    let pool = Pubkey::from(*pool);
    let start_str;
    let start_be;
    let seeds: [&[u8]; 3] = match venue {
        Venue::OrcaWhirlpool => {
            start_str = start_tick_index.to_string();
            [b"tick_array", pool.as_ref(), start_str.as_bytes()]
        }
        Venue::RaydiumClmm => {
            start_be = start_tick_index.to_be_bytes();
            [b"tick_array", pool.as_ref(), &start_be]
        }
    };
    let (pda, _) = Pubkey::find_program_address(&seeds, &program);
    pda.to_bytes()
}

fn program_id(venue: Venue) -> &'static str {
    match venue {
        Venue::OrcaWhirlpool => ORCA_WHIRLPOOL_PROGRAM,
        Venue::RaydiumClmm => RAYDIUM_CLMM_PROGRAM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_index_matches_orca_docs() {
        // tick=200, spacing=2, 88 slots → ticks_in_array=176 → start=176
        assert_eq!(start_index(200, 2 * 88), 176);
        assert_eq!(start_index(0, 88), 0);
        assert_eq!(start_index(-1, 88), -88);
        assert_eq!(start_index(-88, 88), -88);
        assert_eq!(start_index(88, 88), 88);
    }

    #[test]
    fn nearby_includes_current_plus_minus_two() {
        let starts = nearby_start_indices(200, 2, 88);
        assert_eq!(starts, vec![-176, 0, 176, 352, 528]);
    }

    #[test]
    fn orca_and_raydium_pdas_differ() {
        let pool = parse_pubkey(super::super::constants::ORCA_WHIRLPOOL_SOL_USDC);
        let orca = derive_tick_array_pda(Venue::OrcaWhirlpool, &pool, 0);
        let raydium = derive_tick_array_pda(Venue::RaydiumClmm, &pool, 0);
        assert_ne!(orca, raydium);
        assert_ne!(orca, [0u8; 32]);
    }
}
