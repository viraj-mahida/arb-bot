use orca_whirlpools_core::{
    sqrt_price_to_tick_index, try_apply_swap_fee, try_get_amount_delta_a, try_get_amount_delta_b,
    try_get_next_sqrt_price_from_b, try_reverse_apply_swap_fee, FEE_RATE_DENOMINATOR,
};

use super::side::Side;

/// 2^64 as f64. Exact: 2^64 is a power of two, so it fits in the mantissa.
const Q64: f64 = (1u128 << 64) as f64;

/// Marginal edge after both pool fees: `(1-fs)(1-fb) * P_sell^2 > P_buy^2`.
pub(super) fn has_edge(sell: &Side, buy: &Side) -> bool {
    if sell.sqrt <= buy.sqrt || sell.liq == 0 || buy.liq == 0 {
        return false;
    }
    let k = fee_keep(sell.fee) * fee_keep(buy.fee);
    (sell.sqrt as f64) * k.sqrt() > buy.sqrt as f64
}

/// USDC that equalizes post-fee sqrt prices inside the current constant-L range.
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

/// Move both sides by `usdc` (sell output = buy input). Returns gross SOL in.
pub(super) fn apply_usdc(sell: &mut Side, buy: &mut Side, usdc: u64) -> Option<u64> {
    let next_sell = try_get_next_sqrt_price_from_b(sell.sqrt, sell.liq, usdc, false).ok()?;
    let buy_net = try_apply_swap_fee(usdc, buy.fee).ok()?;
    let next_buy = try_get_next_sqrt_price_from_b(buy.sqrt, buy.liq, buy_net, true).ok()?;
    let sol_net = try_get_amount_delta_a(sell.sqrt, next_sell, sell.liq, true).ok()?;
    let sol_gross = try_reverse_apply_swap_fee(sol_net, sell.fee).ok()?;

    sell.sqrt = next_sell;
    sell.tick = sqrt_price_to_tick_index(next_sell);
    buy.sqrt = next_buy;
    buy.tick = sqrt_price_to_tick_index(next_buy);
    Some(sol_gross)
}

pub(super) fn amount_b(a: u128, b: u128, liq: u128, round_up: bool) -> u64 {
    try_get_amount_delta_b(a, b, liq, round_up).unwrap_or(u64::MAX)
}

pub(super) fn reverse_fee(amount: u64, fee: u32) -> u64 {
    try_reverse_apply_swap_fee(amount, fee).unwrap_or(u64::MAX)
}

fn fee_keep(fee: u32) -> f64 {
    let den = f64::from(FEE_RATE_DENOMINATOR);
    (den - f64::from(fee)) / den
}
