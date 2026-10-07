// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in read-only Solana qualification through the public Rust API.
//!
//! Ordinary runs skip these tests. Explicit runs require every input and fail
//! on missing configuration or provider errors. Account and token-account reads
//! are independent so an unavailable token account cannot hide account proof.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "solana-http")]

use regit_web3::{
    chains::solana::{SolanaClient, SolanaHttpConfig},
    domain::solana::{
        Commitment, Context, ExecutionContext, ExecutionOutcome, Hash, Network, Operation, Pubkey,
        ReadOptions, Signature, StatusOptions, TransactionReadOptions, TransferIntent,
        TransferPreparation,
    },
    error::Error,
};
use serde_json::json;

#[path = "support/live.rs"]
mod live;

fn configuration() -> Result<SolanaHttpConfig, Error> {
    Ok(SolanaHttpConfig::new(
        Network::new(
            Hash::parse(&live::required("REGIT_WEB3_SOLANA_GENESIS_HASH")?)?,
            live::required("REGIT_WEB3_SOLANA_NETWORK_ALIAS")?,
        )?,
        live::http_config("REGIT_WEB3_SOLANA")?,
    ))
}

fn options() -> Result<ReadOptions, Error> {
    let commitment = match live::required("REGIT_WEB3_SOLANA_COMMITMENT")?.as_str() {
        "processed" => Commitment::Processed,
        "confirmed" => Commitment::Confirmed,
        "finalized" => Commitment::Finalized,
        _ => return Err(Error::Configuration),
    };
    Ok(ReadOptions::new(commitment, None))
}

fn check_context(
    context: &Context,
    client: &SolanaClient,
    options: ReadOptions,
    operation: Operation,
    method: &str,
    before: u64,
    after: u64,
) {
    assert_eq!(context.schema_version(), 1);
    assert_eq!(context.network(), client.config().network());
    assert_eq!(context.network().genesis_hash(), client.genesis_hash());
    assert_eq!(context.operation(), operation);
    assert_eq!(context.requested_options(), options);
    assert!(context.slot() > 0);
    assert_eq!(context.source().method(), method);
    assert_eq!(
        context.source().provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(
        context.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=after).contains(&context.retrieved_at().unix_seconds()));
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit Solana network, endpoint and present account inputs"]
async fn solana_native_and_account_live() -> Result<(), Box<dyn std::error::Error>> {
    let address = Pubkey::parse(&live::required("REGIT_WEB3_SOLANA_ACCOUNT")?)?;
    let options = options()?;
    let before = live::unix_seconds()?;
    let client = SolanaClient::connect(configuration()?).await?;
    let balance = client.get_native_balance(address, options).await?;
    let after = live::unix_seconds()?;
    check_context(
        balance.context(),
        &client,
        options,
        Operation::NativeBalance,
        "getBalance",
        before,
        after,
    );
    assert_eq!(balance.value().address(), address);
    assert_eq!(balance.value().amount().decimals(), Some(9));
    live::print_and_roundtrip(&balance)?;

    // The previously reported slot supplies a real lower bound, not a state pin.
    let account_options = ReadOptions::new(options.commitment(), Some(balance.context().slot()));
    let before = live::unix_seconds()?;
    let lookup = client.get_account(address, account_options).await?;
    let after = live::unix_seconds()?;
    check_context(
        lookup.context(),
        &client,
        account_options,
        Operation::Account,
        "getAccountInfo",
        before,
        after,
    );
    assert!(lookup.context().slot() >= balance.context().slot());
    let account = lookup.value().account().ok_or(Error::UnavailableData)?;
    assert_eq!(account.address(), address);
    assert_eq!(account.network(), client.config().network());
    assert_eq!(account.amount().decimals(), Some(9));
    // Retain provenance and metadata without dumping the program's bytecode.
    println!(
        "{}",
        serde_json::to_string(&json!({
            "context": lookup.context(),
            "value": { "address": account.address(), "owner": account.owner(),
                "amount": account.amount(), "executable": account.executable(),
                "data_bytes": account.data().len(), "rent_epoch": account.rent_epoch() }
        }))?
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit Solana network, endpoint and token-account identity inputs"]
async fn solana_token_account_live() -> Result<(), Box<dyn std::error::Error>> {
    let address = Pubkey::parse(&live::required("REGIT_WEB3_SOLANA_TOKEN_ACCOUNT")?)?;
    let mint = Pubkey::parse(&live::required("REGIT_WEB3_SOLANA_TOKEN_MINT")?)?;
    let program = Pubkey::parse(&live::required("REGIT_WEB3_SOLANA_TOKEN_PROGRAM")?)?;
    let decimals = live::required("REGIT_WEB3_SOLANA_TOKEN_DECIMALS")?.parse::<u8>()?;
    let options = options()?;
    let before = live::unix_seconds()?;
    let client = SolanaClient::connect(configuration()?).await?;
    let balance = client.get_token_balance(address, options).await?;
    let after = live::unix_seconds()?;
    check_context(
        balance.context(),
        &client,
        options,
        Operation::TokenBalance,
        "getAccountInfo",
        before,
        after,
    );
    assert_eq!(balance.value().token_account(), address);
    assert_eq!(balance.value().asset().mint(), mint);
    assert_eq!(balance.value().asset().token_program(), program);
    assert_eq!(balance.value().asset().decimals(), Some(decimals));
    assert_eq!(balance.value().amount().decimals(), Some(decimals));
    live::print_and_roundtrip(&balance)?;
    Ok(())
}

fn check_execution_context(
    context: &ExecutionContext,
    client: &SolanaClient,
    method: &str,
    before: u64,
    after: u64,
) {
    assert_eq!(context.schema_version(), 1);
    assert_eq!(context.network(), client.config().network());
    assert_eq!(context.source().method(), method);
    assert_eq!(
        context.source().provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(
        context.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=after).contains(&context.retrieved_at().unix_seconds()));
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit transaction, network and public unsigned simulation inputs; performs no submission"]
async fn solana_execution_reads_live() -> Result<(), Box<dyn std::error::Error>> {
    let signature = Signature::parse(&live::required("REGIT_WEB3_SOLANA_TRANSACTION_SIGNATURE")?)?;
    let payer = Pubkey::parse(&live::required("REGIT_WEB3_SOLANA_SIMULATION_FEE_PAYER")?)?;
    let recipient = Pubkey::parse(&live::required("REGIT_WEB3_SOLANA_SIMULATION_RECIPIENT")?)?;
    let lamports = live::required("REGIT_WEB3_SOLANA_SIMULATION_LAMPORTS")?.parse::<u64>()?;
    let expected = expected_simulation_succeeds()?;
    let read_options = options()?;
    let transaction_options = TransactionReadOptions::new(read_options.commitment(), 1)?;
    let client = SolanaClient::connect(configuration()?).await?;

    let before = live::unix_seconds()?;
    let transaction = client
        .get_transaction(signature, transaction_options)
        .await?;
    check_execution_context(
        transaction.context(),
        &client,
        "getTransaction",
        before,
        live::unix_seconds()?,
    );
    assert_eq!(transaction.context().evaluation_slot(), None);
    let body = transaction
        .value()
        .transaction
        .as_ref()
        .ok_or(Error::UnavailableData)?
        .body();
    assert_eq!(body.signature(), signature);
    live::print_and_roundtrip(&transaction)?;

    let before = live::unix_seconds()?;
    let status = client
        .get_transaction_status(
            signature,
            StatusOptions {
                search_transaction_history: true,
            },
        )
        .await?;
    check_execution_context(
        status.context(),
        &client,
        "getSignatureStatuses",
        before,
        live::unix_seconds()?,
    );
    assert!(status.context().evaluation_slot().is_some());
    assert_eq!(status.value().signature, signature);
    assert_eq!(
        status.value().status.as_ref().map(|v| v.inclusion_slot),
        transaction
            .value()
            .transaction
            .as_ref()
            .map(regit_web3::domain::solana::Transaction::inclusion_slot)
    );
    live::print_and_roundtrip(&status)?;

    let (lifetime, contextual_options) = qualify_blockhash(&client, read_options).await?;

    let prepared = TransferPreparation::new(
        client.config().network().clone(),
        TransferIntent::Native {
            fee_payer: payer,
            sender: payer,
            recipient,
            lamports,
        },
        lifetime,
    )?;
    let before = live::unix_seconds()?;
    let fee = client
        .get_fee_for_message(
            prepared.unsigned_transaction().message().clone(),
            contextual_options,
        )
        .await?;
    check_execution_context(
        fee.context(),
        &client,
        "getFeeForMessage",
        before,
        live::unix_seconds()?,
    );
    assert!(fee.value().lamports.is_some());
    live::print_and_roundtrip(&fee)?;

    let before = live::unix_seconds()?;
    let simulation = client
        .simulate_transaction(prepared.unsigned_transaction().clone(), contextual_options)
        .await?;
    check_execution_context(
        simulation.context(),
        &client,
        "simulateTransaction",
        before,
        live::unix_seconds()?,
    );
    assert_eq!(
        matches!(simulation.value().outcome(), ExecutionOutcome::Succeeded),
        expected
    );
    live::print_and_roundtrip(&simulation)?;
    Ok(())
}

async fn qualify_blockhash(
    client: &SolanaClient,
    read_options: ReadOptions,
) -> Result<(regit_web3::domain::solana::BlockhashLifetime, ReadOptions), Box<dyn std::error::Error>>
{
    let before = live::unix_seconds()?;
    let latest = client.get_latest_blockhash(read_options).await?;
    check_execution_context(
        latest.context(),
        client,
        "getLatestBlockhash",
        before,
        live::unix_seconds()?,
    );
    let minimum = latest
        .context()
        .evaluation_slot()
        .ok_or(Error::UnavailableData)?;
    let contextual_options = ReadOptions::new(read_options.commitment(), Some(minimum));
    let lifetime = latest.value().lifetime;
    live::print_and_roundtrip(&latest)?;

    let before = live::unix_seconds()?;
    let valid = client
        .is_blockhash_valid(lifetime.blockhash, contextual_options)
        .await?;
    check_execution_context(
        valid.context(),
        client,
        "isBlockhashValid",
        before,
        live::unix_seconds()?,
    );
    assert_eq!(valid.value().blockhash, lifetime.blockhash);
    assert!(valid.value().valid);
    live::print_and_roundtrip(&valid)?;

    let before = live::unix_seconds()?;
    let height = client.get_block_height(read_options.commitment()).await?;
    check_execution_context(
        height.context(),
        client,
        "getBlockHeight",
        before,
        live::unix_seconds()?,
    );
    assert_eq!(height.context().evaluation_slot(), None);
    assert!(height.value().height <= lifetime.last_valid_block_height);
    live::print_and_roundtrip(&height)?;

    Ok((lifetime, contextual_options))
}

fn expected_simulation_succeeds() -> Result<bool, Error> {
    match live::required("REGIT_WEB3_SOLANA_SIMULATION_EXPECTED_OUTCOME")?.as_str() {
        "success" => Ok(true),
        "failure" => Ok(false),
        _ => Err(Error::Configuration),
    }
}
