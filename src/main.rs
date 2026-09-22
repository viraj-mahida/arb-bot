use crate::adapters::{RpcClient, connect_grpc, ingest_pool_updates};
use crate::core::{PoolCache, PoolRegistry};

pub(crate) mod adapters;
pub(crate) mod core;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    dotenvy::dotenv().ok();

    let registry = PoolRegistry::sol_usdc_clmm();
    crate::core::log::startup(&registry);
    let cache = PoolCache::new();
    let rpc = RpcClient::from_env();
    crate::core::log::rpc_ready();

    let (tx, stream) = connect_grpc(&registry).await.expect("failed to get stream");
    ingest_pool_updates(stream, tx, &registry, &cache, &rpc).await;
}
