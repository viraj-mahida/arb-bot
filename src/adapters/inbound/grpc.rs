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
    SubscribeRequest,
    SubscribeRequestFilterBlocksMeta,
    SubscribeRequestFilterTransactions
};

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

    let mut transactions = HashMap::new();
    transactions.insert(
        "all-txn".to_string(), 
        SubscribeRequestFilterTransactions{
            ..Default::default()
        }
    );

    let mut blocks_meta = HashMap::new();
    blocks_meta.insert(
        "all-blocks".to_string(),
        SubscribeRequestFilterBlocksMeta{}
    );

    let request = SubscribeRequest{
        transactions,
        blocks_meta,
        ..Default::default()
    };

    tx.send(request).await.expect("Failed to send tx");

    Ok((tx, stream))
}