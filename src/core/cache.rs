use std::collections::HashMap;
use std::sync::RwLock;

use super::types::{ClmmPoolState, TickArrayRef, TickArraySnapshot, Venue};

/// In-memory CLMM snapshots: pools by pubkey, tick arrays by PDA.
#[derive(Debug, Default)]
pub struct PoolCache {
    pools: RwLock<HashMap<[u8; 32], ClmmPoolState>>,
    expected_arrays: RwLock<HashMap<[u8; 32], TickArrayRef>>,
    tick_arrays: RwLock<HashMap<[u8; 32], TickArraySnapshot>>,
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

    #[allow(dead_code)] // swap ix building will look up by pubkey
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
        self.pools.read().expect("pool cache lock poisoned").len()
    }

    /// Remember PDAs we will subscribe to. Returns addresses not seen before.
    pub fn expect_tick_arrays(&self, refs: &[TickArrayRef]) -> Vec<[u8; 32]> {
        let mut expected = self
            .expected_arrays
            .write()
            .expect("tick array lock poisoned");
        let mut fresh = Vec::new();
        for &r in refs {
            if expected.insert(r.pubkey, r).is_none() {
                fresh.push(r.pubkey);
            }
        }
        fresh
    }

    pub fn tick_array_ref(&self, pubkey: &[u8; 32]) -> Option<TickArrayRef> {
        self.expected_arrays
            .read()
            .expect("tick array lock poisoned")
            .get(pubkey)
            .copied()
    }

    pub fn upsert_tick_array(&self, state: TickArraySnapshot) {
        let mut arrays = self.tick_arrays.write().expect("tick array lock poisoned");
        if let Some(existing) = arrays.get(&state.pubkey)
            && existing.write_version > state.write_version
        {
            return;
        }
        arrays.insert(state.pubkey, state);
    }

    pub fn tick_arrays(&self, pool: &[u8; 32]) -> Vec<TickArraySnapshot> {
        self.tick_arrays
            .read()
            .expect("tick array lock poisoned")
            .values()
            .filter(|array| array.pool == *pool)
            .cloned()
            .collect()
    }

    pub fn expected_tick_array_count(&self, pool: &[u8; 32]) -> usize {
        self.expected_arrays
            .read()
            .expect("tick array lock poisoned")
            .values()
            .filter(|array| array.pool == *pool)
            .count()
    }
}
