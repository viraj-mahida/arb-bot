use super::super::types::{ClmmPoolState, TickArraySnapshot};
use super::types::{QuoteError, RoundTrip};
use super::walk;
use super::quote_exact_in;

/// Simulate a two-hop arbitrage: sell SOL on `sell`, then buy SOL on `buy`.
///
/// Leg 1: SOL → USDC on the sell pool (`a_to_b = true`).
/// Leg 2: spend all USDC received → SOL on the buy pool (`a_to_b = false`).
pub fn round_trip(
    sell: &ClmmPoolState,
    sell_arrays: &[TickArraySnapshot],
    buy: &ClmmPoolState,
    buy_arrays: &[TickArraySnapshot],
    sol_in: u64,
) -> Result<RoundTrip, QuoteError> {
    let out_usdc = quote_exact_in(sell, sell_arrays, sol_in, true)?;
    let back_sol = quote_exact_in(buy, buy_arrays, out_usdc.amount_out, false)?;
    Ok(RoundTrip {
        sell: sell.venue,
        buy: buy.venue,
        sol_in,
        usdc_mid: out_usdc.amount_out,
        sol_out: back_sol.amount_out,
        fully_filled: out_usdc.amount_in == sol_in && back_sol.amount_in == out_usdc.amount_out,
    })
}

/// Size the round-trip from both tick books, then quote that size with venue math.
///
/// The walk finds the SOL-in where marginal PnL hits zero (max total PnL).
/// [`round_trip`] then applies Orca/Raydium swap math so `min_out` would match
/// the chain.
pub fn best_round_trip(
    sell: &ClmmPoolState,
    sell_arrays: &[TickArraySnapshot],
    buy: &ClmmPoolState,
    buy_arrays: &[TickArraySnapshot],
) -> Result<RoundTrip, QuoteError> {
    let sol_in = walk::max_pnl_sol_in(sell, sell_arrays, buy, buy_arrays)?;
    if sol_in == 0 {
        return Err(QuoteError::NoEdge);
    }
    round_trip(sell, sell_arrays, buy, buy_arrays, sol_in)
}
