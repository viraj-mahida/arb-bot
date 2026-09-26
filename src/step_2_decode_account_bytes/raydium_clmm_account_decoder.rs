//! Byte layouts of Raydium CLMM accounts, and how to read them.
//!
//! Offsets come from Raydium's program source (`PoolState` and
//! `TickArrayState` structs). Every offset counts from the first byte of the
//! account, which is the start of the 8-byte Anchor discriminator.

use super::read_little_endian_numbers::{
    read_i32_little_endian, read_i128_little_endian, read_public_key, read_u16_little_endian,
    read_u128_little_endian,
};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, DexSpecificSwapAccounts,
    InitializedTickWithLiquidityChange, TickArrayAccountWithInitializedTicks, TickArrayPdaToWatch,
    WatchedPoolConfig,
};

// ── PoolState (pool) account ──────────────────────────────────────────────
//
//   byte   size  field
//   0      8     Anchor discriminator
//   8      1     bump
//   9      32    amm_config (holds the fee tier)
//   41     32    owner
//   73     32    token_mint_0   (token A)
//   105    32    token_mint_1   (token B)
//   137    32    token_vault_0
//   169    32    token_vault_1
//   201    32    observation_key (price-history account, needed by swaps)
//   233    1     mint_decimals_0
//   234    1     mint_decimals_1
//   235    2     tick_spacing   (u16)
//   237    16    liquidity      (u128)
//   253    16    sqrt_price_x64 (u128, Q64.64)
//   269    4     tick_current   (i32)
//
// Raydium keeps the fee in the separate `amm_config` account. This decoder
// starts from the `WatchedPoolConfig` fee; Step 1 then replaces it with the
// real `trade_fee_rate` read by [`decode_raydium_fee_config_trade_fee_rate`].
const RAYDIUM_POOL_FEE_CONFIG_AT_BYTE: usize = 9;
const RAYDIUM_POOL_PRICE_OBSERVATION_AT_BYTE: usize = 201;
const RAYDIUM_POOL_TOKEN_A_MINT_AT_BYTE: usize = 73;
const RAYDIUM_POOL_TOKEN_B_MINT_AT_BYTE: usize = 105;
const RAYDIUM_POOL_TOKEN_A_VAULT_AT_BYTE: usize = 137;
const RAYDIUM_POOL_TOKEN_B_VAULT_AT_BYTE: usize = 169;
const RAYDIUM_POOL_TOKEN_A_DECIMALS_AT_BYTE: usize = 233;
const RAYDIUM_POOL_TOKEN_B_DECIMALS_AT_BYTE: usize = 234;
const RAYDIUM_POOL_TICK_SPACING_AT_BYTE: usize = 235;
const RAYDIUM_POOL_LIQUIDITY_AT_BYTE: usize = 237;
const RAYDIUM_POOL_SQRT_PRICE_AT_BYTE: usize = 253;
const RAYDIUM_POOL_CURRENT_TICK_AT_BYTE: usize = 269;

pub fn decode_raydium_pool_account(
    pool_config: &WatchedPoolConfig,
    account_data: &[u8],
    slot: u64,
    geyser_write_version: u64,
) -> Option<ConcentratedLiquidityPoolState> {
    if pool_config.dex != DexProgram::RaydiumClmm {
        return None;
    }
    Some(ConcentratedLiquidityPoolState {
        pool_address: pool_config.pool_address,
        dex: pool_config.dex,
        token_a_mint: read_public_key(account_data, RAYDIUM_POOL_TOKEN_A_MINT_AT_BYTE)?,
        token_b_mint: read_public_key(account_data, RAYDIUM_POOL_TOKEN_B_MINT_AT_BYTE)?,
        token_a_vault: read_public_key(account_data, RAYDIUM_POOL_TOKEN_A_VAULT_AT_BYTE)?,
        token_b_vault: read_public_key(account_data, RAYDIUM_POOL_TOKEN_B_VAULT_AT_BYTE)?,
        token_a_decimals: *account_data.get(RAYDIUM_POOL_TOKEN_A_DECIMALS_AT_BYTE)?,
        token_b_decimals: *account_data.get(RAYDIUM_POOL_TOKEN_B_DECIMALS_AT_BYTE)?,
        tick_spacing: read_u16_little_endian(account_data, RAYDIUM_POOL_TICK_SPACING_AT_BYTE)?,
        active_liquidity_at_current_price: read_u128_little_endian(
            account_data,
            RAYDIUM_POOL_LIQUIDITY_AT_BYTE,
        )?,
        sqrt_price_q64_64: read_u128_little_endian(account_data, RAYDIUM_POOL_SQRT_PRICE_AT_BYTE)?,
        current_tick_index: read_i32_little_endian(
            account_data,
            RAYDIUM_POOL_CURRENT_TICK_AT_BYTE,
        )?,
        fee_rate_in_millionths: pool_config.fee_rate_in_millionths(),
        dex_specific_swap_accounts: DexSpecificSwapAccounts::RaydiumClmm {
            fee_config_address: read_public_key(account_data, RAYDIUM_POOL_FEE_CONFIG_AT_BYTE)?,
            price_observation_address: read_public_key(
                account_data,
                RAYDIUM_POOL_PRICE_OBSERVATION_AT_BYTE,
            )?,
        },
        slot,
        geyser_write_version_for_ordering: geyser_write_version,
    })
}

// ── AmmConfig (fee tier) account ──────────────────────────────────────────
//
//   byte   size  field
//   0      8     Anchor discriminator
//   8      1     bump
//   9      2     index
//   11     32    owner
//   43     4     protocol_fee_rate (u32)
//   47     4     trade_fee_rate    (u32, millionths — the swap fee)
const RAYDIUM_FEE_CONFIG_TRADE_FEE_RATE_AT_BYTE: usize = 47;

/// The swap fee (millionths of the input) stored in a Raydium `amm_config` account.
///
/// Returns `None` if the bytes are too short or the fee does not fit the
/// shared `u16` field (a fee above 6.5% would be a sign of a wrong account).
pub fn decode_raydium_fee_config_trade_fee_rate(account_data: &[u8]) -> Option<u16> {
    let bytes = account_data.get(
        RAYDIUM_FEE_CONFIG_TRADE_FEE_RATE_AT_BYTE..RAYDIUM_FEE_CONFIG_TRADE_FEE_RATE_AT_BYTE + 4,
    )?;
    u16::try_from(u32::from_le_bytes(bytes.try_into().ok()?)).ok()
}

// ── TickArrayState account ────────────────────────────────────────────────
//
//   byte   size        field
//   0      8           Anchor discriminator
//   8      32          pool_id (the pool this array belongs to)
//   40     4           start_tick_index (i32)
//   44     60 × 168    ticks[60]
//   ...                (padding / tail we do not need)
//
// One tick slot (168 bytes):
//   +0   4   tick (i32) — Raydium stores the index in the slot itself
//   +4   16  liquidity_net (i128)
//   +20  16  liquidity_gross (u128)
//   +36  …   fee / reward growth (not needed for quoting)
//
// `liquidity_gross` is the total liquidity that references this tick. If it is
// zero, no position starts or ends here, so the slot is uninitialized.
pub(crate) const RAYDIUM_TICK_ARRAY_DISCRIMINATOR: [u8; 8] = [192, 155, 85, 205, 49, 249, 129, 42];
const RAYDIUM_TICK_ARRAY_POOL_ADDRESS_AT_BYTE: usize = 8;
const RAYDIUM_TICK_ARRAY_START_TICK_AT_BYTE: usize = 40;
pub(crate) const RAYDIUM_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE: usize = 44;
const RAYDIUM_TICK_SLOT_SIZE_IN_BYTES: usize = 168;
const RAYDIUM_TICK_SLOTS_PER_ARRAY: usize = 60;
const RAYDIUM_TICK_SLOT_TICK_INDEX_OFFSET: usize = 0;
const RAYDIUM_TICK_SLOT_LIQUIDITY_NET_OFFSET: usize = 4;
const RAYDIUM_TICK_SLOT_LIQUIDITY_GROSS_OFFSET: usize = 20;
/// Smallest account size that still contains all 60 tick slots.
pub(crate) const RAYDIUM_TICK_ARRAY_MINIMUM_SIZE_IN_BYTES: usize =
    RAYDIUM_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE
        + RAYDIUM_TICK_SLOT_SIZE_IN_BYTES * RAYDIUM_TICK_SLOTS_PER_ARRAY;

pub fn decode_raydium_tick_array_account(
    watched_tick_array: &TickArrayPdaToWatch,
    account_data: &[u8],
    slot: u64,
    geyser_write_version: u64,
) -> Option<TickArrayAccountWithInitializedTicks> {
    if watched_tick_array.dex != DexProgram::RaydiumClmm
        || account_data.len() < RAYDIUM_TICK_ARRAY_MINIMUM_SIZE_IN_BYTES
    {
        return None;
    }
    if account_data.get(..8)? != RAYDIUM_TICK_ARRAY_DISCRIMINATOR {
        return None;
    }
    // Guards against a wrong PDA: the array must say it belongs to the pool we expect.
    if read_public_key(account_data, RAYDIUM_TICK_ARRAY_POOL_ADDRESS_AT_BYTE)?
        != watched_tick_array.pool_address
    {
        return None;
    }

    let start_tick_index =
        read_i32_little_endian(account_data, RAYDIUM_TICK_ARRAY_START_TICK_AT_BYTE)?;
    let mut initialized_ticks = Vec::new();
    for slot_position in 0..RAYDIUM_TICK_SLOTS_PER_ARRAY {
        let slot_start = RAYDIUM_TICK_ARRAY_FIRST_TICK_SLOT_AT_BYTE
            + slot_position * RAYDIUM_TICK_SLOT_SIZE_IN_BYTES;
        let liquidity_gross = read_u128_little_endian(
            account_data,
            slot_start + RAYDIUM_TICK_SLOT_LIQUIDITY_GROSS_OFFSET,
        )?;
        if liquidity_gross == 0 {
            continue;
        }
        initialized_ticks.push(InitializedTickWithLiquidityChange {
            tick_index: read_i32_little_endian(
                account_data,
                slot_start + RAYDIUM_TICK_SLOT_TICK_INDEX_OFFSET,
            )?,
            liquidity_added_when_price_crosses_upward: read_i128_little_endian(
                account_data,
                slot_start + RAYDIUM_TICK_SLOT_LIQUIDITY_NET_OFFSET,
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
