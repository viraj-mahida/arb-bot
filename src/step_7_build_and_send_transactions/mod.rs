//! # Step 7 — Build and send transactions
//!
//! **What came before:** Step 5 found the trade size where a round trip earns
//! the most, and Step 4 quoted it with each DEX's exact math.
//!
//! **What this step does:** turns that quote into real money — carefully.
//!
//! 1. **Decide** ([`decide_if_trade_is_worth_it`]): subtract every cost
//!    (network fee, priority fee, Jito tip, flash-loan fee), refuse stale data,
//!    cap the size, and set the minimum outputs that make a loss impossible.
//! 2. **Fund** leg 1 from the wallet ([`trading_wallet`]: wrap SOL) or with a
//!    flash loan ([`flash_loan_instructions`]). `FUNDING_MODE` picks.
//! 3. **Build** the two swap instructions
//!    ([`orca_whirlpool_swap_instruction`], [`raydium_clmm_swap_instruction`]),
//!    plus compute budget ([`compute_budget_instructions`]) and tip
//!    ([`jito_tip_instruction`]).
//! 4. **Assemble and sign** a v0 transaction
//!    ([`assemble_arbitrage_transaction`]).
//! 5. **Simulate, then send** ([`send_and_confirm`]): always dry-run first;
//!    only send when `SEND_TRANSACTIONS=true`.
//!
//! [`arbitrage_trade_executor`] ties these together and makes sure only one
//! trade is in flight at a time.
//!
//! **The safety net, in one sentence:** leg 2's minimum output is
//! `start amount + all costs + minimum profit`, so if prices moved and the trade
//! would not pay, the chain reverts the whole transaction (and a Jito bundle
//! that would revert is never even included).
//!
//! **Current limitations:**
//! - Both tokens are assumed to use the classic token program (true for SOL and USDC).
//! - Raydium's tick-array bitmap extension account is not passed (only needed far from the current price).
//! - The flash-loan fee is estimated from `FLASH_LOAN_FEE_BPS`, not read from the reserve.
//! - Flash-loan transactions usually need an address lookup table to fit in 1,232 bytes.

pub mod arbitrage_trade_executor;
pub mod assemble_arbitrage_transaction;
pub mod compute_budget_instructions;
pub mod decide_if_trade_is_worth_it;
pub mod flash_loan_instructions;
pub mod jito_tip_instruction;
pub mod orca_whirlpool_swap_instruction;
pub mod raydium_clmm_swap_instruction;
pub mod send_and_confirm;
pub mod swap_leg_instruction;
pub mod trading_wallet;
pub mod well_known_program_addresses;

pub use arbitrage_trade_executor::ArbitrageTradeExecutor;
pub use decide_if_trade_is_worth_it::{ApprovedArbitrageTrade, WhyTradeWasSkipped};
