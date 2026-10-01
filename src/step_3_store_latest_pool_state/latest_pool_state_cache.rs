//! **Sub-step 3.1.** In-memory cache holding the newest pool and tick-array state.
//!
//! **Why a cache:** asking an RPC server for state takes tens of milliseconds;
//! arbitrage opportunities often last a single slot (~400ms) and are contested
//! by other bots. So we keep everything in memory, updated by the live stream,
//! and all math reads from here instantly.
//!
//! **Why `RwLock`:** many readers (quoting) can read at the same time, while a
//! writer (a new Geyser update) briefly takes exclusive access.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Instant;

use super::shared_pool_types::{
    ConcentratedLiquidityPoolState, TickArrayAccountWithInitializedTicks, TickArrayPdaToWatch,
};
use super::solana_public_key_helpers::PublicKeyBytes;

/// Lookup tables keyed by account address, plus "how fresh is the stream" clocks.
#[derive(Debug, Default)]
pub struct LatestPoolStateCache {
    /// Newest decoded state of each watched pool. Shared with quotes so a read
    /// clones the `Arc`, not the pool.
    pool_state_by_pool_address:
        RwLock<HashMap<PublicKeyBytes, Arc<ConcentratedLiquidityPoolState>>>,
    /// Pools grouped by `(token_a_mint, token_b_mint)`. Only pools in the same
    /// group can form a two-pool round trip; the DEX does not matter.
    ///
    /// Keyed in each pool's own A/B order: the quoting math assumes both legs
    /// agree on which mint is token A, so a SOL/USDC pool and a USDC/SOL pool
    /// land in different groups.
    pool_addresses_by_mint_pair:
        RwLock<HashMap<(PublicKeyBytes, PublicKeyBytes), Vec<PublicKeyBytes>>>,
    /// Tick-array addresses we decided to watch, and what each one belongs to.
    /// Filled *before* their data arrives, so incoming bytes can be identified.
    /// `addresses_by_pool` answers "how many are we watching for this pool?".
    watched_tick_arrays: RwLock<WatchedTickArrays>,
    /// Newest decoded contents of each tick array we have received.
    /// `addresses_by_pool` avoids scanning every tick array on each quote.
    tick_arrays: RwLock<StoredTickArrays>,
    /// Raydium swap fee (millionths) keyed by `amm_config` address. Fee tiers
    /// practically never change, so each config is read once via RPC.
    raydium_fee_rate_by_fee_config_address: RwLock<HashMap<PublicKeyBytes, u16>>,
    /// Highest slot number any Geyser message has shown us: "what time is it on-chain".
    newest_slot_seen_from_stream: AtomicU64,
    /// Wall-clock time of the last Geyser account message. If the stream goes
    /// quiet, every cached price may be out of date, so trading must pause.
    last_stream_update_received_at: RwLock<Option<Instant>>,
}

impl LatestPoolStateCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a pool's state, unless we already hold a newer write (same rule as tick arrays).
    pub fn save_pool_state(&self, pool_state: ConcentratedLiquidityPoolState) {
        let mut pools = write_lock(&self.pool_state_by_pool_address);
        if let Some(existing) = pools.get(&pool_state.pool_address)
            && (existing.slot, existing.geyser_write_version_for_ordering)
                > (
                    pool_state.slot,
                    pool_state.geyser_write_version_for_ordering,
                )
        {
            return;
        }
        let mint_pair = (pool_state.token_a_mint, pool_state.token_b_mint);
        let pool_address = pool_state.pool_address;
        let is_first_sighting = pools.insert(pool_address, Arc::new(pool_state)).is_none();
        drop(pools);
        if is_first_sighting {
            write_lock(&self.pool_addresses_by_mint_pair)
                .entry(mint_pair)
                .or_default()
                .push(pool_address);
        }
    }

    /// Every other cached pool that trades the same token A / token B as `pool_address`.
    ///
    /// These are the only pools whose round trips with `pool_address` can
    /// change when `pool_address` updates.
    pub fn other_pools_with_same_mint_pair(
        &self,
        pool_address: &PublicKeyBytes,
    ) -> Vec<Arc<ConcentratedLiquidityPoolState>> {
        let pools = read_lock(&self.pool_state_by_pool_address);
        let Some(pool) = pools.get(pool_address) else {
            return Vec::new();
        };
        read_lock(&self.pool_addresses_by_mint_pair)
            .get(&(pool.token_a_mint, pool.token_b_mint))
            .into_iter()
            .flatten()
            .filter(|address| *address != pool_address)
            .filter_map(|address| pools.get(address).cloned())
            .collect()
    }

    /// Look a pool up by its address.
    pub fn pool_state_by_address(
        &self,
        pool_address: &PublicKeyBytes,
    ) -> Option<Arc<ConcentratedLiquidityPoolState>> {
        read_lock(&self.pool_state_by_pool_address)
            .get(pool_address)
            .cloned()
    }

    /// Remember tick-array addresses we want to watch.
    ///
    /// Returns only the addresses that were *not* already watched, so the caller
    /// subscribes and fetches each tick array exactly once.
    pub fn start_watching_tick_arrays(
        &self,
        tick_arrays: &[TickArrayPdaToWatch],
    ) -> Vec<PublicKeyBytes> {
        let mut watched = write_lock(&self.watched_tick_arrays);
        let mut newly_watched_addresses = Vec::new();
        for &tick_array in tick_arrays {
            let was_already_watched = watched
                .by_address
                .insert(tick_array.tick_array_address, tick_array)
                .is_some();
            if !was_already_watched {
                watched
                    .addresses_by_pool
                    .entry(tick_array.pool_address)
                    .or_default()
                    .push(tick_array.tick_array_address);
                newly_watched_addresses.push(tick_array.tick_array_address);
            }
        }
        newly_watched_addresses
    }

    /// If `address` is a tick array we watch, what it belongs to.
    pub fn watched_tick_array(&self, address: &PublicKeyBytes) -> Option<TickArrayPdaToWatch> {
        read_lock(&self.watched_tick_arrays)
            .by_address
            .get(address)
            .copied()
    }

    /// Store a tick array's contents, unless we already hold a newer write.
    ///
    /// Geyser messages can arrive out of order; comparing the write version
    /// keeps an old message from overwriting newer data.
    pub fn save_tick_array(&self, tick_array: TickArrayAccountWithInitializedTicks) {
        let mut tick_arrays = write_lock(&self.tick_arrays);
        let address = tick_array.tick_array_address;
        if let Some(existing) = tick_arrays.by_address.get(&address)
            && existing.geyser_write_version_for_ordering
                > tick_array.geyser_write_version_for_ordering
        {
            return;
        }
        let pool_address = tick_array.pool_address;
        let is_new = !tick_arrays.by_address.contains_key(&address);
        tick_arrays.by_address.insert(address, Arc::new(tick_array));
        if is_new {
            tick_arrays
                .addresses_by_pool
                .entry(pool_address)
                .or_default()
                .push(address);
        }
    }

    /// Every cached tick array that belongs to `pool_address`.
    ///
    /// Each entry is an `Arc` clone: the bytes stay in the cache, and the caller shares them.
    pub fn tick_arrays_for_pool(
        &self,
        pool_address: &PublicKeyBytes,
    ) -> Vec<Arc<TickArrayAccountWithInitializedTicks>> {
        let tick_arrays = read_lock(&self.tick_arrays);
        let Some(addresses) = tick_arrays.addresses_by_pool.get(pool_address) else {
            return Vec::new();
        };
        addresses
            .iter()
            .filter_map(|address| tick_arrays.by_address.get(address).cloned())
            .collect()
    }

    /// How many tick arrays we have decoded for `pool_address`.
    pub fn loaded_tick_array_count_for_pool(&self, pool_address: &PublicKeyBytes) -> usize {
        read_lock(&self.tick_arrays)
            .addresses_by_pool
            .get(pool_address)
            .map(Vec::len)
            .unwrap_or(0)
    }

    /// How many tick arrays we are watching for `pool_address` (loaded or not yet).
    pub fn watched_tick_array_count_for_pool(&self, pool_address: &PublicKeyBytes) -> usize {
        read_lock(&self.watched_tick_arrays)
            .addresses_by_pool
            .get(pool_address)
            .map(Vec::len)
            .unwrap_or(0)
    }

    pub fn raydium_fee_rate_for_fee_config(
        &self,
        fee_config_address: &PublicKeyBytes,
    ) -> Option<u16> {
        read_lock(&self.raydium_fee_rate_by_fee_config_address)
            .get(fee_config_address)
            .copied()
    }

    pub fn save_raydium_fee_rate_for_fee_config(
        &self,
        fee_config_address: PublicKeyBytes,
        fee_rate_in_millionths: u16,
    ) {
        write_lock(&self.raydium_fee_rate_by_fee_config_address)
            .insert(fee_config_address, fee_rate_in_millionths);
    }

    /// Call on every Geyser account message: remembers the newest slot and "now".
    pub fn record_stream_update(&self, slot: u64) {
        self.newest_slot_seen_from_stream
            .fetch_max(slot, Ordering::Relaxed);
        *write_lock(&self.last_stream_update_received_at) = Some(Instant::now());
    }

    pub fn newest_slot_seen_from_stream(&self) -> u64 {
        self.newest_slot_seen_from_stream.load(Ordering::Relaxed)
    }

    /// Milliseconds since the last Geyser account message (`None` = none yet).
    pub fn milliseconds_since_last_stream_update(&self) -> Option<u64> {
        read_lock(&self.last_stream_update_received_at)
            .map(|received_at| u64::try_from(received_at.elapsed().as_millis()).unwrap_or(u64::MAX))
    }
}

// A "poisoned" lock means another thread panicked while holding it; the data
// may be half-written, so crashing loudly is safer than trading on it.
fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read()
        .expect("cache lock poisoned by a panic in another thread")
}

fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .expect("cache lock poisoned by a panic in another thread")
}

/// Watched tick-array addresses, plus an index from pool to those addresses.
#[derive(Debug, Default)]
struct WatchedTickArrays {
    by_address: HashMap<PublicKeyBytes, TickArrayPdaToWatch>,
    addresses_by_pool: HashMap<PublicKeyBytes, Vec<PublicKeyBytes>>,
}

/// Decoded tick arrays, plus an index from pool to those addresses.
#[derive(Debug, Default)]
struct StoredTickArrays {
    by_address: HashMap<PublicKeyBytes, Arc<TickArrayAccountWithInitializedTicks>>,
    addresses_by_pool: HashMap<PublicKeyBytes, Vec<PublicKeyBytes>>,
}
