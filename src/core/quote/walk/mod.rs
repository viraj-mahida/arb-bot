//! Dual-book tick walk: size a round-trip from liquidity, not a guessed SOL amount.
//!
//! - [`side`]: one pool's live sqrt price, L, and initialized ticks
//! - [`math`]: fees, marginal edge, equalizing USDC, one USDC step
//! - this file: lockstep loop across both books

mod math;
mod side;

use orca_whirlpools_core::tick_index_to_sqrt_price;

use super::super::types::{ClmmPoolState, TickArraySnapshot};
use super::types::QuoteError;
use math::{amount_b, apply_usdc, equalizing_usdc, has_edge, reverse_fee};
use side::Side;

const MAX_STEPS: u32 = 256;

/// Walk both books together and return the SOL-in that maximises round-trip PnL.
///
/// Sell leg is SOL→USDC (`a_to_b`): price falls, we consume the dear book.
/// Buy leg is USDC→SOL: price rises, we consume the cheap book. USDC is the
/// matching token (every unit sold on one side is spent on the other).
///
/// Within a constant-L range we take the min of:
/// 1. USDC that drives **marginal** PnL to zero (max total PnL, not max volume)
/// 2. USDC to the next initialized tick / cached-window edge on each side
///
/// The result is a size hint. [`super::round_trip`] still runs venue math for
/// the amounts that would go on a transaction.
pub fn max_pnl_sol_in(
    sell: &ClmmPoolState,
    sell_arrays: &[TickArraySnapshot],
    buy: &ClmmPoolState,
    buy_arrays: &[TickArraySnapshot],
) -> Result<u64, QuoteError> {
    let mut sell = Side::new(sell, sell_arrays)?;
    let mut buy = Side::new(buy, buy_arrays)?;
    if !has_edge(&sell, &buy) {
        return Ok(0);
    }

    let mut sol_in: u64 = 0;
    for _ in 0..MAX_STEPS {
        if !has_edge(&sell, &buy) {
            break;
        }
        
        let (sell_tick, sell_init) = sell.next_down(); 
        let (buy_tick, buy_init) = buy.next_up();
        let sell_bound = tick_index_to_sqrt_price(sell_tick);
        let buy_bound = tick_index_to_sqrt_price(buy_tick);

        // Already at the bound: cross an initialized tick, or stop at the window.
        if sell_bound >= sell.sqrt {
            if sell_init {
                sell.cross(sell_tick, true);
                continue;
            }
            break;
        }
        if buy_bound <= buy.sqrt {
            if buy_init {
                buy.cross(buy_tick, false);
                continue;
            }
            break;
        }

        let sell_max = amount_b(sell.sqrt, sell_bound, sell.liq, false);
        let buy_net_max = amount_b(buy.sqrt, buy_bound, buy.liq, true);
        let buy_max = reverse_fee(buy_net_max, buy.fee);
        let eq = equalizing_usdc(&sell, &buy);

        let usdc = [eq, sell_max, buy_max]
            .into_iter()
            .filter(|&x| x > 0)
            .min()
            .unwrap_or(0);
        if usdc == 0 {
            break;
        }

        let Some(step_sol) = apply_usdc(&mut sell, &mut buy, usdc) else {
            break;
        };
        sol_in = sol_in.saturating_add(step_sol);
        if sol_in == u64::MAX {
            break;
        }

        let hit_eq = eq > 0 && usdc == eq && usdc < sell_max && usdc < buy_max;
        if hit_eq {
            break;
        }
        if usdc == sell_max {
            if sell_init {
                sell.cross(sell_tick, true);
            } else {
                break;
            }
        }
        if usdc == buy_max {
            if buy_init {
                buy.cross(buy_tick, false);
            } else {
                break;
            }
        }
    }
    Ok(sol_in)
}
