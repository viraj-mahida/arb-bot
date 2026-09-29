//! **Sub-step 6.3.** Simulate first, send only if allowed, then wait for the result.
//!
//! **Start here:** [`main_simulate_then_send_if_allowed`].
//!
//! 1. **Simulate** on the RPC node, only if `RPC_SIMULATION=true`. It catches
//!    a missing account or too few compute units, but costs a full RPC round
//!    trip, so it is off by default: the local quote already sized the trade,
//!    and the on-chain minimum-output checks revert a stale trade anyway.
//! 2. **Stop** unless `SEND_TRANSACTIONS=true` (the safe default).
//! 3. **Send** the Jito-shaped transaction (compute-unit limit + tip, no
//!    priority price) as a bundle. If that HTTP send fails, fall back to the
//!    RPC-shaped transaction (compute-unit limit + priority price, no tip).
//!    The two are never sent together. With no block engine, only the RPC
//!    shape is built.
//! 4. **Confirm** by polling the signature until it is `confirmed`, fails,
//!    or the blockhash expires. Then measure the real SOL change.

use std::time::Duration;

use super::assemble_arbitrage_transaction::SignedTransaction;
use super::decide_if_trade_is_worth_it::ApprovedArbitrageTrade;
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

/// The two signed shapes. At least one is present.
pub struct RouteTransactions<'a> {
    /// Compute-unit limit + tip, no priority price.
    pub jito: Option<&'a SignedTransaction>,
    /// Compute-unit limit + priority price, no tip.
    pub rpc: Option<&'a SignedTransaction>,
}

pub async fn main_simulate_then_send_if_allowed(
    clients: &SendingClients<'_>,
    routes: &RouteTransactions<'_>,
    wallet: &PublicKeyBytes,
    trade: &ApprovedArbitrageTrade,
    simulate_on_rpc: bool,
    send_real_transactions: bool,
) {
    if !simulate_on_rpc && !send_real_transactions {
        return print_logs::send_skipped_simulate_only(trade);
    }
    let Some(primary) = routes.jito.or(routes.rpc) else {
        return print_logs::trade_build_failed("no transaction to send");
    };

    let balance_before = if simulate_on_rpc {
        let balance_before = clients.rpc.get_balance(wallet).await.ok();
        let simulation = match clients
            .rpc
            .simulate_transaction(&primary.wire_bytes, wallet)
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
            return print_logs::send_skipped_simulate_only(trade);
        }
        balance_before
    } else {
        // Read concurrently so the balance lookup never delays the send; the
        // transaction cannot land before this read is served.
        let (balance_before, accepted) = tokio::join!(
            clients.rpc.get_balance(wallet),
            submit(clients, routes, trade)
        );
        let Some(accepted) = accepted else {
            return;
        };
        return wait_for_confirmation(
            clients.rpc,
            &accepted.signature_base58,
            wallet,
            balance_before.ok(),
            trade,
        )
        .await;
    };

    let Some(accepted) = submit(clients, routes, trade).await else {
        return;
    };
    wait_for_confirmation(
        clients.rpc,
        &accepted.signature_base58,
        wallet,
        balance_before,
        trade,
    )
    .await;
}

/// Returns the transaction some route accepted.
///
/// Jito is tried first. RPC runs only when there is no block engine, or when
/// `sendBundle` itself fails — not when a bundle is merely dropped later.
async fn submit<'a>(
    clients: &SendingClients<'_>,
    routes: &RouteTransactions<'a>,
    trade: &ApprovedArbitrageTrade,
) -> Option<&'a SignedTransaction> {
    if let (Some(jito), Some(transaction)) = (clients.jito, routes.jito) {
        match jito.send_bundle(&transaction.wire_bytes).await {
            Ok(_bundle_id) => {
                print_logs::sent("jito bundle", &transaction.signature_base58, trade);
                return Some(transaction);
            }
            Err(error) => print_logs::send_failed(
                "jito bundle",
                &error,
                trade,
                Some(&transaction.signature_base58),
            ),
        }
    }
    let transaction = routes.rpc?;
    match clients.rpc.send_transaction(&transaction.wire_bytes).await {
        Ok(signature) => {
            print_logs::sent("rpc", &signature, trade);
            Some(transaction)
        }
        Err(error) => {
            print_logs::send_failed("rpc", &error, trade, Some(&transaction.signature_base58));
            None
        }
    }
}

async fn wait_for_confirmation(
    rpc: &SolanaRpcClient,
    signature: &str,
    wallet: &PublicKeyBytes,
    balance_before: Option<u64>,
    trade: &ApprovedArbitrageTrade,
) {
    for _ in 0..CONFIRMATION_POLL_ATTEMPTS {
        tokio::time::sleep(CONFIRMATION_POLL_INTERVAL).await;
        let Ok(Some(status)) = rpc.get_signature_status(signature).await else {
            continue;
        };
        if let Some(error) = status.error {
            return print_logs::transaction_failed_on_chain(signature, &error, trade);
        }
        if status.confirmation_status == "confirmed" || status.confirmation_status == "finalized" {
            let balance_after = rpc.get_balance(wallet).await.ok();
            return print_logs::transaction_landed(
                signature,
                lamports_change(balance_before, balance_after),
                trade,
            );
        }
    }
    print_logs::transaction_not_landed(signature, trade);
}

fn lamports_change(before: Option<u64>, after: Option<u64>) -> Option<i128> {
    Some(i128::from(after?) - i128::from(before?))
}
