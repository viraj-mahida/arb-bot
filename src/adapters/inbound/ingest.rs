use std::collections::HashSet;

use futures::StreamExt;
use yellowstone_grpc_client::{GeyserStream, SubscribeRequestSink};
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;

use super::{RpcClient, subscribe_accounts};
use crate::adapters::decoder::{decode_clmm_pool, decode_tick_array};
use crate::core::{
    PoolCache, PoolRegistry, encode_pubkey, log, pubkey_from_slice, tick_arrays_around,
};

pub async fn ingest_pool_updates(
    mut stream: GeyserStream,
    mut tx: SubscribeRequestSink,
    registry: &PoolRegistry,
    cache: &PoolCache,
    rpc: &RpcClient,
) {
    let mut subscribed: HashSet<String> = registry.subscribe_addresses().into_iter().collect();
    log::ingest_ready();

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

                if let Some(spec) = registry.get(&pubkey) {
                    let Some(state) =
                        decode_clmm_pool(spec, &info.data, account.slot, info.write_version)
                    else {
                        log::decode_pool_failed(spec.venue.as_str(), spec.address_bs58);
                        continue;
                    };

                    let needed = tick_arrays_around(&state);
                    cache.upsert(state.clone());
                    let fresh = cache.expect_tick_arrays(&needed);
                    let have = cache.tick_arrays(&state.pubkey).len();
                    let want = cache.expected_tick_array_count(&state.pubkey);
                    log::pool_update(&state, have, want);

                    if !fresh.is_empty() {
                        log::subscribing_tick_arrays(state.venue, fresh.len());
                        for pda in &fresh {
                            subscribed.insert(encode_pubkey(pda));
                        }
                        if let Err(e) =
                            subscribe_accounts(&mut tx, subscribed.iter().cloned().collect()).await
                        {
                            log::subscribe_tick_arrays_failed(e);
                        }
                        // Geyser only pushes later writes; RPC loads the current book once.
                        hydrate_tick_arrays(rpc, cache, &fresh).await;
                    }
                    log::snapshot(cache);
                    continue;
                }

                let Some(id) = cache.tick_array_ref(&pubkey) else {
                    continue;
                };
                let Some(array) =
                    decode_tick_array(&id, &info.data, account.slot, info.write_version)
                else {
                    log::decode_tick_array_failed(id.venue.as_str(), &pubkey, false);
                    continue;
                };
                cache.upsert_tick_array(array.clone());
                log::tick_array_update(&array);
            }
            Err(e) => log::stream_error(e),
        }
    }
}

async fn hydrate_tick_arrays(rpc: &RpcClient, cache: &PoolCache, pubkeys: &[[u8; 32]]) {
    log::hydrate_start(pubkeys.len());
    let accounts = match rpc.get_multiple_accounts(pubkeys).await {
        Ok(accounts) => accounts,
        Err(e) => {
            log::hydrate_failed(e);
            return;
        }
    };

    let mut decoded = 0;
    let mut missing = 0;
    let mut failed = 0;
    for (pubkey, account) in pubkeys.iter().zip(accounts) {
        let Some(account) = account else {
            missing += 1;
            continue;
        };
        let Some(id) = cache.tick_array_ref(pubkey) else {
            failed += 1;
            continue;
        };
        let Some(array) = decode_tick_array(&id, &account.data, account.slot, 0) else {
            log::decode_tick_array_failed(id.venue.as_str(), pubkey, true);
            failed += 1;
            continue;
        };
        cache.upsert_tick_array(array);
        decoded += 1;
    }
    log::hydrate_done(decoded, pubkeys.len(), missing, failed);
}
