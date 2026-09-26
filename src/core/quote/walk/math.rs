use orca_whirlpools_core::{
    sqrt_price_to_tick_index, try_apply_swap_fee, try_get_amount_delta_a, try_get_amount_delta_b,
    try_get_next_sqrt_price_from_b, try_reverse_apply_swap_fee, FEE_RATE_DENOMINATOR,
};

use super::side::Side;

/// 2^64 as f64. Exact: 2^64 is a power of two, so it fits in the mantissa.
///
/// A CLMM stores `sqrt(price)` as an integer in Q64.64 format:
/// `sqrt_price_x64 = sqrt(price) * 2^64`. Dividing by `Q64` therefore
/// converts the stored value back to the mathematical square-root price.
const Q64: f64 = (1u128 << 64) as f64;

/// Check whether an infinitesimally small round trip is profitable after fees.
///
/// Let `ks` and `kb` be the fractions kept by the sell and buy pools. For a
/// tiny amount of SOL:
///
/// ```text
/// USDC_out ≈ P_sell * ks * SOL_in
/// SOL_back ≈ USDC_out * kb / P_buy
/// ```
///
/// Therefore:
///
/// ```text
/// SOL_back / SOL_in ≈ ks * kb * P_sell / P_buy
/// ```
///
/// We need that ratio to exceed `1`. Since `P = (sqrt_price / Q64)^2`,
/// taking square roots gives:
///
/// ```text
/// sqrt(ks * kb) * sell.sqrt > buy.sqrt
/// ```
///
/// This avoids sizing a trade when there is no fee-adjusted edge. Without it,
/// the bot could submit a transaction that loses money while LPs collect the
/// swap fees.
pub(super) fn has_edge(sell: &Side, buy: &Side) -> bool {
    if sell.sqrt <= buy.sqrt || sell.liq == 0 || buy.liq == 0 {
        return false;
    }
     let k = fee_keep(sell.fee) * fee_keep(buy.fee);
    (sell.sqrt as f64) * k.sqrt() > buy.sqrt as f64
}

/// Find the USDC amount where marginal round-trip profit becomes zero.
///
/// `x` is gross USDC leaving the sell pool and entering the buy pool. Inside
/// one tick range, liquidity is constant, so the CLMM curve gives:
///
/// ```text
/// sell.sqrt_after = sell.sqrt - x * Q64 / sell.liq
/// buy.sqrt_after  = buy.sqrt  + x * kb * Q64 / buy.liq
/// ```
///
/// The sell pool charges `ks` on the SOL input and the buy pool keeps `kb` of
/// the USDC input. The marginal edge is gone when the fee-adjusted prices are
/// equal:
///
/// ```text
/// sqrt(ks * kb) * sell.sqrt_after = buy.sqrt_after
/// ```
///
/// Solving that equation for `x` produces:
///
/// ```text
/// x = (sqrt(ks * kb) * sell.sqrt - buy.sqrt)
///     / (Q64 * (sqrt(ks * kb) / sell.liq + kb / buy.liq))
/// ```
///
/// This is the PnL-maximizing point within the current ranges: before it,
/// another small trade is profitable; after it, price impact costs more than
/// the remaining spread. It prevents the user/company from over-trading while
/// still allowing LPs to earn fees and receive the price rebalancing flow.
pub(super) fn equalizing_usdc(sell: &Side, buy: &Side) -> u64 {
    if sell.liq == 0 || buy.liq == 0 {
        return 0;
    }
    let k = (fee_keep(sell.fee) * fee_keep(buy.fee)).sqrt();
    let numer = k * sell.sqrt as f64 - buy.sqrt as f64;
    if numer <= 0.0 {
        return 0;
    }
    let denom = Q64 * (k / sell.liq as f64 + fee_keep(buy.fee) / buy.liq as f64);
    if denom <= 0.0 {
        return 0;
    }
    let usdc = numer / denom;
    if !usdc.is_finite() || usdc <= 0.0 {
        return 0;
    }
    usdc.min(u64::MAX as f64) as u64
}

/// Move both pool states by the same USDC amount and return gross SOL input.
///
/// The two legs share `usdc`: it is the output of the sell leg and the gross
/// input of the buy leg. For token B, constant liquidity gives:
///
/// ```text
/// delta_sqrt_price = amount_b * Q64 / liquidity
/// ```
///
/// Selling SOL for USDC moves the sell price down, so the helper subtracts
/// that delta. Buying SOL with USDC moves the buy price up, but only
/// `usdc * kb` reaches the curve after the buy fee. The exact dependency
/// functions also apply integer rounding and price-bound checks.
pub(super) fn apply_usdc(sell: &mut Side, buy: &mut Side, usdc: u64) -> Option<u64> {
    let next_sell = try_get_next_sqrt_price_from_b(sell.sqrt, sell.liq, usdc, false).ok()?;
    let buy_net = try_apply_swap_fee(usdc, buy.fee).ok()?;
    let next_buy = try_get_next_sqrt_price_from_b(buy.sqrt, buy.liq, buy_net, true).ok()?;
    // For token A, the amount moved between two prices is:
    //
    // `delta_a = liquidity * Q64 * delta_sqrt / (sqrt_1 * sqrt_2)`.
    //
    // This is the net SOL consumed by the curve, before reversing the sell
    // pool's input fee.
    let sol_net = try_get_amount_delta_a(sell.sqrt, next_sell, sell.liq, true).ok()?;
    let sol_gross = try_reverse_apply_swap_fee(sol_net, sell.fee).ok()?;

    sell.sqrt = next_sell;
    sell.tick = sqrt_price_to_tick_index(next_sell);
    buy.sqrt = next_buy;
    buy.tick = sqrt_price_to_tick_index(next_buy);
    Some(sol_gross)
}

/// Return the token-B amount between two sqrt prices at constant liquidity.
///
/// From the CLMM invariant:
///
/// ```text
/// delta_b = liquidity * |sqrt_1 - sqrt_2| / Q64
/// ```
///
/// This tells the walker how much USDC remains before a side reaches its next
/// tick. The dependency performs the 256-bit arithmetic and the requested
/// integer rounding so a boundary is not calculated with floating-point error.
pub(super) fn amount_b(a: u128, b: u128, liq: u128, round_up: bool) -> u64 {
    try_get_amount_delta_b(a, b, liq, round_up).unwrap_or(u64::MAX)
}

/// Convert a net amount back to the gross amount required before the fee.
///
/// If `k = (D - fee) / D`, applying a fee is:
///
/// ```text
/// net = gross * k
/// ```
///
/// Solving for the required gross amount gives:
///
/// ```text
/// gross = net / k = net * D / (D - fee)
/// ```
///
/// The dependency rounds up because being one unit short would fail to deliver
/// the requested net amount on-chain.
pub(super) fn reverse_fee(amount: u64, fee: u32) -> u64 {
    try_reverse_apply_swap_fee(amount, fee).unwrap_or(u64::MAX)
}

/// Return the fraction of an input that remains after a swap fee.
///
/// The protocol represents fees as millionths, so `fee` means `fee / D` of
/// the input is removed. Starting from:
///
/// ```text
/// net = gross - gross * fee / D
/// ```
///
/// factoring out `gross` gives:
///
/// ```text
/// net = gross * (D - fee) / D
/// ```
///
/// `fee_keep` returns that final multiplier. For example, fee `400` means
/// `400 / 1_000_000 = 0.0004 = 0.04%`, so the pool keeps `0.9996` of the input.
fn fee_keep(fee: u32) -> f64 {
    let den = f64::from(FEE_RATE_DENOMINATOR);
    (den - f64::from(fee)) / den
}