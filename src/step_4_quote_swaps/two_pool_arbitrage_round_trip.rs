//! Chain two swap quotes into an arbitrage round trip.
//!
//! The idea in plain words: pool S pays 101 USDC per SOL, pool B sells SOL for
//! 100 USDC. Sell SOL on S, take the USDC to B, buy SOL back — you end with
//! more SOL than you started with, minus two small fees. Real bots do the two
//! legs inside one transaction so either both happen or neither does.
//!
//! Same cycle, other way round: "buy cheap on B, sell dear on S" is the same
//! trade started from USDC instead of SOL. We start from SOL (token A).

use super::quote_swap_exact_input::quote_swap_exact_input;
use super::swap_quote_types::{
    DirectedRoundTripQuote, PROBE_TRADE_INPUT_AMOUNT, SwapDirection, TwoPoolArbitrageRoundTrip,
    WhySwapQuoteFailed,
};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, LatestPoolStateCache,
    TickArrayAccountWithInitializedTicks,
};

/// Quote selling `start_token_amount_in` of token A on `sell_pool`, then
/// swapping all the token B received back into token A on `buy_pool`.
pub fn quote_two_pool_round_trip(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    start_token_amount_in: u64,
) -> Result<TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed> {
    let sell_leg = quote_swap_exact_input(
        sell_pool,
        sell_pool_tick_arrays,
        start_token_amount_in,
        SwapDirection::TokenAToTokenB,
    )?;
    let buy_leg = quote_swap_exact_input(
        buy_pool,
        buy_pool_tick_arrays,
        sell_leg.output_amount,
        SwapDirection::TokenBToTokenA,
    )?;
    Ok(TwoPoolArbitrageRoundTrip {
        sell_pool_dex: sell_pool.dex,
        buy_pool_dex: buy_pool.dex,
        start_token_amount_in,
        bridge_token_amount_between_legs: sell_leg.output_amount,
        start_token_amount_out: buy_leg.output_amount,
        both_swaps_fully_filled: sell_leg.input_amount_used == start_token_amount_in
            && buy_leg.input_amount_used == sell_leg.output_amount,
    })
}

/// The two SOL/USDC pools this bot watches, plus the tick arrays cached for each.
///
/// `None` until both DEXes have appeared in the cache.
pub(crate) struct CachedOrcaAndRaydiumPools {
    pub raydium_pool: ConcentratedLiquidityPoolState,
    pub raydium_tick_arrays: Vec<TickArrayAccountWithInitializedTicks>,
    pub orca_pool: ConcentratedLiquidityPoolState,
    pub orca_tick_arrays: Vec<TickArrayAccountWithInitializedTicks>,
}

impl CachedOrcaAndRaydiumPools {
    pub(crate) fn from_cache(cache: &LatestPoolStateCache) -> Option<Self> {
        let raydium_pool = cache.first_pool_on_dex(DexProgram::RaydiumClmm)?;
        let orca_pool = cache.first_pool_on_dex(DexProgram::OrcaWhirlpool)?;
        Some(Self {
            raydium_tick_arrays: cache.tick_arrays_for_pool(&raydium_pool.pool_address),
            orca_tick_arrays: cache.tick_arrays_for_pool(&orca_pool.pool_address),
            raydium_pool,
            orca_pool,
        })
    }

    /// Quote raydium→orca and orca→raydium. At most one direction can be profitable.
    pub(crate) fn quote_both_directions(
        &self,
        mut quote: impl FnMut(
            &ConcentratedLiquidityPoolState,
            &[TickArrayAccountWithInitializedTicks],
            &ConcentratedLiquidityPoolState,
            &[TickArrayAccountWithInitializedTicks],
        ) -> Result<TwoPoolArbitrageRoundTrip, WhySwapQuoteFailed>,
    ) -> [DirectedRoundTripQuote; 2] {
        [
            DirectedRoundTripQuote {
                sell_pool_dex: self.raydium_pool.dex,
                buy_pool_dex: self.orca_pool.dex,
                result: quote(
                    &self.raydium_pool,
                    &self.raydium_tick_arrays,
                    &self.orca_pool,
                    &self.orca_tick_arrays,
                ),
            },
            DirectedRoundTripQuote {
                sell_pool_dex: self.orca_pool.dex,
                buy_pool_dex: self.raydium_pool.dex,
                result: quote(
                    &self.orca_pool,
                    &self.orca_tick_arrays,
                    &self.raydium_pool,
                    &self.raydium_tick_arrays,
                ),
            },
        ]
    }
}

/// Round trips at the small fixed probe size, both directions.
pub fn quote_probe_round_trips_from_cache(
    cache: &LatestPoolStateCache,
) -> Option<[DirectedRoundTripQuote; 2]> {
    Some(
        CachedOrcaAndRaydiumPools::from_cache(cache)?.quote_both_directions(
            |sell_pool, sell_tick_arrays, buy_pool, buy_tick_arrays| {
                quote_two_pool_round_trip(
                    sell_pool,
                    sell_tick_arrays,
                    buy_pool,
                    buy_tick_arrays,
                    PROBE_TRADE_INPUT_AMOUNT,
                )
            },
        ),
    )
}
