use futures::StreamExt;
use yellowstone_grpc_client::GeyserStream;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;

use crate::adapters::decoder::decode_clmm_pool;
use crate::core::{encode_pubkey, pubkey_from_slice, PoolCache, PoolRegistry, Venue};

pub async fn ingest_pool_updates(mut stream: GeyserStream, registry: &PoolRegistry, cache: &PoolCache) {
    while let Some(message) = stream.next().await {
        match message {
            Ok(update) => {
                let Some(UpdateOneof::Account(account)) = update.update_oneof else {
                    continue;
                };
                let Some(info) = &account.account else {
                    continue;
                };
                let Some(pubkey) = pubkey_from_slice(&info.pubkey) else {
                    continue;
                };
                let Some(spec) = registry.get(&pubkey) else {
                    continue;
                };

                let Some(state) = decode_clmm_pool(spec, &info.data, account.slot, info.write_version)
                else {
                    eprintln!(
                        "failed to decode {} pool {}",
                        spec.venue.as_str(),
                        spec.address_bs58
                    );
                    continue;
                };

                cache.upsert(state);
                log_cache(cache);
            }
            Err(e) => eprintln!("stream error: {e}"),
        }
    }
}

fn log_cache(cache: &PoolCache) {
    for pool in cache.snapshot() {
        println!(
            "cache {} {} slot={} tick={} spacing={} liq={} px={:.4}",
            pool.venue.as_str(),
            encode_pubkey(&pool.pubkey),
            pool.slot,
            pool.tick,
            pool.tick_spacing,
            pool.liquidity,
            pool.spot_price()
        );
    }

    let raydium = cache.get_venue(Venue::RaydiumClmm);
    let orca = cache.get_venue(Venue::OrcaWhirlpool);
    if let (Some(r), Some(o)) = (raydium, orca) {
        let rp = r.spot_price();
        let op = o.spot_price();
        println!(
            "cached={} raydium={rp:.4} orca={op:.4} Δ={:.4}",
            cache.len(),
            rp - op
        );
    }
}
