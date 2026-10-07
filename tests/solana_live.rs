// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in read-only Solana qualification through the public Rust API.
//!
//! Ordinary runs skip these tests. Explicit runs require every input and fail
//! on missing configuration or provider errors. Account and token-account reads
//! are independent so an unavailable token account cannot hide account proof.

#![cfg(feature = "solana-http")]

use regit_web3::{
    chains::solana::{SolanaClient, SolanaHttpConfig},
    domain::solana::{Commitment, Context, Hash, Network, Operation, Pubkey, ReadOptions},
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
