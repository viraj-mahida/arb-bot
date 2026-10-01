//! Unit tests, kept out of the step folders so the logic stays easy to read.
//!
//! The layout mirrors the steps. Each test name reads as the scenario it
//! proves, and each test has a one-line doc saying what concept it checks.

/// Small trade size used by the quote and size tests: 0.1 SOL in lamports.
pub(crate) const PROBE_TRADE_INPUT_AMOUNT: u64 = 100_000_000;

mod recent_blockhash_cache;
mod step_2_decode_account_bytes;
mod step_3_store_latest_pool_state;
mod step_4_quote_swaps;
mod step_5_find_best_arbitrage_size;
mod step_6_build_and_send_transactions;
mod test_pool_builders;
