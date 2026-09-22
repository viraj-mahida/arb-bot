use crate::core::{
    PoolCache, QuoteError, RoundTrip, Venue, PROBE_SOL_LAMPORTS, best_round_trip, round_trip,
};

pub(super) fn log_quotes(cache: &PoolCache) {
    let Some(raydium) = cache.get_venue(Venue::RaydiumClmm) else {
        return;
    };
    let Some(orca) = cache.get_venue(Venue::OrcaWhirlpool) else {
        return;
    };
    let r_arrays = cache.tick_arrays(&raydium.pubkey);
    let o_arrays = cache.tick_arrays(&orca.pubkey);

    log_round_trip(
        "quote",
        "raydium_clmm→orca_whirlpool",
        round_trip(&raydium, &r_arrays, &orca, &o_arrays, PROBE_SOL_LAMPORTS),
    );
    log_round_trip(
        "quote",
        "orca_whirlpool→raydium_clmm",
        round_trip(&orca, &o_arrays, &raydium, &r_arrays, PROBE_SOL_LAMPORTS),
    );
    log_round_trip(
        "size",
        "raydium_clmm→orca_whirlpool",
        best_round_trip(&raydium, &r_arrays, &orca, &o_arrays),
    );
    log_round_trip(
        "size",
        "orca_whirlpool→raydium_clmm",
        best_round_trip(&orca, &o_arrays, &raydium, &r_arrays),
    );
}

fn log_round_trip(tag: &str, fallback: &str, result: Result<RoundTrip, QuoteError>) {
    match result {
        Ok(rt) => {
            let label = format!("{}→{}", rt.sell.as_str(), rt.buy.as_str());
            let pnl_sol = rt.pnl_lamports() as f64 / 1e9;
            let partial = if rt.fully_filled {
                ""
            } else {
                "  (partial fill)"
            };
            let edge = if rt.pnl_lamports() > 0 { "  EDGE" } else { "" };
            println!(
                "[{tag}]  {:.2} SOL  {label}  {:.4} SOL -> {:.4} USDC -> {:.4} SOL  pnl={pnl_sol:+.6} SOL{partial}{edge}",
                rt.sol_in as f64 / 1e9,
                rt.sol_in as f64 / 1e9,
                rt.usdc_mid as f64 / 1e6,
                rt.sol_out as f64 / 1e9,
            );
        }
        Err(e) => println!("[{tag}]  {fallback}  skipped: {e}"),
    }
}
