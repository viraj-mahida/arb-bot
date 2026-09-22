//! Educational stdout logs: *what* arrived, *why* we RPC, *what* the cache holds.

mod quotes;

use super::{
    ClmmPoolState, PoolCache, PoolRegistry, TickArraySnapshot, Venue, encode_pubkey, short_pubkey,
};

pub fn startup(registry: &PoolRegistry) {
    println!("arb-bot  (educational SOL/USDC CLMM watcher)");
    println!();
    println!("pipeline");
    println!("  1. Geyser (Yellowstone gRPC) streams live account writes");
    println!("  2. Decoder turns bytes into pool state + tick arrays");
    println!("  3. RPC getMultipleAccounts loads the current tick-array book once");
    println!("     (Geyser does not replay history; it only pushes later writes)");
    println!("  4. In-memory cache holds pool + nearby tick arrays");
    println!("  5. Local compute_swap quotes a 0.1 SOL probe, then dual-walks both tick books for size");
    println!("     (pool fees on; no gas/tip)");
    println!();
    println!("watching");
    for spec in registry.specs() {
        println!(
            "  {:<14}  {}  ({} bps fee, decimals {}/{})",
            spec.venue.as_str(),
            spec.address_bs58,
            spec.fee_bps,
            spec.decimals_a,
            spec.decimals_b
        );
    }
    println!();
    println!("log keys");
    println!("  [geyser/pool]   a pool account changed (price / liquidity / current tick)");
    println!("  [geyser/ticks]  a tick-array PDA changed (liquidity curve around price)");
    println!("  [rpc/hydrate]   one-shot fetch of tick-array bytes we just subscribed to");
    println!(
        "  [snapshot]      both venues as cached right now; Δ = raydium − orca (USDC per SOL)"
    );
    println!(
        "  [quote]         0.1 SOL sold on A, USDC bought back to SOL on B (local math, not a tx)"
    );
    println!(
        "  [size]          dual-walk SOL-in (max local PnL from tick liquidity, still not a tx)"
    );
    println!();
}

pub fn grpc_connecting() {
    println!("[grpc] connecting to Yellowstone…");
}

pub fn grpc_subscribed(pool_count: usize) {
    println!("[grpc] subscribed to {pool_count} pool account(s), commitment=processed");
}

pub fn rpc_ready() {
    println!("[rpc] HTTP client ready  (getMultipleAccounts, timeout 10s)");
}

pub fn ingest_ready() {
    println!("[geyser] waiting for account writes…");
    println!();
}

pub fn pool_update(pool: &ClmmPoolState, have_arrays: usize, want_arrays: usize) {
    println!(
        "[geyser/pool]  {:<14}  {}  slot={:<10} tick={:<7} spacing={}  liq={:<12.3e}  px={:.4}  arrays={}/{}",
        pool.venue.as_str(),
        short_pubkey(&pool.pubkey),
        pool.slot,
        pool.tick,
        pool.tick_spacing,
        pool.liquidity as f64,
        pool.spot_price(),
        have_arrays,
        want_arrays,
    );
}

pub fn subscribing_tick_arrays(venue: Venue, n: usize) {
    println!(
        "               first time seeing {n} nearby tick-array PDAs for {} — subscribe + RPC hydrate",
        venue.as_str()
    );
}

pub fn subscribe_tick_arrays_failed(err: impl std::fmt::Display) {
    eprintln!("[error] failed to subscribe tick-array accounts: {err}");
}

pub fn hydrate_start(n: usize) {
    println!(
        "[rpc/hydrate]  requesting {n} tick-array account(s)  (current on-chain bytes, not a live stream)"
    );
}

pub fn hydrate_done(decoded: usize, requested: usize, missing: usize, failed: usize) {
    println!(
        "[rpc/hydrate]  decoded {decoded}/{requested}  missing={missing}  decode_fail={failed}"
    );
}

pub fn hydrate_failed(err: impl std::fmt::Display) {
    eprintln!("[error] RPC hydrate failed: {err}");
}

pub fn tick_array_update(array: &TickArraySnapshot) {
    println!(
        "[geyser/ticks] {:<14}  pool={}  start={:<8} slot={:<10} initialized={}",
        array.venue.as_str(),
        short_pubkey(&array.pool),
        array.start_tick_index,
        array.slot,
        array.ticks.len(),
    );
}

pub fn decode_pool_failed(venue: &str, address: &str) {
    eprintln!("[error] could not decode {venue} pool {address}");
}

pub fn decode_tick_array_failed(venue: &str, pubkey: &[u8; 32], via_rpc: bool) {
    let src = if via_rpc { " (from RPC)" } else { "" };
    eprintln!(
        "[error] could not decode {venue} tick array {}{src}",
        encode_pubkey(pubkey)
    );
}

pub fn stream_error(err: impl std::fmt::Display) {
    eprintln!("[error] Geyser stream: {err}");
}

pub fn snapshot(cache: &PoolCache) {
    println!("[snapshot]  {} pool(s) cached", cache.len());
    println!(
        "  {:<14}  {:<10}  {:>10}  {:>7}  {:>10}  {:>8}  {:>6}",
        "venue", "pool", "slot", "tick", "USDC/SOL", "arrays", "ticks"
    );

    let mut rows: Vec<ClmmPoolState> = cache.snapshot();
    rows.sort_by_key(|p| match p.venue {
        Venue::RaydiumClmm => 0u8,
        Venue::OrcaWhirlpool => 1,
    });

    for pool in &rows {
        let arrays = cache.tick_arrays(&pool.pubkey);
        let n_ticks: usize = arrays.iter().map(|array| array.ticks.len()).sum();
        let want = cache.expected_tick_array_count(&pool.pubkey);
        println!(
            "  {:<14}  {:<10}  {:>10}  {:>7}  {:>10.4}  {:>5}/{:<2}  {:>6}",
            pool.venue.as_str(),
            short_pubkey(&pool.pubkey),
            pool.slot,
            pool.tick,
            pool.spot_price(),
            arrays.len(),
            want,
            n_ticks,
        );
    }

    let raydium = cache.get_venue(Venue::RaydiumClmm);
    let orca = cache.get_venue(Venue::OrcaWhirlpool);
    match (raydium, orca) {
        (Some(r), Some(o)) => {
            let rp = r.spot_price();
            let op = o.spot_price();
            let delta = rp - op;
            let mid = (rp + op) / 2.0;
            let bps = if mid > 0.0 {
                delta / mid * 10_000.0
            } else {
                0.0
            };
            println!(
                "  Δ raydium−orca = {delta:+.4} USDC/SOL  ({bps:+.2} bps of mid)  — raw spots, not a trade"
            );
            quotes::log_quotes(cache);
        }
        _ => println!("  (need both venues in cache before a spread is printed)"),
    }
    println!();
}
