//! Round-trip quotes at the profit-maximizing size from Step 5.
//!
//! Formatting only. The quotes themselves are produced by Steps 4 and 5.

use std::sync::Arc;

use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks,
};
use crate::step_4_quote_swaps::{
    DirectedRoundTripQuote, TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed,
    main_quote_two_pool_round_trip, pool_label,
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
        Ok(round_trip) => {
            print_successful_round_trip(tag, &quote.direction_label(), round_trip);
            sample_profit_curve(
                &quote.sell_pool,
                &tick_array_refs(&quote.sell_pool_tick_arrays),
                &quote.buy_pool,
                &tick_array_refs(&quote.buy_pool_tick_arrays),
                round_trip.start_token_amount_in,
            );
        }
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

fn tick_array_refs(
    tick_arrays: &[Arc<TickArrayAccountWithInitializedTicks>],
) -> Vec<&TickArrayAccountWithInitializedTicks> {
    tick_arrays.iter().map(Arc::as_ref).collect()
}

/// A few exact quotes around the best size, for the visualizer's profit curve.
///
/// No-op unless the dashboard is on, and then at most once every two seconds.
fn sample_profit_curve(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
    best_input_amount: u64,
) {
    if !crate::dashboard_events::curve_sample_due() {
        return;
    }
    const FRACTIONS: [f64; 6] = [0.2, 0.4, 0.6, 0.8, 1.0, 1.35];
    let mut points = Vec::with_capacity(FRACTIONS.len());
    for fraction in FRACTIONS {
        let size = ((best_input_amount as f64) * fraction) as u64;
        if size == 0 {
            continue;
        }
        let Ok(round_trip) = main_quote_two_pool_round_trip(
            sell_pool,
            sell_pool_tick_arrays,
            buy_pool,
            buy_pool_tick_arrays,
            size,
        ) else {
            continue;
        };
        points.push((
            round_trip.start_token_amount_in as f64 / LAMPORTS_PER_SOL,
            round_trip.profit_in_start_token() as f64 / LAMPORTS_PER_SOL,
        ));
    }
    if !points.is_empty() {
        crate::dashboard_events::profit_curve(
            &points,
            &pool_label(sell_pool),
            &pool_label(buy_pool),
        );
    }
}
