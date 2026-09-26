//! Pick the right decoder for an account, based on which DEX owns it.
//!
//! Adding a new DEX means adding one match arm in each function below plus its
//! own decoder file; the rest of the bot does not change.

use super::{orca_whirlpool_account_decoder, raydium_clmm_account_decoder};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, TickArrayAccountWithInitializedTicks,
    TickArrayPdaToWatch, WatchedPoolConfig,
};

/// Decode a pool account's bytes. `None` means the bytes did not match the expected layout.
pub fn decode_pool_account(
    pool_config: &WatchedPoolConfig,
    account_data: &[u8],
    slot: u64,
    geyser_write_version: u64,
) -> Option<ConcentratedLiquidityPoolState> {
    match pool_config.dex {
        DexProgram::RaydiumClmm => raydium_clmm_account_decoder::decode_raydium_pool_account(
            pool_config,
            account_data,
            slot,
            geyser_write_version,
        ),
        DexProgram::OrcaWhirlpool => orca_whirlpool_account_decoder::decode_orca_pool_account(
            pool_config,
            account_data,
            slot,
            geyser_write_version,
        ),
    }
}

/// Decode a tick-array account's bytes. `None` means the bytes did not match the expected layout.
pub fn decode_tick_array_account(
    watched_tick_array: &TickArrayPdaToWatch,
    account_data: &[u8],
    slot: u64,
    geyser_write_version: u64,
) -> Option<TickArrayAccountWithInitializedTicks> {
    match watched_tick_array.dex {
        DexProgram::RaydiumClmm => raydium_clmm_account_decoder::decode_raydium_tick_array_account(
            watched_tick_array,
            account_data,
            slot,
            geyser_write_version,
        ),
        DexProgram::OrcaWhirlpool => orca_whirlpool_account_decoder::decode_orca_tick_array_account(
            watched_tick_array,
            account_data,
            slot,
            geyser_write_version,
        ),
    }
}
