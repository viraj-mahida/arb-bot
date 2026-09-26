//! Work out which tick-array accounts a pool needs, and compute their addresses.
//!
//! **The problem:** a pool account only stores the *current* price and the
//! *current* liquidity. The information about where liquidity changes (the
//! initialized ticks) lives in separate tick-array accounts, each covering a
//! fixed window of ticks. To quote a big swap we need the windows around the
//! current price.
//!
//! **The trick — PDAs:** a Program Derived Address is an address computed from
//! "seeds" (here: the word `tick_array`, the pool address, and the window's
//! start tick) plus the program's address. Because it is deterministic, we can
//! compute the address of any tick array ourselves and subscribe to it, without
//! searching the blockchain.

use solana_pubkey::Pubkey;

use super::known_program_and_pool_addresses::{
    ORCA_WHIRLPOOL_PROGRAM_ADDRESS, RAYDIUM_CLMM_PROGRAM_ADDRESS,
};
use super::shared_pool_types::{ConcentratedLiquidityPoolState, DexProgram, TickArrayPdaToWatch};
use super::solana_public_key_helpers::{PublicKeyBytes, parse_base58_public_key};

/// Lowest tick either DEX allows (price ≈ 1.0001^-443636, practically zero).
const LOWEST_ALLOWED_TICK_INDEX: i32 = -443636;
/// Highest tick either DEX allows (price ≈ 1.0001^443636, practically infinite).
const HIGHEST_ALLOWED_TICK_INDEX: i32 = 443636;
/// We watch the tick array holding the current price plus this many on each side.
///
/// 2 on each side = 5 arrays total, which comfortably covers normal price moves
/// for SOL/USDC. A trade larger than this window is capped by the sizing step
/// instead of being quoted with liquidity we have not loaded.
const TICK_ARRAYS_TO_WATCH_ON_EACH_SIDE: i32 = 2;

/// Addresses of the tick arrays around the pool's current price (current ± 2).
pub fn tick_array_pdas_near_current_price(
    pool: &ConcentratedLiquidityPoolState,
) -> Vec<TickArrayPdaToWatch> {
    if pool.tick_spacing == 0 {
        return Vec::new();
    }
    tick_array_start_indices_near_tick(
        pool.current_tick_index,
        pool.tick_spacing,
        pool.dex.ticks_per_tick_array(),
    )
    .into_iter()
    .map(|start_tick_index| TickArrayPdaToWatch {
        tick_array_address: derive_tick_array_pda_address(
            pool.dex,
            &pool.pool_address,
            start_tick_index,
        ),
        pool_address: pool.pool_address,
        dex: pool.dex,
        start_tick_index,
        tick_spacing: pool.tick_spacing,
    })
    .collect()
}

/// Start ticks of the window containing `tick_index` and its neighbours.
///
/// Example: tick spacing 2 and 88 slots per array → one array spans
/// 2 × 88 = 176 ticks. For tick 200 the current window starts at 176, so the
/// five windows start at -176, 0, 176, 352, 528.
pub(crate) fn tick_array_start_indices_near_tick(
    tick_index: i32,
    tick_spacing: u16,
    ticks_per_tick_array: i32,
) -> Vec<i32> {
    let ticks_covered_by_one_array = ticks_per_tick_array * i32::from(tick_spacing);
    if ticks_covered_by_one_array == 0 {
        return Vec::new();
    }
    let current_start = tick_array_start_index_containing_tick(tick_index, ticks_covered_by_one_array);
    let lowest_possible_start =
        tick_array_start_index_containing_tick(LOWEST_ALLOWED_TICK_INDEX, ticks_covered_by_one_array);
    let highest_possible_start =
        tick_array_start_index_containing_tick(HIGHEST_ALLOWED_TICK_INDEX, ticks_covered_by_one_array);
    (-TICK_ARRAYS_TO_WATCH_ON_EACH_SIDE..=TICK_ARRAYS_TO_WATCH_ON_EACH_SIDE)
        .map(|offset| current_start + offset * ticks_covered_by_one_array)
        // Near the extreme ends of the price range, neighbours would not exist on-chain.
        .filter(|&start| start >= lowest_possible_start && start <= highest_possible_start)
        .collect()
}

/// First tick of the fixed-width window that contains `tick_index`.
///
/// This is floor division (rounding toward minus infinity), not Rust's normal
/// integer division (which rounds toward zero). With 88-tick windows:
/// - tick 5 → window 0..87 → start 0
/// - tick -1 → window -88..-1 → start -88 (plain `/` would wrongly give 0)
pub fn tick_array_start_index_containing_tick(tick_index: i32, ticks_covered_by_one_array: i32) -> i32 {
    tick_index.div_euclid(ticks_covered_by_one_array) * ticks_covered_by_one_array
}

/// Compute a tick-array account's address from its pool and start tick.
///
/// Both DEXes use seeds `["tick_array", pool_address, start_tick]`, but they
/// encode `start_tick` differently:
/// - Orca writes it as decimal text, e.g. the bytes of `"-176"`.
/// - Raydium writes it as 4 big-endian bytes.
///
/// Using the wrong encoding gives a valid-looking but empty address, so this
/// detail matters.
pub(crate) fn derive_tick_array_pda_address(
    dex: DexProgram,
    pool_address: &PublicKeyBytes,
    start_tick_index: i32,
) -> PublicKeyBytes {
    let program_address = Pubkey::from(parse_base58_public_key(program_address_for_dex(dex)));
    let pool_address = Pubkey::from(*pool_address);
    let start_tick_as_decimal_text;
    let start_tick_as_big_endian_bytes;
    let seeds: [&[u8]; 3] = match dex {
        DexProgram::OrcaWhirlpool => {
            start_tick_as_decimal_text = start_tick_index.to_string();
            [b"tick_array", pool_address.as_ref(), start_tick_as_decimal_text.as_bytes()]
        }
        DexProgram::RaydiumClmm => {
            start_tick_as_big_endian_bytes = start_tick_index.to_be_bytes();
            [b"tick_array", pool_address.as_ref(), &start_tick_as_big_endian_bytes]
        }
    };
    // The second value is the "bump" byte that keeps the address off the ed25519 curve; we don't need it.
    let (tick_array_address, _bump) = Pubkey::find_program_address(&seeds, &program_address);
    tick_array_address.to_bytes()
}

fn program_address_for_dex(dex: DexProgram) -> &'static str {
    match dex {
        DexProgram::OrcaWhirlpool => ORCA_WHIRLPOOL_PROGRAM_ADDRESS,
        DexProgram::RaydiumClmm => RAYDIUM_CLMM_PROGRAM_ADDRESS,
    }
}
