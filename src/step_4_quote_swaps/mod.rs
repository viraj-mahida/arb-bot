//! # Step 4 — Quote swaps
//!
//! **What came before:** Step 3's cache holds the latest state of each pool
//! and its nearby tick arrays.
//!
//! **What this step does:** answers "if I put in N of token X, how much of
//! token Y comes out?" — a *quote* — using the exact same integer math the DEX
//! program runs on-chain. Then it chains two quotes into an arbitrage round
//! trip: swap on one pool, swap back on the other, and compare what you end
//! with against what you started with.
//!
//! ## From first principles: how a pool sets its price
//!
//! **Constant-product AMM (Automated Market Maker).** A pool holds `x` of
//! token A and `y` of token B and promises to keep `x * y = k` constant.
//! Example: 100 SOL and 10,000 USDC, so `k = 1,000,000` and the price is
//! `y / x = 100 USDC per SOL`. If you take 10 SOL out, 90 remain, so the pool
//! needs `1,000,000 / 90 = 11,111` USDC — you must pay about 1,111 USDC, not
//! 1,000. The price moved against you as you traded. That is **price impact**
//! (sometimes called slippage).
//!
//! **Concentrated liquidity (CLMM).** In a plain AMM every deposit is spread
//! across all prices from zero to infinity, so most of it is never used. In a
//! CLMM each liquidity provider picks a price range (say 90–110 USDC per SOL),
//! and their money only works inside it. Near the current price there is far
//! more depth, so traders get less price impact.
//!
//! **Ticks.** To make ranges manageable, the price line is cut into steps:
//! tick `i` means price `1.0001^i`, so each tick is a 0.01% move. Ranges may
//! only start and end on ticks. Where a range starts or ends, active liquidity
//! changes — those are the *initialized ticks* stored in tick-array accounts.
//!
//! **Liquidity `L` and square-root price.** Inside one tick range the pool
//! behaves like a plain `x * y = k` pool with `L = sqrt(k)`. Pools store
//! `sqrt(price)` instead of price because then the token amounts become simple
//! straight-line formulas (derived in Step 5's formulas file).
//!
//! **Q64.64.** That square-root price is stored as `sqrt(price) * 2^64`, an
//! integer, because on-chain math must be exact and identical on every
//! validator. Same idea as storing dollars as cents.
//!
//! **Fees.** Each swap pays a small percentage of its input (0.04% for our
//! pools) to the liquidity providers. The fee is taken first; only the rest
//! moves the price.
//!
//! ## Terms used in this step
//! - **Exact input:** you fix how much you *send*; the pool tells you how much you *receive*.
//! - **Token A / token B:** the pool's two sides. For our pools A = SOL, B = USDC.
//! - **Lamports:** SOL's smallest unit (1 SOL = 1,000,000,000 lamports). All
//!   amounts in code are raw integer units like this, never decimals.
//!
//! **What comes next:** Step 5 decides *how much* to trade, then uses this
//! step to quote that amount precisely.

mod functions;
mod swap_quote_types;

mod quote_swap_exact_input;
mod two_pool_arbitrage_round_trip;

pub use functions::is_tick_array_for_current_price_cached;
#[allow(unused_imports)]
// single-swap quoting; used by tests and by the future transaction step
pub use quote_swap_exact_input::main_quote_swap_exact_input;
pub use swap_quote_types::*;
pub(crate) use two_pool_arbitrage_round_trip::CachedOrcaAndRaydiumPools;
pub use two_pool_arbitrage_round_trip::main_quote_two_pool_round_trip;
