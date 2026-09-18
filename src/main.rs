use crate::adapters::{connect_grpc, ingest_pool_updates, RpcClient};
use crate::core::{PoolCache, PoolRegistry};

mod adapters;
mod core;

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    dotenvy::dotenv().ok();

    let registry = PoolRegistry::sol_usdc_clmm();
    let cache = PoolCache::new();
    let rpc = RpcClient::from_env();

    let (tx, stream) = connect_grpc(&registry).await.expect("failed to get stream");
    ingest_pool_updates(stream, tx, &registry, &cache, &rpc).await;
}
