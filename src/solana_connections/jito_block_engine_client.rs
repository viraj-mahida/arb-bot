//! HTTP JSON-RPC client for a Jito block engine.
//!
//! **Bundle:** up to 5 signed transactions that a Jito-running validator
//! executes back-to-back, all or nothing, in the order given. Ours holds one
//! transaction (both legs + tip). If it would fail, it is dropped instead of
//! landing as a failed transaction — so losing a race costs no fee.
//!
//! Methods used:
//! - `getTipAccounts` — the accounts a tip may be paid to.
//! - `sendBundle` — submit; returns a bundle id.
//!
//! Whether it landed is checked through RPC with the transaction signature,
//! the same way for both sending routes.

use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::solana_rpc_client::encode_base64;
use crate::step_3_store_latest_pool_state::{PublicKeyBytes, public_key_from_byte_slice};

const JITO_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct JitoBlockEngineClient {
    bundles_url: String,
    http_client: reqwest::Client,
}

impl JitoBlockEngineClient {
    /// `block_engine_url` like `https://mainnet.block-engine.jito.wtf`.
    pub fn new(block_engine_url: &str) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(JITO_REQUEST_TIMEOUT)
            .build()
            .expect("failed to build HTTP client for Jito");
        Self { bundles_url: format!("{block_engine_url}/api/v1/bundles"), http_client }
    }

    pub async fn get_tip_accounts(&self) -> Result<Vec<PublicKeyBytes>, String> {
        let addresses: Vec<String> = self.call("getTipAccounts", serde_json::json!([])).await?;
        Ok(addresses
            .iter()
            .filter_map(|address| bs58::decode(address).into_vec().ok())
            .filter_map(|bytes| public_key_from_byte_slice(&bytes))
            .collect())
    }

    /// Submit one signed transaction as a bundle; returns the bundle id.
    pub async fn send_bundle(&self, transaction_wire_bytes: &[u8]) -> Result<String, String> {
        self.call(
            "sendBundle",
            serde_json::json!([[encode_base64(transaction_wire_bytes)], { "encoding": "base64" }]),
        )
        .await
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, params: serde_json::Value) -> Result<T, String> {
        let request = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let response: JitoResponse<T> = self
            .http_client
            .post(&self.bundles_url)
            .json(&request)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())?;
        if let Some(error) = response.error {
            return Err(format!("jito {method}: {error}"));
        }
        response.result.ok_or_else(|| format!("jito {method}: response missing result"))
    }
}

#[derive(Deserialize)]
struct JitoResponse<T> {
    result: Option<T>,
    error: Option<serde_json::Value>,
}