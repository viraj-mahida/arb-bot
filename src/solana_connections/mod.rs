//! # Solana connections — the bot's two network links
//!
//! A bot talks to the Solana blockchain through two kinds of servers:
//!
//! - **RPC (Remote Procedure Call)** — plain request/response over HTTP.
//!   "Give me account X", "send this transaction", "simulate this
//!   transaction". Good for one-off questions and for *sending*, but it has to
//!   be asked every time.
//!   See [`solana_rpc_client`].
//!
//! - **Geyser gRPC (Yellowstone)** — a live push stream. We subscribe once
//!   ("tell me whenever these accounts change") and the validator pushes every
//!   new write the moment it happens. Much faster than polling RPC, but it only
//!   reports *future* changes, never the current state.
//!   See [`geyser_grpc_client`]. The same subscription also asks for
//!   `blocks_meta`, so each new blockhash arrives on that stream;
//!   see [`recent_blockhash_cache`].
//!
//! The bot needs both: Geyser to react instantly, RPC to fill in the state that
//! existed before we subscribed and to simulate and send transactions.
//!
//! - **Jito block engine** — a third, optional link used only for *sending*:
//!   it accepts bundles and auctions block space by tip.
//!   See [`jito_block_engine_client`].
//!
//! Endpoints come from environment variables (`.env`): `RPC_URL`, `GRPC_URL`,
//! `X_TOKEN` (the Geyser provider's access token), and `JITO_BLOCK_ENGINE_URL`.

mod geyser_grpc_client;
mod jito_block_engine_client;
mod recent_blockhash_cache;
mod solana_rpc_client;

pub use geyser_grpc_client::*;
pub use jito_block_engine_client::*;
pub use recent_blockhash_cache::RecentBlockhashCache;
pub use solana_rpc_client::*;
