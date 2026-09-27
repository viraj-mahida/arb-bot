//! **Sub-step 6.4.** Glue for Step 6: set up once at startup, then decide → build → simulate → send.
//!
//! **Start here:** [`ArbitrageTradeExecutor::prepare`] (startup) and
//! [`ArbitrageTradeExecutor::main_consider_trading`] (every pool update).
//!
//! **Single flight:** only one trade runs at a time. Pool updates arrive many
//! times per second; without this guard the bot could fire a second trade
//! with the same SOL before the first one finished, or trade against state the
//! first trade is about to change.
//!
//! **Background task:** building, simulating, and confirming take network
//! round trips (hundreds of milliseconds). They run in a spawned tokio task so
//! the Geyser loop keeps consuming updates and the cache stays fresh.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use solana_message::AddressLookupTableAccount;

use super::assemble_arbitrage_transaction::{
    FundingSource, TransactionFeeSettings, compile_and_sign_v0_transaction,
    decode_address_lookup_table, main_arbitrage_instructions,
};
use super::decide_if_trade_is_worth_it::{
    ApprovedArbitrageTrade, CacheFreshness, TradeDecisionRules, main_decide_if_trade_is_worth_it,
};
use super::flash_loan_instructions::{FlashLoanProvider, KaminoFlashLoanAccounts};
use super::jito_tip_instruction::{default_jito_tip_accounts, pick_tip_account};
use super::send_and_confirm::{
    RouteTransactions, SendingClients, main_simulate_then_send_if_allowed,
};
use super::trading_wallet::TradingWallet;
use crate::bot_settings::{BotSettingsFromEnvironment, FundingMode};
use crate::print_logs;
use crate::solana_connections::{JitoBlockEngineClient, SolanaRpcClient};
use crate::step_3_store_latest_pool_state::{LatestPoolStateCache, PublicKeyBytes};
use crate::step_4_quote_swaps::{CachedPoolWithTickArrays, pool_label};

pub struct ArbitrageTradeExecutor {
    settings: BotSettingsFromEnvironment,
    rules: TradeDecisionRules,
    wallet: TradingWallet,
    rpc: SolanaRpcClient,
    jito: Option<JitoBlockEngineClient>,
    jito_tip_accounts: Vec<PublicKeyBytes>,
    flash_loan_provider: Option<FlashLoanProvider>,
    address_lookup_tables: Vec<AddressLookupTableAccount>,
    trade_in_flight: AtomicBool,
}

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
            FundingMode::FlashLoan(kamino) => {
                let reserve_bytes = fetch_one_account(rpc, kamino.sol_reserve_address).await?;
                Some(FlashLoanProvider::Kamino(
                    KaminoFlashLoanAccounts::from_reserve_account_bytes(
                        kamino.lending_market_address,
                        kamino.sol_reserve_address,
                        &reserve_bytes,
                    )?,
                ))
            }
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

        print_logs::trading_ready(&settings, &wallet.address(), address_lookup_tables.len());
        Ok(Some(Arc::new(Self {
            rules: TradeDecisionRules::from_settings(&settings),
            settings,
            wallet,
            rpc: rpc.clone(),
            jito,
            jito_tip_accounts,
            flash_loan_provider,
            address_lookup_tables,
            trade_in_flight: AtomicBool::new(false),
        })))
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
        if self
            .trade_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return print_logs::trade_already_in_flight();
        }
        let executor = Arc::clone(self);
        tokio::spawn(async move {
            executor.build_and_send(&trade).await;
            executor.trade_in_flight.store(false, Ordering::Release);
        });
    }

    async fn build_and_send(&self, trade: &ApprovedArbitrageTrade) {
        let recent_blockhash = match self.rpc.get_latest_blockhash().await {
            Ok(blockhash) => blockhash,
            Err(error) => return print_logs::trade_build_failed(&error),
        };
        let Ok(recent_blockhash) = recent_blockhash.parse::<solana_hash::Hash>() else {
            return print_logs::trade_build_failed("RPC returned an invalid blockhash");
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
                Err(error) => return print_logs::trade_build_failed(&error),
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
            Err(error) => return print_logs::trade_build_failed(&error),
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
            self.settings.simulate_on_rpc_before_sending,
            self.settings.send_real_transactions,
        )
        .await;
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
