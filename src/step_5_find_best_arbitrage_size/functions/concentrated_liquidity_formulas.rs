//! The math behind the walk, derived from first principles.
//!
//! ## 1. Two reserves from one liquidity number
//!
//! Inside one tick range a pool behaves like `x * y = k`, with `x` = token A
//! (SOL) and `y` = token B (USDC). Define:
//!
//! - liquidity `L = sqrt(x * y)`
//! - square-root price `s = sqrt(price) = sqrt(y / x)`
//!
//! Multiply and divide them:
//!
//! ```text
//! L * s = sqrt(x*y) * sqrt(y/x) = sqrt(y²) = y      →  y = L * s
//! L / s = sqrt(x*y) / sqrt(y/x) = sqrt(x²) = x      →  x = L / s
//! ```
//!
//! Check with a real pool: 100 SOL and 10,000 USDC → `L = 1,000`, `s = 10`.
//! `L * s = 10,000` USDC ✓ and `L / s = 100` SOL ✓.
//!
//! ## 2. How far a trade moves the price
//!
//! With `L` fixed, moving from `s1` to `s2`:
//!
//! ```text
//! Δy (USDC) = L * |s2 − s1|             ← a straight line in s
//! Δx (SOL)  = L * |1/s2 − 1/s1| = L * |s1 − s2| / (s1 * s2)
//! ```
//!
//! Turned around: sending `Δy` USDC through a pool moves `s` by `Δy / L`.
//! Deep pool (big `L`) → small move. That single idea drives everything below.
//!
//! On-chain `s` is stored as `S = s * 2^64` (Q64.64), so the stored-number
//! versions are `Δy = L * |S2 − S1| / 2^64` and
//! `Δx = L * 2^64 * |S1 − S2| / (S1 * S2)`. The absolute value just means
//! "an amount of tokens is never negative"; direction is tracked separately.
//!
//! ## 3. Fees
//!
//! A fee of `f` millionths keeps `k = (1,000,000 − f) / 1,000,000` of the
//! input. Example: fee 400 → `k = 0.9996`.
//!
//! ## Why the Orca helpers are used for Raydium too
//!
//! Orca and Raydium are the same kind of machine (the Uniswap v3 design): same
//! tick definition, same Q64.64 prices, same `L` formulas. So the math helpers
//! from `orca_whirlpools_core` are correct for both. Venue-exact rounding is
//! still checked afterwards, when Step 4 quotes the final size with each DEX's
//! own library.

use orca_whirlpools_core::{
    FEE_RATE_DENOMINATOR, sqrt_price_to_tick_index, try_apply_swap_fee, try_get_amount_delta_a,
    try_get_amount_delta_b, try_get_next_sqrt_price_from_b, try_reverse_apply_swap_fee,
};

use super::pool_price_walker_along_ticks::PoolPriceWalkerAlongTicks;

/// `2^64` as a float, for undoing the Q64.64 scaling. Exact, since it is a power of two.
const TWO_TO_THE_POWER_64: f64 = (1u128 << 64) as f64;

/// Would a tiny round trip make money after both fees?
///
/// Sell 1 unit of SOL on the sell pool at price `P_sell`, keep `k_sell` after
/// its fee; the buy pool keeps `k_buy` of that USDC and sells SOL at `P_buy`:
///
/// ```text
/// SOL back per SOL in = k_sell * k_buy * P_sell / P_buy
/// ```
///
/// Profitable when that is above 1, i.e. `k_sell * k_buy * P_sell > P_buy`.
/// Pools store `s = sqrt(P)`, so take the square root of both sides:
///
/// ```text
/// sqrt(k_sell * k_buy) * s_sell > s_buy
/// ```
///
/// Example: 101 vs 100 USDC, both fees 0.04% → 101 × 0.9996 × 0.9996 = 100.919,
/// which is more than 100, so there is a ~0.92% edge. At 100.05 vs 100 the
/// fees (0.08% total) eat the whole gap and the answer is "no".
pub(crate) fn is_price_gap_bigger_than_both_fees(
    sell_pool: &PoolPriceWalkerAlongTicks,
    buy_pool: &PoolPriceWalkerAlongTicks,
) -> bool {
    if sell_pool.sqrt_price_q64_64 <= buy_pool.sqrt_price_q64_64
        || sell_pool.active_liquidity == 0
        || buy_pool.active_liquidity == 0
    {
        return false;
    }
    let both_fees_kept = fraction_of_input_left_after_fee(sell_pool.fee_rate_in_millionths)
        * fraction_of_input_left_after_fee(buy_pool.fee_rate_in_millionths);
    (sell_pool.sqrt_price_q64_64 as f64) * both_fees_kept.sqrt() > buy_pool.sqrt_price_q64_64 as f64
}

/// How much USDC (bridge token) can flow before the gap after fees closes,
/// assuming neither pool crosses a tick on the way.
///
/// Let `x` be the USDC moved and `q = sqrt(k_sell * k_buy)`. From section 2:
///
/// ```text
/// sell pool pays out x USDC        → s_sell' = s_sell − x / L_sell
/// buy pool receives x, keeps k_buy → s_buy'  = s_buy  + x * k_buy / L_buy
/// ```
///
/// The gap is gone when `q * s_sell' = s_buy'`. Substitute and solve for `x`:
///
/// ```text
/// q*s_sell − q*x/L_sell = s_buy + x*k_buy/L_buy
/// x = (q*s_sell − s_buy) / (q/L_sell + k_buy/L_buy)
/// ```
///
/// In words: **size = price gap ÷ how fast trading closes it.** The top is the
/// gap after fees; the bottom is how much each USDC moves the two prices
/// (bigger for shallow pools). With stored Q64.64 prices the bottom gets an
/// extra `2^64`.
///
/// Example (normalized L = 1,000,000 on both, 101 vs 100, 0.04% fees) → about
/// 22,900 USDC, after which another tiny trade earns exactly zero.
pub(crate) fn bridge_amount_until_price_gap_closes(
    sell_pool: &PoolPriceWalkerAlongTicks,
    buy_pool: &PoolPriceWalkerAlongTicks,
) -> u64 {
    if sell_pool.active_liquidity == 0 || buy_pool.active_liquidity == 0 {
        return 0;
    }
    let buy_pool_fee_kept = fraction_of_input_left_after_fee(buy_pool.fee_rate_in_millionths);
    let square_root_of_both_fees_kept =
        (fraction_of_input_left_after_fee(sell_pool.fee_rate_in_millionths) * buy_pool_fee_kept)
            .sqrt();

    let price_gap_after_fees = square_root_of_both_fees_kept * sell_pool.sqrt_price_q64_64 as f64
        - buy_pool.sqrt_price_q64_64 as f64;
    if price_gap_after_fees <= 0.0 {
        return 0;
    }
    let how_fast_each_usdc_closes_the_gap = TWO_TO_THE_POWER_64
        * (square_root_of_both_fees_kept / sell_pool.active_liquidity as f64
            + buy_pool_fee_kept / buy_pool.active_liquidity as f64);
    if how_fast_each_usdc_closes_the_gap <= 0.0 {
        return 0;
    }
    let usdc_amount = price_gap_after_fees / how_fast_each_usdc_closes_the_gap;
    if !usdc_amount.is_finite() || usdc_amount <= 0.0 {
        return 0;
    }
    usdc_amount.min(u64::MAX as f64) as u64
}

/// Simulate one step: `bridge_amount` USDC leaves the sell pool and enters the
/// buy pool. Moves both walkers' prices and returns how much SOL (gross, fee
/// included) we had to sell to get that USDC. `None` if the math overflows or
/// a price would leave the allowed range.
pub(crate) fn move_both_pool_prices_by_bridge_amount(
    sell_pool: &mut PoolPriceWalkerAlongTicks,
    buy_pool: &mut PoolPriceWalkerAlongTicks,
    bridge_amount: u64,
) -> Option<u64> {
    // Sell pool pays out USDC → its USDC reserve shrinks → price goes down (`false` = subtract).
    let sell_pool_new_sqrt_price = try_get_next_sqrt_price_from_b(
        sell_pool.sqrt_price_q64_64,
        sell_pool.active_liquidity,
        bridge_amount,
        false,
    )
    .ok()?;
    // Buy pool takes its fee first; only the rest reaches the curve and pushes the price up.
    let bridge_amount_after_buy_pool_fee =
        try_apply_swap_fee(bridge_amount, buy_pool.fee_rate_in_millionths).ok()?;
    let buy_pool_new_sqrt_price = try_get_next_sqrt_price_from_b(
        buy_pool.sqrt_price_q64_64,
        buy_pool.active_liquidity,
        bridge_amount_after_buy_pool_fee,
        true,
    )
    .ok()?;

    // SOL the sell pool's curve absorbs for that price move: Δx = L * |S1 − S2| * 2^64 / (S1 * S2).
    // Rounded up, because it is an amount we must pay.
    let start_token_reaching_curve = try_get_amount_delta_a(
        sell_pool.sqrt_price_q64_64,
        sell_pool_new_sqrt_price,
        sell_pool.active_liquidity,
        true,
    )
    .ok()?;
    // The sell pool also takes its fee from our SOL, so we must send slightly more.
    let start_token_including_fee =
        try_reverse_apply_swap_fee(start_token_reaching_curve, sell_pool.fee_rate_in_millionths)
            .ok()?;

    sell_pool.sqrt_price_q64_64 = sell_pool_new_sqrt_price;
    sell_pool.current_tick_index = sqrt_price_to_tick_index(sell_pool_new_sqrt_price);
    buy_pool.sqrt_price_q64_64 = buy_pool_new_sqrt_price;
    buy_pool.current_tick_index = sqrt_price_to_tick_index(buy_pool_new_sqrt_price);
    Some(start_token_including_fee)
}

/// Token B (USDC) needed to move a pool between two square-root prices:
/// `Δy = L * |S1 − S2| / 2^64`. Tells the walker how much USDC is left before
/// a pool reaches its next tick.
///
/// On overflow returns `u64::MAX`, so a broken value can never win the
/// "smallest distance" comparison; the other limits stop the step instead.
pub(crate) fn token_b_amount_between_sqrt_prices(
    sqrt_price_a: u128,
    sqrt_price_b: u128,
    liquidity: u128,
    round_up: bool,
) -> u64 {
    try_get_amount_delta_b(sqrt_price_a, sqrt_price_b, liquidity, round_up).unwrap_or(u64::MAX)
}

/// The amount you must send so that `amount_after_fee` remains after the fee.
///
/// `after = before * k`, so `before = after / k = after * 1,000,000 / (1,000,000 − fee)`.
/// Rounded up: being one unit short would fail on-chain.
pub(crate) fn amount_before_fee_was_taken(
    amount_after_fee: u64,
    fee_rate_in_millionths: u32,
) -> u64 {
    try_reverse_apply_swap_fee(amount_after_fee, fee_rate_in_millionths).unwrap_or(u64::MAX)
}

/// Share of the input left after the fee, e.g. fee 400 millionths (0.04%) → 0.9996.
fn fraction_of_input_left_after_fee(fee_rate_in_millionths: u32) -> f64 {
    let one_million = f64::from(FEE_RATE_DENOMINATOR);
    (one_million - f64::from(fee_rate_in_millionths)) / one_million
}
