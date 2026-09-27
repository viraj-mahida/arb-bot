//! Raydium's real swap fee lives in a separate `amm_config` account.
//!
//! Many pools share one config (one per fee tier), and fee tiers practically
//! never change, so we read each config once via RPC and remember it. Until
//! that works, the pool keeps the fee from `WatchedPoolConfig`.

use crate::print_logs;
use crate::solana_connections::SolanaRpcClient;
use crate::step_2_decode_account_bytes::raydium_clmm_account_decoder::decode_raydium_fee_config_trade_fee_rate;
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexSpecificSwapAccounts, LatestPoolStateCache,
};

/// Replace `pool_state`'s fee with the one in its Raydium `amm_config` account.
pub(crate) async fn apply_raydium_fee_from_fee_config(
    rpc_client: &SolanaRpcClient,
    cache: &LatestPoolStateCache,
    pool_state: &mut ConcentratedLiquidityPoolState,
) {
    let DexSpecificSwapAccounts::RaydiumClmm {
        fee_config_address, ..
    } = pool_state.dex_specific_swap_accounts
    else {
        return;
    };
    if let Some(fee_rate) = cache.raydium_fee_rate_for_fee_config(&fee_config_address) {
        pool_state.fee_rate_in_millionths = fee_rate;
        return;
    }
    let fetched = rpc_client
        .get_multiple_accounts(&[fee_config_address])
        .await;
    let fee_rate = fetched
        .ok()
        .and_then(|accounts| accounts.into_iter().next().flatten())
        .and_then(|account| decode_raydium_fee_config_trade_fee_rate(&account.account_data));
    match fee_rate {
        Some(fee_rate) => {
            cache.save_raydium_fee_rate_for_fee_config(fee_config_address, fee_rate);
            print_logs::raydium_fee_config_loaded(fee_rate);
            pool_state.fee_rate_in_millionths = fee_rate;
        }
        None => print_logs::raydium_fee_config_load_failed(),
    }
}
