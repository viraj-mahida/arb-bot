use crate::adapters::{connect_grpc, decode_price};

mod adapters;
mod core;

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
    .install_default()
    .expect("failed to install rustls crypto provider");

    dotenvy::dotenv().ok();
    
    let (_, stream) = connect_grpc().await.expect("failed to get stream");
    decode_price(stream).await;
}