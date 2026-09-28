//! Walk both pools' tick books to find the start-token input that maximizes profit.
//!
//! **Start here:** [`main_find_input_amount_that_maximizes_profit`].
//!
//! Used by [`super::most_profitable_round_trip`] (sub-step 5.1) to pick the size
//! that is then quoted with Step 4's exact DEX math.

use orca_whirlpools_core::tick_index_to_sqrt_price;

use super::functions::{
    PoolPriceWalkerAlongTicks, amount_before_fee_was_taken, bridge_amount_until_price_gap_closes,
    is_price_gap_bigger_than_both_fees, move_both_pool_prices_by_bridge_amount,
    token_b_amount_between_sqrt_prices,
};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks,
};
use crate::step_4_quote_swaps::{SwapDirection, WhySwapQuoteFailed};

/// Safety cap on walk steps, so malformed tick data can never loop forever.
/// Each step crosses at least one tick, and we only cache ~5 tick arrays, so
/// real walks finish far below this.
const MAXIMUM_WALK_STEPS: u32 = 256;

/// Which of the three step-ending events happened on this step.
///
/// More than one can be true at once: e.g. the gap may close exactly at the
/// sell pool's next tick.
struct WhatStopsThisWalkStep {
    price_gap_after_fees_is_gone: bool,
    sell_pool_reached_next_tick: bool,
    buy_pool_reached_next_tick: bool,
}

/// Walk both pools together and return the start-token input (lamports) that
/// maximizes the round trip's profit. Returns 0 when there is no profitable gap.
///
/// **Start here** for the size walk. This is a size estimate from the tick books;
/// the exact amounts that would go into a transaction come from quoting this
/// size with Step 4.
pub fn main_find_input_amount_that_maximizes_profit(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[TickArrayAccountWithInitializedTicks],
) -> Result<u64, WhySwapQuoteFailed> {
    let mut sell_pool = PoolPriceWalkerAlongTicks::new(sell_pool, sell_pool_tick_arrays)?;
    let mut buy_pool = PoolPriceWalkerAlongTicks::new(buy_pool, buy_pool_tick_arrays)?;
    if !is_price_gap_bigger_than_both_fees(&sell_pool, &buy_pool) {
        return Ok(0);
    }

    let mut total_start_token_input: u64 = 0;
    for _ in 0..MAXIMUM_WALK_STEPS {
        if sell_pool.active_liquidity == 0
            || buy_pool.active_liquidity == 0
            || !is_price_gap_bigger_than_both_fees(&sell_pool, &buy_pool)
        {
            break;
        }

        // Selling token A pushes the sell pool's price DOWN toward its next tick below.
        // Buying token A pushes the buy pool's price UP toward its next tick above.
        let sell_pool_next_tick = sell_pool.next_initialized_tick_below_price();
        let buy_pool_next_tick = buy_pool.next_initialized_tick_above_price();
        let sell_pool_next_tick_sqrt_price =
            tick_index_to_sqrt_price(sell_pool_next_tick.tick_index);
        let buy_pool_next_tick_sqrt_price = tick_index_to_sqrt_price(buy_pool_next_tick.tick_index);

        // If a price already sits exactly on its next boundary, cross it first
        // (or stop, if the boundary is just the edge of the data we cached).
        if sell_pool_next_tick_sqrt_price >= sell_pool.sqrt_price_q64_64 {
            if sell_pool_next_tick.is_initialized_tick {
                sell_pool.cross_tick_and_update_liquidity(
                    sell_pool_next_tick.tick_index,
                    SwapDirection::TokenAToTokenB,
                );
                continue;
            }
            break;
        }
        if buy_pool_next_tick_sqrt_price <= buy_pool.sqrt_price_q64_64 {
            if buy_pool_next_tick.is_initialized_tick {
                buy_pool.cross_tick_and_update_liquidity(
                    buy_pool_next_tick.tick_index,
                    SwapDirection::TokenBToTokenA,
                );
                continue;
            }
            break;
        }

        // How much bridge token (USDC) until each event happens?
        //
        // Rounding rule: round DOWN what a pool pays out (never promise more
        // than it can give), round UP what we must pay in (never be 1 unit short).
        let bridge_amount_until_sell_pool_hits_next_tick = token_b_amount_between_sqrt_prices(
            sell_pool.sqrt_price_q64_64,
            sell_pool_next_tick_sqrt_price,
            sell_pool.active_liquidity,
            false,
        );
        let bridge_amount_after_fee_until_buy_pool_hits_next_tick =
            token_b_amount_between_sqrt_prices(
                buy_pool.sqrt_price_q64_64,
                buy_pool_next_tick_sqrt_price,
                buy_pool.active_liquidity,
                true,
            );
        // The buy pool takes its fee first, so we must send a bit more than the curve needs.
        let bridge_amount_until_buy_pool_hits_next_tick = amount_before_fee_was_taken(
            bridge_amount_after_fee_until_buy_pool_hits_next_tick,
            buy_pool.fee_rate_in_millionths,
        );
        let bridge_amount_until_price_gap_is_gone =
            bridge_amount_until_price_gap_closes(&sell_pool, &buy_pool);

        // Take the smallest positive distance: that event happens first.
        let bridge_amount_this_step = [
            bridge_amount_until_price_gap_is_gone,
            bridge_amount_until_sell_pool_hits_next_tick,
            bridge_amount_until_buy_pool_hits_next_tick,
        ]
        .into_iter()
        .filter(|&amount| amount > 0)
        .min()
        .unwrap_or(0);
        if bridge_amount_this_step == 0 {
            break;
        }

        let Some(start_token_input_this_step) = move_both_pool_prices_by_bridge_amount(
            &mut sell_pool,
            &mut buy_pool,
            bridge_amount_this_step,
        ) else {
            break;
        };
        total_start_token_input =
            total_start_token_input.saturating_add(start_token_input_this_step);
        if total_start_token_input == u64::MAX {
            break;
        }

        let what_stopped_this_step = WhatStopsThisWalkStep {
            price_gap_after_fees_is_gone: bridge_amount_until_price_gap_is_gone > 0
                && bridge_amount_this_step == bridge_amount_until_price_gap_is_gone
                && bridge_amount_this_step < bridge_amount_until_sell_pool_hits_next_tick
                && bridge_amount_this_step < bridge_amount_until_buy_pool_hits_next_tick,
            sell_pool_reached_next_tick: bridge_amount_this_step
                == bridge_amount_until_sell_pool_hits_next_tick,
            buy_pool_reached_next_tick: bridge_amount_this_step
                == bridge_amount_until_buy_pool_hits_next_tick,
        };

        if what_stopped_this_step.price_gap_after_fees_is_gone {
            break;
        }
        // Reaching the edge of our cached tick arrays (not a real initialized
        // tick) means we do not know the liquidity beyond it, so we stop there.
        if what_stopped_this_step.sell_pool_reached_next_tick {
            if sell_pool_next_tick.is_initialized_tick {
                sell_pool.cross_tick_and_update_liquidity(
                    sell_pool_next_tick.tick_index,
                    SwapDirection::TokenAToTokenB,
                );
            } else {
                break;
            }
        }
        if what_stopped_this_step.buy_pool_reached_next_tick {
            if buy_pool_next_tick.is_initialized_tick {
                buy_pool.cross_tick_and_update_liquidity(
                    buy_pool_next_tick.tick_index,
                    SwapDirection::TokenBToTokenA,
                );
            } else {
                break;
            }
        }
    }
    Ok(total_start_token_input)
}
