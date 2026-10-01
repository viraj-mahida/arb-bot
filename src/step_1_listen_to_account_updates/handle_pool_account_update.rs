//! **Sub-step 1.2.** What to do when a pool account changes (usually because someone swapped).
//!
//! **Start here:** [`AccountUpdateListener::main_handle_pool_account_update`].

use super::account_update_loop::AccountUpdateListener;
use super::functions::{apply_raydium_fee_from_fee_config, load_tick_arrays_not_yet_streamed};
use crate::print_logs;
use crate::solana_connections::subscribe_to_account_updates;
use crate::step_2_decode_account_bytes::main_decode_pool_account;
use crate::step_3_store_latest_pool_state::{
    WatchedPoolConfig, encode_public_key_as_base58, tick_array_pdas_near_current_price,
};
use crate::step_5_find_best_arbitrage_size::main_quote_round_trips_touching_pool;

impl AccountUpdateListener<'_> {
    /// 1. Decode the pool bytes and save them.
    /// 2. Work out which tick arrays surround the (possibly new) price.
    /// 3. For any we were not watching yet: subscribe on Geyser and load them once via RPC.
    /// 4. Print a snapshot of all pools, then the profit-maximizing quotes.
    /// 5. If trading is configured, let Step 6 decide whether to trade.
    pub(super) async fn main_handle_pool_account_update(
        &mut self,
        pool_config: &WatchedPoolConfig,
        account_data: &[u8],
        slot: u64,
        geyser_write_version: u64,
    ) {
        let Some(mut pool_state) =
            main_decode_pool_account(pool_config, account_data, slot, geyser_write_version)
        else {
            print_logs::pool_decode_failed(pool_config.dex, pool_config.pool_address_base58);
            return;
        };
        apply_raydium_fee_from_fee_config(self.rpc_client, self.cache, &mut pool_state).await;

        let previous_pool = self.cache.pool_state_by_address(&pool_state.pool_address);
        let tick_arrays_near_price = tick_array_pdas_near_current_price(&pool_state);
        self.cache.save_pool_state(pool_state.clone());
        // ignr: visualizer visitors follow the real (unshifted) price.
        crate::dashboard_events::ignr_note_pool_change(previous_pool.as_deref(), &pool_state);
        let newly_watched_tick_arrays = self
            .cache
            .start_watching_tick_arrays(&tick_arrays_near_price);
        let loaded_tick_array_count = self
            .cache
            .loaded_tick_array_count_for_pool(&pool_state.pool_address);
        let watched_tick_array_count = self
            .cache
            .watched_tick_array_count_for_pool(&pool_state.pool_address);
        print_logs::pool_account_updated(
            &pool_state,
            loaded_tick_array_count,
            watched_tick_array_count,
        );

        if !newly_watched_tick_arrays.is_empty() {
            print_logs::starting_to_watch_tick_arrays(
                pool_state.dex,
                newly_watched_tick_arrays.len(),
            );
            for tick_array_address in &newly_watched_tick_arrays {
                self.all_subscribed_addresses_base58
                    .insert(encode_public_key_as_base58(tick_array_address));
            }
            let full_address_list = self
                .all_subscribed_addresses_base58
                .iter()
                .cloned()
                .collect();
            if let Err(error) = subscribe_to_account_updates(
                &mut self.geyser_subscription_sender,
                full_address_list,
            )
            .await
            {
                print_logs::tick_array_subscribe_failed(error);
            }
            load_tick_arrays_not_yet_streamed(
                self.rpc_client,
                self.cache,
                &newly_watched_tick_arrays,
            )
            .await;
        }

        print_logs::pool_snapshot(self.cache, &pool_state.pool_address);
        let quotes = main_quote_round_trips_touching_pool(self.cache, &pool_state.pool_address);
        print_logs::print_arbitrage_quotes(&quotes);
        if let Some(trade_executor) = &self.trade_executor {
            trade_executor.main_consider_trading(self.cache, &quotes);
        }
    }
}
