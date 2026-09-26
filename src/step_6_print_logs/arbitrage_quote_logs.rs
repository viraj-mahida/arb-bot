//! Round-trip quotes in both directions: a fixed 0.1 SOL probe, then the best size.
//!
//! We do not know in advance which pool is more expensive, so we try both
//! directions. At most one of them can be profitable at a time.

use crate::step_3_store_latest_pool_state::{DexProgram, LatestPoolStateCache};
use crate::step_4_quote_swaps::{
    PROBE_TRADE_INPUT_AMOUNT, TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed, quote_two_pool_round_trip,
};
use crate::step_5_find_best_arbitrage_size::quote_most_profitable_two_pool_round_trip;

const LAMPORTS_PER_SOL: f64 = 1e9;
const MICRO_USDC_PER_USDC: f64 = 1e6;

pub(super) fn print_arbitrage_quotes(cache: &LatestPoolStateCache) {
    let Some(raydium_pool) = cache.first_pool_on_dex(DexProgram::RaydiumClmm) else {
        return;
    };
    let Some(orca_pool) = cache.first_pool_on_dex(DexProgram::OrcaWhirlpool) else {
        return;
    };
    let raydium_tick_arrays = cache.tick_arrays_for_pool(&raydium_pool.pool_address);
    let orca_tick_arrays = cache.tick_arrays_for_pool(&orca_pool.pool_address);

    print_round_trip(
        "quote 0.1 SOL",
        "raydium_clmm→orca_whirlpool",
        quote_two_pool_round_trip(&raydium_pool, &raydium_tick_arrays, &orca_pool, &orca_tick_arrays, PROBE_TRADE_INPUT_AMOUNT),
    );
    print_round_trip(
        "quote 0.1 SOL",
        "orca_whirlpool→raydium_clmm",
        quote_two_pool_round_trip(&orca_pool, &orca_tick_arrays, &raydium_pool, &raydium_tick_arrays, PROBE_TRADE_INPUT_AMOUNT),
    );
    print_round_trip(
        "best size",
        "raydium_clmm→orca_whirlpool",
        quote_most_profitable_two_pool_round_trip(&raydium_pool, &raydium_tick_arrays, &orca_pool, &orca_tick_arrays),
    );
    print_round_trip(
        "best size",
        "orca_whirlpool→raydium_clmm",
        quote_most_profitable_two_pool_round_trip(&orca_pool, &orca_tick_arrays, &raydium_pool, &raydium_tick_arrays),
    );
}

fn print_round_trip(
    tag: &str,
    direction_label_if_failed: &str,
    result: Result<TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed>,
) {
    match result {
        Ok(round_trip) => {
            let direction_label = format!("{}→{}", round_trip.sell_pool_dex.name(), round_trip.buy_pool_dex.name());
            let profit_in_sol = round_trip.profit_in_start_token() as f64 / LAMPORTS_PER_SOL;
            let partial_fill_note = if round_trip.both_swaps_fully_filled { "" } else { "  (partial fill)" };
            let profitable_note = if round_trip.profit_in_start_token() > 0 { "  PROFITABLE" } else { "" };
            println!(
                "[{tag}]  {direction_label}  {:.4} SOL -> {:.4} USDC -> {:.4} SOL  profit={profit_in_sol:+.6} SOL{partial_fill_note}{profitable_note}",
                round_trip.start_token_amount_in as f64 / LAMPORTS_PER_SOL,
                round_trip.bridge_token_amount_between_legs as f64 / MICRO_USDC_PER_USDC,
                round_trip.start_token_amount_out as f64 / LAMPORTS_PER_SOL,
            );
        }
        Err(reason) => println!("[{tag}]  {direction_label_if_failed}  skipped: {reason}"),
    }
}
