//! Fixed on-chain addresses the bot needs to know in advance.
//!
//! **Programs** are the DEX smart contracts. Their addresses never change.
//! **Pools** are accounts owned by those programs; each pool trades one token pair
//! at one fee tier. These are today's SOL/USDC pools.
//!
//! **Next:** load pools from a config file or discover them by scanning each
//! DEX program's accounts, instead of hard-coding them.

/// Raydium's concentrated-liquidity (CLMM) program.
pub const RAYDIUM_CLMM_PROGRAM_ADDRESS: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";
/// Orca's Whirlpool program.
pub const ORCA_WHIRLPOOL_PROGRAM_ADDRESS: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";

/// Raydium CLMM pool trading SOL (token A) against USDC (token B).
pub const RAYDIUM_CLMM_SOL_USDC_POOL_ADDRESS: &str = "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv";
/// Orca Whirlpool pool trading SOL (token A) against USDC (token B).
pub const ORCA_WHIRLPOOL_SOL_USDC_POOL_ADDRESS: &str =
    "Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE";

/// Raydium CLMM SOL/USDC, 0.01% fee tier.
pub const RAYDIUM_CLMM_SOL_USDC_1_BP_POOL_ADDRESS: &str =
    "8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj";
/// Raydium CLMM SOL/USDC, 0.02% fee tier.
pub const RAYDIUM_CLMM_SOL_USDC_2_BP_POOL_ADDRESS: &str =
    "CYbD9RaToYMtWKA7QZyoLahnHdWq553Vm62Lh6qWtuxq";
/// Orca Whirlpool SOL/USDC, 0.01% fee tier.
pub const ORCA_WHIRLPOOL_SOL_USDC_1_BP_POOL_ADDRESS: &str =
    "83v8iPyZihDEjDdY8RdZddyZNyUtXngz69Lgo9Kt5d6d";
/// Orca Whirlpool SOL/USDC, 0.02% fee tier.
pub const ORCA_WHIRLPOOL_SOL_USDC_2_BP_POOL_ADDRESS: &str =
    "FpCMFDFGYotvufJ7HrFHsWEiiQCGbkLCtwHiDnh7o28Q";

/// Swap fee of the two main SOL/USDC pools (the first two above), in basis
/// points (1 bp = 0.01%). The others use the tier in their name.
///
/// 4 bps = 0.04%: swapping 1,000 USDC costs 0.40 USDC in fees.
pub const SOL_USDC_POOL_FEE_IN_BASIS_POINTS: u16 = 4;

/// SOL uses 9 decimals: 1 SOL = 1,000,000,000 lamports.
pub const SOL_DECIMALS: u8 = 9;
/// USDC uses 6 decimals: 1 USDC = 1,000,000 micro-USDC.
pub const USDC_DECIMALS: u8 = 6;
