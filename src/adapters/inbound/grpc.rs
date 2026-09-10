use std::collections::HashMap;
use futures::{SinkExt};

use yellowstone_grpc_client::{
    ClientTlsConfig,
    GeyserGrpcClient,
    GeyserGrpcClientResult,
    GeyserStream,
    SubscribeRequestSink
};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel,
    SubscribeRequest,
    SubscribeRequestFilterAccounts,
};

use crate::core::{ORCA_WHIRLPOOL_SOL_USDC, RAYDIUM_CLMM_SOL_USDC};

pub async fn connect_grpc() -> GeyserGrpcClientResult<(SubscribeRequestSink, GeyserStream)> {
    let endpoint = std::env::var("GRPC_URL").expect("GRPC_URL missing");
    let token = std::env::var("X_TOKEN").expect("X_TOKEN missing");

    let mut builder = GeyserGrpcClient::build_from_shared(endpoint.clone()).expect("Failed to create gRPC client");

    if endpoint.starts_with("https://") {
        builder = builder.tls_config(ClientTlsConfig::new().with_native_roots()).expect("Failed to configure TLS");
    }

    builder = builder.x_token(Some(token.as_str())).expect("Failed to set X-token");

    let mut client = builder.connect().await.expect("Failed to connect to gRPC");

    let (mut tx, stream) = client.subscribe().await.expect("Failed to subscribe gRPC");

    let mut accounts = HashMap::new();
    accounts.insert(
        "pools".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec![
                RAYDIUM_CLMM_SOL_USDC.into(),   // Raydium CLMM 0.04% SOL-USDC
                ORCA_WHIRLPOOL_SOL_USDC.into(), // Orca CLMM 0.04% SOL-USDC
            ],
            ..Default::default()
        },
    );

    let request = SubscribeRequest {
        accounts,
        commitment: Some(CommitmentLevel::Processed as i32),
        ..Default::default()
    };

    tx.send(request).await.expect("Failed to send tx");

    Ok((tx, stream))
}