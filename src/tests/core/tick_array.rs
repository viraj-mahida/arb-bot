use crate::core::constants::ORCA_WHIRLPOOL_SOL_USDC;
use crate::core::tick_array::{derive_tick_array_pda, nearby_start_indices, start_index};
use crate::core::{parse_pubkey, Venue};

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
    let pool = parse_pubkey(ORCA_WHIRLPOOL_SOL_USDC);
    let orca = derive_tick_array_pda(Venue::OrcaWhirlpool, &pool, 0);
    let raydium = derive_tick_array_pda(Venue::RaydiumClmm, &pool, 0);
    assert_ne!(orca, raydium);
    assert_ne!(orca, [0u8; 32]);
}
