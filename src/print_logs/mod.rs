//! # Print logs
//!
//! Not a pipeline step. Listen, decode, quote, size, and send each produce
//! something worth seeing; this module only formats it. Each line starts with
//! a tag saying where the information came from:
//!
//! - `[geyser/pool]`       live stream: a pool account changed
//! - `[geyser/tick-array]` live stream: a tick-array account changed
//! - `[rpc/tick-array]`    one-time RPC load of tick arrays we just started watching
//! - `[snapshot]`          what the cache holds right now, both pools side by side
//! - `[best size]`         round trip at the profit-maximizing size from Step 5
//! - `[trade]`             trading setup at startup (wallet, funding mode, safety switch)
//! - `[decide]`            Step 6's verdict after every cost
//! - `[simulate]`          dry-run of the signed transaction
//! - `[send]`              real submission and confirmation
//!
//! All printing lives here so the numbered steps stay about logic, not formatting.
//!
//! The same lines are copied to `logs/arb-bot.log` (override with `LOG_FILE`)
//! because the live stream scrolls the terminal faster than a person can read.

#[macro_use]
mod output;
mod account_update_logs;
mod arbitrage_quote_logs;
mod pool_snapshot_logs;
mod startup_banner;
mod trade_execution_logs;

pub use account_update_logs::*;
pub use arbitrage_quote_logs::*;
pub use pool_snapshot_logs::*;
pub use startup_banner::*;
pub use trade_execution_logs::*;
