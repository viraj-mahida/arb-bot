//! Byte layouts of Orca Whirlpool accounts, and how to read them.
//!
//! Offsets come from Orca's program source (`Whirlpool` and `TickArray`
//! structs). Every offset counts from the first byte of the account, which is
//! the start of the 8-byte Anchor discriminator.

use super::read_little_endian_numbers::{
    read_i32_little_endian, read_i128_little_endian, read_public_key, read_u16_little_endian,
    read_u128_little_endian,
};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, DexSpecificSwapAccounts,
    InitializedTickWithLiquidityChange, TickArrayAccountWithInitializedTicks, TickArrayPdaToWatch,
    WatchedPoolConfig,
};

// ── Whirlpool (pool) account ──────────────────────────────────────────────
//
//   byte   size  field
//   0      8     Anchor discriminator
//   41     2     tick_spacing        (u16)
//   45     2     fee_rate            (u16, millionths)
//   49     16    liquidity           (u128)
//   65     16    sqrt_price          (u128, Q64.64)
//   81     4     tick_current_index  (i32)
//   101    32    token_mint_a
//   133    32    token_vault_a
//   181    32    token_mint_b
//   213    32    token_vault_b
//
// Token decimals are not stored in the pool; they live on the mint accounts,
// so we take them from `WatchedPoolConfig` instead.
const ORCA_POOL_TICK_SPACING_AT_BYTE: usize = 41;
const ORCA_POOL_FEE_RATE_AT_BYTE: usize = 45;
const ORCA_POOL_LIQUIDITY_AT_BYTE: usize = 49;
const ORCA_POOL_SQRT_PRICE_AT_BYTE: usize = 65;
const ORCA_POOL_CURRENT_TICK_AT_BYTE: usize = 81;
const ORCA_POOL_TOKEN_A_MINT_AT_BYTE: usize = 101;
const ORCA_POOL_TOKEN_A_VAULT_AT_BYTE: usize = 133;
const ORCA_POOL_TOKEN_B_MINT_AT_BYTE: usize = 181;
const ORCA_POOL_TOKEN_B_VAULT_AT_BYTE: usize = 213;

pub fn decode_orca_pool_account(
    pool_config: &WatchedPoolConfig,
    account_data: &[u8],
    slot: u64,
    geyser_write_version: u64,
) -> Option<ConcentratedLiquidityPoolState> {
    if pool_config.dex != DexProgram::OrcaWhirlpool {
        return None;
    }
    Some(ConcentratedLiquidityPoolState {
        pool_address: pool_config.pool_address,
        dex: pool_config.dex,
        tick_spacing: read_u16_little_endian(account_data, ORCA_POOL_TICK_SPACING_AT_BYTE)?,
        // Orca stores the fee on-chain, so we prefer it; the config value is only a fallback.
        fee_rate_in_millionths: read_u16_little_endian(account_data, ORCA_POOL_FEE_RATE_AT_BYTE)
            .unwrap_or_else(|| pool_config.fee_rate_in_millionths()),
        active_liquidity_at_current_price: read_u128_little_endian(
            account_data,
            ORCA_POOL_LIQUIDITY_AT_BYTE,
        )?,
        sqrt_price_q64_64: read_u128_little_endian(account_data, ORCA_POOL_SQRT_PRICE_AT_BYTE)?,
        current_tick_index: read_i32_little_endian(account_data, ORCA_POOL_CURRENT_TICK_AT_BYTE)?,
        token_a_mint: read_public_key(account_data, ORCA_POOL_TOKEN_A_MINT_AT_BYTE)?,
        token_a_vault: read_public_key(account_data, ORCA_POOL_TOKEN_A_VAULT_AT_BYTE)?,
        token_b_mint: read_public_key(account_data, ORCA_POOL_TOKEN_B_MINT_AT_BYTE)?,
        token_b_vault: read_public_key(account_data, ORCA_POOL_TOKEN_B_VAULT_AT_BYTE)?,
        token_a_decimals: pool_config.token_a_decimals,
        token_b_decimals: pool_config.token_b_decimals,
        dex_specific_swap_accounts: DexSpecificSwapAccounts::OrcaWhirlpool,
        slot,
        geyser_write_version_for_ordering: geyser_write_version,
    })
}

// ── TickArray account (Orca's "FixedTickArray") ───────────────────────────
//
//   byte   size        field
//   0      8           Anchor discriminator
//   8      4           start_tick_index (i32)
//   12     88 × 113    ticks[88]
//   9956   32          whirlpool (the pool this array belongs to)
//   total  9988
//
// One tick slot (113 bytes):
//   +0   1   initialized (0 = unused, 1 = someone's range starts/ends here)
//   +1   16  liquidity_net (i128)
//   +17  96  liquidity_gross, fee growth, reward growth (not needed for quoting)
//
// Orca slots do not store their own tick index: slot `i` means tick
// `start_tick_index + i * tick_spacing`.
pub(crate) const ORCA_TICK_ARRAY_ACCOUNT_SIZE_IN_BYTES: usize = 9988;
pub(crate) const ORCA_TICK_ARRAY_DISCRIMINATOR: [u8; 8] = [69, 97, 189, 190, 110, 7, 66, 187];
const ORCA_TICK_ARRAY_START_TICK_AT_BYTE: usize = 8;
pub(crate) const ORCA_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE: usize = 12;
pub(crate) const ORCA_TICK_ARRAY_POOL_ADDRESS_AT_BYTE: usize = 9956;
const ORCA_TICK_SLOT_SIZE_IN_BYTES: usize = 113;
const ORCA_TICK_SLOTS_PER_ARRAY: usize = 88;
const ORCA_TICK_SLOT_LIQUIDITY_NET_OFFSET: usize = 1;

pub fn decode_orca_tick_array_account(
    watched_tick_array: &TickArrayPdaToWatch,
    account_data: &[u8],
    slot: u64,
    geyser_write_version: u64,
) -> Option<TickArrayAccountWithInitializedTicks> {
    if watched_tick_array.dex != DexProgram::OrcaWhirlpool
        || account_data.len() < ORCA_TICK_ARRAY_ACCOUNT_SIZE_IN_BYTES
    {
        return None;
    }
    if account_data.get(..8)? != ORCA_TICK_ARRAY_DISCRIMINATOR {
        return None;
    }
    // Guards against a wrong PDA: the array must say it belongs to the pool we expect.
    if read_public_key(account_data, ORCA_TICK_ARRAY_POOL_ADDRESS_AT_BYTE)?
        != watched_tick_array.pool_address
    {
        return None;
    }

    let start_tick_index =
        read_i32_little_endian(account_data, ORCA_TICK_ARRAY_START_TICK_AT_BYTE)?;
    let tick_spacing = i32::from(watched_tick_array.tick_spacing);
    let mut initialized_ticks = Vec::new();
    for slot_position in 0..ORCA_TICK_SLOTS_PER_ARRAY {
        let slot_start =
            ORCA_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE + slot_position * ORCA_TICK_SLOT_SIZE_IN_BYTES;
        let is_initialized = *account_data.get(slot_start)? != 0;
        if !is_initialized {
            continue;
        }
        initialized_ticks.push(InitializedTickWithLiquidityChange {
            tick_index: start_tick_index + (slot_position as i32) * tick_spacing,
            liquidity_added_when_price_crosses_upward: read_i128_little_endian(
                account_data,
                slot_start + ORCA_TICK_SLOT_LIQUIDITY_NET_OFFSET,
            )?,
        });
    }

    Some(TickArrayAccountWithInitializedTicks {
        tick_array_address: watched_tick_array.tick_array_address,
        pool_address: watched_tick_array.pool_address,
        dex: watched_tick_array.dex,
        start_tick_index,
        initialized_ticks,
        slot,
        geyser_write_version_for_ordering: geyser_write_version,
    })
}
