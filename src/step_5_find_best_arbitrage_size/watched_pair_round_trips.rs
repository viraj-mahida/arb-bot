//! Quote the watched Orca/Raydium pair: a small probe, then the best size.
//!
//! We do not know in advance which pool is more expensive, so both sell→buy
//! directions are quoted. At most one of them can be profitable at a time.

use super::quote_most_profitable_two_pool_round_trip;
use crate::step_3_store_latest_pool_state::LatestPoolStateCache;
use crate::step_4_quote_swaps::{
    CachedOrcaAndRaydiumPools, DirectedRoundTripQuote, quote_probe_round_trips_from_cache,
};

/// Probe quotes (Step 4) and best-size quotes (this step) for the watched pair.
pub struct WatchedPairRoundTripQuotes {
    pub probe_round_trips: [DirectedRoundTripQuote; 2],
    pub best_size_round_trips: [DirectedRoundTripQuote; 2],
}

/// `None` until both pools are in the cache.
pub fn quote_watched_orca_and_raydium_pair(
    cache: &LatestPoolStateCache,
) -> Option<WatchedPairRoundTripQuotes> {
    let pair = CachedOrcaAndRaydiumPools::from_cache(cache)?;
    Some(WatchedPairRoundTripQuotes {
        probe_round_trips: quote_probe_round_trips_from_cache(cache)?,
        best_size_round_trips: pair
            .quote_both_directions(quote_most_profitable_two_pool_round_trip),
    })
}
