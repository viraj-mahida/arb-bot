//! One-time RPC load of tick arrays we just started watching.
//!
//! Geyser will tell us about *future* changes to these accounts, but they may
//! not change for a long time. Without this load, the bot would quote swaps as
//! if the pool had no liquidity steps at all.

use crate::print_logs;
use crate::solana_connections::SolanaRpcClient;
use crate::step_2_decode_account_bytes::main_decode_tick_array_account;
use crate::step_3_store_latest_pool_state::{LatestPoolStateCache, PublicKeyBytes};

/// RPC data has no Geyser write version, so it uses 0 — the oldest possible.
/// Any real Geyser update for the same account will then always win.
const WRITE_VERSION_FOR_RPC_LOADED_DATA: u64 = 0;

pub(crate) async fn load_tick_arrays_not_yet_streamed(
    rpc_client: &SolanaRpcClient,
    cache: &LatestPoolStateCache,
    tick_array_addresses: &[PublicKeyBytes],
) {
    print_logs::rpc_tick_array_load_started(tick_array_addresses.len());
    let accounts = match rpc_client.get_multiple_accounts(tick_array_addresses).await {
        Ok(accounts) => accounts,
        Err(error) => {
            print_logs::rpc_tick_array_load_failed(error);
            return;
        }
    };

    let mut decoded_count = 0;
    // A missing account means nobody has ever placed liquidity in that tick range.
    let mut missing_on_chain_count = 0;
    let mut failed_to_decode_count = 0;
    for (tick_array_address, maybe_account) in tick_array_addresses.iter().zip(accounts) {
        let Some(account) = maybe_account else {
            missing_on_chain_count += 1;
            continue;
        };
        let Some(watched_tick_array) = cache.watched_tick_array(tick_array_address) else {
            failed_to_decode_count += 1;
            continue;
        };
        let Some(tick_array) = main_decode_tick_array_account(
            &watched_tick_array,
            &account.account_data,
            account.slot,
            WRITE_VERSION_FOR_RPC_LOADED_DATA,
        ) else {
            print_logs::tick_array_decode_failed(watched_tick_array.dex, tick_array_address, true);
            failed_to_decode_count += 1;
            continue;
        };
        cache.save_tick_array(tick_array);
        decoded_count += 1;
    }
    print_logs::rpc_tick_array_load_finished(
        decoded_count,
        tick_array_addresses.len(),
        missing_on_chain_count,
        failed_to_decode_count,
    );
}
