//! # Step 1 — Listen to account updates
//!
//! **What came before:** `main.rs` connected to Geyser and subscribed to the
//! pool accounts we watch.
//!
//! **What this step does:** loops forever over the live stream. Each message
//! says "account X now holds these bytes". There are only two kinds of
//! accounts we care about:
//!
//! 1. **A pool account** ([`handle_pool_account_update`]) — price or liquidity
//!    changed. We decode it, store it, and check whether the price has moved
//!    close enough to new tick arrays that we should start watching them too.
//! 2. **A tick-array account** ([`handle_tick_array_account_update`]) — the map
//!    of where liquidity changes was updated (someone added or removed
//!    liquidity). We decode it and store it.
//!
//! **A catch:** Geyser only sends *future* writes. A tick array that nobody
//! touches for an hour would stay unknown for an hour. So the first time we
//! start watching a tick array, we also fetch its current bytes once over RPC
//! ([`load_tick_arrays_not_yet_streamed`]).
//!
//! **What comes next:** decoding (Step 2), storing (Step 3), and printing a
//! fresh snapshot with quotes (Steps 4–6) after every pool update.

mod account_update_loop;
mod handle_pool_account_update;
mod handle_tick_array_account_update;
mod load_tick_arrays_not_yet_streamed;

pub use account_update_loop::process_account_updates_forever;
