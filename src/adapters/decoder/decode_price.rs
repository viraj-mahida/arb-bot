use yellowstone_grpc_client::GeyserStream;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use futures::{StreamExt};

use crate::core::{ORCA_WHIRLPOOL_SOL_USDC, RAYDIUM_CLMM_SOL_USDC};

struct PriceTick {
    venue: &'static str, //"raydium" | "ocra"
    price: f64,
    slot: u64
}

fn printProfit(raydium: Option<f64>, orca: Option<f64>) {
    if let (Some(r), Some(o)) = (raydium, orca) {
        let diff = r-o;
        if diff.abs() > 0.0 {
            println!("raydium{r:.4} orca={o:.4} Δ={diff:.4}")
        }
    }
}

pub async fn decode_price(mut stream: GeyserStream) {
    let mut raydium: Option<f64> = None;
    let mut orca: Option<f64> = None;

    while let Some(message) = stream.next().await {
        match message {
            Ok(update) => {
                if let Some(UpdateOneof::Account(account)) = update.update_oneof {
                    println!("got account");
                    if let Some(info) = &account.account {
                        let pool_pubkey = bs58::encode(&info.pubkey).into_string();
                        //Raydium Pool
                        match pool_pubkey.as_str() {
                            RAYDIUM_CLMM_SOL_USDC => {
                                println!("Raydium transactions received!");
                                if let Ok(bytes) = info.data[253..269].try_into() as Result<[u8; 16], _> {
                                    let sqrt_price_x64 = u128::from_le_bytes(bytes) as f64;
                                    let sqrt = sqrt_price_x64 / 2f64.powi(64);
                                    let dec0 = info.data[233] as i32; //SOL  9 decimals
                                    let dec1 = info.data[234] as i32; //USDC 6 decimals
                                    let price = sqrt.powi(2) * 10f64.powi(dec0 - dec1);
                                    
                                    raydium = Some(price);
                                    printProfit(raydium, orca);
                                }
                            }
                            ORCA_WHIRLPOOL_SOL_USDC => {
                                println!("Orca transactions received!");
                                if let Ok(bytes) = <[u8; 16]>::try_from(&info.data[65..81]) {
                                    let sqrt_price_x64 = u128::from_le_bytes(bytes) as f64;
                                    let sqrt = sqrt_price_x64 / 2f64.powi(64);
                                    let price = sqrt.powi(2) * 10f64.powi(3);

                                    orca = Some(price);
                                    printProfit(raydium, orca);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            Err(e)=>eprintln!("Stream error Raydium: {e}")
        }
    }
}