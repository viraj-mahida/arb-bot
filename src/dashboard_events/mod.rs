//! Live events for the Rbot visualizer ("Arb City").
//!
//! The numbered steps stay about trading logic. They already report through
//! `print_logs`; those functions also publish a JSON event here. A local
//! WebSocket (`DASHBOARD=true`) streams the events to the browser, and the
//! same lines are appended to `logs/dashboard-events.jsonl` for a replay.
//!
//! Nothing here runs unless `DASHBOARD=true`, except
//! [`with_demo_price_shift`], which is a no-op unless `DEMO_PRICE_SHIFT_BPS`
//! is set.

mod server;

use std::collections::VecDeque;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use orca_whirlpools_core::sqrt_price_to_tick_index;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, encode_public_key_as_base58,
};

const CHANNEL_CAPACITY: usize = 2048;
const RECENT_EVENT_LIMIT: usize = 400;
/// Profit-curve sampling walks the DEX quote math several times. Once every
/// two seconds is enough for the chart and keeps the Geyser loop responsive.
const CURVE_INTERVAL_MS: u64 = 2_000;

struct Hub {
    sender: broadcast::Sender<String>,
    recent: Mutex<VecDeque<String>>,
    jsonl: Mutex<Option<File>>,
    /// The balance is published once, then pool updates fill `recent` and drop it.
    /// Keep the last wallet line so a city that connects later still shows it.
    latest_wallet: Mutex<Option<String>>,
}

static HUB: OnceLock<Hub> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static LAST_CURVE_AT_MS: AtomicU64 = AtomicU64::new(0);

/// Signature fee, inclusion bribe, and minimum leftover for the route tried first.
///
/// The city displays these on Rbot's laptop. They are the same figures Step 6
/// subtracts, so the screen is not a second copy of `.env` typed into the UI.
struct PrimaryRouteCosts {
    network_fee_lamports: u64,
    priority_fee_lamports: u64,
    jito_tip_lamports: u64,
    flash_loan_fee_bps: u64,
    min_profit_lamports: u64,
}

static PRIMARY_ROUTE_COSTS: OnceLock<PrimaryRouteCosts> = OnceLock::new();

/// Remember the primary route's costs. Call once at startup, before any quote.
///
/// A Jito route passes a tip and a zero priority fee. An RPC route does the opposite.
/// Flash-loan bps are applied per quote, because that fee depends on the trade size.
pub fn remember_primary_route_costs(
    network_fee_lamports: u64,
    priority_fee_lamports: u64,
    jito_tip_lamports: u64,
    flash_loan_fee_bps: u64,
    min_profit_lamports: u64,
) {
    let _ = PRIMARY_ROUTE_COSTS.set(PrimaryRouteCosts {
        network_fee_lamports,
        priority_fee_lamports,
        jito_tip_lamports,
        flash_loan_fee_bps,
        min_profit_lamports,
    });
}

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| env_flag("DASHBOARD"))
}

pub fn demo_price_shift_bps() -> u64 {
    static BPS: OnceLock<u64> = OnceLock::new();
    *BPS.get_or_init(|| {
        std::env::var("DEMO_PRICE_SHIFT_BPS")
            .ok()
            .map(|value| value.trim().replace('_', ""))
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    })
}

/// Raise Orca's price by `DEMO_PRICE_SHIFT_BPS` on a *copy* used for quoting,
/// logs, and the dashboard. The cache keeps the real on-chain price.
///
/// The square-root price and the tick index are moved together, using Orca's
/// own tick conversion, so the quote libraries still accept the pool.
pub fn with_demo_price_shift(
    mut pool: ConcentratedLiquidityPoolState,
) -> ConcentratedLiquidityPoolState {
    let basis_points = demo_price_shift_bps();
    if basis_points == 0 || pool.dex != DexProgram::OrcaWhirlpool {
        return pool;
    }
    let scale = (1.0 + (basis_points as f64) / 10_000.0).sqrt();
    let shifted = (pool.sqrt_price_q64_64 as f64 * scale).round();
    if !(shifted.is_finite() && shifted > 0.0) {
        return pool;
    }
    let shifted = shifted as u128;
    pool.sqrt_price_q64_64 = shifted;
    pool.current_tick_index = sqrt_price_to_tick_index(shifted);
    pool
}

/// Open the broadcast channel, the JSONL file, and the WebSocket server.
/// Call once, before the first event.
pub fn install() {
    if !enabled() {
        return;
    }
    let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
    let jsonl = open_jsonl();
    let _ = HUB.set(Hub {
        sender,
        recent: Mutex::new(VecDeque::with_capacity(RECENT_EVENT_LIMIT)),
        jsonl: Mutex::new(jsonl),
        latest_wallet: Mutex::new(None),
    });
    server::spawn(port());
}

pub fn subscribe() -> Option<broadcast::Receiver<String>> {
    HUB.get().map(|hub| hub.sender.subscribe())
}

pub fn recent() -> Vec<String> {
    HUB.get()
        .and_then(|hub| hub.recent.lock().ok())
        .map(|recent| recent.iter().cloned().collect())
        .unwrap_or_default()
}

pub fn latest_wallet() -> Option<String> {
    HUB.get()
        .and_then(|hub| hub.latest_wallet.lock().ok())
        .and_then(|latest| latest.clone())
}

pub fn events_file_path() -> String {
    std::env::var("DASHBOARD_EVENTS_FILE")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or_else(|| "logs/dashboard-events.jsonl".to_string())
}

/// True at most once per [`CURVE_INTERVAL_MS`].
pub fn curve_sample_due() -> bool {
    if !enabled() {
        return false;
    }
    let now = unix_millis();
    let previous = LAST_CURVE_AT_MS.load(Ordering::Relaxed);
    if now.saturating_sub(previous) < CURVE_INTERVAL_MS {
        return false;
    }
    LAST_CURVE_AT_MS.store(now, Ordering::Relaxed);
    true
}

pub fn publish(mut value: Value) {
    let Some(hub) = HUB.get() else {
        return;
    };
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "id".to_string(),
            json!(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
        );
        object.insert("t".to_string(), json!(unix_millis()));
    }
    let Ok(line) = serde_json::to_string(&value) else {
        return;
    };
    if let Ok(mut recent) = hub.recent.lock() {
        if recent.len() >= RECENT_EVENT_LIMIT {
            recent.pop_front();
        }
        recent.push_back(line.clone());
    }
    if let Ok(mut file) = hub.jsonl.lock()
        && let Some(file) = file.as_mut()
    {
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
    if value.get("type").and_then(|kind| kind.as_str()) == Some("wallet")
        && let Ok(mut latest) = hub.latest_wallet.lock()
    {
        *latest = Some(line.clone());
    }
    let _ = hub.sender.send(line);
}

pub fn geyser(slot: u64, dex: &str, kind: &str, line: &str) {
    publish(json!({
        "type": "geyser",
        "slot": slot,
        "dex": dex,
        "kind": kind,
        "line": line,
    }));
}

pub fn note_pool_change(
    previous: Option<&ConcentratedLiquidityPoolState>,
    next: &ConcentratedLiquidityPoolState,
) {
    let Some(previous) = previous else {
        return;
    };
    if previous.pool_address != next.pool_address {
        return;
    }
    let old_price = previous.human_readable_price_token_b_per_token_a();
    let new_price = next.human_readable_price_token_b_per_token_a();
    let price_moved = (new_price - old_price).abs() > old_price.abs() * 1e-7 + 1e-6;
    if price_moved {
        // Selling SOL into the pool pushes the USDC-per-SOL price down.
        let direction = if new_price < old_price {
            "solToUsdc"
        } else {
            "usdcToSol"
        };
        publish(json!({
            "type": "swap",
            "dex": next.dex.name(),
            "pool": pool_id(next),
            "direction": direction,
            "previousPrice": old_price,
            "price": new_price,
            "slot": next.slot,
        }));
    } else if previous.active_liquidity_at_current_price != next.active_liquidity_at_current_price {
        publish(json!({
            "type": "liquidity",
            "dex": next.dex.name(),
            "pool": pool_id(next),
            "liquidity": next.active_liquidity_at_current_price.to_string(),
            "tickCount": 0,
            "slot": next.slot,
            "source": "pool",
        }));
    }
}

pub fn liquidity_touch(dex: &str, pool: &str, slot: u64, tick_count: usize, start_tick: i32) {
    publish(json!({
        "type": "liquidity",
        "dex": dex,
        "pool": pool,
        "tickCount": tick_count,
        "startTick": start_tick,
        "slot": slot,
        "source": "tickArray",
    }));
}

pub fn boards(
    pool: &ConcentratedLiquidityPoolState,
    other: &ConcentratedLiquidityPoolState,
    gap_bps: f64,
    fees_bps: f64,
    beats_fees: bool,
) {
    publish(json!({
        "type": "boards",
        "gapBps": gap_bps,
        "feesBps": fees_bps,
        "beatsFees": beats_fees,
        "pools": [pool_json(pool), pool_json(other)],
    }));
}

pub fn quote(
    direction: &str,
    input_sol: f64,
    bridge_usdc: f64,
    output_sol: f64,
    profit_sol: f64,
    profitable: bool,
    partial: bool,
) {
    let mut event = json!({
        "type": "quote",
        "direction": direction,
        "inputSol": input_sol,
        "bridgeUsdc": bridge_usdc,
        "outputSol": output_sol,
        "profitSol": profit_sol,
        "profitable": profitable,
        "partial": partial,
    });
    if let Some(costs) = PRIMARY_ROUTE_COSTS.get() {
        attach_quote_costs(&mut event, input_sol, profit_sol, profitable, partial, costs);
    }
    publish(event);
}

fn attach_quote_costs(
    event: &mut Value,
    input_sol: f64,
    profit_sol: f64,
    profitable: bool,
    partial: bool,
    costs: &PrimaryRouteCosts,
) {
    let input_lamports = u64::try_from(sol_to_lamports(input_sol).max(0)).unwrap_or(u64::MAX);
    let flash_fee = flash_loan_fee_lamports(input_lamports, costs.flash_loan_fee_bps);
    let total_costs = u128::from(costs.network_fee_lamports)
        + u128::from(costs.priority_fee_lamports)
        + u128::from(costs.jito_tip_lamports)
        + u128::from(flash_fee);
    let net_lamports = sol_to_lamports(profit_sol) - i128::try_from(total_costs).unwrap_or(i128::MAX);
    let worth_it = profitable
        && !partial
        && net_lamports >= i128::from(costs.min_profit_lamports);
    event["feeSol"] = json!(lamports_to_sol(i128::from(costs.network_fee_lamports)));
    event["tipSol"] = json!(lamports_to_sol(i128::from(costs.jito_tip_lamports)));
    event["prioritySol"] = json!(lamports_to_sol(i128::from(costs.priority_fee_lamports)));
    event["flashSol"] = json!(lamports_to_sol(i128::from(flash_fee)));
    event["minProfitSol"] = json!(lamports_to_sol(i128::from(costs.min_profit_lamports)));
    event["netSol"] = json!(lamports_to_sol(net_lamports));
    event["worthIt"] = json!(worth_it);
}

fn sol_to_lamports(sol: f64) -> i128 {
    if !sol.is_finite() {
        return 0;
    }
    (sol * 1_000_000_000.0).round() as i128
}

fn lamports_to_sol(lamports: i128) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

fn flash_loan_fee_lamports(input_lamports: u64, fee_bps: u64) -> u64 {
    let scaled = u128::from(input_lamports).saturating_mul(u128::from(fee_bps));
    u64::try_from(scaled.div_ceil(10_000)).unwrap_or(u64::MAX)
}

pub fn profit_curve(points: &[(f64, f64)], sell_pool: &str, buy_pool: &str) {
    publish(json!({
        "type": "curve",
        "sellPool": sell_pool,
        "buyPool": buy_pool,
        "points": points
            .iter()
            .map(|(input_sol, profit_sol)| json!({"inputSol": input_sol, "profitSol": profit_sol}))
            .collect::<Vec<_>>(),
    }));
}

pub fn skip(direction: &str, reason: &str) {
    publish(json!({
        "type": "skip",
        "direction": direction,
        "reason": reason,
    }));
}

pub fn approved(
    direction: &str,
    sell_dex: &str,
    buy_dex: &str,
    input_sol: f64,
    expected_out_sol: f64,
    profit_sol: f64,
    network_fee: u64,
    priority_fee: u64,
    jito_tip: u64,
    flash_loan_fee: u64,
    capped: bool,
    instructions: &[String],
) {
    publish(json!({
        "type": "approved",
        "direction": direction,
        "sellDex": sell_dex,
        "buyDex": buy_dex,
        "inputSol": input_sol,
        "expectedOutSol": expected_out_sol,
        "profitSol": profit_sol,
        "costs": {
            "network": network_fee,
            "priority": priority_fee,
            "tip": jito_tip,
            "flash": flash_loan_fee,
        },
        "flashLoan": flash_loan_fee > 0,
        "capped": capped,
        "instructions": instructions,
    }));
}

pub fn simulation(
    ok: bool,
    compute_units: Option<u64>,
    wallet_change_sol: Option<f64>,
    error: Option<&str>,
) {
    publish(json!({
        "type": "simulation",
        "ok": ok,
        "computeUnits": compute_units,
        "walletChangeSol": wallet_change_sol,
        "error": error,
    }));
}

pub fn sent(route: &str, signature: &str) {
    publish(json!({
        "type": "sent",
        "route": route,
        "signature": signature,
    }));
}

pub fn send_skipped() {
    publish(json!({"type": "sendSkipped"}));
}

pub fn landed(ok: bool, signature: &str, wallet_change_sol: Option<f64>, error: Option<&str>) {
    publish(json!({
        "type": "landed",
        "ok": ok,
        "signature": signature,
        "walletChangeSol": wallet_change_sol,
        "error": error,
    }));
}

pub fn status(
    send_transactions: bool,
    simulate: bool,
    funding: &str,
    wallet: Option<&str>,
    wallet_sol: Option<f64>,
) {
    publish(json!({
        "type": "status",
        "demoBps": demo_price_shift_bps(),
        "sendTransactions": send_transactions,
        "simulate": simulate,
        "funding": funding,
        "wallet": wallet,
        "walletSol": wallet_sol,
    }));
}

pub fn wallet_sol(sol: f64) {
    publish(json!({"type": "wallet", "walletSol": sol}));
}

pub fn log_line(kind: &str, line: &str) {
    publish(json!({
        "type": "log",
        "kind": kind,
        "line": line,
    }));
}

/// `dex:first-4-of-address`, same shape as the log label, so two Orca fee tiers stay distinct.
pub fn pool_id(pool: &ConcentratedLiquidityPoolState) -> String {
    let address = encode_public_key_as_base58(&pool.pool_address);
    format!("{}:{}", pool.dex.name(), &address[..4.min(address.len())])
}

fn pool_json(pool: &ConcentratedLiquidityPoolState) -> Value {
    json!({
        "id": pool_id(pool),
        "dex": pool.dex.name(),
        "price": pool.human_readable_price_token_b_per_token_a(),
        "tick": pool.current_tick_index,
        "liquidity": pool.active_liquidity_at_current_price.to_string(),
        "feeBps": f64::from(pool.fee_rate_in_millionths) / 100.0,
        "slot": pool.slot,
        "sqrtPrice": pool.sqrt_price_q64_64.to_string(),
    })
}

fn port() -> u16 {
    std::env::var("DASHBOARD_PORT")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(8787)
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref().map(str::trim),
        Some("true" | "1" | "yes")
    )
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn open_jsonl() -> Option<File> {
    let path = events_file_path();
    let path = std::path::Path::new(&path);
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        && let Err(error) = create_dir_all(parent)
    {
        eprintln!("[dashboard] could not create {}: {error}", parent.display());
        return None;
    }
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
    {
        Ok(file) => Some(file),
        Err(error) => {
            eprintln!("[dashboard] could not open {}: {error}", path.display());
            None
        }
    }
}
