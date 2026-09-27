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
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Instant;

use super::shared_pool_types::{
    ConcentratedLiquidityPoolState, DexProgram, TickArrayAccountWithInitializedTicks,
    TickArrayPdaToWatch,
};
use super::solana_public_key_helpers::PublicKeyBytes;

/// Lookup tables keyed by account address, plus "how fresh is the stream" clocks.
#[derive(Debug, Default)]
pub struct LatestPoolStateCache {
    /// Newest decoded state of each watched pool.
    pool_state_by_pool_address: RwLock<HashMap<PublicKeyBytes, ConcentratedLiquidityPoolState>>,
    /// Tick-array addresses we decided to watch, and what each one belongs to.
    /// Filled *before* their data arrives, so incoming bytes can be identified.
    watched_tick_array_by_address: RwLock<HashMap<PublicKeyBytes, TickArrayPdaToWatch>>,
    /// Newest decoded contents of each tick array we have received.
    tick_array_by_address: RwLock<HashMap<PublicKeyBytes, TickArrayAccountWithInitializedTicks>>,
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
        pools.insert(pool_state.pool_address, pool_state);
    }

    /// Look a pool up by its address. This is the lookup multi-pool code should use.
    #[allow(dead_code)] // used later when building swap instructions for a specific pool
    pub fn pool_state_by_address(
        &self,
        pool_address: &PublicKeyBytes,
    ) -> Option<ConcentratedLiquidityPoolState> {
        read_lock(&self.pool_state_by_pool_address)
            .get(pool_address)
            .cloned()
    }

    /// Any one pool owned by `dex`.
    ///
    /// TODO? Current limitation: we watch exactly one pool per DEX, so "the first one"
    /// is "the only one". With several pools per DEX the result would be
    /// arbitrary (hash-map order), and callers must switch to
    /// [`Self::pool_state_by_address`].
    pub fn first_pool_on_dex(&self, dex: DexProgram) -> Option<ConcentratedLiquidityPoolState> {
        read_lock(&self.pool_state_by_pool_address)
            .values()
            .find(|pool| pool.dex == dex)
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
        let mut watched = write_lock(&self.watched_tick_array_by_address);
        let mut newly_watched_addresses = Vec::new();
        for &tick_array in tick_arrays {
            let was_already_watched = watched
                .insert(tick_array.tick_array_address, tick_array)
                .is_some();
            if !was_already_watched {
                newly_watched_addresses.push(tick_array.tick_array_address);
            }
        }
        newly_watched_addresses
    }

    /// If `address` is a tick array we watch, what it belongs to.
    pub fn watched_tick_array(&self, address: &PublicKeyBytes) -> Option<TickArrayPdaToWatch> {
        read_lock(&self.watched_tick_array_by_address)
            .get(address)
            .copied()
    }

    /// Store a tick array's contents, unless we already hold a newer write.
    ///
    /// Geyser messages can arrive out of order; comparing the write version
    /// keeps an old message from overwriting newer data.
    pub fn save_tick_array(&self, tick_array: TickArrayAccountWithInitializedTicks) {
        let mut tick_arrays = write_lock(&self.tick_array_by_address);
        if let Some(existing) = tick_arrays.get(&tick_array.tick_array_address)
            && existing.geyser_write_version_for_ordering
                > tick_array.geyser_write_version_for_ordering
        {
            return;
        }
        tick_arrays.insert(tick_array.tick_array_address, tick_array);
    }

    /// Every cached tick array that belongs to `pool_address`.
    pub fn tick_arrays_for_pool(
        &self,
        pool_address: &PublicKeyBytes,
    ) -> Vec<TickArrayAccountWithInitializedTicks> {
        read_lock(&self.tick_array_by_address)
            .values()
            .filter(|tick_array| tick_array.pool_address == *pool_address)
            .cloned()
            .collect()
    }

    /// How many tick arrays we are watching for `pool_address` (loaded or not yet).
    pub fn watched_tick_array_count_for_pool(&self, pool_address: &PublicKeyBytes) -> usize {
        read_lock(&self.watched_tick_array_by_address)
            .values()
            .filter(|tick_array| tick_array.pool_address == *pool_address)
            .count()
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
