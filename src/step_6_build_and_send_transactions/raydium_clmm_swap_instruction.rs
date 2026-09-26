//! Raydium CLMM `swap_v2` instruction.
//!
//! Instruction data (little-endian, Borsh):
//!
//! | bytes | field                        | our value |
//! |-------|------------------------------|-----------|
//! | 8     | discriminator                | `swap_v2` |
//! | 8     | amount (u64)                 | exact input |
//! | 8     | other_amount_threshold (u64) | minimum output |
//! | 16    | sqrt_price_limit_x64 (u128)  | 0 = none |
//! | 1     | is_base_input                | 1 (true: amount is the input) |
//!
//! Unlike Orca, Raydium names accounts by *input/output* instead of A/B, so the
//! direction decides which wallet account and vault come first. Tick arrays go
//! last, as "remaining accounts".
//!
//! Current limitation: the optional tick-array *bitmap extension* account is
//! not passed. It is only needed when liquidity sits extremely far from the
//! current price, which does not happen for SOL/USDC.

use solana_instruction::{AccountMeta, Instruction};

use super::swap_leg_instruction::{NO_PRICE_LIMIT, SWAP_V2_ANCHOR_DISCRIMINATOR, SwapLeg};
use super::well_known_program_addresses::{
    MEMO_PROGRAM_ADDRESS, TOKEN_2022_PROGRAM_ADDRESS, TOKEN_PROGRAM_ADDRESS, program, pubkey,
    read_only, read_only_program, writable,
};
use crate::step_3_store_latest_pool_state::known_program_and_pool_addresses::RAYDIUM_CLMM_PROGRAM_ADDRESS;
use crate::step_3_store_latest_pool_state::tick_array_pda_derivation::tick_array_start_index_containing_tick;
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, DexSpecificSwapAccounts, PublicKeyBytes,
    TickArrayAccountWithInitializedTicks,
};

/// Tick arrays passed per swap. Three windows cover far more than our trade sizes move the price.
const RAYDIUM_TICK_ARRAYS_PER_SWAP: usize = 3;

pub fn raydium_clmm_swap_v2_instruction(leg: &SwapLeg) -> Instruction {
    let pool = leg.pool;
    let a_to_b = leg.direction.is_a_to_b();
    let DexSpecificSwapAccounts::RaydiumClmm {
        fee_config_address,
        price_observation_address,
    } = pool.dex_specific_swap_accounts
    else {
        panic!("raydium swap built for a pool without Raydium accounts");
    };

    let mut data = Vec::with_capacity(41);
    data.extend_from_slice(&SWAP_V2_ANCHOR_DISCRIMINATOR);
    data.extend_from_slice(&leg.amount_in.to_le_bytes());
    data.extend_from_slice(&leg.minimum_amount_out.to_le_bytes());
    data.extend_from_slice(&NO_PRICE_LIMIT.to_le_bytes());
    data.push(1); // is_base_input = true

    let (input_account, output_account, input_vault, output_vault, input_mint, output_mint) =
        if a_to_b {
            (
                leg.wallet_token_a_account,
                leg.wallet_token_b_account,
                pool.token_a_vault,
                pool.token_b_vault,
                pool.token_a_mint,
                pool.token_b_mint,
            )
        } else {
            (
                leg.wallet_token_b_account,
                leg.wallet_token_a_account,
                pool.token_b_vault,
                pool.token_a_vault,
                pool.token_b_mint,
                pool.token_a_mint,
            )
        };

    let mut accounts = vec![
        AccountMeta::new_readonly(pubkey(leg.wallet), true),
        read_only(fee_config_address),
        writable(pool.pool_address),
        writable(input_account),
        writable(output_account),
        writable(input_vault),
        writable(output_vault),
        writable(price_observation_address),
        read_only_program(TOKEN_PROGRAM_ADDRESS),
        read_only_program(TOKEN_2022_PROGRAM_ADDRESS),
        read_only_program(MEMO_PROGRAM_ADDRESS),
        read_only(input_mint),
        read_only(output_mint),
    ];
    accounts.extend(
        raydium_tick_arrays_in_swap_direction(pool, leg.cached_tick_arrays, a_to_b)
            .into_iter()
            .map(writable),
    );
    Instruction::new_with_bytes(program(RAYDIUM_CLMM_PROGRAM_ADDRESS), &data, accounts)
}

/// Cached tick arrays on the swap's side of the price, nearest first.
///
/// Every account passed here must really exist on-chain: Raydium stops reading
/// remaining accounts at the first one that is not a tick array. Arrays in our
/// cache were all loaded from the chain, so they qualify. Raydium itself skips
/// arrays that have no initialized ticks, so passing an empty one is harmless.
pub fn raydium_tick_arrays_in_swap_direction(
    pool: &ConcentratedLiquidityPoolState,
    cached_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    a_to_b: bool,
) -> Vec<PublicKeyBytes> {
    let ticks_per_array =
        DexProgram::RaydiumClmm.ticks_per_tick_array() * i32::from(pool.tick_spacing);
    let current_start =
        tick_array_start_index_containing_tick(pool.current_tick_index, ticks_per_array);
    let mut on_swap_side: Vec<&TickArrayAccountWithInitializedTicks> = cached_tick_arrays
        .iter()
        .filter(|array| array.pool_address == pool.pool_address)
        .filter(|array| {
            if a_to_b {
                array.start_tick_index <= current_start
            } else {
                array.start_tick_index >= current_start
            }
        })
        .collect();
    if a_to_b {
        on_swap_side.sort_by_key(|array| std::cmp::Reverse(array.start_tick_index));
    } else {
        on_swap_side.sort_by_key(|array| array.start_tick_index);
    }
    on_swap_side
        .into_iter()
        .take(RAYDIUM_TICK_ARRAYS_PER_SWAP)
        .map(|array| array.tick_array_address)
        .collect()
}
