//! Lines printed by Step 6: setup, decision, simulation, sending.

use crate::bot_settings::{BotSettingsFromEnvironment, FundingMode};
use crate::step_3_store_latest_pool_state::{PublicKeyBytes, encode_public_key_as_base58};
use crate::step_6_build_and_send_transactions::{ApprovedArbitrageTrade, WhyTradeWasSkipped};

const LAMPORTS_PER_SOL: f64 = 1e9;
/// Program logs printed after a failed simulation (the last lines hold the error).
const SIMULATION_LOG_LINES_TO_SHOW: usize = 8;

fn sol(lamports: impl Into<f64>) -> f64 {
    lamports.into() / LAMPORTS_PER_SOL
}

pub fn trading_disabled(reason: &str) {
    println!("[trade] disabled: {reason}  (watching and quoting only)");
}

pub fn trading_ready(
    settings: &BotSettingsFromEnvironment,
    wallet: &PublicKeyBytes,
    lookup_table_count: usize,
) {
    let funding = match settings.funding_mode {
        FundingMode::OwnWallet => "own wallet SOL",
        FundingMode::FlashLoan(_) => "Kamino flash loan",
    };
    let mode = if settings.send_real_transactions {
        "LIVE — real transactions"
    } else {
        "simulate only"
    };
    println!(
        "[trade] wallet {}  funding={funding}  mode={mode}",
        encode_public_key_as_base58(wallet)
    );
    println!(
        "[trade] max input {:.4} SOL  min profit {:.6} SOL  priority {} µlamports/CU × {} CU  tip {:.6} SOL  via {}  lookup tables {lookup_table_count}",
        sol(settings.max_trade_input_lamports as f64),
        sol(settings.min_profit_after_costs_lamports as f64),
        settings.priority_fee_micro_lamports_per_compute_unit,
        settings.compute_unit_limit,
        sol(settings.jito_tip_lamports as f64),
        settings
            .jito_block_engine_url
            .as_deref()
            .unwrap_or("RPC (no Jito)"),
    );
}

/// Only reasons worth attention are printed; "not profitable" is the normal
/// case and already visible in the `[best size]` lines.
pub fn trade_skipped(direction_label: &str, reason: &WhyTradeWasSkipped) {
    if matches!(
        reason,
        WhyTradeWasSkipped::NotProfitableAfterCosts { .. } | WhyTradeWasSkipped::QuoteFailed(_)
    ) {
        return;
    }
    println!("[decide]  {direction_label}  skip: {reason}");
}

pub fn trade_approved(trade: &ApprovedArbitrageTrade) {
    let capped_note = if trade.size_was_capped {
        "  (capped by MAX_TRADE_INPUT_LAMPORTS)"
    } else {
        ""
    };
    println!(
        "[decide]  {}→{}  TRADE  in {:.4} SOL  expected out {:.6} SOL  costs {:.6} SOL (fee {} + priority {} + tip {} + flash {} lamports)  net {:+.6} SOL{capped_note}",
        trade.sell_pool.dex.name(),
        trade.buy_pool.dex.name(),
        sol(trade.start_token_amount_in as f64),
        sol(trade.expected_start_token_out as f64),
        sol(trade.costs.total() as f64),
        trade.costs.network_signature_fee,
        trade.costs.priority_fee,
        trade.costs.jito_tip,
        trade.costs.flash_loan_fee,
        trade.expected_profit_after_costs as f64 / LAMPORTS_PER_SOL,
    );
}

pub fn trade_already_in_flight() {
    println!("[decide]  skip: previous trade still in flight");
}

pub fn trade_build_failed(error: &str) {
    println!("[simulate] could not build transaction: {error}");
}

pub fn simulation_succeeded(compute_units: Option<u64>, wallet_lamports_change: Option<i128>) {
    let units = compute_units.map_or("?".to_string(), |units| units.to_string());
    let change = wallet_lamports_change.map_or("?".to_string(), |change| {
        format!("{:+.6} SOL", change as f64 / LAMPORTS_PER_SOL)
    });
    println!(
        "[simulate] ok  compute units {units}  wallet SOL change {change} (network fee may be excluded)"
    );
}

pub fn simulation_failed(error: &str, logs: &[String]) {
    println!("[simulate] FAILED: {error}");
    for line in logs.iter().rev().take(SIMULATION_LOG_LINES_TO_SHOW).rev() {
        println!("[simulate]   {line}");
    }
}

pub fn simulation_request_failed(error: &str) {
    println!("[simulate] request failed: {error}");
}

pub fn send_skipped_simulate_only() {
    println!("[send] skipped: SEND_TRANSACTIONS=false (simulate-only mode)");
}

pub fn sent(route: &str, signature: &str) {
    println!("[send] submitted via {route}  signature {signature}");
}

pub fn send_failed(route: &str, error: &str) {
    println!("[send] {route} failed: {error}");
}

pub fn transaction_landed(signature: &str, wallet_lamports_change: Option<i128>) {
    let change = wallet_lamports_change.map_or("?".to_string(), |change| {
        format!("{:+.6} SOL", change as f64 / LAMPORTS_PER_SOL)
    });
    println!("[send] CONFIRMED {signature}  wallet SOL change {change}");
}

pub fn transaction_failed_on_chain(signature: &str, error: &str) {
    println!("[send] landed but FAILED {signature}: {error}");
}

pub fn transaction_not_landed(signature: &str) {
    println!("[send] not landed (dropped or expired) {signature}");
}
