//! Result types of quoting: one swap, a two-pool round trip, and why a quote can fail.

use crate::step_3_store_latest_pool_state::DexProgram;

/// Small trade size used in unit tests: 0.1 SOL in lamports.
#[cfg(test)]
pub const PROBE_TRADE_INPUT_AMOUNT: u64 = 100_000_000;

/// Which way a swap goes through a pool.
///
/// **On-chain name:** both DEX SDKs call this `a_to_b: bool`.
///
/// Selling token A into the pool makes token A more plentiful there, so the
/// price of A (in B) goes *down*. Buying token A with token B makes the price
/// go *up*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapDirection {
    /// Put token A in, get token B out (e.g. SOL → USDC). Price goes down.
    TokenAToTokenB,
    /// Put token B in, get token A out (e.g. USDC → SOL). Price goes up.
    TokenBToTokenA,
}

impl SwapDirection {
    /// The `a_to_b` boolean the Orca and Raydium libraries expect.
    pub fn is_a_to_b(self) -> bool {
        matches!(self, SwapDirection::TokenAToTokenB)
    }
}

/// Result of quoting one exact-input swap on one pool.
///
/// All amounts are raw integer token units (lamports, micro-USDC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapQuoteForExactInput {
    /// How much input the pool would actually take. Can be less than asked if
    /// the swap runs out of cached liquidity (a "partial fill").
    pub input_amount_used: u64,
    /// How much output you would receive, after fees.
    pub output_amount: u64,
    /// The part of the input kept as the swap fee (goes to liquidity providers).
    #[allow(dead_code)] // shown in future fee breakdowns
    pub fee_amount: u64,
}

/// A simulated two-pool arbitrage round trip.
///
/// Leg 1: swap the start token for the bridge token on the *sell* pool.
/// Leg 2: swap all of that bridge token back into the start token on the *buy* pool.
/// If the two pools disagree on price by more than their fees, you end with
/// more start token than you began with — that difference is the profit.
///
/// Today: start token = SOL (token A), bridge token = USDC (token B).
/// Future: routes through three or more pools would generalize this into a
/// list of legs.
#[derive(Debug, Clone, Copy)]
pub struct TwoPoolArbitrageRoundTrip {
    /// DEX where we sell the start token (the pool that pays more for it).
    pub sell_pool_dex: DexProgram,
    /// DEX where we buy the start token back (the pool that charges less for it).
    pub buy_pool_dex: DexProgram,
    /// Start token sent into leg 1.
    pub start_token_amount_in: u64,
    /// Bridge token received from leg 1 and spent in leg 2.
    pub bridge_token_amount_between_legs: u64,
    /// Start token received back from leg 2.
    pub start_token_amount_out: u64,
    /// `true` when both swaps used their whole input (no partial fill).
    pub both_swaps_fully_filled: bool,
}

/// One attempted sell→buy round trip, including the direction when quoting failed.
///
/// The DEX names are stored separately from [`TwoPoolArbitrageRoundTrip`] so a
/// skipped quote can still be labeled `raydium_clmm→orca_whirlpool`.
#[derive(Debug)]
pub struct DirectedRoundTripQuote {
    pub sell_pool_dex: DexProgram,
    pub buy_pool_dex: DexProgram,
    pub result: Result<TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed>,
}

impl DirectedRoundTripQuote {
    pub fn direction_label(&self) -> String {
        format!("{}→{}", self.sell_pool_dex.name(), self.buy_pool_dex.name())
    }
}

impl TwoPoolArbitrageRoundTrip {
    /// Profit (positive) or loss (negative) in raw start-token units (lamports today).
    ///
    /// Uses `i128` so the subtraction can go negative without overflowing.
    /// Note: this ignores transaction costs (network fee, priority fee, Jito
    /// tip), which a real bot must subtract before deciding to trade.
    pub fn profit_in_start_token(&self) -> i128 {
        i128::from(self.start_token_amount_out) - i128::from(self.start_token_amount_in)
    }
}

/// Why a quote could not be produced from the cached data.
#[derive(Debug)]
pub enum WhySwapQuoteFailed {
    /// We have not received any tick arrays for this pool yet.
    NoTickArraysCachedYet,
    /// We have some tick arrays, but not the one covering the current price,
    /// so the swap math has nowhere to start.
    TickArrayForCurrentPriceNotCachedYet,
    /// The pool reports tick spacing 0, which would make every window zero-wide.
    TickSpacingIsZero,
    /// Orca's quote library accepts at most 6 tick arrays.
    MoreThanSixTickArraysForOrca,
    /// The price gap between the two pools is smaller than their combined fees.
    NoProfitablePriceGapAfterFees,
    /// Orca's official quote library rejected the swap.
    OrcaQuoteLibraryError(&'static str),
    /// Raydium's quote library rejected the swap.
    RaydiumQuoteLibraryError(&'static str),
}

impl std::fmt::Display for WhySwapQuoteFailed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTickArraysCachedYet => write!(formatter, "no tick arrays cached yet"),
            Self::TickArrayForCurrentPriceNotCachedYet => {
                write!(formatter, "tick array for the current price not cached yet")
            }
            Self::TickSpacingIsZero => write!(formatter, "tick_spacing is 0"),
            Self::MoreThanSixTickArraysForOrca => {
                write!(formatter, "more than 6 tick arrays in the window")
            }
            Self::NoProfitablePriceGapAfterFees => {
                write!(formatter, "no price gap left after fees")
            }
            Self::OrcaQuoteLibraryError(reason) => write!(formatter, "orca: {reason}"),
            Self::RaydiumQuoteLibraryError(reason) => write!(formatter, "raydium: {reason}"),
        }
    }
}
