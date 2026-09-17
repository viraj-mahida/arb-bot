use std::collections::HashMap;

use super::constants::{ORCA_WHIRLPOOL_SOL_USDC, POOL_FEES, RAYDIUM_CLMM_SOL_USDC};
use super::pubkey::parse_pubkey;
use super::types::Venue;

#[derive(Debug, Clone)]
pub struct PoolSpec {
    pub address: [u8; 32],
    pub address_bs58: &'static str,
    pub venue: Venue,
    pub decimals_a: u8,
    pub decimals_b: u8,
    pub fee_bps: u16,
}

impl PoolSpec {
    fn parse(
        address_bs58: &'static str,
        venue: Venue,
        decimals_a: u8,
        decimals_b: u8,
        fee_bps: u16,
    ) -> Self {
        Self {
            address: parse_pubkey(address_bs58),
            address_bs58,
            venue,
            decimals_a,
            decimals_b,
            fee_bps,
        }
    }

    //TODO? should this multiply or divide?
    pub fn fee_rate(&self) -> u16 {
        self.fee_bps.saturating_mul(100)
    }
}

/// Static list of CLMM pools we subscribe to and decode.
/// TODO? single HashMap<[u8; 32], PoolSpec> is simpler and directly maps addresses to their data. Vec + index is used if ordered access or index-based lookup is needed.
#[derive(Debug, Clone)]
pub struct PoolRegistry {
    specs: Vec<PoolSpec>,
    by_address: HashMap<[u8; 32], usize>,
}

impl PoolRegistry {
    pub fn sol_usdc_clmm() -> Self {
        let specs = vec![
            PoolSpec::parse(
                RAYDIUM_CLMM_SOL_USDC,
                Venue::RaydiumClmm,
                9,
                6,
                u16::from(POOL_FEES), // TODO? POOL_FEES can be u16 from env only
            ),
            PoolSpec::parse(
                ORCA_WHIRLPOOL_SOL_USDC,
                Venue::OrcaWhirlpool,
                9,
                6,
                u16::from(POOL_FEES),
            ),
        ];
        let by_address = specs
            .iter()
            .enumerate()
            .map(|(i, spec)| (spec.address, i))
            .collect();
        Self { specs, by_address }
    }

    pub fn get(&self, pubkey: &[u8; 32]) -> Option<&PoolSpec> {
        self.by_address.get(pubkey).map(|&i| &self.specs[i])
    }

    #[allow(dead_code)]
    pub fn specs(&self) -> &[PoolSpec] {
        &self.specs
    }

    pub fn subscribe_addresses(&self) -> Vec<String> {
        self.specs
            .iter()
            .map(|spec| spec.address_bs58.to_string())
            .collect()
    }
}
