//! **Sub-step 6.4.** Glue for Step 6: set up once at startup, then decide → build → simulate → send.
//!
//! **Start here:** [`ArbitrageTradeExecutor::prepare`] (startup) and
//! [`ArbitrageTradeExecutor::main_consider_trading`] (every pool update).
//!
//! **Single flight while building:** only one trade is built and submitted at
//! a time. The lock drops once a route accepts the transaction, not after the
//! confirmation poll (that poll can run for about 60 seconds). A one-second
//! pause then stops the same still-cached quote from being sent again on the
//! next pool update. Confirmation keeps running in the background.
//!
//! **Background task:** building, simulating, and confirming take network
//! round trips (hundreds of milliseconds). The recent blockhash is not one of
//! them: `blocks_meta` on the same Geyser subscription fills a cache, replaced
//! only when a newer slot arrives. The rest runs in a spawned tokio task so
//! the Geyser loop keeps consuming updates and the pool cache stays fresh.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use solana_message::AddressLookupTableAccount;

use super::assemble_arbitrage_transaction::{
    FundingSource, TransactionFeeSettings, compile_and_sign_v0_transaction,
    decode_address_lookup_table, main_arbitrage_instructions,
};
use super::decide_if_trade_is_worth_it::{
    ApprovedArbitrageTrade, CacheFreshness, TradeDecisionRules, main_decide_if_trade_is_worth_it,
};
use super::flash_loan_instructions::{
    FlashLoanProvider, JupiterFlashLoanAccounts, KaminoFlashLoanAccounts,
};
use super::jito_tip_instruction::{default_jito_tip_accounts, pick_tip_account};
use super::send_and_confirm::{
    RouteTransactions, SendingClients, main_simulate_then_send_if_allowed, wait_for_confirmation,
};
use super::trading_wallet::TradingWallet;
use crate::bot_settings::{BotSettingsFromEnvironment, FlashLoanLender, FundingMode};
use crate::print_logs;
use crate::solana_connections::{JitoBlockEngineClient, RecentBlockhashCache, SolanaRpcClient};
use crate::step_3_store_latest_pool_state::{LatestPoolStateCache, PublicKeyBytes};
use crate::step_4_quote_swaps::{CachedPoolWithTickArrays, pool_label};

pub struct ArbitrageTradeExecutor {
    settings: BotSettingsFromEnvironment,
    rules: TradeDecisionRules,
    wallet: TradingWallet,
    rpc: SolanaRpcClient,
    blockhash_cache: RecentBlockhashCache,
    jito: Option<JitoBlockEngineClient>,
    jito_tip_accounts: Vec<PublicKeyBytes>,
    flash_loan_provider: Option<FlashLoanProvider>,
    address_lookup_tables: Vec<AddressLookupTableAccount>,
    trade_in_flight: AtomicBool,
    /// Earliest time another trade may be submitted. Set when a send is accepted.
    next_submit_at: std::sync::Mutex<Instant>,
}

/// After a send is accepted, ignore new trades this long. Long enough that the
/// next pool update does not resend the same quote; short enough that a later
/// gap is not skipped for the whole confirmation poll.
const AFTER_SUBMIT_COOLDOWN: Duration = Duration::from_secs(1);

impl ArbitrageTradeExecutor {
    /// Load the wallet and every on-chain account trading needs.
    ///
    /// `Ok(None)` = trading not configured (no `WALLET_KEYPAIR_PATH`); the bot
    /// keeps watching. `Err` = configured but broken; better to say so loudly.
    pub async fn prepare(
        settings: BotSettingsFromEnvironment,
        rpc: &SolanaRpcClient,
    ) -> Result<Option<Arc<Self>>, String> {
        let Some(keypair_path) = settings.wallet_keypair_path.clone() else {
            return Ok(None);
        };
        let wallet = TradingWallet::load_from_keypair_file(&keypair_path)?;

        let flash_loan_provider = match &settings.funding_mode {
            FundingMode::OwnWallet => None,
            FundingMode::FlashLoan(flash) => match &flash.lender {
                FlashLoanLender::Kamino {
                    lending_market_address,
                    sol_reserve_address,
                } => {
                    let reserve_bytes = fetch_one_account(rpc, *sol_reserve_address).await?;
                    Some(FlashLoanProvider::Kamino(
                        KaminoFlashLoanAccounts::from_reserve_account_bytes(
                            *lending_market_address,
                            *sol_reserve_address,
                            &reserve_bytes,
                        )?,
                    ))
                }
                FlashLoanLender::Jupiter => {
                    let admin_bytes =
                        fetch_one_account(rpc, JupiterFlashLoanAccounts::flashloan_admin_address())
                            .await?;
                    let accounts =
                        JupiterFlashLoanAccounts::from_admin_account_bytes(&admin_bytes)?;
                    // The cost model was built from JUPITER_FLASH_LOAN_FEE_BPS; a higher
                    // live fee would make every profit estimate too optimistic.
                    if u64::from(accounts.fee_in_basis_points)
                        > flash.flash_loan_fee_in_basis_points
                    {
                        return Err(format!(
                            "Jupiter flash-loan fee is now {} bps; set JUPITER_FLASH_LOAN_FEE_BPS to at least that",
                            accounts.fee_in_basis_points
                        ));
                    }
                    Some(FlashLoanProvider::Jupiter(accounts))
                }
            },
        };

        let mut address_lookup_tables = Vec::new();
        for &table_address in &settings.address_lookup_table_addresses {
            let table_bytes = fetch_one_account(rpc, table_address).await?;
            address_lookup_tables.push(
                decode_address_lookup_table(table_address, &table_bytes)
                    .ok_or("address lookup table account too short")?,
            );
        }

        let jito = settings
            .jito_block_engine_url
            .as_deref()
            .map(JitoBlockEngineClient::new);
        let jito_tip_accounts = match &jito {
            Some(client) => client
                .get_tip_accounts()
                .await
                .ok()
                .filter(|accounts| !accounts.is_empty()),
            None => None,
        }
        .unwrap_or_else(default_jito_tip_accounts);

        let blockhash_cache = RecentBlockhashCache::new();
        print_logs::trading_ready(&settings, &wallet.address(), address_lookup_tables.len());
        if let Ok(lamports) = rpc.get_balance(&wallet.address()).await {
            print_logs::wallet_balance(lamports);
        } else {
            print_logs::wallet_balance_unreadable();
        }
        Ok(Some(Arc::new(Self {
            rules: TradeDecisionRules::from_settings(&settings),
            settings,
            wallet,
            rpc: rpc.clone(),
            blockhash_cache,
            jito,
            jito_tip_accounts,
            flash_loan_provider,
            address_lookup_tables,
            trade_in_flight: AtomicBool::new(false),
            next_submit_at: std::sync::Mutex::new(Instant::now()),
        })))
    }

    /// Write a `blocks_meta` update from the shared Geyser stream into the cache.
    pub fn note_blockhash(&self, slot: u64, blockhash: &str) {
        self.blockhash_cache.store(slot, blockhash);
    }

    pub fn blockhash_cache(&self) -> &RecentBlockhashCache {
        &self.blockhash_cache
    }

    /// Called after every pool update: check both directions against every
    /// pool sharing the updated pool's mint pair, and start the most profitable
    /// approved trade in the background if no trade is running.
    pub fn main_consider_trading(
        self: &Arc<Self>,
        cache: &LatestPoolStateCache,
        updated_pool_address: &PublicKeyBytes,
    ) {
        let Some(updated_pool) = cache.pool_state_by_address(updated_pool_address) else {
            return;
        };
        let updated_pool = CachedPoolWithTickArrays::from_cache(cache, updated_pool);
        let freshness = CacheFreshness {
            newest_slot_seen_from_stream: cache.newest_slot_seen_from_stream(),
            milliseconds_since_last_stream_update: cache.milliseconds_since_last_stream_update(),
        };

        let mut best_trade: Option<ApprovedArbitrageTrade> = None;
        for other_pool in cache.other_pools_with_same_mint_pair(updated_pool_address) {
            let other_pool = CachedPoolWithTickArrays::from_cache(cache, other_pool);
            for (sell, buy) in [(&updated_pool, &other_pool), (&other_pool, &updated_pool)] {
                let direction_label =
                    format!("{}→{}", pool_label(&sell.pool), pool_label(&buy.pool));
                match main_decide_if_trade_is_worth_it(
                    &sell.pool,
                    &sell.tick_arrays,
                    &buy.pool,
                    &buy.tick_arrays,
                    freshness,
                    &self.rules,
                ) {
                    Ok(trade) => {
                        print_logs::trade_approved(&trade);
                        let beats_best = best_trade.as_ref().is_none_or(|best| {
                            trade.expected_profit_after_costs > best.expected_profit_after_costs
                        });
                        if beats_best {
                            best_trade = Some(trade);
                        }
                    }
                    Err(reason) => print_logs::trade_skipped(&direction_label, &reason),
                }
            }
        }
        if let Some(trade) = best_trade {
            self.start_trade_unless_one_is_running(trade);
        }
    }

    fn start_trade_unless_one_is_running(self: &Arc<Self>, trade: ApprovedArbitrageTrade) {
        if let Ok(next_submit_at) = self.next_submit_at.lock()
            && Instant::now() < *next_submit_at
        {
            return print_logs::trade_on_cooldown();
        }
        if self
            .trade_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return print_logs::trade_already_in_flight();
        }
        let executor = Arc::clone(self);
        tokio::spawn(async move {
            let submitted = executor.build_and_send(&trade).await;
            if submitted.is_some()
                && let Ok(mut next_submit_at) = executor.next_submit_at.lock()
            {
                *next_submit_at = Instant::now() + AFTER_SUBMIT_COOLDOWN;
            }
            executor.trade_in_flight.store(false, Ordering::Release);
            if let Some(submitted) = submitted {
                wait_for_confirmation(
                    &executor.rpc,
                    &submitted.signature_base58,
                    &executor.wallet.address(),
                    submitted.balance_before,
                    &trade,
                )
                .await;
            }
        });
    }

    /// `Some` when a route accepted the transaction. Confirmation is separate.
    async fn build_and_send(
        &self,
        trade: &ApprovedArbitrageTrade,
    ) -> Option<super::send_and_confirm::SubmittedTransaction> {
        let recent_blockhash = match self.blockhash_cache.fresh_hash() {
            Some(hash) => hash,
            None => match self.rpc.get_latest_blockhash().await {
                Ok(blockhash) => {
                    print_logs::blockhash_cache_stale_using_rpc();
                    match blockhash.parse::<solana_hash::Hash>() {
                        Ok(hash) => hash,
                        Err(_) => {
                            print_logs::trade_build_failed("RPC returned an invalid blockhash");
                            return None;
                        }
                    }
                }
                Err(error) => {
                    print_logs::trade_build_failed(&error);
                    return None;
                }
            },
        };

        let funding = match &self.flash_loan_provider {
            None => FundingSource::OwnWallet,
            Some(provider) => FundingSource::FlashLoan(provider),
        };
        let sign = |fees: TransactionFeeSettings| {
            let instructions = main_arbitrage_instructions(&self.wallet, trade, &funding, &fees);
            compile_and_sign_v0_transaction(
                &self.wallet,
                &instructions,
                &self.address_lookup_tables,
                recent_blockhash.clone(),
            )
        };

        let jito_transaction = match self
            .jito
            .as_ref()
            .and_then(|_| pick_tip_account(&self.jito_tip_accounts))
        {
            Some(tip_account) => match sign(TransactionFeeSettings {
                compute_unit_limit: self.settings.compute_unit_limit,
                priority_fee_micro_lamports_per_compute_unit: 0,
                jito_tip: Some((tip_account, self.settings.jito_tip_lamports)),
            }) {
                Ok(transaction) => Some(transaction),
                Err(error) => {
                    print_logs::trade_build_failed(&error);
                    return None;
                }
            },
            None => None,
        };
        let rpc_transaction = match sign(TransactionFeeSettings {
            compute_unit_limit: self.settings.compute_unit_limit,
            priority_fee_micro_lamports_per_compute_unit: self
                .settings
                .priority_fee_micro_lamports_per_compute_unit,
            jito_tip: None,
        }) {
            Ok(transaction) => transaction,
            Err(error) => {
                print_logs::trade_build_failed(&error);
                return None;
            }
        };

        let clients = SendingClients {
            rpc: &self.rpc,
            jito: self.jito.as_ref(),
        };
        let routes = RouteTransactions {
            jito: jito_transaction.as_ref(),
            rpc: Some(&rpc_transaction),
        };
        main_simulate_then_send_if_allowed(
            &clients,
            &routes,
            &self.wallet.address(),
            trade,
            self.settings.simulate_on_rpc_before_sending,
            self.settings.send_real_transactions,
        )
        .await
    }
}

async fn fetch_one_account(
    rpc: &SolanaRpcClient,
    address: PublicKeyBytes,
) -> Result<Vec<u8>, String> {
    rpc.get_multiple_accounts(&[address])
        .await?
        .into_iter()
        .next()
        .flatten()
        .map(|account| account.account_data)
        .ok_or_else(|| {
            format!(
                "account {} not found",
                crate::step_3_store_latest_pool_state::encode_public_key_as_base58(&address)
            )
        })
}
