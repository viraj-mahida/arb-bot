//! The list of pools the bot watches, and the static facts about each one.
//!
//! **Why it exists:** before we can listen to anything, we must tell Geyser
//! *which* accounts to stream. This list is that answer. It also holds facts
//! that are not stored inside the pool account itself (for example the token
//! decimals, which live on separate mint accounts).
//!
//! **Future:** production bots watch hundreds of pools across many token pairs.
//! That only needs a new constructor that builds a longer list; the lookup by
//! address below already works for any number of pools.

use std::collections::HashMap;

use super::known_program_and_pool_addresses::{
    ORCA_WHIRLPOOL_SOL_USDC_POOL_ADDRESS, RAYDIUM_CLMM_SOL_USDC_POOL_ADDRESS, SOL_DECIMALS,
    SOL_USDC_POOL_FEE_IN_BASIS_POINTS, USDC_DECIMALS,
};
use super::shared_pool_types::DexProgram;
use super::solana_public_key_helpers::{PublicKeyBytes, parse_base58_public_key};

/// Static facts about one pool we watch.
#[derive(Debug, Clone)]
pub struct WatchedPoolConfig {
    /// Pool address as raw bytes (for fast lookups).
    pub pool_address: PublicKeyBytes,
    /// Same pool address as a base58 string (for Geyser subscriptions and logs).
    pub pool_address_base58: &'static str,
    /// Which DEX program owns the pool.
    pub dex: DexProgram,
    /// Decimals of token A (SOL = 9).
    pub token_a_decimals: u8,
    /// Decimals of token B (USDC = 6).
    pub token_b_decimals: u8,
    /// Swap fee in basis points (1 bp = 0.01%).
    pub fee_in_basis_points: u16,
}

impl WatchedPoolConfig {
    fn new(
        pool_address_base58: &'static str,
        dex: DexProgram,
        token_a_decimals: u8,
        token_b_decimals: u8,
        fee_in_basis_points: u16,
    ) -> Self {
        Self {
            pool_address: parse_base58_public_key(pool_address_base58),
            pool_address_base58,
            dex,
            token_a_decimals,
            token_b_decimals,
            fee_in_basis_points,
        }
    }

    /// The same fee in the unit the DEX programs use: millionths of the input.
    ///
    /// 1 basis point = 1/10,000 = 100/1,000,000, so we multiply by 100.
    /// Example: 4 bps × 100 = 400 millionths = 0.04%.
    pub fn fee_rate_in_millionths(&self) -> u16 {
        self.fee_in_basis_points.saturating_mul(100)
    }
}

/// All pools we watch, with a fast "address → config" lookup.
///
/// A `Vec` keeps a stable order (so logs always list pools the same way) and a
/// `HashMap` from address to position gives instant lookup when a Geyser
/// update arrives.
#[derive(Debug, Clone)]
pub struct WatchedPools {
    configs_in_display_order: Vec<WatchedPoolConfig>,
    position_by_pool_address: HashMap<PublicKeyBytes, usize>,
}

impl WatchedPools {
    /// Today's setup: one SOL/USDC pool on Raydium CLMM and one on Orca Whirlpool.
    pub fn sol_usdc_pools_on_orca_and_raydium() -> Self {
        Self::from_configs(vec![
            WatchedPoolConfig::new(
                RAYDIUM_CLMM_SOL_USDC_POOL_ADDRESS,
                DexProgram::RaydiumClmm,
                SOL_DECIMALS,
                USDC_DECIMALS,
                SOL_USDC_POOL_FEE_IN_BASIS_POINTS,
            ),
            WatchedPoolConfig::new(
                ORCA_WHIRLPOOL_SOL_USDC_POOL_ADDRESS,
                DexProgram::OrcaWhirlpool,
                SOL_DECIMALS,
                USDC_DECIMALS,
                SOL_USDC_POOL_FEE_IN_BASIS_POINTS,
            ),
        ])
    }

    fn from_configs(configs_in_display_order: Vec<WatchedPoolConfig>) -> Self {
        let position_by_pool_address = configs_in_display_order
            .iter()
            .enumerate()
            .map(|(position, config)| (config.pool_address, position))
            .collect();
        Self {
            configs_in_display_order,
            position_by_pool_address,
        }
    }

    /// Config for a pool address, or `None` if we are not watching that account as a pool.
    pub fn config_for_pool_address(&self, address: &PublicKeyBytes) -> Option<&WatchedPoolConfig> {
        self.position_by_pool_address
            .get(address)
            .map(|&position| &self.configs_in_display_order[position])
    }

    /// Every watched pool, in a stable order.
    pub fn all_configs(&self) -> &[WatchedPoolConfig] {
        &self.configs_in_display_order
    }

    /// Base58 pool addresses, in the format Geyser's subscribe request expects.
    pub fn pool_addresses_to_subscribe(&self) -> Vec<String> {
        self.configs_in_display_order
            .iter()
            .map(|config| config.pool_address_base58.to_string())
            .collect()
    }
}
