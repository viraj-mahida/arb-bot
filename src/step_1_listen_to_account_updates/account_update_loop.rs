//! **Sub-step 1.1.** The main loop: read the next Geyser message and hand it to the right handler.
//!
//! **Start here:** [`main_process_account_updates_forever`].

use std::collections::HashSet;
use std::sync::Arc;

use futures::StreamExt;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;

use crate::print_logs;
use crate::solana_connections::{
    GeyserAccountUpdateStream, GeyserSubscriptionSender, SolanaRpcClient,
};
use crate::step_3_store_latest_pool_state::{
    LatestPoolStateCache, WatchedPools, public_key_from_byte_slice,
};
use crate::step_6_build_and_send_transactions::ArbitrageTradeExecutor;

/// Everything the handlers need while processing updates.
pub(super) struct AccountUpdateListener<'a> {
    /// Used to widen the Geyser subscription when we start watching new tick arrays.
    pub(super) geyser_subscription_sender: GeyserSubscriptionSender,
    /// Every address currently subscribed (pools + tick arrays). Resent in full
    /// on each change, because a Geyser subscribe request replaces the old one.
    pub(super) all_subscribed_addresses_base58: HashSet<String>,
    pub(super) cache: &'a LatestPoolStateCache,
    pub(super) rpc_client: &'a SolanaRpcClient,
    /// `None` when trading is not configured (watch-only).
    pub(super) trade_executor: Option<Arc<ArbitrageTradeExecutor>>,
}

/// Process Geyser account updates until the stream ends.
pub async fn main_process_account_updates_forever(
    mut account_update_stream: GeyserAccountUpdateStream,
    geyser_subscription_sender: GeyserSubscriptionSender,
    watched_pools: &WatchedPools,
    cache: &LatestPoolStateCache,
    rpc_client: &SolanaRpcClient,
    trade_executor: Option<Arc<ArbitrageTradeExecutor>>,
) {
    let mut listener = AccountUpdateListener {
        geyser_subscription_sender,
        all_subscribed_addresses_base58: watched_pools
            .pool_addresses_to_subscribe()
            .into_iter()
            .collect(),
        cache,
        rpc_client,
        trade_executor,
    };
    print_logs::waiting_for_account_updates();

    while let Some(message) = account_update_stream.next().await {
        let update = match message {
            Ok(update) => update,
            Err(error) => {
                print_logs::geyser_stream_error(error);
                continue;
            }
        };
        // The same subscription asks for accounts and blocks_meta. Ping and
        // other message kinds are ignored; a ping reply would replace the filters.
        match update.update_oneof {
            Some(UpdateOneof::BlockMeta(meta)) => {
                if let Some(executor) = &listener.trade_executor {
                    executor.note_blockhash(meta.slot, &meta.blockhash);
                }
            }
            Some(UpdateOneof::Account(account_update)) => {
                let Some(account) = &account_update.account else {
                    continue;
                };
                let Some(account_address) = public_key_from_byte_slice(&account.pubkey) else {
                    continue;
                };
                cache.record_stream_update(account_update.slot);

                if let Some(pool_config) = watched_pools.config_for_pool_address(&account_address) {
                    listener
                        .main_handle_pool_account_update(
                            pool_config,
                            &account.data,
                            account_update.slot,
                            account.write_version,
                        )
                        .await;
                } else if let Some(watched_tick_array) = cache.watched_tick_array(&account_address)
                {
                    listener.main_handle_tick_array_account_update(
                        &watched_tick_array,
                        &account.data,
                        account_update.slot,
                        account.write_version,
                    );
                }
            }
            _ => {}
        }
    }
}
