use super::super::types::{ClmmPoolState, TickArraySnapshot};
use super::types::{Quote, QuoteError};

use solana_clmm_raydium::{InitializedTick as RaydiumTick, SwapPool, compute_swap_full};

/// Raydium CLMM quote via `solana_clmm_raydium`.
///
/// Raydium's API wants a flat, sorted list of initialized ticks (not per-array
/// facades). `liquidity_net` is how much active liquidity changes when price
/// crosses that tick — the swap walk steps through these as price moves.
pub(crate) fn quote_raydium(
    pool: &ClmmPoolState,
    arrays: &[TickArraySnapshot],
    amount_in: u64,
    a_to_b: bool,
) -> Result<Quote, QuoteError> {
    // Flatten all cached arrays into one list, then sort for the swap walk.
    let mut ticks: Vec<RaydiumTick> = arrays
        .iter()
        .flat_map(|array| {
            array.ticks.iter().map(|tick| RaydiumTick {
                tick: tick.tick,
                liquidity_net: tick.liquidity_net,
            })
        })
        .collect();
    ticks.sort_by_key(|tick| tick.tick);
    // Overlapping cache windows may contain the same tick twice; keep one.
    ticks.dedup_by_key(|tick| tick.tick);

    let swap_pool = SwapPool {
        sqrt_price_x64: pool.sqrt_price_x64,
        liquidity: pool.liquidity,
        tick_current: pool.tick,
        tick_spacing: pool.tick_spacing,
        fee_rate_pips: u32::from(pool.fee_rate),
    };
    // Last two args: exact-in (`true`) and swap direction (`a_to_b`).
    let result = compute_swap_full(&swap_pool, &ticks, amount_in, 0, true, a_to_b)
        .map_err(|e| QuoteError::Raydium(e.reason()))?;
    Ok(Quote {
        amount_in: result.amount_in,
        amount_out: result.amount_out,
        fee: result.fee_amount,
    })
}
