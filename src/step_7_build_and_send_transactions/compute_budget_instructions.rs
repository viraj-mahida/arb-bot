//! Compute budget: how much work the transaction may do, and what it pays per unit.
//!
//! **Compute units (CU):** every instruction costs compute units, a measure of
//! CPU work. A transaction gets 200,000 CU per instruction by default (max
//! 1.4 million). Two CLMM swaps plus a flash loan need a few hundred thousand.
//!
//! **Priority fee:** an optional price per compute unit, in *micro-lamports*
//! (1 lamport = 1,000,000 micro-lamports). Total = limit × price / 1,000,000.
//! Validators order waiting transactions by this price, so paying more gets
//! the trade in earlier — which matters when other bots chase the same gap.
//!
//! Requesting a tight limit also helps: the fee is charged on the *requested*
//! limit, and smaller transactions are easier for the scheduler to fit.

use solana_instruction::Instruction;

use super::well_known_program_addresses::{COMPUTE_BUDGET_PROGRAM_ADDRESS, program};

/// `SetComputeUnitLimit(units)`: data = `[2, units as u32 little-endian]`.
pub fn set_compute_unit_limit(compute_units: u32) -> Instruction {
    let mut data = vec![2];
    data.extend_from_slice(&compute_units.to_le_bytes());
    Instruction::new_with_bytes(program(COMPUTE_BUDGET_PROGRAM_ADDRESS), &data, vec![])
}

/// `SetComputeUnitPrice(micro_lamports)`: data = `[3, price as u64 little-endian]`.
pub fn set_compute_unit_price(micro_lamports_per_compute_unit: u64) -> Instruction {
    let mut data = vec![3];
    data.extend_from_slice(&micro_lamports_per_compute_unit.to_le_bytes());
    Instruction::new_with_bytes(program(COMPUTE_BUDGET_PROGRAM_ADDRESS), &data, vec![])
}
