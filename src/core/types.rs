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

    pub fn ticks_per_array(self) -> i32 {
        match self {
            Venue::RaydiumClmm => 60,
            Venue::OrcaWhirlpool => 88,
        }
    }
}

/// Venue-agnostic CLMM snapshot. Nearby tick arrays live on `PoolCache`.
#[derive(Debug, Clone)]
#[allow(dead_code)] // mint/vault are for swap ix building later
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

/// PDA we derived (and subscribed to) for one tick-array account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickArrayRef {
    pub pubkey: [u8; 32],
    pub pool: [u8; 32],
    pub venue: Venue,
    pub start_tick_index: i32,
    pub tick_spacing: u16,
}

/// One initialized tick on the liquidity curve. `liquidity_net` is ΔL when price crosses this tick left→right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializedTick {
    pub tick: i32,
    pub liquidity_net: i128,
}

/// Decoded tick-array snapshot, keyed on cache by `pubkey`.
#[derive(Debug, Clone)]
pub struct TickArraySnapshot {
    pub pubkey: [u8; 32],
    pub pool: [u8; 32],
    pub venue: Venue,
    pub start_tick_index: i32,
    pub ticks: Vec<InitializedTick>,
    pub slot: u64,
    pub write_version: u64,
}
