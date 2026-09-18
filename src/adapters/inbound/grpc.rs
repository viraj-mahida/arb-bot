use futures::SinkExt;
use std::collections::HashMap;

use yellowstone_grpc_client::{
    ClientTlsConfig, GeyserGrpcClient, GeyserGrpcClientResult, GeyserStream, SubscribeRequestSink,
    SubscribeRequestSinkError,
};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
};

use crate::core::{PoolRegistry, log};

pub async fn connect_grpc(
    registry: &PoolRegistry,
) -> GeyserGrpcClientResult<(SubscribeRequestSink, GeyserStream)> {
    let endpoint = std::env::var("GRPC_URL").expect("GRPC_URL missing");
    let token = std::env::var("X_TOKEN").expect("X_TOKEN missing");

    log::grpc_connecting();
    let mut builder = GeyserGrpcClient::build_from_shared(endpoint.clone())
        .expect("Failed to create gRPC client");

    if endpoint.starts_with("https://") {
        builder = builder
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .expect("Failed to configure TLS");
    }

    builder = builder
        .x_token(Some(token.as_str()))
        .expect("Failed to set X-token");

    let mut client = builder.connect().await.expect("Failed to connect to gRPC");

    let (mut tx, stream) = client.subscribe().await.expect("Failed to subscribe gRPC");

    let pools = registry.subscribe_addresses();
    subscribe_accounts(&mut tx, pools.clone())
        .await
        .expect("Failed to send tx");
    log::grpc_subscribed(pools.len());

    Ok((tx, stream))
}

pub async fn subscribe_accounts(
    tx: &mut SubscribeRequestSink,
    addresses: Vec<String>,
) -> Result<(), SubscribeRequestSinkError> {
    let mut accounts = HashMap::new();
    accounts.insert(
        "pools".to_string(),
        SubscribeRequestFilterAccounts {
            account: addresses,
            ..Default::default()
        },
    );

    tx.send(SubscribeRequest {
        accounts,
        commitment: Some(CommitmentLevel::Processed as i32),
        ..Default::default()
    })
    .await
}
