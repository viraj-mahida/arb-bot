//! Tick-book walk math (not numbered pipeline sub-steps).

mod concentrated_liquidity_formulas;
mod pool_price_walker_along_ticks;

pub(crate) use concentrated_liquidity_formulas::{
    amount_before_fee_was_taken, bridge_amount_until_price_gap_closes,
    is_price_gap_bigger_than_both_fees, move_both_pool_prices_by_bridge_amount,
    token_b_amount_between_sqrt_prices,
};
pub(crate) use pool_price_walker_along_ticks::PoolPriceWalkerAlongTicks;
