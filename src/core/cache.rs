use std::collections::HashMap;
use std::sync::RwLock;

use super::types::{ClmmPoolState, Venue};

/// In-memory CLMM pool snapshots, keyed by pool pubkey.
/// Tick arrays can share this lock later as a second map, or hang off each pool.
#[derive(Debug, Default)]
pub struct PoolCache {
    pools: RwLock<HashMap<[u8; 32], ClmmPoolState>>,
}

impl PoolCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&self, state: ClmmPoolState) {
        self.pools
            .write()
            .expect("pool cache lock poisoned")
            .insert(state.pubkey, state);
    }

    #[allow(dead_code)] // quoting will look up by pubkey
    pub fn get(&self, pubkey: &[u8; 32]) -> Option<ClmmPoolState> {
        self.pools
            .read()
            .expect("pool cache lock poisoned")
            .get(pubkey)
            .cloned()
    }

    /// TODO? Caveat: if you later store several Raydium (or Orca) pools, 
    /// find still returns only the first one it hits. 
    /// HashMap order is not stable, so which one is undefined. 
    /// Fine while you have one pool per venue.
    pub fn get_venue(&self, venue: Venue) -> Option<ClmmPoolState> {
        self.pools
            .read()
            .expect("pool cache lock poisoned")
            .values()
            .find(|pool| pool.venue == venue)
            .cloned()
    }

    pub fn snapshot(&self) -> Vec<ClmmPoolState> {
        self.pools
            .read()
            .expect("pool cache lock poisoned")
            .values()
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.pools
            .read()
            .expect("pool cache lock poisoned")
            .len()
    }
}
