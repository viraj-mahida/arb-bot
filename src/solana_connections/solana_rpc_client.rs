//! HTTP JSON-RPC client for a Solana RPC node.
//!
//! **Today:** only `getMultipleAccounts`, used once per tick array to load its
//! current bytes (Geyser only streams changes that happen after we subscribe).
//!
//! **Future:** the same client will gain `sendTransaction` (submit a signed
//! arbitrage transaction), `simulateTransaction` (dry-run it first to see
//! whether it would succeed), and `getLatestBlockhash` (every transaction must
//! reference a recent blockhash so validators know it is fresh).

use std::time::Duration;

use serde::Deserialize;

use crate::step_3_store_latest_pool_state::{PublicKeyBytes, encode_public_key_as_base58};

/// Give up on a request after this long, so a stuck node cannot freeze the bot.
const RPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Connection to one Solana RPC node.
pub struct SolanaRpcClient {
    rpc_url: String,
    http_client: reqwest::Client,
}

/// One account's raw bytes, plus the slot the RPC node read them at.
pub struct AccountDataAtSlot {
    pub account_data: Vec<u8>,
    pub slot: u64,
}

impl SolanaRpcClient {
    /// Build a client from the `RPC_URL` environment variable.
    pub fn from_env() -> Self {
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL missing from environment / .env");
        let http_client = reqwest::Client::builder()
            .timeout(RPC_REQUEST_TIMEOUT)
            .build()
            .expect("failed to build HTTP client for RPC");
        Self { rpc_url, http_client }
    }

    /// Call the `getMultipleAccounts` RPC method: fetch many accounts in one request.
    ///
    /// Returns one entry per requested address, in the same order. An entry is
    /// `None` when that account does not exist on-chain (for example a tick
    /// array nobody has created because no liquidity was ever placed there).
    ///
    /// We ask for `processed` commitment — the freshest data the node has seen,
    /// even if the block is not yet confirmed by the network — because speed
    /// matters more than finality when only *reading* prices.
    pub async fn get_multiple_accounts(
        &self,
        account_addresses: &[PublicKeyBytes],
    ) -> Result<Vec<Option<AccountDataAtSlot>>, String> {
        if account_addresses.is_empty() {
            return Ok(Vec::new());
        }

        let addresses_base58: Vec<String> =
            account_addresses.iter().map(encode_public_key_as_base58).collect();
        let json_rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getMultipleAccounts",
            "params": [
                addresses_base58,
                { "encoding": "base64", "commitment": "processed" }
            ]
        });

        let response: JsonRpcResponse = self
            .http_client
            .post(&self.rpc_url)
            .json(&json_rpc_request)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())?;
        if let Some(error) = response.error {
            return Err(error.to_string());
        }

        let Some(result) = response.result else {
            return Err("rpc response missing result".into());
        };
        if result.value.len() != account_addresses.len() {
            return Err(format!(
                "rpc returned {} accounts, expected {}",
                result.value.len(),
                account_addresses.len()
            ));
        }

        let slot = result.context.slot;
        result
            .value
            .into_iter()
            .map(|maybe_account| match maybe_account {
                None => Ok(None),
                Some(account) => Ok(Some(AccountDataAtSlot {
                    account_data: account.data.decode_base64()?,
                    slot,
                })),
            })
            .collect()
    }
}

// ── JSON shapes of the RPC response ───────────────────────────────────────
//
// {
//   "result": {
//     "context": { "slot": 123 },
//     "value": [ { "data": ["<base64>", "base64"], ... } | null, ... ]
//   },
//   "error": { ... }            // only present when the call failed
// }

#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<GetMultipleAccountsResult>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct GetMultipleAccountsResult {
    context: ResponseContext,
    value: Vec<Option<AccountInfoJson>>,
}

#[derive(Deserialize)]
struct ResponseContext {
    slot: u64,
}

#[derive(Deserialize)]
struct AccountInfoJson {
    data: Base64AccountData,
}

/// RPC returns account bytes as `["<base64 text>", "base64"]`.
#[derive(Deserialize)]
struct Base64AccountData(Vec<String>);

impl Base64AccountData {
    fn decode_base64(&self) -> Result<Vec<u8>, String> {
        let Some(base64_text) = self.0.first() else {
            return Err("account data missing base64 payload".into());
        };
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, base64_text)
            .map_err(|error| error.to_string())
    }
}
