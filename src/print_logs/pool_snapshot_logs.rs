//! One line after every pool update: both prices, the gap, and whether it beats the fees.

use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, LatestPoolStateCache,
};

pub fn pool_snapshot(cache: &LatestPoolStateCache) {
    let raydium_pool = cache.first_pool_on_dex(DexProgram::RaydiumClmm);
    let orca_pool = cache.first_pool_on_dex(DexProgram::OrcaWhirlpool);
    let (Some(raydium_pool), Some(orca_pool)) = (raydium_pool, orca_pool) else {
        log_line!("[spread]  waiting until both pools have sent their first update");
        return;
    };

    let raydium_price = raydium_pool.human_readable_price_token_b_per_token_a();
    let orca_price = orca_pool.human_readable_price_token_b_per_token_a();
    let midpoint_price = (raydium_price + orca_price) / 2.0;
    // 1 basis point (bp) = 0.01%, the usual unit for small price differences.
    let gap_in_basis_points = if midpoint_price > 0.0 {
        (raydium_price - orca_price).abs() / midpoint_price * 10_000.0
    } else {
        0.0
    };
    let higher_dex = if raydium_price >= orca_price {
        DexProgram::RaydiumClmm
    } else {
        DexProgram::OrcaWhirlpool
    };
    let fees_in_basis_points = fee_in_basis_points(&raydium_pool) + fee_in_basis_points(&orca_pool);
    let verdict = if gap_in_basis_points > fees_in_basis_points {
        "gap beats fees → checking best size"
    } else {
        "gap smaller than fees → no arbitrage"
    };

    log_line!(
        "[spread]  raydium {raydium_price:.4}  orca {orca_price:.4} USDC/SOL  gap {gap_in_basis_points:.2} bps ({} higher)  fees {fees_in_basis_points:.2} bps  {verdict}",
        higher_dex.name(),
    );
}

fn fee_in_basis_points(pool: &ConcentratedLiquidityPoolState) -> f64 {
    f64::from(pool.fee_rate_in_millionths) / 100.0
}
