use std::time::Duration;

use serde::Deserialize;

use crate::core::encode_pubkey;

const RPC_TIMEOUT: Duration = Duration::from_secs(10);

pub struct RpcClient {
    url: String,
    http: reqwest::Client,
}

pub struct FetchedAccount {
    pub data: Vec<u8>,
    pub slot: u64,
}

impl RpcClient {
    pub fn from_env() -> Self {
        let url = std::env::var("RPC_URL").expect("RPC_URL missing");
        let http = reqwest::Client::builder()
            .timeout(RPC_TIMEOUT)
            .build()
            .expect("failed to build RPC client");
        Self { url, http }
    }

    pub async fn get_multiple_accounts(
        &self,
        pubkeys: &[[u8; 32]],
    ) -> Result<Vec<Option<FetchedAccount>>, String> {
        if pubkeys.is_empty() {
            return Ok(Vec::new());
        }

        let keys: Vec<String> = pubkeys.iter().map(encode_pubkey).collect();
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getMultipleAccounts",
            "params": [
                keys,
                { "encoding": "base64", "commitment": "processed" }
            ]
        });

        let envelope: RpcEnvelope = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if let Some(error) = envelope.error {
            return Err(error.to_string());
        }
        
        let Some(result) = envelope.result else {
            return Err("rpc response missing result".into());
        };
        if result.value.len() != pubkeys.len() {
            return Err(format!(
                "rpc returned {} accounts, expected {}",
                result.value.len(),
                pubkeys.len()
            ));
        }

        let slot = result.context.slot;
        result
            .value
            .into_iter()
            .map(|account| match account {
                None => Ok(None),
                Some(account) => Ok(Some(FetchedAccount {
                    data: account.data.decode()?,
                    slot,
                })),
            })
            .collect()
    }
}

#[derive(Deserialize)]
struct RpcEnvelope {
    result: Option<RpcResult>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RpcResult {
    context: RpcContext,
    value: Vec<Option<UiAccount>>,
}

#[derive(Deserialize)]
struct RpcContext {
    slot: u64,
}

#[derive(Deserialize)]
struct UiAccount {
    data: UiAccountData,
}

#[derive(Deserialize)]
struct UiAccountData(Vec<String>);

impl UiAccountData {
    fn decode(&self) -> Result<Vec<u8>, String> {
        let Some(encoded) = self.0.first() else {
            return Err("account data missing base64 payload".into());
        };
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
            .map_err(|e| e.to_string())
    }
}
