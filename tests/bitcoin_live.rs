// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in Bitcoin indexed-read qualification through the public Rust API.
//!
//! Ordinary runs skip this test; explicit runs fail on missing inputs and source
//! errors. Each operation is a separate current index observation, not a shared
//! historical snapshot. No signing, preparation or submission occurs.

#![cfg(feature = "bitcoin-esplora")]

use regit_web3::{
    chains::bitcoin::{EsploraClient, EsploraConfig},
    domain::bitcoin::{Address, Context, HistoryCursor, Network, NetworkId, Operation, Txid},
    error::Error,
};

#[path = "support/live.rs"]
mod live;

fn configuration() -> Result<EsploraConfig, Error> {
    let network = match live::required("REGIT_WEB3_BITCOIN_NETWORK")?.as_str() {
        "mainnet" => Network::Mainnet,
        "testnet3" => Network::Testnet3,
        "testnet4" => Network::Testnet4,
        "signet" => Network::Signet,
        "regtest" => Network::Regtest,
        _ => return Err(Error::Configuration),
    };
    Ok(EsploraConfig::new(
        NetworkId::new(network, live::required("REGIT_WEB3_BITCOIN_NETWORK_ALIAS")?)?,
        live::http_config("REGIT_WEB3_BITCOIN")?,
    ))
}

fn check_context(
    context: &Context,
    client: &EsploraClient,
    operation: &Operation,
    method: &str,
    before: u64,
    after: u64,
) {
    assert_eq!(context.schema_version(), 1);
    assert_eq!(context.network(), client.config().network());
    assert_eq!(context.operation(), operation);
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
#[ignore = "requires explicit Bitcoin network, Esplora endpoint, address and transaction inputs"]
async fn bitcoin_indexed_reads_live() -> Result<(), Box<dyn std::error::Error>> {
    let config = configuration()?;
    let address = Address::parse(
        &live::required("REGIT_WEB3_BITCOIN_ADDRESS")?,
        config.network().network(),
    )?;
    let txid = Txid::parse(&live::required("REGIT_WEB3_BITCOIN_TXID")?)?;
    let client = EsploraClient::connect(config).await?;

    let before = live::unix_seconds()?;
    let balance = client.get_address_balance(address.clone()).await?;
    let after = live::unix_seconds()?;
    check_context(
        balance.context(),
        &client,
        &Operation::AddressBalance {
            address: address.clone(),
        },
        "address",
        before,
        after,
    );
    assert_eq!(balance.value().address(), &address);
    assert_eq!(balance.value().confirmed().amount().decimals(), Some(8));
    live::print_and_roundtrip(&balance)?;

    let cursor = HistoryCursor::Recent;
    let before = live::unix_seconds()?;
    let history = client.get_address_history(address.clone(), cursor).await?;
    let after = live::unix_seconds()?;
    check_context(
        history.context(),
        &client,
        &Operation::AddressHistory {
            address: address.clone(),
            cursor,
        },
        "address-txs",
        before,
        after,
    );
    assert_eq!(history.value().address(), &address);
    assert_eq!(history.value().cursor(), cursor);
    assert!(history.value().entries().len() <= 75);
    live::print_and_roundtrip(&history)?;

    let before = live::unix_seconds()?;
    let fees = client.get_fee_estimates().await?;
    let after = live::unix_seconds()?;
    check_context(
        fees.context(),
        &client,
        &Operation::FeeEstimates,
        "fee-estimates",
        before,
        after,
    );
    live::print_and_roundtrip(&fees)?;

    let before = live::unix_seconds()?;
    let status = client.get_transaction_status(txid).await?;
    let after = live::unix_seconds()?;
    check_context(
        status.context(),
        &client,
        &Operation::TransactionStatus { txid },
        "tx-status",
        before,
        after,
    );
    live::print_and_roundtrip(&status)?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit Bitcoin network, Esplora endpoint and transaction inputs"]
async fn bitcoin_transaction_live() -> Result<(), Box<dyn std::error::Error>> {
    let txid = Txid::parse(&live::required("REGIT_WEB3_BITCOIN_TXID")?)?;
    let client = EsploraClient::connect(configuration()?).await?;
    let before = live::unix_seconds()?;
    let transaction = client.get_transaction(txid).await?;
    let after = live::unix_seconds()?;
    check_context(
        transaction.context(),
        &client,
        &Operation::Transaction { txid },
        "tx-with-hex",
        before,
        after,
    );
    assert_eq!(transaction.value().body().txid(), txid);
    assert_eq!(
        transaction.value().network(),
        client.config().network().network()
    );
    live::print_and_roundtrip(&transaction)?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "derived_from_canonical_bytes": {
                "txid": transaction.value().body().txid(), "wtxid": transaction.value().body().wtxid(),
                "version": transaction.value().body().version(), "lock_time": transaction.value().body().lock_time(),
                "size": transaction.value().body().size(), "weight": transaction.value().body().weight(),
                "vsize": transaction.value().body().vsize(), "coinbase": transaction.value().body().is_coinbase(),
                "input_count": transaction.value().body().inputs().len(), "output_count": transaction.value().body().outputs().len()
            },
            "fee_from_indexed_previous_outputs": transaction.value().fee_from_previous_outputs()
        }))?
    );
    Ok(())
}
