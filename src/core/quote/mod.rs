//! Local CLMM quotes from the in-memory cache.
//!
//! ## What this module does
//!
//! An **AMM** (Automated Market Maker) is a pool of two tokens. You swap one
//! token for the other at a price set by the pool's math. A **CLMM**
//! (Concentrated Liquidity Market Maker) like Orca Whirlpools or Raydium CLMM
//! spreads liquidity across discrete **price ticks** instead of the full curve.
//!
//! This bot compares prices across venues to find **arbitrage**: buy cheap on
//! one DEX, sell dear on another. Before sending a real transaction we need a
//! **quote** — how many tokens you get for a given input — computed locally
//! from cached pool state. After a fixed 0.1 SOL probe, a dual-book tick walk
//! sizes the trade from liquidity (max PnL, not a guessed SOL amount), then
//! venue math quotes that size.
//!
//! ## How quotes are computed
//!
//! We replay the same integer swap math the DEX runs on-chain (walking ticks,
//! applying fees). We do **not** call `simulateTransaction` or LiteSVM; that
//! would be slower and depend on RPC.
//!
//! ## Key terms (for readers new to the domain)
//!
//! - **Lamports**: smallest unit of SOL on Solana (1 SOL = 1_000_000_000 lamports).
//! - **Exact-in**: you specify how much you *send*; the pool tells you how much you *receive*.
//! - **Tick**: one step on the price ladder. Price moves as swaps consume liquidity.
//! - **Tick array**: an on-chain account holding a contiguous window of ticks.
//! - **`a_to_b`**: swap direction. On our SOL/USDC pools, mint A = SOL and mint B = USDC,
//!   so `a_to_b = true` means SOL → USDC (sell SOL), `false` means USDC → SOL (buy SOL).

mod arbitrage;
mod orca;
mod raydium;
mod tick_arrays;
mod types;
mod walk;

pub use arbitrage::{best_round_trip, round_trip};
pub use types::*;
pub use walk::max_pnl_sol_in;

use super::types::{ClmmPoolState, TickArraySnapshot, Venue};

/// Quote an exact-in swap on one CLMM pool.
///
/// # Arguments
/// - `pool`: current price, liquidity, and fee tier.
/// - `arrays`: nearby tick-array snapshots from the cache (liquidity steps).
/// - `amount_in`: how much of the input token to swap (raw units).
/// - `a_to_b`: swap direction — `true` = SOL→USDC, `false` = USDC→SOL.
///
/// Dispatches to the venue-specific math (Orca vs Raydium) but returns the
/// same `Quote` shape either way.
pub fn quote_exact_in(
    pool: &ClmmPoolState,
    arrays: &[TickArraySnapshot],
    amount_in: u64,
    a_to_b: bool,
) -> Result<Quote, QuoteError> {
    if amount_in == 0 {
        return Ok(Quote {
            amount_in: 0,
            amount_out: 0,
            fee: 0,
        });
    }
    // The swap walk starts at the pool's current tick; we need the tick array
    // that contains that tick, or the venue math cannot begin.
    if !tick_arrays::has_current_array(pool, arrays) {
        return Err(if arrays.is_empty() {
            QuoteError::NoTickArrays
        } else {
            QuoteError::MissingCurrentArray
        });
    }
    // Each DEX has its own on-chain layout; we call the matching quote library.
    match pool.venue {
        Venue::OrcaWhirlpool => orca::quote_orca(pool, arrays, amount_in, a_to_b),
        Venue::RaydiumClmm => raydium::quote_raydium(pool, arrays, amount_in, a_to_b),
    }
}
