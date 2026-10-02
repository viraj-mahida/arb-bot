//! **Sub-step 5.2.** Quote every round trip the latest pool update could have changed.
//!
//! **Start here:** [`main_quote_round_trips_touching_pool`].
//!
//! Only pools trading the same two mints can arbitrage each other, and when
//! pool P updates only the pairs that include P have new prices. So with `n`
//! pools in P's mint-pair group we quote `n - 1` pairs, not every pair.
//!
//! Each pair is quoted in one direction: sell token A where its price is
//! higher, buy it back where the price is lower. The reverse loses money.
//! Equal prices are skipped.

use super::main_quote_most_profitable_two_pool_round_trip;
use crate::step_3_store_latest_pool_state::{LatestPoolStateCache, PublicKeyBytes};
use crate::step_4_quote_swaps::{
    CachedPoolWithTickArrays, DirectedRoundTripQuote, quote_sell_high_buy_low,
};

/// Best-size round trip for every pair that includes the updated pool.
/// Sell the higher-priced pool, buy the lower-priced one.
pub struct RoundTripQuotesTouchingPool {
    pub best_size_round_trips: Vec<DirectedRoundTripQuote>,
}

/// Empty until another pool with the same mint pair is in the cache.
pub fn main_quote_round_trips_touching_pool(
    cache: &LatestPoolStateCache,
    updated_pool_address: &PublicKeyBytes,
) -> RoundTripQuotesTouchingPool {
    let Some(updated_pool) = cache.pool_state_by_address(updated_pool_address) else {
        return RoundTripQuotesTouchingPool {
            best_size_round_trips: Vec::new(),
        };
    };
    let updated_pool = CachedPoolWithTickArrays::from_cache(cache, updated_pool);
    let best_size_round_trips = cache
        .other_pools_with_same_mint_pair(updated_pool_address)
        .into_iter()
        .flat_map(|other_pool| {
            let other_pool = CachedPoolWithTickArrays::from_cache(cache, other_pool);
            quote_sell_high_buy_low(
                &updated_pool,
                &other_pool,
                main_quote_most_profitable_two_pool_round_trip,
            )
        })
        .collect();
    RoundTripQuotesTouchingPool {
        best_size_round_trips,
    }
}
