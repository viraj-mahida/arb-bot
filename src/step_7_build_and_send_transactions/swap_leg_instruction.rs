//! One swap ("leg") of the round trip, and the DEX-agnostic way to turn it into an instruction.
//!
//! **Instruction vs transaction:** an *instruction* is one call into one
//! program ("swap on this pool"). A *transaction* is a list of instructions
//! that run in order and succeed or fail together — so leg 1 and leg 2 either
//! both happen or neither does. That is what makes arbitrage safe on Solana.

use solana_instruction::Instruction;

use super::orca_whirlpool_swap_instruction::orca_whirlpool_swap_v2_instruction;
use super::raydium_clmm_swap_instruction::raydium_clmm_swap_v2_instruction;
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, PublicKeyBytes, TickArrayAccountWithInitializedTicks,
};
use crate::step_4_quote_swaps::SwapDirection;

/// Anchor programs pick the function to run from the first 8 bytes of the
/// instruction data: `sha256("global:<function name>")[..8]`. Orca and Raydium
/// both name their function `swap_v2`, so the bytes are the same.
pub const SWAP_V2_ANCHOR_DISCRIMINATOR: [u8; 8] = [0x2b, 0x04, 0xed, 0x0b, 0x1a, 0xc9, 0x1e, 0x62];

/// Passing 0 as the price limit tells both programs "no limit, use the most
/// extreme price allowed". Our protection is the minimum output instead.
pub const NO_PRICE_LIMIT: u128 = 0;

/// Everything needed to build one exact-input swap instruction.
pub struct SwapLeg<'a> {
    pub pool: &'a ConcentratedLiquidityPoolState,
    /// Cached tick arrays of this pool (Raydium passes the ones that exist on-chain).
    pub cached_tick_arrays: &'a [TickArrayAccountWithInitializedTicks],
    pub direction: SwapDirection,
    /// Exact amount of the input token to swap.
    pub amount_in: u64,
    /// The program reverts if the output would be smaller (`other_amount_threshold`).
    pub minimum_amount_out: u64,
    /// The wallet signing the swap.
    pub wallet: PublicKeyBytes,
    /// The wallet's token account for the pool's token A / token B.
    pub wallet_token_a_account: PublicKeyBytes,
    pub wallet_token_b_account: PublicKeyBytes,
}

/// Build the right DEX's swap instruction for this leg.
pub fn build_swap_instruction(leg: &SwapLeg) -> Instruction {
    match leg.pool.dex {
        DexProgram::OrcaWhirlpool => orca_whirlpool_swap_v2_instruction(leg),
        DexProgram::RaydiumClmm => raydium_clmm_swap_v2_instruction(leg),
    }
}
