//! After every pool update: the updated pool's price against each pool with the same mints.

use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, LatestPoolStateCache, PublicKeyBytes,
};
use crate::step_4_quote_swaps::pool_label;

pub fn pool_snapshot(cache: &LatestPoolStateCache, updated_pool_address: &PublicKeyBytes) {
    let Some(updated_pool) = cache.pool_state_by_address(updated_pool_address) else {
        return;
    };
    // Demo mode shifts Orca on the copy used for the spread line and the
    // dashboard boards. The cache itself is unchanged.
    let updated_pool = crate::dashboard_events::ignr_share_or_shift_pool(updated_pool);
    let other_pools = cache.other_pools_with_same_mint_pair(updated_pool_address);
    if other_pools.is_empty() {
        log_line!(
            "[spread]  {}  waiting for another pool with the same mints",
            pool_label(&updated_pool)
        );
        return;
    }
    for other_pool in other_pools {
        let other_pool = crate::dashboard_events::ignr_share_or_shift_pool(other_pool);
        print_spread(&updated_pool, &other_pool);
    }
}

fn print_spread(
    pool: &ConcentratedLiquidityPoolState,
    other_pool: &ConcentratedLiquidityPoolState,
) {
    let price = pool.human_readable_price_token_b_per_token_a();
    let other_price = other_pool.human_readable_price_token_b_per_token_a();
    let midpoint_price = (price + other_price) / 2.0;
    // 1 basis point (bp) = 0.01%, the usual unit for small price differences.
    let gap_in_basis_points = if midpoint_price > 0.0 {
        (price - other_price).abs() / midpoint_price * 10_000.0
    } else {
        0.0
    };
    let higher_pool = if price >= other_price {
        pool
    } else {
        other_pool
    };
    let fees_in_basis_points = fee_in_basis_points(pool) + fee_in_basis_points(other_pool);
    let verdict = if gap_in_basis_points > fees_in_basis_points {
        "gap beats fees → checking best size"
    } else {
        "gap smaller than fees → no arbitrage"
    };

    log_line!(
        "[spread]  {} {price:.4}  {} {other_price:.4}  gap {gap_in_basis_points:.2} bps ({} higher)  fees {fees_in_basis_points:.2} bps  {verdict}",
        pool_label(pool),
        pool_label(other_pool),
        pool_label(higher_pool),
    );
    crate::dashboard_events::boards(
        pool,
        other_pool,
        gap_in_basis_points,
        fees_in_basis_points,
        gap_in_basis_points > fees_in_basis_points,
    );
}

fn fee_in_basis_points(pool: &ConcentratedLiquidityPoolState) -> f64 {
    f64::from(pool.fee_rate_in_millionths) / 100.0
}
