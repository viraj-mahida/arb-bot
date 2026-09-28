//! Round-trip quotes at the profit-maximizing size from Step 5.
//!
//! Formatting only. The quotes themselves are produced by Steps 4 and 5.

use crate::step_4_quote_swaps::{
    DirectedRoundTripQuote, TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed,
};
use crate::step_5_find_best_arbitrage_size::RoundTripQuotesTouchingPool;

const LAMPORTS_PER_SOL: f64 = 1e9;
const MICRO_USDC_PER_USDC: f64 = 1e6;

pub fn print_arbitrage_quotes(quotes: &RoundTripQuotesTouchingPool) {
    for quote in &quotes.best_size_round_trips {
        print_round_trip("best size", quote);
    }
}

fn print_round_trip(tag: &str, quote: &DirectedRoundTripQuote) {
    match &quote.result {
        Ok(round_trip) => print_successful_round_trip(tag, &quote.direction_label(), round_trip),
        // Already stated by the `[spread]` line; repeating it twice per update is noise.
        Err(WhySwapQuoteFailed::NoProfitablePriceGapAfterFees) => {}
        Err(reason) => log_line!("[{tag}]  {}  skipped: {reason}", quote.direction_label()),
    }
}

fn print_successful_round_trip(
    tag: &str,
    direction_label: &str,
    round_trip: &TwoPoolArbitrageRoundTrip,
) {
    let profit_in_sol = round_trip.profit_in_start_token() as f64 / LAMPORTS_PER_SOL;
    let partial_fill_note = if round_trip.both_swaps_fully_filled {
        ""
    } else {
        "  (partial fill)"
    };
    let profitable_note = if round_trip.profit_in_start_token() > 0 {
        "  PROFITABLE"
    } else {
        ""
    };
    log_line!(
        "[{tag}]  {direction_label}  {:.4} SOL -> {:.4} USDC -> {:.4} SOL  profit={profit_in_sol:+.6} SOL{partial_fill_note}{profitable_note}",
        round_trip.start_token_amount_in as f64 / LAMPORTS_PER_SOL,
        round_trip.bridge_token_amount_between_legs as f64 / MICRO_USDC_PER_USDC,
        round_trip.start_token_amount_out as f64 / LAMPORTS_PER_SOL,
    );
    crate::dashboard_events::quote(
        direction_label,
        round_trip.start_token_amount_in as f64 / LAMPORTS_PER_SOL,
        round_trip.bridge_token_amount_between_legs as f64 / MICRO_USDC_PER_USDC,
        round_trip.start_token_amount_out as f64 / LAMPORTS_PER_SOL,
        profit_in_sol,
        round_trip.profit_in_start_token() > 0,
        !round_trip.both_swaps_fully_filled,
    );
}
