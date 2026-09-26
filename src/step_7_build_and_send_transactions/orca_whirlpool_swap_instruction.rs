//! Orca Whirlpool `swap_v2` instruction.
//!
//! Instruction data (little-endian, Borsh):
//!
//! | bytes | field                     | our value |
//! |-------|---------------------------|-----------|
//! | 8     | discriminator             | `swap_v2` |
//! | 8     | amount (u64)              | exact input |
//! | 8     | other_amount_threshold    | minimum output |
//! | 16    | sqrt_price_limit (u128)   | 0 = none |
//! | 1     | amount_specified_is_input | 1 (true) |
//! | 1     | a_to_b                    | direction |
//! | 1     | remaining_accounts_info   | 0 (None) |
//!
//! Accounts, in the exact order the program expects: token programs A and B,
//! memo program, signer, whirlpool, both mints, then (wallet account, pool
//! vault) for A and for B, three tick arrays in swap direction, and the oracle.

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::swap_leg_instruction::{NO_PRICE_LIMIT, SWAP_V2_ANCHOR_DISCRIMINATOR, SwapLeg};
use super::well_known_program_addresses::{
    MEMO_PROGRAM_ADDRESS, TOKEN_PROGRAM_ADDRESS, program, pubkey, read_only, read_only_program, writable,
};
use crate::step_3_store_latest_pool_state::known_program_and_pool_addresses::ORCA_WHIRLPOOL_PROGRAM_ADDRESS;
use crate::step_3_store_latest_pool_state::tick_array_pda_derivation::{
    derive_tick_array_pda_address, tick_array_start_index_containing_tick,
};
use crate::step_3_store_latest_pool_state::{ConcentratedLiquidityPoolState, DexProgram, PublicKeyBytes};

pub fn orca_whirlpool_swap_v2_instruction(leg: &SwapLeg) -> Instruction {
    let pool = leg.pool;
    let a_to_b = leg.direction.is_a_to_b();

    let mut data = Vec::with_capacity(43);
    data.extend_from_slice(&SWAP_V2_ANCHOR_DISCRIMINATOR);
    data.extend_from_slice(&leg.amount_in.to_le_bytes());
    data.extend_from_slice(&leg.minimum_amount_out.to_le_bytes());
    data.extend_from_slice(&NO_PRICE_LIMIT.to_le_bytes());
    data.push(1); // amount_specified_is_input = true
    data.push(u8::from(a_to_b));
    data.push(0); // remaining_accounts_info = None

    let [tick_array_0, tick_array_1, tick_array_2] = orca_tick_arrays_in_swap_direction(pool, a_to_b);
    let accounts = vec![
        read_only_program(TOKEN_PROGRAM_ADDRESS),
        read_only_program(TOKEN_PROGRAM_ADDRESS),
        read_only_program(MEMO_PROGRAM_ADDRESS),
        AccountMeta::new_readonly(pubkey(leg.wallet), true),
        writable(pool.pool_address),
        read_only(pool.token_a_mint),
        read_only(pool.token_b_mint),
        writable(leg.wallet_token_a_account),
        writable(pool.token_a_vault),
        writable(leg.wallet_token_b_account),
        writable(pool.token_b_vault),
        writable(tick_array_0),
        writable(tick_array_1),
        writable(tick_array_2),
        writable(orca_oracle_address(&pool.pool_address)),
    ];
    Instruction::new_with_bytes(program(ORCA_WHIRLPOOL_PROGRAM_ADDRESS), &data, accounts)
}

/// The three tick arrays the swap may walk through, starting at the current price.
///
/// Orca's own SDK rule: when the price goes *up* (b→a), start one tick
/// spacing higher, because a price sitting exactly on an array's last tick
/// already belongs to the next array for an upward swap. These accounts may
/// not exist on-chain; Orca accepts that for arrays it never reaches.
pub fn orca_tick_arrays_in_swap_direction(pool: &ConcentratedLiquidityPoolState, a_to_b: bool) -> [PublicKeyBytes; 3] {
    let ticks_per_array = DexProgram::OrcaWhirlpool.ticks_per_tick_array() * i32::from(pool.tick_spacing);
    let shift = if a_to_b { 0 } else { i32::from(pool.tick_spacing) };
    let first_start = tick_array_start_index_containing_tick(pool.current_tick_index + shift, ticks_per_array);
    let step = if a_to_b { -ticks_per_array } else { ticks_per_array };
    [0, 1, 2].map(|offset| {
        derive_tick_array_pda_address(DexProgram::OrcaWhirlpool, &pool.pool_address, first_start + offset * step)
    })
}

/// The pool's oracle PDA: seeds `["oracle", whirlpool]`.
pub fn orca_oracle_address(whirlpool: &PublicKeyBytes) -> PublicKeyBytes {
    let (address, _bump) =
        Pubkey::find_program_address(&[b"oracle", whirlpool.as_ref()], &program(ORCA_WHIRLPOOL_PROGRAM_ADDRESS));
    address.to_bytes()
}
