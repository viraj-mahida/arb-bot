//! Yellowstone Geyser gRPC client: a live stream of account changes.
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
};

use crate::step_3_store_latest_pool_state::WatchedPools;
use crate::step_6_print_logs;

/// The half of the connection we write subscription requests into.
pub type GeyserSubscriptionSender = SubscribeRequestSink;
/// The half of the connection the validator pushes account updates into.
pub type GeyserAccountUpdateStream = GeyserStream;

/// Connect to Geyser (`GRPC_URL` + `X_TOKEN` from the environment) and
/// subscribe to every watched pool account.
pub async fn connect_to_geyser_grpc(
    watched_pools: &WatchedPools,
) -> GeyserGrpcClientResult<(GeyserSubscriptionSender, GeyserAccountUpdateStream)> {
    let grpc_url = std::env::var("GRPC_URL").expect("GRPC_URL missing from environment / .env");
    let access_token = std::env::var("X_TOKEN").expect("X_TOKEN missing from environment / .env");

    step_6_print_logs::geyser_connecting();
    let mut client_builder = GeyserGrpcClient::build_from_shared(grpc_url.clone())
        .expect("failed to create gRPC client");

    if grpc_url.starts_with("https://") {
        client_builder = client_builder
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .expect("failed to configure TLS");
    }

    client_builder = client_builder
        .x_token(Some(access_token.as_str()))
        .expect("failed to set X-token");

    let mut client = client_builder.connect().await.expect("failed to connect to gRPC");
    let (mut subscription_sender, account_update_stream) =
        client.subscribe().await.expect("failed to open gRPC subscription");

    let pool_addresses = watched_pools.pool_addresses_to_subscribe();
    subscribe_to_account_updates(&mut subscription_sender, pool_addresses.clone())
        .await
        .expect("failed to send subscribe request");
    step_6_print_logs::geyser_subscribed(pool_addresses.len());

    Ok((subscription_sender, account_update_stream))
}

/// Tell Geyser which accounts to stream.
///
/// Important: each request *replaces* the previous filter, it does not add to
/// it. So when we start watching new tick arrays, we must send the complete
/// list (pools + every tick array so far), not just the new addresses.
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

    subscription_sender
        .send(SubscribeRequest {
            accounts: account_filters,
            commitment: Some(CommitmentLevel::Processed as i32),
            ..Default::default()
        })
        .await
}
