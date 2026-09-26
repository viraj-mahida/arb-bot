//! Fixed on-chain addresses the bot needs to know in advance.
//!
//! **Programs** are the DEX smart contracts. Their addresses never change.
//! **Pools** are accounts owned by those programs; each pool trades one token pair
//! at one fee tier. These are today's two SOL/USDC pools.
//!
//! **Future:** a production bot would load pools from a config file or discover
//! them by scanning each DEX program's accounts, instead of hard-coding them.

/// Raydium's concentrated-liquidity (CLMM) program.
pub const RAYDIUM_CLMM_PROGRAM_ADDRESS: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";
/// Orca's Whirlpool program.
pub const ORCA_WHIRLPOOL_PROGRAM_ADDRESS: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";

/// Raydium CLMM pool trading SOL (token A) against USDC (token B).
pub const RAYDIUM_CLMM_SOL_USDC_POOL_ADDRESS: &str =
    "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv";
/// Orca Whirlpool pool trading SOL (token A) against USDC (token B).
pub const ORCA_WHIRLPOOL_SOL_USDC_POOL_ADDRESS: &str =
    "Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE";

/// Swap fee of both SOL/USDC pools above, in basis points (1 bp = 0.01%).
///
/// 4 bps = 0.04%: swapping 1,000 USDC costs 0.40 USDC in fees.
pub const SOL_USDC_POOL_FEE_IN_BASIS_POINTS: u16 = 4;

/// SOL uses 9 decimals: 1 SOL = 1,000,000,000 lamports.
pub const SOL_DECIMALS: u8 = 9;
/// USDC uses 6 decimals: 1 USDC = 1,000,000 micro-USDC.
pub const USDC_DECIMALS: u8 = 6;
