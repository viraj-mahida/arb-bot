//! Simulate first, send only if allowed, then wait for the result.
//!
//! 1. **Simulate** on the RPC node. Free, and it catches almost everything:
//!    a stale price (minimum output not met), a missing account, too few
//!    compute units. A failed simulation stops here.
//! 2. **Stop** unless `SEND_TRANSACTIONS=true` (the safe default).
//! 3. **Send** through Jito as a bundle (falls back to plain RPC if Jito
//!    refuses), or through RPC when no block engine is configured.
//! 4. **Confirm** by polling the signature until it is `confirmed`, fails,
//!    or the blockhash expires. Then measure the real SOL change.

use std::time::Duration;

use super::assemble_arbitrage_transaction::SignedTransaction;
use crate::print_logs;
use crate::solana_connections::{JitoBlockEngineClient, SolanaRpcClient};
use crate::step_3_store_latest_pool_state::PublicKeyBytes;

const CONFIRMATION_POLL_INTERVAL: Duration = Duration::from_millis(500);
/// ~60 s: roughly how long a blockhash stays valid, so after this it can never land.
const CONFIRMATION_POLL_ATTEMPTS: u32 = 120;

pub struct SendingClients<'a> {
    pub rpc: &'a SolanaRpcClient,
    pub jito: Option<&'a JitoBlockEngineClient>,
}

pub async fn simulate_then_send_if_allowed(
    clients: &SendingClients<'_>,
    transaction: &SignedTransaction,
    wallet: &PublicKeyBytes,
    send_real_transactions: bool,
) {
    let balance_before = clients.rpc.get_balance(wallet).await.ok();
    let simulation = match clients
        .rpc
        .simulate_transaction(&transaction.wire_bytes, wallet)
        .await
    {
        Ok(simulation) => simulation,
        Err(error) => return print_logs::simulation_request_failed(&error),
    };
    if let Some(error) = &simulation.error {
        return print_logs::simulation_failed(error, &simulation.logs);
    }
    print_logs::simulation_succeeded(
        simulation.compute_units_consumed,
        lamports_change(balance_before, simulation.watch_address_lamports_after),
    );

    if !send_real_transactions {
        return print_logs::send_skipped_simulate_only();
    }
    if !submit(clients, transaction).await {
        return;
    }
    wait_for_confirmation(
        clients.rpc,
        &transaction.signature_base58,
        wallet,
        balance_before,
    )
    .await;
}

/// Returns `true` if some route accepted the transaction.
async fn submit(clients: &SendingClients<'_>, transaction: &SignedTransaction) -> bool {
    if let Some(jito) = clients.jito {
        match jito.send_bundle(&transaction.wire_bytes).await {
            Ok(_bundle_id) => {
                print_logs::sent("jito bundle", &transaction.signature_base58);
                return true;
            }
            Err(error) => print_logs::send_failed("jito bundle", &error),
        }
    }
    match clients.rpc.send_transaction(&transaction.wire_bytes).await {
        Ok(signature) => {
            print_logs::sent("rpc", &signature);
            true
        }
        Err(error) => {
            print_logs::send_failed("rpc", &error);
            false
        }
    }
}

async fn wait_for_confirmation(
    rpc: &SolanaRpcClient,
    signature: &str,
    wallet: &PublicKeyBytes,
    balance_before: Option<u64>,
) {
    for _ in 0..CONFIRMATION_POLL_ATTEMPTS {
        tokio::time::sleep(CONFIRMATION_POLL_INTERVAL).await;
        let Ok(Some(status)) = rpc.get_signature_status(signature).await else {
            continue;
        };
        if let Some(error) = status.error {
            return print_logs::transaction_failed_on_chain(signature, &error);
        }
        if status.confirmation_status == "confirmed" || status.confirmation_status == "finalized" {
            let balance_after = rpc.get_balance(wallet).await.ok();
            return print_logs::transaction_landed(
                signature,
                lamports_change(balance_before, balance_after),
            );
        }
    }
    print_logs::transaction_not_landed(signature);
}

fn lamports_change(before: Option<u64>, after: Option<u64>) -> Option<i128> {
    Some(i128::from(after?) - i128::from(before?))
}
