//! **Sub-step 1.3.** What to do when a tick-array account changes (someone added or removed liquidity).
//!
//! **Start here:** [`AccountUpdateListener::main_handle_tick_array_account_update`].

use super::account_update_loop::AccountUpdateListener;
use crate::print_logs;
use crate::step_2_decode_account_bytes::main_decode_tick_array_account;
use crate::step_3_store_latest_pool_state::TickArrayPdaToWatch;

impl AccountUpdateListener<'_> {
    /// Decode the tick-array bytes and save them into the cache.
    pub(super) fn main_handle_tick_array_account_update(
        &mut self,
        watched_tick_array: &TickArrayPdaToWatch,
        account_data: &[u8],
        slot: u64,
        geyser_write_version: u64,
    ) {
        let Some(tick_array) = main_decode_tick_array_account(
            watched_tick_array,
            account_data,
            slot,
            geyser_write_version,
        ) else {
            print_logs::tick_array_decode_failed(
                watched_tick_array.dex,
                &watched_tick_array.tick_array_address,
                false,
            );
            return;
        };
        self.cache.save_tick_array(tick_array.clone());
        print_logs::tick_array_account_updated(&tick_array);
    }
}
