use std::collections::HashSet;

use futures::StreamExt;
use yellowstone_grpc_client::{GeyserStream, SubscribeRequestSink};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;

use super::subscribe_accounts;
use crate::adapters::decoder::{decode_clmm_pool, decode_tick_array};
use crate::core::{
    PoolCache, PoolRegistry, Venue, encode_pubkey, pubkey_from_slice, tick_arrays_around,
};

pub async fn ingest_pool_updates(
    mut stream: GeyserStream,
    mut tx: SubscribeRequestSink,
    registry: &PoolRegistry,
    cache: &PoolCache,
) {
    let mut subscribed: HashSet<String> = registry.subscribe_addresses().into_iter().collect();

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


                // check if it's a pool pubkey
                if let Some(spec) = registry.get(&pubkey) {
                    let Some(state) =
                        decode_clmm_pool(spec, &info.data, account.slot, info.write_version)
                    else {
                        eprintln!(
                            "failed to decode {} pool {}",
                            spec.venue.as_str(),
                            spec.address_bs58
                        );
                        continue;
                    };

                    let needed = tick_arrays_around(&state);
                    cache.upsert(state);
                    let fresh = cache.expect_tick_arrays(&needed);
                    if !fresh.is_empty() {
                        for pda in &fresh {
                            subscribed.insert(encode_pubkey(pda));
                        }
                        if let Err(e) =
                            subscribe_accounts(&mut tx, subscribed.iter().cloned().collect()).await
                        {
                            eprintln!("failed to subscribe tick arrays: {e}");
                        }
                    }
                    log_cache(cache);
                    continue;
                }


                // check if it's PDA for tick_array
                let Some(id) = cache.tick_array_ref(&pubkey) else {
                    continue;
                };
                let Some(array) =
                    decode_tick_array(&id, &info.data, account.slot, info.write_version)
                else {
                    eprintln!(
                        "failed to decode {} tick array {}",
                        id.venue.as_str(),
                        encode_pubkey(&pubkey)
                    );
                    continue;
                };
                cache.upsert_tick_array(array);
                log_cache(cache);
            }
            Err(e) => eprintln!("stream error: {e}"),
        }
    }
}

fn log_cache(cache: &PoolCache) {
    for pool in cache.snapshot() {
        let arrays = cache.tick_arrays(&pool.pubkey);
        let n_ticks: usize = arrays.iter().map(|array| array.ticks.len()).sum();
        println!(
            "cache {} {} slot={} tick={} spacing={} liq={} px={:.4} arrays={}/{} ticks={}",
            pool.venue.as_str(),
            encode_pubkey(&pool.pubkey),
            pool.slot,
            pool.tick,
            pool.tick_spacing,
            pool.liquidity,
            pool.spot_price(),
            arrays.len(),
            cache.expected_tick_array_count(&pool.pubkey),
            n_ticks
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
