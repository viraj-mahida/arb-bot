//! Yellowstone Geyser gRPC client: one live stream of account changes and blockhashes.
//!
//! **Geyser** is a plugin inside a Solana validator that can forward every
//! account write, transaction, and slot as it happens. **Yellowstone** is the
//! popular open-source gRPC version of it, offered by most RPC providers.
//!
//! The connection is two-way:
//! - a *sender* on which we send "subscribe to these accounts" requests, and
//! - a *stream* on which the validator pushes account updates back to us.

use futures::SinkExt;
use std::collections::HashMap;

use yellowstone_grpc_client::{
    ClientTlsConfig, GeyserGrpcClient, GeyserGrpcClientResult, GeyserStream, SubscribeRequestSink,
    SubscribeRequestSinkError,
};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterBlocksMeta,
};

use super::RecentBlockhashCache;

use crate::print_logs;
use crate::step_3_store_latest_pool_state::WatchedPools;

/// The half of the connection we write subscription requests into.
pub type GeyserSubscriptionSender = SubscribeRequestSink;
/// The half of the connection the validator pushes account updates into.
pub type GeyserAccountUpdateStream = GeyserStream;

/// Open a Yellowstone client from `GRPC_URL` + `X_TOKEN`.
async fn connect_geyser_client() -> Result<GeyserGrpcClient, String> {
    let grpc_url =
        std::env::var("GRPC_URL").map_err(|_| "GRPC_URL missing from environment / .env")?;
    let access_token =
        std::env::var("X_TOKEN").map_err(|_| "X_TOKEN missing from environment / .env")?;

    let mut client_builder = GeyserGrpcClient::build_from_shared(grpc_url.clone())
        .map_err(|error| format!("failed to create gRPC client: {error}"))?;

    if grpc_url.starts_with("https://") {
        client_builder = client_builder
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .map_err(|error| format!("failed to configure TLS: {error}"))?;
    }

    client_builder = client_builder
        .x_token(Some(access_token.as_str()))
        .map_err(|error| format!("failed to set X-token: {error}"))?;

    client_builder
        .connect()
        .await
        .map_err(|error| format!("failed to connect to gRPC: {error}"))
}

/// Connect to Geyser (`GRPC_URL` + `X_TOKEN` from the environment) and
/// subscribe to every watched pool account, plus `blocks_meta` so the
/// blockhash cache updates on the same connection.
///
/// `blockhash_cache` is set only when trading is configured. The unary
/// `GetLatestBlockhash` seeds it before the first slot arrives. Watch-only
/// runs pass `None` and ignore `blocks_meta` messages.
pub async fn connect_to_geyser_grpc(
    watched_pools: &WatchedPools,
    blockhash_cache: Option<&RecentBlockhashCache>,
) -> GeyserGrpcClientResult<(GeyserSubscriptionSender, GeyserAccountUpdateStream)> {
    print_logs::geyser_connecting();
    let mut client = connect_geyser_client()
        .await
        .expect("failed to connect to gRPC");
    if let Some(cache) = blockhash_cache
        && let Ok(latest) = client
            .get_latest_blockhash(Some(CommitmentLevel::Processed))
            .await
    {
        cache.store(latest.slot, &latest.blockhash);
    }
    let (mut subscription_sender, account_update_stream) = client
        .subscribe()
        .await
        .expect("failed to open gRPC subscription");

    let pool_addresses = watched_pools.pool_addresses_to_subscribe();
    subscribe_to_account_updates(&mut subscription_sender, pool_addresses.clone())
        .await
        .expect("failed to send subscribe request");
    print_logs::geyser_subscribed(pool_addresses.len());

    Ok((subscription_sender, account_update_stream))
}

/// Tell Geyser which accounts to stream, and keep `blocks_meta` on the same request.
///
/// Important: each request *replaces* the previous filter, it does not add to
/// it. So when we start watching new tick arrays, we must send the complete
/// list (pools + every tick array so far), not just the new addresses — and
/// we must send `blocks_meta` again, or the blockhash feed stops.
///
/// `Processed` commitment = send each write as soon as the validator executes
/// it, before the network has voted on the block. Fastest possible signal;
/// occasionally a processed block is later skipped, which is acceptable for
/// price watching.
pub async fn subscribe_to_account_updates(
    subscription_sender: &mut GeyserSubscriptionSender,
    all_account_addresses_base58: Vec<String>,
) -> Result<(), SubscribeRequestSinkError> {
    let mut account_filters = HashMap::new();
    account_filters.insert(
        "watched_accounts".to_string(),
        SubscribeRequestFilterAccounts {
            account: all_account_addresses_base58,
            ..Default::default()
        },
    );
    let mut blocks_meta = HashMap::new();
    blocks_meta.insert(
        "blocks_meta".to_string(),
        SubscribeRequestFilterBlocksMeta {},
    );

    subscription_sender
        .send(SubscribeRequest {
            accounts: account_filters,
            blocks_meta,
            commitment: Some(CommitmentLevel::Processed as i32),
            ..Default::default()
        })
        .await
}
