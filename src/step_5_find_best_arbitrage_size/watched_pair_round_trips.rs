//! **Sub-step 5.2.** Quote the watched Orca/Raydium pair at the profit-maximizing size.
//!
//! **Start here:** [`main_quote_watched_orca_and_raydium_pair`].
//!
//! We do not know in advance which pool is more expensive, so both sell→buy
//! directions are quoted. At most one of them can be profitable at a time.

use super::main_quote_most_profitable_two_pool_round_trip;
use crate::step_3_store_latest_pool_state::LatestPoolStateCache;
use crate::step_4_quote_swaps::{CachedOrcaAndRaydiumPools, DirectedRoundTripQuote};

/// Best-size round trips for the watched pair, both directions.
pub struct WatchedPairRoundTripQuotes {
    pub best_size_round_trips: [DirectedRoundTripQuote; 2],
}

/// `None` until both pools are in the cache.
pub fn main_quote_watched_orca_and_raydium_pair(
    cache: &LatestPoolStateCache,
) -> Option<WatchedPairRoundTripQuotes> {
    let pair = CachedOrcaAndRaydiumPools::from_cache(cache)?;
    Some(WatchedPairRoundTripQuotes {
        best_size_round_trips: pair
            .quote_both_directions(main_quote_most_profitable_two_pool_round_trip),
    })
}
