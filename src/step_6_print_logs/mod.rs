//! # Step 6 — Print logs
//!
//! **What came before:** every earlier step produced something worth seeing:
//! a connection, an account update, a decoded pool, a quote.
//!
//! **What this step does:** prints it in a way a learner can follow. Each line
//! starts with a tag saying where the information came from:
//!
//! - `[geyser/pool]`       live stream: a pool account changed
//! - `[geyser/tick-array]` live stream: a tick-array account changed
//! - `[rpc/tick-array]`    one-time RPC load of tick arrays we just started watching
//! - `[snapshot]`          what the cache holds right now, both pools side by side
//! - `[quote 0.1 SOL]`     round trip at a small fixed size
//! - `[best size]`         round trip at the profit-maximizing size from Step 5
//! - `[trade]`             trading setup at startup (wallet, funding mode, safety switch)
//! - `[decide]`            Step 7's verdict after every cost
//! - `[simulate]`          dry-run of the signed transaction
//! - `[send]`              real submission and confirmation
//!
//! All printing lives here so the other steps stay about logic, not formatting.
//!
//! **What comes next:** Step 7 turns a profitable quote into a transaction.

mod account_update_logs;
mod arbitrage_quote_logs;
mod pool_snapshot_logs;
mod startup_banner;
mod trade_execution_logs;

pub use account_update_logs::*;
pub use pool_snapshot_logs::*;
pub use startup_banner::*;
pub use trade_execution_logs::*;
