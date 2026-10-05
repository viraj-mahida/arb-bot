//! Lines printed by Step 6: setup, decision, simulation, sending.

use crate::bot_settings::{BotSettingsFromEnvironment, FundingMode};
use crate::solana_connections::SolanaRpcClient;
use crate::step_3_store_latest_pool_state::{PublicKeyBytes, encode_public_key_as_base58};
use crate::step_4_quote_swaps::pool_label;
use crate::step_6_build_and_send_transactions::{ApprovedArbitrageTrade, WhyTradeWasSkipped};

const LAMPORTS_PER_SOL: f64 = 1e9;
/// Program logs printed after a failed simulation (the last lines hold the error).
const SIMULATION_LOG_LINES_TO_SHOW: usize = 8;

fn sol(lamports: impl Into<f64>) -> f64 {
    lamports.into() / LAMPORTS_PER_SOL
}

/// Startup trading lines, including the wallet balance read that exists only to print.
pub async fn ignr_trading_ready(
    settings: &BotSettingsFromEnvironment,
    wallet: &PublicKeyBytes,
    lookup_table_count: usize,
    rpc: &SolanaRpcClient,
) {
    trading_ready(settings, wallet, lookup_table_count);
    if let Ok(lamports) = rpc.get_balance(wallet).await {
        wallet_balance(lamports);
    } else {
        wallet_balance_unreadable();
    }
}

pub fn trading_disabled(reason: &str) {
    log_line!("[trade] disabled: {reason}  (watching and quoting only)");
}

pub fn trading_ready(
    settings: &BotSettingsFromEnvironment,
    wallet: &PublicKeyBytes,
    lookup_table_count: usize,
) {
    let funding = match &settings.funding_mode {
        FundingMode::OwnWallet => "own wallet SOL",
        FundingMode::FlashLoan(flash) => flash.lender_name(),
    };
    let mode = if settings.send_real_transactions {
        "LIVE — real transactions"
    } else {
        "simulate only"
    };
    log_line!(
        "[trade] wallet {}  funding={funding}  mode={mode}",
        encode_public_key_as_base58(wallet)
    );
    log_line!(
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
    let funding = match settings.funding_mode {
        FundingMode::OwnWallet => "wallet",
        FundingMode::FlashLoan(_) => "flash_loan",
    };
    crate::dashboard_events::status(
        settings.send_real_transactions,
        settings.simulate_on_rpc_before_sending,
        funding,
        Some(&encode_public_key_as_base58(wallet)),
        None,
    );
}

pub fn wallet_balance(lamports: u64) {
    log_line!(
        "[trade] wallet balance {:.4} SOL",
        lamports as f64 / LAMPORTS_PER_SOL
    );
    crate::dashboard_events::wallet_sol(lamports as f64 / LAMPORTS_PER_SOL);
}

pub fn wallet_balance_unreadable() {
    log_line!("[trade] could not read the wallet balance");
}

/// Only reasons worth attention are printed; "not profitable" is the normal
/// case and already visible in the `[best size]` lines.
pub fn trade_skipped(direction_label: &str, reason: &WhyTradeWasSkipped) {
    crate::dashboard_events::skip(direction_label, &reason.to_string());
    if matches!(
        reason,
        WhyTradeWasSkipped::NotProfitableAfterCosts { .. } | WhyTradeWasSkipped::QuoteFailed(_)
    ) {
        return;
    }
    log_line!("[decide]  {direction_label}  skip: {reason}");
}

pub fn trade_approved(trade: &ApprovedArbitrageTrade) {
    let capped_note = if trade.size_was_capped {
        "  (capped by MAX_TRADE_INPUT_LAMPORTS)"
    } else {
        ""
    };
    log_line!(
        "[decide]  {}→{}  TRADE  in {:.4} SOL  expected out {:.6} SOL  costs {:.6} SOL (fee {} + priority {} + tip {} + flash {} lamports)  net {:+.6} SOL{capped_note}",
        pool_label(&trade.sell_pool),
        pool_label(&trade.buy_pool),
        sol(trade.start_token_amount_in as f64),
        sol(trade.expected_start_token_out as f64),
        sol(trade.costs.total() as f64),
        trade.costs.network_signature_fee,
        trade.costs.priority_fee,
        trade.costs.jito_tip,
        trade.costs.flash_loan_fee,
        trade.expected_profit_after_costs as f64 / LAMPORTS_PER_SOL,
    );
    let flash_loan = trade.costs.flash_loan_fee > 0;
    let sell = pool_label(&trade.sell_pool);
    let buy = pool_label(&trade.buy_pool);
    let instructions = [
        "watch the boards".to_string(),
        "read the sell pool".to_string(),
        "read the buy pool".to_string(),
        if flash_loan {
            "check the flash bank".to_string()
        } else {
            "check the wallet can cover it".to_string()
        },
        "run the local quote".to_string(),
        "send the quote to Jito".to_string(),
    ];
    crate::dashboard_events::approved(
        &format!("{sell}→{buy}"),
        &sell,
        &buy,
        sol(trade.start_token_amount_in as f64),
        sol(trade.expected_start_token_out as f64),
        trade.expected_profit_after_costs as f64 / LAMPORTS_PER_SOL,
        trade.costs.network_signature_fee,
        trade.costs.priority_fee,
        trade.costs.jito_tip,
        trade.costs.flash_loan_fee,
        trade.size_was_capped,
        &instructions,
    );
}

pub fn trade_already_in_flight() {
    log_line!("[decide]  skip: previous trade still being submitted");
}

pub fn trade_on_cooldown() {
    log_line!("[decide]  skip: last bundle was submitted less than a second ago");
}

pub fn trade_build_failed(error: &str) {
    log_line!("[simulate] could not build transaction: {error}");
}

pub fn simulation_succeeded(compute_units: Option<u64>, wallet_lamports_change: Option<i128>) {
    let units = compute_units.map_or("?".to_string(), |units| units.to_string());
    let change = wallet_lamports_change.map_or("?".to_string(), |change| {
        format!("{:+.6} SOL", change as f64 / LAMPORTS_PER_SOL)
    });
    log_line!(
        "[simulate] ok  compute units {units}  wallet SOL change {change} (network fee may be excluded)"
    );
    crate::dashboard_events::simulation(
        true,
        compute_units,
        wallet_lamports_change.map(|change| change as f64 / LAMPORTS_PER_SOL),
        None,
    );
}

pub fn simulation_failed(error: &str, logs: &[String]) {
    log_line!("[simulate] FAILED: {error}");
    crate::dashboard_events::simulation(false, None, None, Some(error));
    for line in logs.iter().rev().take(SIMULATION_LOG_LINES_TO_SHOW).rev() {
        log_line!("[simulate]   {line}");
    }
}

pub fn simulation_request_failed(error: &str) {
    log_line!("[simulate] request failed: {error}");
    crate::dashboard_events::simulation(false, None, None, Some(error));
}

pub fn send_skipped_simulate_only(trade: &ApprovedArbitrageTrade, route: &str) {
    send_block(
        "skipped: SEND_TRANSACTIONS=false (dry-run mode)",
        trade,
        &[],
    );
    crate::dashboard_events::send_skipped(route);
}

pub fn sent(route: &str, signature: &str, trade: &ApprovedArbitrageTrade) {
    send_block(
        &format!("submitted via {route}"),
        trade,
        &[format!("signature {signature}")],
    );
    crate::dashboard_events::sent(route, signature);
}

pub fn send_failed(
    route: &str,
    error: &str,
    trade: &ApprovedArbitrageTrade,
    signature: Option<&str>,
) {
    let mut details = vec![format!("error: {error}")];
    if let Some(signature) = signature {
        details.push(format!("signature {signature}"));
    }
    send_block(&format!("{route} failed"), trade, &details);
}

pub fn transaction_landed(
    signature: &str,
    wallet_lamports_change: Option<i128>,
    trade: &ApprovedArbitrageTrade,
) {
    let change = wallet_lamports_change.map_or("?".to_string(), |change| {
        format!("{:+.6} SOL", change as f64 / LAMPORTS_PER_SOL)
    });
    send_block(
        "CONFIRMED",
        trade,
        &[
            format!("signature {signature}"),
            format!("wallet SOL change {change}"),
        ],
    );
    crate::dashboard_events::landed(
        true,
        signature,
        wallet_lamports_change.map(|change| change as f64 / LAMPORTS_PER_SOL),
        None,
    );
}

pub fn transaction_failed_on_chain(signature: &str, error: &str, trade: &ApprovedArbitrageTrade) {
    send_block(
        "landed but FAILED",
        trade,
        &[format!("error: {error}"), format!("signature {signature}")],
    );
    crate::dashboard_events::landed(false, signature, None, Some(error));
}

pub fn transaction_not_landed(signature: &str, trade: &ApprovedArbitrageTrade) {
    send_block(
        "not landed (dropped or expired)",
        trade,
        &[format!("signature {signature}")],
    );
    crate::dashboard_events::landed(false, signature, None, Some("not landed"));
}

/// Blank lines and `>>> SEND <<<` bracket every send outcome so a search finds the whole trade.
fn send_block(headline: &str, trade: &ApprovedArbitrageTrade, details: &[String]) {
    let capped_note = if trade.size_was_capped {
        "  (capped by MAX_TRADE_INPUT_LAMPORTS)"
    } else {
        ""
    };
    log_line!();
    log_line!(">>> SEND <<<");
    log_line!("[send] {headline}");
    log_line!(
        "  {}→{}",
        pool_label(&trade.sell_pool),
        pool_label(&trade.buy_pool),
    );
    log_line!(
        "  best size {:.4} SOL  pool profit {:+.6} SOL",
        sol(trade.best_size_lamports as f64),
        trade.best_size_pool_profit_lamports as f64 / LAMPORTS_PER_SOL,
    );
    log_line!(
        "  in {:.4} SOL  expected out {:.6} SOL",
        sol(trade.start_token_amount_in as f64),
        sol(trade.expected_start_token_out as f64),
    );
    log_line!(
        "  costs {:.6} SOL (fee {} + priority {} + tip {} + flash {} lamports)",
        sol(trade.costs.total() as f64),
        trade.costs.network_signature_fee,
        trade.costs.priority_fee,
        trade.costs.jito_tip,
        trade.costs.flash_loan_fee,
    );
    log_line!(
        "  net {:+.6} SOL{capped_note}",
        trade.expected_profit_after_costs as f64 / LAMPORTS_PER_SOL,
    );
    for detail in details {
        log_line!("  {detail}");
    }
    log_line!(">>> SEND <<<");
    log_line!();
}
