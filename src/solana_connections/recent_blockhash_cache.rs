//! Recent blockhash, taken from the same Yellowstone stream as account updates.
//!
//! Every transaction must name a blockhash from the last ~150 slots (~60 s).
//! `getLatestBlockhash` is that value, but it is a round trip per trade.
//! The account subscription also asks for `blocks_meta`: one small message per
//! slot whose `blockhash` is the same field. This cache stores it and replaces
//! the value only when a newer slot arrives.
//!
//! A processed block can still be skipped. A transaction stamped with that
//! hash then cannot land — the same risk as quoting from processed pool state.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::print_logs;

/// How long a cached hash may be used. A healthy stream updates every slot
/// (~400 ms). Silence this long means the feed is down; the hash may also be
/// close to the 150-slot expiry, so the trade falls back to RPC.
const MAX_CACHE_AGE: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct CachedBlockhash {
    hash: solana_hash::Hash,
    slot: u64,
    updated_at: Instant,
}

/// Latest blockhash seen on the shared Geyser stream.
#[derive(Clone)]
pub struct RecentBlockhashCache {
    latest: std::sync::Arc<Mutex<Option<CachedBlockhash>>>,
    announced: std::sync::Arc<AtomicBool>,
}

impl RecentBlockhashCache {
    pub fn new() -> Self {
        Self {
            latest: std::sync::Arc::new(Mutex::new(None)),
            announced: std::sync::Arc::new(AtomicBool::new(false)),
        }
    }

    /// Store `blockhash` when `slot` is strictly newer than what we already have.
    pub fn store(&self, slot: u64, blockhash: &str) {
        if blockhash.is_empty() {
            return;
        }
        let Ok(mut slot_guard) = self.latest.lock() else {
            return;
        };
        if !is_newer_slot(slot_guard.as_ref().map(|cached| cached.slot), slot) {
            return;
        }
        let Ok(hash) = blockhash.parse::<solana_hash::Hash>() else {
            print_logs::blockhash_stream_error(&format!("invalid blockhash {blockhash}"));
            return;
        };
        *slot_guard = Some(CachedBlockhash {
            hash,
            slot,
            updated_at: Instant::now(),
        });
        drop(slot_guard);
        if !self.announced.swap(true, Ordering::Relaxed) {
            print_logs::blockhash_cache_live(slot, blockhash);
        }
    }

    /// The cached hash when one has arrived within [`MAX_CACHE_AGE`].
    pub fn fresh_hash(&self) -> Option<solana_hash::Hash> {
        let Ok(guard) = self.latest.lock() else {
            return None;
        };
        let cached = guard.as_ref()?;
        if cached.updated_at.elapsed() > MAX_CACHE_AGE {
            return None;
        }
        Some(cached.hash.clone())
    }
}

pub(crate) fn is_newer_slot(cached_slot: Option<u64>, incoming_slot: u64) -> bool {
    cached_slot.is_none_or(|slot| incoming_slot > slot)
}
