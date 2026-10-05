//! # Step 3 — Store the latest pool state
//!
//! **What came before:** Step 1 heard that an account changed on the blockchain,
//! and Step 2 turned that account's raw bytes into a Rust struct.
//!
//! **What this step does:** keeps the newest copy of every pool and every tick
//! array in memory, so the quoting steps can do math instantly without asking
//! the network again. Think of it as a whiteboard that always shows the latest
//! prices; every new message from the blockchain erases and rewrites one cell.
//!
//! It also holds the things every other step shares:
//! - the vocabulary types ([`shared_pool_types`]) — pool, tick, tick array, DEX,
//! - the list of pools we care about ([`watched_pool_config`]),
//! - fixed on-chain addresses ([`known_program_and_pool_addresses`]),
//! - how tick-array addresses are computed ([`tick_array_pda_derivation`]),
//! - helpers to parse Solana addresses ([`solana_public_key_helpers`]).
//!
//! **What comes next:** Step 4 reads this cache to quote swaps, and Step 5 reads
//! it to find the most profitable arbitrage size.

mod functions;
pub mod known_program_and_pool_addresses;
mod shared_pool_types;
mod watched_pool_config;

mod latest_pool_state_cache;

pub use functions::solana_public_key_helpers;
pub use functions::solana_public_key_helpers::*;
pub use functions::tick_array_pda_derivation;
pub use functions::tick_array_pda_derivation::tick_array_pdas_near_current_price;
pub use latest_pool_state_cache::*;
pub use shared_pool_types::*;
pub use watched_pool_config::*;
