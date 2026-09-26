//! The last check before spending money: is this trade *really* worth it?
//!
//! Step 5 found a round trip that ends with more SOL than it started with.
//! That "gross" profit ignores what it costs to get a transaction on-chain:
//!
//! - **Network (signature) fee:** 5,000 lamports per signature, always paid.
//! - **Priority fee:** `compute_unit_limit × price_per_unit / 1,000,000`
//!   lamports. Validators schedule higher-paying transactions first.
//! - **Jito tip:** a flat payment to the block builder for bundle inclusion.
//! - **Flash-loan fee:** a small percentage of the borrowed amount (flash mode only).
//!
//! A trade is approved only if, after all of that, at least
//! `MIN_PROFIT_LAMPORTS` is left. Before even quoting, we refuse to trade on
//! **stale** data: if the Geyser stream went quiet or a pool's state is many
//! slots behind the newest slot we have seen, the prices in memory may no
//! longer be real.
//!
//! The approved trade also carries two **minimum outputs** (`other_amount_threshold`
//! in the DEX programs). They turn "we hope it is profitable" into "the chain
//! guarantees it is, or the whole transaction reverts".

use crate::bot_settings::BotSettingsFromEnvironment;
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, TickArrayAccountWithInitializedTicks,
};
use crate::step_4_quote_swaps::{
    SwapDirection, WhySwapQuoteFailed, quote_swap_exact_input, quote_two_pool_round_trip,
};
use crate::step_5_find_best_arbitrage_size::quote_most_profitable_two_pool_round_trip;

/// Solana charges this many lamports per transaction signature.
pub const LAMPORTS_PER_SIGNATURE: u64 = 5_000;
/// Our transactions carry exactly one signature: the bot wallet's.
const SIGNATURES_PER_TRANSACTION: u64 = 1;
const MICRO_LAMPORTS_PER_LAMPORT: u128 = 1_000_000;
const BASIS_POINTS_PER_WHOLE: u128 = 10_000;

/// The numbers the decision needs, pulled out of the settings.
#[derive(Debug, Clone, Copy)]
pub struct TradeDecisionRules {
    pub max_trade_input_lamports: u64,
    pub min_profit_after_costs_lamports: u64,
    pub max_pool_state_age_in_slots: u64,
    pub max_milliseconds_since_last_stream_update: u64,
    pub slippage_tolerance_in_basis_points: u64,
    pub compute_unit_limit: u32,
    pub priority_fee_micro_lamports_per_compute_unit: u64,
    pub jito_tip_lamports: u64,
    pub flash_loan_fee_in_basis_points: u64,
}

impl TradeDecisionRules {
    pub fn from_settings(settings: &BotSettingsFromEnvironment) -> Self {
        Self {
            max_trade_input_lamports: settings.max_trade_input_lamports,
            min_profit_after_costs_lamports: settings.min_profit_after_costs_lamports,
            max_pool_state_age_in_slots: settings.max_pool_state_age_in_slots,
            max_milliseconds_since_last_stream_update: settings.max_milliseconds_since_last_stream_update,
            slippage_tolerance_in_basis_points: settings.slippage_tolerance_in_basis_points,
            compute_unit_limit: settings.compute_unit_limit,
            priority_fee_micro_lamports_per_compute_unit: settings.priority_fee_micro_lamports_per_compute_unit,
            jito_tip_lamports: if settings.jito_block_engine_url.is_some() { settings.jito_tip_lamports } else { 0 },
            flash_loan_fee_in_basis_points: settings.flash_loan_fee_in_basis_points(),
        }
    }
}

/// Everything it costs to land one arbitrage transaction, in lamports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstimatedTransactionCosts {
    pub network_signature_fee: u64,
    pub priority_fee: u64,
    pub jito_tip: u64,
    pub flash_loan_fee: u64,
}

impl EstimatedTransactionCosts {
    pub fn total(&self) -> u64 {
        self.network_signature_fee
            .saturating_add(self.priority_fee)
            .saturating_add(self.jito_tip)
            .saturating_add(self.flash_loan_fee)
    }
}

/// Add up the costs of a trade that starts with `start_token_amount_in` lamports.
///
/// Fractions of a lamport are rounded *up*: over-estimating a cost can only
/// make us skip a trade, under-estimating could make us lose money.
pub fn estimate_transaction_costs(rules: &TradeDecisionRules, start_token_amount_in: u64) -> EstimatedTransactionCosts {
    let priority_fee_micro_lamports =
        u128::from(rules.compute_unit_limit) * u128::from(rules.priority_fee_micro_lamports_per_compute_unit);
    let flash_loan_fee_scaled = u128::from(start_token_amount_in) * u128::from(rules.flash_loan_fee_in_basis_points);
    EstimatedTransactionCosts {
        network_signature_fee: LAMPORTS_PER_SIGNATURE * SIGNATURES_PER_TRANSACTION,
        priority_fee: saturate_to_u64(priority_fee_micro_lamports.div_ceil(MICRO_LAMPORTS_PER_LAMPORT)),
        jito_tip: rules.jito_tip_lamports,
        flash_loan_fee: saturate_to_u64(flash_loan_fee_scaled.div_ceil(BASIS_POINTS_PER_WHOLE)),
    }
}

/// How fresh the cache is, measured at decision time.
#[derive(Debug, Clone, Copy)]
pub struct CacheFreshness {
    pub newest_slot_seen_from_stream: u64,
    /// `None` = no Geyser message received yet.
    pub milliseconds_since_last_stream_update: Option<u64>,
}

/// A trade that passed every check, with the exact amounts to put in the instructions.
#[derive(Debug, Clone)]
pub struct ApprovedArbitrageTrade {
    /// Pool where leg 1 sells SOL for USDC.
    pub sell_pool: ConcentratedLiquidityPoolState,
    pub sell_pool_tick_arrays: Vec<TickArrayAccountWithInitializedTicks>,
    /// Pool where leg 2 buys SOL back with USDC.
    pub buy_pool: ConcentratedLiquidityPoolState,
    pub buy_pool_tick_arrays: Vec<TickArrayAccountWithInitializedTicks>,
    /// SOL (lamports) put into leg 1.
    pub start_token_amount_in: u64,
    /// Leg 1 reverts if it would return less USDC than this.
    pub leg_1_minimum_bridge_token_out: u64,
    /// USDC put into leg 2 (equal to leg 1's minimum, so it is always available).
    pub leg_2_bridge_token_amount_in: u64,
    /// Leg 2 reverts if it would return less SOL than this: start + costs + minimum profit.
    pub leg_2_minimum_start_token_out: u64,
    /// SOL the quote expects leg 2 to return.
    pub expected_start_token_out: u64,
    pub costs: EstimatedTransactionCosts,
    /// `expected_start_token_out - start_token_amount_in - costs` (lamports).
    pub expected_profit_after_costs: i128,
    /// `true` when the best size was larger than `MAX_TRADE_INPUT_LAMPORTS` and got capped.
    pub size_was_capped: bool,
}

/// Why the bot decided not to trade this time.
#[derive(Debug)]
pub enum WhyTradeWasSkipped {
    NoStreamUpdateYet,
    StreamSilentTooLong { milliseconds_since_last_update: u64 },
    PoolStateTooOld { dex: DexProgram, age_in_slots: u64 },
    QuoteFailed(WhySwapQuoteFailed),
    /// The swap would run past the tick arrays we have cached, so the quote is not trustworthy.
    PartialFill,
    NotProfitableAfterCosts { profit_before_costs: i128, total_costs: u64 },
}

impl From<WhySwapQuoteFailed> for WhyTradeWasSkipped {
    fn from(reason: WhySwapQuoteFailed) -> Self {
        Self::QuoteFailed(reason)
    }
}

impl std::fmt::Display for WhyTradeWasSkipped {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoStreamUpdateYet => write!(formatter, "no Geyser update received yet"),
            Self::StreamSilentTooLong { milliseconds_since_last_update } => {
                write!(formatter, "stream silent for {milliseconds_since_last_update} ms (stale)")
            }
            Self::PoolStateTooOld { dex, age_in_slots } => {
                write!(formatter, "{} state is {age_in_slots} slots old (stale)", dex.name())
            }
            Self::QuoteFailed(reason) => write!(formatter, "{reason}"),
            Self::PartialFill => write!(formatter, "swap would leave the cached tick arrays (partial fill)"),
            Self::NotProfitableAfterCosts { profit_before_costs, total_costs } => write!(
                formatter,
                "profit {profit_before_costs} lamports does not cover costs {total_costs} + minimum profit"
            ),
        }
    }
}

/// Run every check and, if they all pass, return the exact trade to build.
///
/// Leg 1 sells SOL on `sell_pool`; leg 2 buys SOL back on `buy_pool`.
pub fn decide_if_trade_is_worth_it(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    freshness: CacheFreshness,
    rules: &TradeDecisionRules,
) -> Result<ApprovedArbitrageTrade, WhyTradeWasSkipped> {
    check_cache_is_fresh(&[sell_pool, buy_pool], freshness, rules)?;

    let mut round_trip =
        quote_most_profitable_two_pool_round_trip(sell_pool, sell_pool_tick_arrays, buy_pool, buy_pool_tick_arrays)?;
    let size_was_capped = round_trip.start_token_amount_in > rules.max_trade_input_lamports;
    if size_was_capped {
        // Profit is a hill in trade size (Step 5), so a smaller size is still
        // on the profitable side — just not at the very top.
        round_trip = quote_two_pool_round_trip(
            sell_pool,
            sell_pool_tick_arrays,
            buy_pool,
            buy_pool_tick_arrays,
            rules.max_trade_input_lamports,
        )?;
    }
    if !round_trip.both_swaps_fully_filled {
        return Err(WhyTradeWasSkipped::PartialFill);
    }

    let leg_1_minimum_bridge_token_out =
        amount_minus_basis_points(round_trip.bridge_token_amount_between_legs, rules.slippage_tolerance_in_basis_points);
    // Leg 2 spends exactly what leg 1 is guaranteed to return; with slippage > 0
    // that is a bit less than the quote, so re-quote leg 2 at that amount.
    let expected_start_token_out = if leg_1_minimum_bridge_token_out == round_trip.bridge_token_amount_between_legs {
        round_trip.start_token_amount_out
    } else {
        let leg_2 = quote_swap_exact_input(
            buy_pool,
            buy_pool_tick_arrays,
            leg_1_minimum_bridge_token_out,
            SwapDirection::TokenBToTokenA,
        )?;
        leg_2.output_amount
    };

    let costs = estimate_transaction_costs(rules, round_trip.start_token_amount_in);
    let profit_before_costs = i128::from(expected_start_token_out) - i128::from(round_trip.start_token_amount_in);
    let expected_profit_after_costs = profit_before_costs - i128::from(costs.total());
    if expected_profit_after_costs < i128::from(rules.min_profit_after_costs_lamports) {
        return Err(WhyTradeWasSkipped::NotProfitableAfterCosts { profit_before_costs, total_costs: costs.total() });
    }

    Ok(ApprovedArbitrageTrade {
        sell_pool: sell_pool.clone(),
        sell_pool_tick_arrays: sell_pool_tick_arrays.to_vec(),
        buy_pool: buy_pool.clone(),
        buy_pool_tick_arrays: buy_pool_tick_arrays.to_vec(),
        start_token_amount_in: round_trip.start_token_amount_in,
        leg_1_minimum_bridge_token_out,
        leg_2_bridge_token_amount_in: leg_1_minimum_bridge_token_out,
        leg_2_minimum_start_token_out: round_trip
            .start_token_amount_in
            .saturating_add(costs.total())
            .saturating_add(rules.min_profit_after_costs_lamports),
        expected_start_token_out,
        costs,
        expected_profit_after_costs,
        size_was_capped,
    })
}

/// Refuse to trade on prices that may no longer be true.
///
/// Two signals: the stream as a whole (if Geyser went quiet, *everything* may
/// be stale) and each pool's own age in slots compared with the newest slot
/// the stream has shown. A busy pool like SOL/USDC changes nearly every slot,
/// so a pool that has not changed for many slots is suspicious.
pub fn check_cache_is_fresh(
    pools: &[&ConcentratedLiquidityPoolState],
    freshness: CacheFreshness,
    rules: &TradeDecisionRules,
) -> Result<(), WhyTradeWasSkipped> {
    let Some(milliseconds_since_last_update) = freshness.milliseconds_since_last_stream_update else {
        return Err(WhyTradeWasSkipped::NoStreamUpdateYet);
    };
    if milliseconds_since_last_update > rules.max_milliseconds_since_last_stream_update {
        return Err(WhyTradeWasSkipped::StreamSilentTooLong { milliseconds_since_last_update });
    }
    for pool in pools {
        let age_in_slots = freshness.newest_slot_seen_from_stream.saturating_sub(pool.slot);
        if age_in_slots > rules.max_pool_state_age_in_slots {
            return Err(WhyTradeWasSkipped::PoolStateTooOld { dex: pool.dex, age_in_slots });
        }
    }
    Ok(())
}

/// `amount × (1 − bps / 10,000)`, rounded down.
fn amount_minus_basis_points(amount: u64, basis_points: u64) -> u64 {
    let kept = BASIS_POINTS_PER_WHOLE.saturating_sub(u128::from(basis_points));
    saturate_to_u64(u128::from(amount) * kept / BASIS_POINTS_PER_WHOLE)
}

fn saturate_to_u64(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
