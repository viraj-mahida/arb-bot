//! # Step 2 — Decode account bytes
//!
//! **What came before:** Step 1 received a message saying "account X now
//! contains these bytes".
//!
//! **What this step does:** turns those raw bytes into the structs from
//! Step 3's shared types (pool state, tick array).
//!
//! **Why it is needed:** on Solana, an account's data is just a byte array. The
//! program that owns the account decides the layout: "bytes 49..65 are the
//! liquidity, bytes 65..81 are the square-root price", and so on. Nothing in
//! the bytes labels them, so we must know each DEX's layout exactly.
//!
//! Two more terms you will see in this step:
//! - **Anchor discriminator:** most Solana programs are written with the Anchor
//!   framework, which puts an 8-byte "type tag" at the start of every account.
//!   Checking it confirms the bytes really are the account type we expect.
//! - **Little-endian:** multi-byte numbers are stored lowest byte first.
//!   The number 1 as 4 bytes is `01 00 00 00`, not `00 00 00 01`.
//!
//! **What comes next:** the decoded structs are saved into Step 3's cache.

mod functions;

mod decoder_for_each_dex;

pub use decoder_for_each_dex::*;
pub use functions::{orca_whirlpool_account_decoder, raydium_clmm_account_decoder};
