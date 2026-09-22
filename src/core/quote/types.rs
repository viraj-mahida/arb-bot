use super::super::types::Venue;

/// Probe trade size: 0.1 SOL (100_000_000 lamports).
///
/// Kept small so the swap stays inside the cached tick-array window on these
/// SOL/USDC books without crossing into arrays we haven't loaded yet.
pub const PROBE_SOL_LAMPORTS: u64 = 100_000_000;

/// Result of a single-pool, exact-in swap quote.
///
/// All amounts are in the token's **raw on-chain units** (lamports for SOL,
/// micro-USDC for USDC) — integers with no decimal point, matching how Solana
/// programs store balances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quote {
    /// Tokens sent into the pool (may be less than requested if the walk runs out of liquidity).
    pub amount_in: u64,
    /// Tokens received from the pool after fees.
    pub amount_out: u64,
    /// Portion of `amount_in` taken as the pool's trading fee.
    pub fee: u64,
}

/// Simulated arbitrage loop: sell SOL on one venue, buy SOL back on another.
///
/// If the two pools disagree on price, `sol_out` can exceed `sol_in` (profit).
/// In practice, fees and spread usually make round-trips lose money unless there
/// is a real mispricing between venues.
#[derive(Debug, Clone, Copy)]
pub struct RoundTrip {
    pub sell: Venue,
    pub buy: Venue,
    /// SOL sold on the first leg.
    pub sol_in: u64,
    /// USDC received from selling SOL (the bridge token between venues).
    pub usdc_mid: u64,
    /// SOL bought back on the second leg.
    pub sol_out: u64,
    /// `true` when both swaps consumed their full input (no partial fill).
    pub fully_filled: bool,
}

impl RoundTrip {
    /// Net P&L in lamports. Positive = profit, negative = loss.
    ///
    /// Uses `i128` so subtracting a large `sol_in` from a smaller `sol_out`
    /// cannot overflow (unlike `u64`, which would wrap on underflow).
    pub fn pnl_lamports(&self) -> i128 {
        i128::from(self.sol_out) - i128::from(self.sol_in)
    }
}

/// Why a quote could not be produced from the cached data.
#[derive(Debug)]
pub enum QuoteError {
    NoTickArrays,
    MissingCurrentArray,
    BadTickSpacing,
    TooManyArrays,
    NoEdge,
    Orca(&'static str),
    Raydium(&'static str),
}

impl std::fmt::Display for QuoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuoteError::NoTickArrays => write!(f, "no tick arrays cached"),
            QuoteError::MissingCurrentArray => write!(f, "current tick array not cached yet"),
            QuoteError::BadTickSpacing => write!(f, "tick_spacing is 0"),
            QuoteError::TooManyArrays => write!(f, "more than 6 tick arrays in the window"),
            QuoteError::NoEdge => write!(f, "no residual edge after fees"),
            QuoteError::Orca(e) => write!(f, "orca: {e}"),
            QuoteError::Raydium(e) => write!(f, "raydium: {e}"),
        }
    }
}
