//! # Step 5 — Find the best arbitrage size
//!
//! **What came before:** Step 4 can quote any trade size exactly. But which
//! size should we trade?
//!
//! **Why size matters:** our own trade moves both prices. Selling SOL on the
//! expensive pool pushes its price down; buying SOL on the cheap pool pushes
//! its price up. The gap closes as we trade:
//!
//! ```text
//! sell pool: 101 ↘
//! buy pool:  100 ↗      …trade more…     both ≈ 100.5  → no gap left
//! ```
//!
//! - Trade too little → leave profit on the table.
//! - Trade too much → the last part of the trade loses money and eats the
//!   profit from the first part.
//!
//! The best size is where one *more* tiny unit of trade would earn exactly
//! zero after fees. Economists call that "marginal profit = 0"; it is the top
//! of the total-profit curve.
//!
//! **How we find it — walking the tick books.** Inside one tick range each
//! pool has constant liquidity, so there is a closed-form formula for the
//! size that closes the gap ([`concentrated_liquidity_formulas`]). But either
//! pool may hit a tick where liquidity changes before the gap closes. So we
//! walk both pools together, one step at a time. Each step goes as far as the
//! *first* of three events:
//!
//! 1. the price gap after fees closes → done, this is the best size;
//! 2. the sell pool's price reaches its next initialized tick → update its liquidity, continue;
//! 3. the buy pool's price reaches its next initialized tick → update its liquidity, continue.
//!
//! We add up the start-token (SOL) input from every step. That total is the
//! answer, and [`most_profitable_round_trip`] then quotes it with Step 4's
//! exact DEX math.
//!
//! **Why measure steps in the bridge token (USDC)?** The USDC that comes out of
//! the sell pool is exactly the USDC that goes into the buy pool, so one number
//! moves both pools at once. SOL amounts differ on the two sides.
//!
//! Current limitation: the walker assumes token A (SOL) is the start token and
//! token B (USDC) is the bridge. Other pairs or directions would need the
//! mirrored formulas.

mod functions;

mod find_input_amount_that_maximizes_profit;
mod most_profitable_round_trip;
mod watched_pair_round_trips;

pub use find_input_amount_that_maximizes_profit::main_find_input_amount_that_maximizes_profit;
pub use most_profitable_round_trip::main_quote_most_profitable_two_pool_round_trip;
pub use watched_pair_round_trips::{
    RoundTripQuotesTouchingPool, main_quote_round_trips_touching_pool,
};
