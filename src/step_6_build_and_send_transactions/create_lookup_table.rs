//! One-off tool: `cargo run -- create-lookup-table`
//!
//! **Why:** a flash-loan round trip touches ~35 accounts. A transaction may be
//! at most 1,232 bytes and each account costs 32 of them, so it will not fit.
//! An *address lookup table* (ALT) stores addresses on-chain; a v0 transaction
//! then names each by a 1-byte index instead.
//!
//! **What goes in:** every account that stays the same from trade to trade —
//! the wallet's token accounts, the flash-loan accounts, the Jito tip accounts,
//! and for each watched pool its vaults, mints, oracle / fee config and the
//! tick arrays around the current price. (Program IDs that are *called* must
//! stay in the message itself; they are added anyway because a program passed
//! as a plain account, like the token program, can use the table.)
//!
//! **Output:** the table address. Put it in `ADDRESS_LOOKUP_TABLES` in `.env`.
//! Costs a little rent (~0.003 SOL), refundable by closing the table.

use std::collections::HashSet;
use std::time::Duration;

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::assemble_arbitrage_transaction::compile_and_sign_v0_transaction;
use super::compute_budget_instructions::set_compute_unit_price;
use super::flash_loan_instructions::JupiterFlashLoanAccounts;
use super::jito_tip_instruction::default_jito_tip_accounts;
use super::orca_whirlpool_swap_instruction::orca_oracle_address;
use super::trading_wallet::TradingWallet;
use super::well_known_program_addresses::{
    ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS, INSTRUCTIONS_SYSVAR_ADDRESS,
    JUPITER_FLASHLOAN_PROGRAM_ADDRESS, JUPITER_LIQUIDITY_PROGRAM_ADDRESS, MEMO_PROGRAM_ADDRESS,
    SYSTEM_PROGRAM_ADDRESS, TOKEN_2022_PROGRAM_ADDRESS, TOKEN_PROGRAM_ADDRESS,
    WRAPPED_SOL_MINT_ADDRESS, program, pubkey,
};
use crate::bot_settings::{BotSettingsFromEnvironment, FlashLoanLender, FundingMode};
use crate::solana_connections::SolanaRpcClient;
use crate::step_2_decode_account_bytes::main_decode_pool_account;
use crate::step_3_store_latest_pool_state::{
    DexSpecificSwapAccounts, PublicKeyBytes, WatchedPools, encode_public_key_as_base58,
    parse_base58_public_key, tick_array_pdas_near_current_price,
};

const LOOKUP_TABLE_PROGRAM_ADDRESS: &str = "AddressLookupTab1e1111111111111111111111111";
const USDC_MINT_ADDRESS: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
/// Addresses per extend transaction. Each one is 32 bytes of instruction data,
/// and the extend transaction has the same 1,232-byte limit as any other, so
/// 20 (640 bytes of addresses) is as many as fit comfortably.
const ADDRESSES_PER_EXTEND: usize = 20;

pub async fn main_create_lookup_table(
    settings: &BotSettingsFromEnvironment,
    rpc: &SolanaRpcClient,
) -> Result<(), String> {
    let keypair_path = settings
        .wallet_keypair_path
        .as_deref()
        .ok_or("WALLET_KEYPAIR_PATH must be set to create a lookup table")?;
    let wallet = TradingWallet::load_from_keypair_file(keypair_path)?;

    let addresses = collect_addresses(settings, rpc, &wallet).await?;
    println!(
        "[alt] {} addresses to store; wallet {}",
        addresses.len(),
        encode_public_key_as_base58(&wallet.address())
    );

    let recent_slot = rpc.get_slot().await?;
    let (table, bump) = Pubkey::find_program_address(
        &[wallet.address().as_ref(), &recent_slot.to_le_bytes()],
        &program(LOOKUP_TABLE_PROGRAM_ADDRESS),
    );
    let table_address = table.to_bytes();

    // Create on its own. Folding the first extend into it pushes the
    // transaction over the size limit.
    send_and_wait(
        rpc,
        &wallet,
        vec![create_instruction(
            &wallet.address(),
            &table_address,
            recent_slot,
            bump,
        )],
    )
    .await?;
    for batch in addresses.chunks(ADDRESSES_PER_EXTEND) {
        send_and_wait(
            rpc,
            &wallet,
            vec![extend_instruction(&wallet.address(), &table_address, batch)],
        )
        .await?;
    }

    println!(
        "[alt] created {}\n[alt] add to .env:  ADDRESS_LOOKUP_TABLES={}",
        encode_public_key_as_base58(&table_address),
        encode_public_key_as_base58(&table_address)
    );
    Ok(())
}

async fn collect_addresses(
    settings: &BotSettingsFromEnvironment,
    rpc: &SolanaRpcClient,
    wallet: &TradingWallet,
) -> Result<Vec<PublicKeyBytes>, String> {
    let mut addresses: Vec<PublicKeyBytes> = Vec::new();
    let mut add = |address: PublicKeyBytes| addresses.push(address);

    let wsol = parse_base58_public_key(WRAPPED_SOL_MINT_ADDRESS);
    let usdc = parse_base58_public_key(USDC_MINT_ADDRESS);
    add(wallet.associated_token_account(&wsol));
    add(wallet.associated_token_account(&usdc));
    add(wsol);
    add(usdc);
    for program_address in [
        TOKEN_PROGRAM_ADDRESS,
        TOKEN_2022_PROGRAM_ADDRESS,
        MEMO_PROGRAM_ADDRESS,
        SYSTEM_PROGRAM_ADDRESS,
        ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ADDRESS,
        INSTRUCTIONS_SYSVAR_ADDRESS,
    ] {
        add(parse_base58_public_key(program_address));
    }
    default_jito_tip_accounts().into_iter().for_each(&mut add);

    if let FundingMode::FlashLoan(flash) = &settings.funding_mode {
        if matches!(flash.lender, FlashLoanLender::Jupiter) {
            let admin_address = JupiterFlashLoanAccounts::flashloan_admin_address();
            let admin_bytes = fetch_account(rpc, admin_address).await?;
            let jupiter = JupiterFlashLoanAccounts::from_admin_account_bytes(&admin_bytes)?;
            for address in [
                jupiter.flashloan_admin,
                jupiter.token_reserve,
                jupiter.borrow_position,
                jupiter.rate_model,
                jupiter.liquidity,
                jupiter.vault,
                parse_base58_public_key(JUPITER_LIQUIDITY_PROGRAM_ADDRESS),
                parse_base58_public_key(JUPITER_FLASHLOAN_PROGRAM_ADDRESS),
            ] {
                add(address);
            }
        }
        // Kamino: its accounts come from the reserve; add them the same way if you use it.
        if let FlashLoanLender::Kamino {
            lending_market_address,
            sol_reserve_address,
        } = &flash.lender
        {
            let reserve_bytes = fetch_account(rpc, *sol_reserve_address).await?;
            let kamino =
                super::flash_loan_instructions::KaminoFlashLoanAccounts::from_reserve_account_bytes(
                    *lending_market_address,
                    *sol_reserve_address,
                    &reserve_bytes,
                )?;
            for address in [
                kamino.lending_market,
                kamino.lending_market_authority,
                kamino.reserve,
                kamino.reserve_supply_vault,
                kamino.reserve_fee_vault,
            ] {
                add(address);
            }
        }
    }

    for pool_config in WatchedPools::sol_usdc_pools().all_configs() {
        let pool_bytes = fetch_account(rpc, pool_config.pool_address).await?;
        let pool = main_decode_pool_account(pool_config, &pool_bytes, 0, 0).ok_or_else(|| {
            format!("could not decode pool {}", pool_config.pool_address_base58)
        })?;
        for address in [
            pool.pool_address,
            pool.token_a_mint,
            pool.token_b_mint,
            pool.token_a_vault,
            pool.token_b_vault,
        ] {
            add(address);
        }
        match pool.dex_specific_swap_accounts {
            DexSpecificSwapAccounts::OrcaWhirlpool => add(orca_oracle_address(&pool.pool_address)),
            DexSpecificSwapAccounts::RaydiumClmm {
                fee_config_address,
                price_observation_address,
            } => {
                add(fee_config_address);
                add(price_observation_address);
            }
        }
        for tick_array in tick_array_pdas_near_current_price(&pool) {
            add(tick_array.tick_array_address);
        }
    }

    let mut seen = HashSet::new();
    addresses.retain(|address| seen.insert(*address));
    Ok(addresses)
}

async fn fetch_account(rpc: &SolanaRpcClient, address: PublicKeyBytes) -> Result<Vec<u8>, String> {
    rpc.get_multiple_accounts(&[address])
        .await?
        .into_iter()
        .next()
        .flatten()
        .map(|account| account.account_data)
        .ok_or_else(|| format!("account {} not found", encode_public_key_as_base58(&address)))
}

/// `CreateLookupTable { recent_slot, bump_seed }` (instruction 0).
fn create_instruction(
    authority_and_payer: &PublicKeyBytes,
    table: &PublicKeyBytes,
    recent_slot: u64,
    bump: u8,
) -> Instruction {
    let mut data = 0u32.to_le_bytes().to_vec();
    data.extend_from_slice(&recent_slot.to_le_bytes());
    data.push(bump);
    Instruction::new_with_bytes(
        program(LOOKUP_TABLE_PROGRAM_ADDRESS),
        &data,
        table_accounts(authority_and_payer, table),
    )
}

/// `ExtendLookupTable { new_addresses }` (instruction 2).
fn extend_instruction(
    authority_and_payer: &PublicKeyBytes,
    table: &PublicKeyBytes,
    new_addresses: &[PublicKeyBytes],
) -> Instruction {
    let mut data = 2u32.to_le_bytes().to_vec();
    data.extend_from_slice(&(new_addresses.len() as u64).to_le_bytes());
    for address in new_addresses {
        data.extend_from_slice(address);
    }
    Instruction::new_with_bytes(
        program(LOOKUP_TABLE_PROGRAM_ADDRESS),
        &data,
        table_accounts(authority_and_payer, table),
    )
}

fn table_accounts(authority_and_payer: &PublicKeyBytes, table: &PublicKeyBytes) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(pubkey(*table), false),
        AccountMeta::new_readonly(pubkey(*authority_and_payer), true),
        AccountMeta::new(pubkey(*authority_and_payer), true),
        AccountMeta::new_readonly(pubkey(parse_base58_public_key(SYSTEM_PROGRAM_ADDRESS)), false),
    ]
}

async fn send_and_wait(
    rpc: &SolanaRpcClient,
    wallet: &TradingWallet,
    mut instructions: Vec<Instruction>,
) -> Result<(), String> {
    instructions.insert(0, set_compute_unit_price(10_000));
    let blockhash = rpc
        .get_latest_blockhash()
        .await?
        .parse::<solana_hash::Hash>()
        .map_err(|_| "RPC returned an invalid blockhash".to_string())?;
    let transaction = compile_and_sign_v0_transaction(wallet, &instructions, &[], blockhash)?;
    let signature = rpc.send_transaction(&transaction.wire_bytes).await?;
    println!("[alt] sent {signature}");
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(1_000)).await;
        if let Ok(Some(status)) = rpc.get_signature_status(&signature).await {
            if let Some(error) = status.error {
                return Err(format!("lookup-table transaction failed: {error}"));
            }
            if status.confirmation_status == "confirmed"
                || status.confirmation_status == "finalized"
            {
                return Ok(());
            }
        }
    }
    Err(format!("lookup-table transaction {signature} was not confirmed in 60 s; re-run"))
}
