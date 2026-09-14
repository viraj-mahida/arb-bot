#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Venue {
    RaydiumClmm,
    OrcaWhirlpool,
}

impl Venue {
    pub fn as_str(self) -> &'static str {
        match self {
            Venue::RaydiumClmm => "raydium_clmm",
            Venue::OrcaWhirlpool => "orca_whirlpool",
        }
    }
}

/// Venue-agnostic CLMM snapshot. Tick arrays live on the cache later, not here.
#[derive(Debug, Clone)]
#[allow(dead_code)] // mint/vault/fee fields are for quoting next
pub struct ClmmPoolState {
    pub pubkey: [u8; 32],
    pub venue: Venue,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,
    pub tick_spacing: u16,
    /// Hundredths of a basis point (Orca `fee_rate`). 400 = 4 bps = 0.04%.
    pub fee_rate: u16,
    pub mint_a: [u8; 32],
    pub mint_b: [u8; 32],
    pub vault_a: [u8; 32],
    pub vault_b: [u8; 32],
    pub decimals_a: u8,
    pub decimals_b: u8,
    pub slot: u64,
    pub write_version: u64,
}

impl ClmmPoolState {
    /// Human-readable token1-per-token0 spot from the current tick. Logging only.
    pub fn spot_price(&self) -> f64 {
        let sqrt = (self.sqrt_price_x64 as f64) / 2f64.powi(64);
        sqrt.powi(2) * 10f64.powi(i32::from(self.decimals_a) - i32::from(self.decimals_b))
    }
}
