//! HTTP JSON-RPC client for a Solana RPC node.
//!
//! **Reading:** `getMultipleAccounts` loads account bytes once (tick arrays,
//! Raydium fee configs, the Kamino reserve, lookup tables), because Geyser only
//! streams changes that happen after we subscribe.
//!
//! **Trading (Step 6):**
//! - `getLatestBlockhash` — every transaction must name a recent blockhash
//!   (~60–90 s validity) so validators know it is fresh and not a replay.
//! - `simulateTransaction` — dry-run on the node: would it succeed, how many
//!   compute units, what did the programs log. Costs nothing.
//! - `sendTransaction` — submit it to the network for real.
//! - `getSignatureStatuses` — poll whether it landed, and if it failed.
//! - `getBalance` — the wallet's SOL, to measure real profit.

use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::step_3_store_latest_pool_state::{PublicKeyBytes, encode_public_key_as_base58};

/// Give up on a request after this long, so a stuck node cannot freeze the bot.
const RPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Connection to one Solana RPC node. Cloning is cheap (shares the HTTP connection pool).
#[derive(Clone)]
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
        Self {
            rpc_url,
            http_client,
        }
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

        let addresses_base58: Vec<String> = account_addresses
            .iter()
            .map(encode_public_key_as_base58)
            .collect();
        let result: GetMultipleAccountsResult = self
            .call(
                "getMultipleAccounts",
                serde_json::json!([addresses_base58, { "encoding": "base64", "commitment": "processed" }]),
            )
            .await?;
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

    /// A recent blockhash (base58) to stamp on a new transaction.
    pub async fn get_latest_blockhash(&self) -> Result<String, String> {
        let result: ValueWithContext<LatestBlockhashJson> = self
            .call(
                "getLatestBlockhash",
                serde_json::json!([{ "commitment": "confirmed" }]),
            )
            .await?;
        Ok(result.value.blockhash)
    }

    /// Current slot at `confirmed` commitment (lookup-table creation needs a recent one).
    pub async fn get_slot(&self) -> Result<u64, String> {
        self.call("getSlot", serde_json::json!([{ "commitment": "confirmed" }]))
            .await
    }

    /// SOL balance of an address, in lamports.
    pub async fn get_balance(&self, address: &PublicKeyBytes) -> Result<u64, String> {
        let result: ValueWithContext<u64> = self
            .call(
                "getBalance",
                serde_json::json!([encode_public_key_as_base58(address), { "commitment": "confirmed" }]),
            )
            .await?;
        Ok(result.value)
    }

    /// Dry-run a signed transaction. Also returns `watch_address`'s lamports
    /// *after* the simulated run, so the caller can compute the balance change.
    pub async fn simulate_transaction(
        &self,
        transaction_wire_bytes: &[u8],
        watch_address: &PublicKeyBytes,
    ) -> Result<SimulationOutcome, String> {
        let result: ValueWithContext<SimulationJson> = self
            .call(
                "simulateTransaction",
                serde_json::json!([
                    encode_base64(transaction_wire_bytes),
                    {
                        "encoding": "base64",
                        "commitment": "processed",
                        "sigVerify": false,
                        "replaceRecentBlockhash": false,
                        "accounts": { "encoding": "base64", "addresses": [encode_public_key_as_base58(watch_address)] }
                    }
                ]),
            )
            .await?;
        let simulation = result.value;
        Ok(SimulationOutcome {
            error: simulation
                .err
                .filter(|error| !error.is_null())
                .map(|error| error.to_string()),
            logs: simulation.logs.unwrap_or_default(),
            compute_units_consumed: simulation.units_consumed,
            watch_address_lamports_after: simulation
                .accounts
                .and_then(|accounts| accounts.into_iter().next().flatten())
                .map(|account| account.lamports),
        })
    }

    /// Submit a signed transaction; returns its signature (transaction id).
    ///
    /// `skipPreflight` because we already simulated; `maxRetries: 0` because a
    /// stale arbitrage should die, not be rebroadcast after the gap has closed.
    pub async fn send_transaction(&self, transaction_wire_bytes: &[u8]) -> Result<String, String> {
        self.call(
            "sendTransaction",
            serde_json::json!([
                encode_base64(transaction_wire_bytes),
                { "encoding": "base64", "skipPreflight": true, "maxRetries": 0 }
            ]),
        )
        .await
    }

    /// Status of one signature: `None` = not seen yet.
    pub async fn get_signature_status(
        &self,
        signature_base58: &str,
    ) -> Result<Option<SignatureStatus>, String> {
        let result: ValueWithContext<Vec<Option<SignatureStatusJson>>> = self
            .call(
                "getSignatureStatuses",
                serde_json::json!([[signature_base58]]),
            )
            .await?;
        Ok(result
            .value
            .into_iter()
            .next()
            .flatten()
            .map(|status| SignatureStatus {
                error: status
                    .err
                    .filter(|error| !error.is_null())
                    .map(|error| error.to_string()),
                confirmation_status: status.confirmation_status.unwrap_or_default(),
            }))
    }

    /// Send one JSON-RPC request and decode its `result`.
    async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, String> {
        let request =
            serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let response: JsonRpcResponse<T> = self
            .http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())?;
        if let Some(error) = response.error {
            return Err(format!("{method}: {error}"));
        }
        response
            .result
            .ok_or_else(|| format!("{method}: response missing result"))
    }
}

/// What `simulateTransaction` reported.
pub struct SimulationOutcome {
    /// `None` = the transaction would succeed.
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub compute_units_consumed: Option<u64>,
    pub watch_address_lamports_after: Option<u64>,
}

/// What `getSignatureStatuses` reported for one transaction.
pub struct SignatureStatus {
    /// `None` = it executed successfully.
    pub error: Option<String>,
    /// `processed`, `confirmed`, or `finalized`.
    pub confirmation_status: String,
}

pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
}

// ── JSON shapes of the RPC responses ──────────────────────────────────────
//
// {
//   "result": { "context": { "slot": 123 }, "value": … },
//   "error": { ... }            // only present when the call failed
// }

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ValueWithContext<T> {
    value: T,
}

#[derive(Deserialize)]
struct LatestBlockhashJson {
    blockhash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimulationJson {
    err: Option<serde_json::Value>,
    logs: Option<Vec<String>>,
    units_consumed: Option<u64>,
    accounts: Option<Vec<Option<AccountLamportsJson>>>,
}

#[derive(Deserialize)]
struct AccountLamportsJson {
    lamports: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignatureStatusJson {
    err: Option<serde_json::Value>,
    confirmation_status: Option<String>,
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
