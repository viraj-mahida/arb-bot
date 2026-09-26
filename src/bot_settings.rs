//! Every knob the trading part of the bot reads from the environment (`.env`).
//!
//! **Why settings live in one place:** trading touches real money. Having every
//! limit, fee, and on/off switch listed in one struct makes it easy to see — and
//! to review — exactly what the bot is allowed to do before you run it.
//!
//! **Safe by default:** with no settings at all, the bot only *watches* and
//! *simulates*. It sends a real transaction only when `SEND_TRANSACTIONS=true`.
//!
//! | Variable                            | Default                                 | Meaning |
//! |-------------------------------------|-----------------------------------------|---------|
//! | `WALLET_KEYPAIR_PATH`               | *(none → trading disabled)*             | JSON keypair file (same format as `solana-keygen`) |
//! | `FUNDING_MODE`                      | `wallet`                                | `wallet` = trade your own SOL, `flash_loan` = borrow it from Kamino |
//! | `SEND_TRANSACTIONS`                 | `false`                                 | `false` = simulate only, `true` = really send |
//! | `MAX_TRADE_INPUT_LAMPORTS`          | `1000000000` (1 SOL)                    | Largest start amount ever put into leg 1 |
//! | `MIN_PROFIT_LAMPORTS`               | `10000`                                 | Profit that must remain *after* all costs |
//! | `PRIORITY_FEE_MICROLAMPORTS_PER_CU` | `10000`                                 | Priority fee price per compute unit |
//! | `COMPUTE_UNIT_LIMIT`                | `400000`                                | Compute units requested for the transaction |
//! | `JITO_BLOCK_ENGINE_URL`             | `https://mainnet.block-engine.jito.wtf` | Where bundles are sent; empty = send via RPC only |
//! | `JITO_TIP_LAMPORTS`                 | `10000`                                 | Tip paid to the Jito block builder |
//! | `MAX_STATE_AGE_SLOTS`               | `150`                                   | Pool state older than this (vs newest slot seen) is stale |
//! | `MAX_STREAM_SILENCE_MS`             | `2000`                                  | If Geyser has been silent this long, all state is stale |
//! | `SLIPPAGE_TOLERANCE_BPS`            | `0`                                     | How much less bridge token leg 1 may return (1 bp = 0.01%) |
//! | `KAMINO_LENDING_MARKET`             | *(required for `flash_loan`)*           | Kamino lending market that owns the reserve |
//! | `KAMINO_SOL_RESERVE`                | *(required for `flash_loan`)*           | Kamino reserve that lends wrapped SOL |
//! | `FLASH_LOAN_FEE_BPS`                | `10`                                    | Flash-loan fee used in the cost estimate; check the reserve's config |
//! | `ADDRESS_LOOKUP_TABLES`             | *(empty)*                               | Comma-separated lookup-table addresses to shrink transactions |

use crate::step_3_store_latest_pool_state::{PublicKeyBytes, parse_base58_public_key};

/// Where the start token for leg 1 comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FundingMode {
    /// Use SOL already sitting in the bot's wallet. Simple, but the trade size
    /// is limited by the wallet balance.
    OwnWallet,
    /// Borrow wrapped SOL from a lending protocol at the start of the
    /// transaction and repay it (plus a small fee) at the end. If the repay
    /// fails, the whole transaction reverts, so the lender never takes risk.
    FlashLoan(KaminoFlashLoanSettings),
}

/// Which Kamino lending reserve to flash-borrow from.
///
/// **What Kamino is:** a lending protocol on Solana. Each *reserve* is a pool
/// of one token that lenders deposit into; each *lending market* groups reserves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KaminoFlashLoanSettings {
    pub lending_market_address: PublicKeyBytes,
    pub sol_reserve_address: PublicKeyBytes,
    /// Fee in basis points. Only used to *estimate* costs; the program charges
    /// its own configured fee, and the leg-2 minimum output protects us either way.
    pub flash_loan_fee_in_basis_points: u64,
}

/// All trading settings, read once at startup.
#[derive(Debug, Clone)]
pub struct BotSettingsFromEnvironment {
    pub wallet_keypair_path: Option<String>,
    pub funding_mode: FundingMode,
    pub send_real_transactions: bool,
    pub max_trade_input_lamports: u64,
    pub min_profit_after_costs_lamports: u64,
    pub priority_fee_micro_lamports_per_compute_unit: u64,
    pub compute_unit_limit: u32,
    pub jito_block_engine_url: Option<String>,
    pub jito_tip_lamports: u64,
    pub max_pool_state_age_in_slots: u64,
    pub max_milliseconds_since_last_stream_update: u64,
    pub slippage_tolerance_in_basis_points: u64,
    pub address_lookup_table_addresses: Vec<PublicKeyBytes>,
}

impl BotSettingsFromEnvironment {
    /// Read every setting; panics with a clear message on a malformed value,
    /// because guessing what a trading limit was meant to be is worse than stopping.
    pub fn from_env() -> Self {
        let funding_mode = match read_text("FUNDING_MODE").as_deref() {
            None | Some("wallet") => FundingMode::OwnWallet,
            Some("flash_loan") => FundingMode::FlashLoan(KaminoFlashLoanSettings {
                lending_market_address: read_required_public_key("KAMINO_LENDING_MARKET"),
                sol_reserve_address: read_required_public_key("KAMINO_SOL_RESERVE"),
                flash_loan_fee_in_basis_points: read_number("FLASH_LOAN_FEE_BPS", 10),
            }),
            Some(other) => panic!("FUNDING_MODE must be 'wallet' or 'flash_loan', got '{other}'"),
        };
        Self {
            wallet_keypair_path: read_text("WALLET_KEYPAIR_PATH"),
            funding_mode,
            send_real_transactions: read_bool("SEND_TRANSACTIONS", false),
            max_trade_input_lamports: read_number("MAX_TRADE_INPUT_LAMPORTS", 1_000_000_000),
            min_profit_after_costs_lamports: read_number("MIN_PROFIT_LAMPORTS", 10_000),
            priority_fee_micro_lamports_per_compute_unit: read_number(
                "PRIORITY_FEE_MICROLAMPORTS_PER_CU",
                10_000,
            ),
            compute_unit_limit: read_number("COMPUTE_UNIT_LIMIT", 400_000),
            jito_block_engine_url: match std::env::var("JITO_BLOCK_ENGINE_URL") {
                Ok(url) if url.trim().is_empty() => None,
                Ok(url) => Some(url.trim().trim_end_matches('/').to_string()),
                Err(_) => Some("https://mainnet.block-engine.jito.wtf".to_string()),
            },
            jito_tip_lamports: read_number("JITO_TIP_LAMPORTS", 10_000),
            max_pool_state_age_in_slots: read_number("MAX_STATE_AGE_SLOTS", 150),
            max_milliseconds_since_last_stream_update: read_number("MAX_STREAM_SILENCE_MS", 2_000),
            slippage_tolerance_in_basis_points: read_number("SLIPPAGE_TOLERANCE_BPS", 0),
            address_lookup_table_addresses: read_text("ADDRESS_LOOKUP_TABLES")
                .map(|list| {
                    list.split(',')
                        .map(str::trim)
                        .filter(|address| !address.is_empty())
                        .map(parse_base58_public_key)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Flash-loan fee in basis points, or 0 when trading from the wallet.
    pub fn flash_loan_fee_in_basis_points(&self) -> u64 {
        match &self.funding_mode {
            FundingMode::OwnWallet => 0,
            FundingMode::FlashLoan(kamino) => kamino.flash_loan_fee_in_basis_points,
        }
    }
}

fn read_text(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_number<T: std::str::FromStr>(name: &str, default: T) -> T {
    match read_text(name) {
        None => default,
        Some(text) => text
            .replace('_', "")
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a whole number, got '{text}'")),
    }
}

fn read_bool(name: &str, default: bool) -> bool {
    match read_text(name).as_deref() {
        None => default,
        Some("true" | "1" | "yes") => true,
        Some("false" | "0" | "no") => false,
        Some(other) => panic!("{name} must be true or false, got '{other}'"),
    }
}

fn read_required_public_key(name: &str) -> PublicKeyBytes {
    let text = read_text(name)
        .unwrap_or_else(|| panic!("{name} is required when FUNDING_MODE=flash_loan"));
    parse_base58_public_key(&text)
}
