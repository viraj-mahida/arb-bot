//! Step 6: cost math, staleness, trade decision, exact instruction bytes, and instruction order.

use solana_instruction::Instruction;
use solana_keypair::Keypair;

use super::test_pool_builders::{DEEP_LIQUIDITY, test_pool_at_tick_with_one_empty_tick_array};
use crate::step_2_decode_account_bytes::raydium_clmm_account_decoder::decode_raydium_fee_config_trade_fee_rate;
use crate::step_3_store_latest_pool_state::known_program_and_pool_addresses::{
    ORCA_WHIRLPOOL_PROGRAM_ADDRESS, RAYDIUM_CLMM_PROGRAM_ADDRESS,
};
use crate::step_3_store_latest_pool_state::{
    ConcentratedLiquidityPoolState, DexProgram, TickArrayAccountWithInitializedTicks,
    parse_base58_public_key,
};
use crate::step_4_quote_swaps::SwapDirection;
use crate::step_5_find_best_arbitrage_size::main_quote_most_profitable_two_pool_round_trip;
use crate::step_6_build_and_send_transactions::assemble_arbitrage_transaction::{
    FundingSource, MAX_TRANSACTION_SIZE_IN_BYTES, TransactionFeeSettings,
    compile_and_sign_v0_transaction, main_arbitrage_instructions,
};
use crate::step_6_build_and_send_transactions::decide_if_trade_is_worth_it::{
    ApprovedArbitrageTrade, CacheFreshness, TradeDecisionRules, WhyTradeWasSkipped,
    check_cache_is_fresh, costs_of_primary_send_route, estimate_transaction_costs,
    main_decide_from_quoted_round_trip,
};
use crate::step_6_build_and_send_transactions::flash_loan_instructions::{
    FlashLoanProvider, JUPITER_BORROW_DISCRIMINATOR, JUPITER_PAYBACK_DISCRIMINATOR,
    JupiterFlashLoanAccounts, KaminoFlashLoanAccounts,
};
use crate::step_6_build_and_send_transactions::orca_whirlpool_swap_instruction::orca_oracle_address;
use crate::step_6_build_and_send_transactions::swap_leg_instruction::{
    SWAP_V2_ANCHOR_DISCRIMINATOR, SwapLeg, build_swap_instruction,
};
use crate::step_6_build_and_send_transactions::trading_wallet::TradingWallet;
use crate::step_6_build_and_send_transactions::well_known_program_addresses::{
    ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS, COMPUTE_BUDGET_PROGRAM_ADDRESS,
    JUPITER_FLASHLOAN_PROGRAM_ADDRESS, KAMINO_LEND_PROGRAM_ADDRESS, SYSTEM_PROGRAM_ADDRESS,
    TOKEN_PROGRAM_ADDRESS, WRAPPED_SOL_MINT_ADDRESS, program,
};

fn test_rules() -> TradeDecisionRules {
    TradeDecisionRules {
        max_trade_input_lamports: 1_000_000_000,
        min_profit_after_costs_lamports: 10_000,
        max_pool_state_age_in_slots: 150,
        max_milliseconds_since_last_stream_update: 2_000,
        slippage_tolerance_in_basis_points: 0,
        compute_unit_limit: 400_000,
        priority_fee_micro_lamports_per_compute_unit: 10_000,
        jito_tip_lamports: 10_000,
        flash_loan_fee_in_basis_points: 1,
    }
}

fn fresh_cache() -> CacheFreshness {
    CacheFreshness {
        newest_slot_seen_from_stream: 1,
        milliseconds_since_last_stream_update: Some(10),
    }
}

/// Quote with Step 5, then run the same decision the live loop runs on that quote.
fn main_decide_if_trade_is_worth_it(
    sell_pool: &ConcentratedLiquidityPoolState,
    sell_pool_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
    buy_pool: &ConcentratedLiquidityPoolState,
    buy_pool_tick_arrays: &[&TickArrayAccountWithInitializedTicks],
    freshness: CacheFreshness,
    rules: &TradeDecisionRules,
) -> Result<ApprovedArbitrageTrade, WhyTradeWasSkipped> {
    check_cache_is_fresh(&[sell_pool, buy_pool], freshness, rules)?;
    let round_trip = main_quote_most_profitable_two_pool_round_trip(
        sell_pool,
        sell_pool_tick_arrays,
        buy_pool,
        buy_pool_tick_arrays,
    )?;
    main_decide_from_quoted_round_trip(
        sell_pool,
        sell_pool_tick_arrays,
        buy_pool,
        buy_pool_tick_arrays,
        round_trip,
        freshness,
        rules,
    )
}

/// Orca priced ~1% above Raydium: selling SOL on Orca and buying it back on Raydium pays.
fn profitable_trade(rules: &TradeDecisionRules) -> ApprovedArbitrageTrade {
    let (mut orca, orca_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 100);
    let (mut raydium, raydium_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::RaydiumClmm, DEEP_LIQUIDITY, 0);
    for pool in [&mut orca, &mut raydium] {
        pool.token_a_mint = parse_base58_public_key(WRAPPED_SOL_MINT_ADDRESS);
        pool.token_b_mint = [7u8; 32];
    }
    raydium.pool_address = [9u8; 32];
    let mut raydium_tick_array = raydium_tick_array;
    raydium_tick_array.pool_address = raydium.pool_address;
    main_decide_if_trade_is_worth_it(
        &orca,
        &[&orca_tick_array],
        &raydium,
        &[&raydium_tick_array],
        fresh_cache(),
        rules,
    )
    .expect("a 1% gap with deep liquidity should be approved")
}

fn program_ids(instructions: &[Instruction]) -> Vec<String> {
    instructions
        .iter()
        .map(|instruction| instruction.program_id.to_string())
        .collect()
}

/// Every cost is added, and fractions of a lamport round up.
#[test]
fn transaction_costs_add_signature_priority_tip_and_flash_fees() {
    let costs = estimate_transaction_costs(&test_rules(), 1_000_000_000);
    assert_eq!(costs.network_signature_fee, 5_000);
    assert_eq!(costs.priority_fee, 4_000); // 400,000 CU × 10,000 µlamports / 1,000,000
    assert_eq!(costs.jito_tip, 10_000);
    assert_eq!(costs.flash_loan_fee, 100_000); // 1 bp of 1 SOL
    assert_eq!(costs.total(), 119_000);

    let jito_route = costs_of_primary_send_route(&test_rules(), 1_000_000_000);
    assert_eq!(jito_route.priority_fee, 0);
    assert_eq!(jito_route.jito_tip, 10_000);
    let rpc_route = costs_of_primary_send_route(
        &TradeDecisionRules {
            jito_tip_lamports: 0,
            ..test_rules()
        },
        1_000_000_000,
    );
    assert_eq!(rpc_route.priority_fee, 4_000);
    assert_eq!(rpc_route.jito_tip, 0);

    let one_lamport_loan = estimate_transaction_costs(
        &TradeDecisionRules {
            flash_loan_fee_in_basis_points: 1,
            ..test_rules()
        },
        1,
    );
    assert_eq!(one_lamport_loan.flash_loan_fee, 1);
}

/// A quiet stream or an old pool blocks trading; fresh data passes.
#[test]
fn stale_data_is_refused() {
    let (pool, _) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 0);
    let rules = test_rules();
    let never_heard = CacheFreshness {
        newest_slot_seen_from_stream: 0,
        milliseconds_since_last_stream_update: None,
    };
    assert!(matches!(
        check_cache_is_fresh(&[&pool], never_heard, &rules),
        Err(WhyTradeWasSkipped::NoStreamUpdateYet)
    ));
    let silent = CacheFreshness {
        newest_slot_seen_from_stream: 1,
        milliseconds_since_last_stream_update: Some(5_000),
    };
    assert!(matches!(
        check_cache_is_fresh(&[&pool], silent, &rules),
        Err(WhyTradeWasSkipped::StreamSilentTooLong { .. })
    ));
    let pool_far_behind = CacheFreshness {
        newest_slot_seen_from_stream: 1_000,
        milliseconds_since_last_stream_update: Some(10),
    };
    assert!(matches!(
        check_cache_is_fresh(&[&pool], pool_far_behind, &rules),
        Err(WhyTradeWasSkipped::PoolStateTooOld {
            age_in_slots: 999,
            ..
        })
    ));
    assert!(check_cache_is_fresh(&[&pool], fresh_cache(), &rules).is_ok());
}

/// The hot path decides from the round trip already quoted for the log.
#[test]
fn precomputed_round_trip_matches_a_fresh_decision() {
    let rules = test_rules();
    let (orca, orca_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 100);
    let (mut raydium, mut raydium_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::RaydiumClmm, DEEP_LIQUIDITY, 0);
    raydium.pool_address = [9u8; 32];
    raydium_tick_array.pool_address = raydium.pool_address;

    let fresh = main_decide_if_trade_is_worth_it(
        &orca,
        &[&orca_tick_array],
        &raydium,
        &[&raydium_tick_array],
        fresh_cache(),
        &rules,
    )
    .expect("a 1% gap with deep liquidity should be approved");
    let round_trip = main_quote_most_profitable_two_pool_round_trip(
        &orca,
        &[&orca_tick_array],
        &raydium,
        &[&raydium_tick_array],
    )
    .expect("the same pools quote");
    let from_quote = main_decide_from_quoted_round_trip(
        &orca,
        &[&orca_tick_array],
        &raydium,
        &[&raydium_tick_array],
        round_trip,
        fresh_cache(),
        &rules,
    )
    .expect("the precomputed round trip should be approved");

    assert_eq!(
        from_quote.start_token_amount_in,
        fresh.start_token_amount_in
    );
    assert_eq!(
        from_quote.expected_start_token_out,
        fresh.expected_start_token_out
    );
    assert_eq!(
        from_quote.expected_profit_after_costs,
        fresh.expected_profit_after_costs
    );
    assert_eq!(
        from_quote.leg_1_minimum_bridge_token_out,
        fresh.leg_1_minimum_bridge_token_out
    );
    assert_eq!(
        from_quote.leg_2_minimum_start_token_out,
        fresh.leg_2_minimum_start_token_out
    );
    assert_eq!(from_quote.best_size_lamports, fresh.best_size_lamports);
    assert_eq!(from_quote.size_was_capped, fresh.size_was_capped);
    assert_eq!(from_quote.costs, fresh.costs);
}

/// Same price on both pools: fees alone make the round trip lose, so no trade.
#[test]
fn equal_prices_are_not_traded() {
    let (orca, orca_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 0);
    let (raydium, raydium_tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::RaydiumClmm, DEEP_LIQUIDITY, 0);
    let decision = main_decide_if_trade_is_worth_it(
        &orca,
        &[&orca_tick_array],
        &raydium,
        &[&raydium_tick_array],
        fresh_cache(),
        &test_rules(),
    );
    assert!(decision.is_err());
}

/// The size is capped, and leg 2's minimum out locks in start + costs + minimum profit.
#[test]
fn approved_trade_is_capped_and_guarded_by_minimum_output() {
    let rules = test_rules();
    let trade = profitable_trade(&rules);
    assert!(trade.size_was_capped);
    assert!(trade.best_size_lamports > trade.start_token_amount_in);
    assert!(trade.best_size_pool_profit_lamports > 0);
    assert_eq!(trade.start_token_amount_in, rules.max_trade_input_lamports);
    assert_eq!(
        trade.leg_2_bridge_token_amount_in,
        trade.leg_1_minimum_bridge_token_out
    );
    assert_eq!(
        trade.leg_2_minimum_start_token_out,
        trade.start_token_amount_in + trade.costs.total() + rules.min_profit_after_costs_lamports
    );
    assert!(trade.expected_start_token_out >= trade.leg_2_minimum_start_token_out);
    assert!(trade.expected_profit_after_costs >= i128::from(rules.min_profit_after_costs_lamports));
}

/// Orca `swap_v2`: exact data bytes and the signer / pool / oracle positions.
#[test]
fn orca_swap_instruction_has_exact_bytes_and_account_order() {
    let (pool, tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::OrcaWhirlpool, DEEP_LIQUIDITY, 0);
    let instruction = build_swap_instruction(&SwapLeg {
        pool: &pool,
        cached_tick_arrays: &[tick_array],
        direction: SwapDirection::TokenAToTokenB,
        amount_in: 1_000,
        minimum_amount_out: 900,
        wallet: [5u8; 32],
        wallet_token_a_account: [6u8; 32],
        wallet_token_b_account: [8u8; 32],
    });
    let mut expected_data = SWAP_V2_ANCHOR_DISCRIMINATOR.to_vec();
    expected_data.extend_from_slice(&1_000u64.to_le_bytes());
    expected_data.extend_from_slice(&900u64.to_le_bytes());
    expected_data.extend_from_slice(&0u128.to_le_bytes());
    expected_data.extend_from_slice(&[1, 1, 0]);
    assert_eq!(instruction.data, expected_data);
    assert_eq!(
        instruction.program_id,
        program(ORCA_WHIRLPOOL_PROGRAM_ADDRESS)
    );
    assert_eq!(instruction.accounts.len(), 15);
    assert!(instruction.accounts[3].is_signer);
    assert_eq!(instruction.accounts[3].pubkey.to_bytes(), [5u8; 32]);
    assert!(instruction.accounts[4].is_writable);
    assert_eq!(instruction.accounts[4].pubkey.to_bytes(), pool.pool_address);
    assert_eq!(instruction.accounts[7].pubkey.to_bytes(), [6u8; 32]);
    assert_eq!(instruction.accounts[9].pubkey.to_bytes(), [8u8; 32]);
    assert_eq!(
        instruction.accounts[14].pubkey.to_bytes(),
        orca_oracle_address(&pool.pool_address)
    );
}

/// Raydium `swap_v2`: 41 data bytes, and for b→a the input side is token B.
#[test]
fn raydium_swap_instruction_orders_accounts_by_input_and_output() {
    let (pool, tick_array) =
        test_pool_at_tick_with_one_empty_tick_array(DexProgram::RaydiumClmm, DEEP_LIQUIDITY, 0);
    let instruction = build_swap_instruction(&SwapLeg {
        pool: &pool,
        cached_tick_arrays: std::slice::from_ref(&tick_array),
        direction: SwapDirection::TokenBToTokenA,
        amount_in: 5_000,
        minimum_amount_out: 4_000,
        wallet: [5u8; 32],
        wallet_token_a_account: [6u8; 32],
        wallet_token_b_account: [8u8; 32],
    });
    assert_eq!(
        instruction.program_id,
        program(RAYDIUM_CLMM_PROGRAM_ADDRESS)
    );
    assert_eq!(instruction.data.len(), 41);
    assert_eq!(&instruction.data[..8], &SWAP_V2_ANCHOR_DISCRIMINATOR);
    assert_eq!(&instruction.data[8..16], &5_000u64.to_le_bytes());
    assert_eq!(&instruction.data[16..24], &4_000u64.to_le_bytes());
    assert_eq!(instruction.data[40], 1);
    assert!(instruction.accounts[0].is_signer);
    assert_eq!(instruction.accounts[1].pubkey.to_bytes(), [3u8; 32]); // amm_config
    assert_eq!(instruction.accounts[3].pubkey.to_bytes(), [8u8; 32]); // input = wallet's token B
    assert_eq!(instruction.accounts[4].pubkey.to_bytes(), [6u8; 32]); // output = wallet's token A
    assert_eq!(instruction.accounts[7].pubkey.to_bytes(), [4u8; 32]); // observation
    assert_eq!(instruction.accounts.len(), 14); // 13 fixed + 1 cached tick array
    assert_eq!(
        instruction.accounts[13].pubkey.to_bytes(),
        tick_array.tick_array_address
    );
}

/// Wallet mode: budget, ATAs, wrap, two swaps, unwrap, tip — in that order — and it fits.
#[test]
fn wallet_funded_transaction_has_expected_instruction_order() {
    let trade = profitable_trade(&test_rules());
    let wallet = TradingWallet::from_keypair(Keypair::new());
    let fees = TransactionFeeSettings {
        compute_unit_limit: 400_000,
        priority_fee_micro_lamports_per_compute_unit: 0,
        jito_tip: Some(([11u8; 32], 10_000)),
    };
    let instructions =
        main_arbitrage_instructions(&wallet, &trade, &FundingSource::OwnWallet, &fees);
    let expected = [
        COMPUTE_BUDGET_PROGRAM_ADDRESS,
        ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS,
        ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS,
        SYSTEM_PROGRAM_ADDRESS,
        TOKEN_PROGRAM_ADDRESS,
        ORCA_WHIRLPOOL_PROGRAM_ADDRESS,
        RAYDIUM_CLMM_PROGRAM_ADDRESS,
        TOKEN_PROGRAM_ADDRESS,
        SYSTEM_PROGRAM_ADDRESS,
    ];
    assert_eq!(program_ids(&instructions), expected.map(String::from));

    let transaction =
        compile_and_sign_v0_transaction(&wallet, &instructions, &[], solana_hash::Hash::default())
            .expect("wallet-mode transaction should fit without a lookup table");
    assert!(transaction.wire_bytes.len() <= MAX_TRANSACTION_SIZE_IN_BYTES);
    assert_eq!(transaction.wire_bytes[0], 1);
}

/// Flash mode: borrow before leg 1, repay after leg 2, and repay points back at the borrow.
#[test]
fn flash_loan_transaction_borrows_first_and_repays_after_both_legs() {
    let trade = profitable_trade(&test_rules());
    let wallet = TradingWallet::from_keypair(Keypair::new());
    let provider = FlashLoanProvider::Kamino(KaminoFlashLoanAccounts {
        lending_market: [20u8; 32],
        lending_market_authority: [21u8; 32],
        reserve: [22u8; 32],
        reserve_liquidity_mint: parse_base58_public_key(WRAPPED_SOL_MINT_ADDRESS),
        reserve_supply_vault: [23u8; 32],
        reserve_fee_vault: [24u8; 32],
    });
    let fees = TransactionFeeSettings {
        compute_unit_limit: 400_000,
        priority_fee_micro_lamports_per_compute_unit: 10_000,
        jito_tip: None,
    };
    let instructions =
        main_arbitrage_instructions(&wallet, &trade, &FundingSource::FlashLoan(&provider), &fees);
    let expected = [
        COMPUTE_BUDGET_PROGRAM_ADDRESS,
        COMPUTE_BUDGET_PROGRAM_ADDRESS,
        ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS,
        ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS,
        KAMINO_LEND_PROGRAM_ADDRESS,
        ORCA_WHIRLPOOL_PROGRAM_ADDRESS,
        RAYDIUM_CLMM_PROGRAM_ADDRESS,
        KAMINO_LEND_PROGRAM_ADDRESS,
        TOKEN_PROGRAM_ADDRESS,
    ];
    assert_eq!(program_ids(&instructions), expected.map(String::from));
    let repay = &instructions[7];
    assert_eq!(
        *repay.data.last().unwrap(),
        4,
        "repay must name the borrow's instruction index"
    );
    assert_eq!(
        &repay.data[8..16],
        &trade.start_token_amount_in.to_le_bytes()
    );
}

fn jupiter_admin_bytes(status: u8, fee: u16) -> Vec<u8> {
    let mut data = vec![0u8; 100];
    data[72] = status;
    data[73..75].copy_from_slice(&fee.to_le_bytes());
    data
}

/// Jupiter: borrow first, payback last, both carrying the amount (payback includes the fee).
#[test]
fn jupiter_flash_loan_transaction_borrows_first_and_repays_with_fee() {
    let trade = profitable_trade(&test_rules());
    let wallet = TradingWallet::from_keypair(Keypair::new());
    let accounts =
        JupiterFlashLoanAccounts::from_admin_account_bytes(&jupiter_admin_bytes(1, 5)).unwrap();
    let provider = FlashLoanProvider::Jupiter(accounts);
    let fees = TransactionFeeSettings {
        compute_unit_limit: 400_000,
        priority_fee_micro_lamports_per_compute_unit: 10_000,
        jito_tip: None,
    };
    let instructions =
        main_arbitrage_instructions(&wallet, &trade, &FundingSource::FlashLoan(&provider), &fees);
    let borrow = &instructions[4];
    let repay = &instructions[7];
    assert_eq!(
        borrow.program_id,
        program(JUPITER_FLASHLOAN_PROGRAM_ADDRESS)
    );
    assert_eq!(repay.program_id, program(JUPITER_FLASHLOAN_PROGRAM_ADDRESS));
    assert_eq!(borrow.accounts.len(), 14);
    assert_eq!(&borrow.data[..8], &JUPITER_BORROW_DISCRIMINATOR);
    assert_eq!(&repay.data[..8], &JUPITER_PAYBACK_DISCRIMINATOR);
    assert_eq!(
        &borrow.data[8..16],
        &trade.start_token_amount_in.to_le_bytes()
    );
    let owed = trade.start_token_amount_in
        + (u128::from(trade.start_token_amount_in) * 5).div_ceil(10_000) as u64;
    assert_eq!(&repay.data[8..16], &owed.to_le_bytes());
}

#[test]
fn jupiter_admin_account_fee_is_read_and_pause_is_refused() {
    let accounts =
        JupiterFlashLoanAccounts::from_admin_account_bytes(&jupiter_admin_bytes(1, 0)).unwrap();
    assert_eq!(accounts.fee_in_basis_points, 0);
    assert_eq!(accounts.amount_to_repay(1_000_000_000), 1_000_000_000);
    assert!(
        JupiterFlashLoanAccounts::from_admin_account_bytes(&jupiter_admin_bytes(0, 0)).is_err()
    );
}

/// Raydium's fee lives at byte 47 of its `amm_config` account.
#[test]
fn raydium_fee_config_fee_is_read_from_byte_47() {
    let mut account_data = vec![0u8; 117];
    account_data[47..51].copy_from_slice(&500u32.to_le_bytes());
    assert_eq!(
        decode_raydium_fee_config_trade_fee_rate(&account_data),
        Some(500)
    );
    assert_eq!(
        decode_raydium_fee_config_trade_fee_rate(&account_data[..40]),
        None
    );
}

/// A Kamino reserve must belong to the configured market and lend wrapped SOL.
#[test]
fn kamino_reserve_is_validated_before_use() {
    let market = [20u8; 32];
    let mut reserve_data = vec![0u8; 256];
    reserve_data[32..64].copy_from_slice(&market);
    reserve_data[128..160].copy_from_slice(&parse_base58_public_key(WRAPPED_SOL_MINT_ADDRESS));
    reserve_data[160..192].copy_from_slice(&[23u8; 32]);
    reserve_data[192..224].copy_from_slice(&[24u8; 32]);
    let accounts =
        KaminoFlashLoanAccounts::from_reserve_account_bytes(market, [22u8; 32], &reserve_data)
            .unwrap();
    assert_eq!(accounts.reserve_supply_vault, [23u8; 32]);
    assert_eq!(accounts.reserve_fee_vault, [24u8; 32]);
    assert!(
        KaminoFlashLoanAccounts::from_reserve_account_bytes([99u8; 32], [22u8; 32], &reserve_data)
            .is_err()
    );
}
