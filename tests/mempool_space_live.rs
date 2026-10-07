// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in read-only qualification through all six public Rust methods.
//! Endpoint, expected network, provider, transaction and capacity are test inputs.
//! Each moving mempool result is independent; this test never submits a transaction.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "mempool-space-http")]

use std::{fmt::Debug, time::Duration};

use regit_web3::{
    config::{HttpConfig, RpcLimits},
    domain::{
        bitcoin::{Network, NetworkId, TransactionStatus, Txid},
        mempool_space::{Context, Operation, TransactionLimit},
    },
    error::Error,
    providers::mempool_space::{MempoolSpaceClient, MempoolSpaceHttpConfig},
};
use serde::{Serialize, de::DeserializeOwned};

#[path = "support/live.rs"]
mod live;

fn configuration() -> Result<MempoolSpaceHttpConfig, Error> {
    let network = match live::required("REGIT_WEB3_MEMPOOL_SPACE_NETWORK")?.as_str() {
        "mainnet" => Network::Mainnet,
        "testnet3" => Network::Testnet3,
        "testnet4" => Network::Testnet4,
        "signet" => Network::Signet,
        "regtest" => Network::Regtest,
        _ => return Err(Error::Configuration),
    };
    let http = live::http_config("REGIT_WEB3_MEMPOOL_SPACE")?;
    let http = HttpConfig::new(
        http.endpoint()
            .clone()
            .with_header("user-agent", "regit-web3-qualification")?,
        RpcLimits::new(
            Duration::from_secs(5),
            Duration::from_secs(45),
            16 * 1024 * 1024,
            2,
        )?,
        http.provider_id(),
    )?;
    Ok(MempoolSpaceHttpConfig::new(
        NetworkId::new(
            network,
            live::required("REGIT_WEB3_MEMPOOL_SPACE_NETWORK_ALIAS")?,
        )?,
        http,
    ))
}
fn check(
    context: &Context,
    client: &MempoolSpaceClient,
    operation: Operation,
    method: &str,
    before: u64,
) -> Result<(), Error> {
    assert_eq!(context.network(), client.config().network());
    assert_eq!(context.operation(), operation);
    assert_eq!(
        context.source().provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(context.source().method(), method);
    assert_eq!(
        context.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=live::unix_seconds()?).contains(&context.retrieved_at().unix_seconds()));
    Ok(())
}
fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(
    value: &T,
) -> Result<(), serde_json::Error> {
    assert_eq!(
        &serde_json::from_str::<T>(&serde_json::to_string(value)?)?,
        value
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit mempool API base, network/provider, confirmed transaction and full-list cap"]
async fn mempool_space_reads_live() -> Result<(), Box<dyn std::error::Error>> {
    let limit = TransactionLimit::new(
        live::required("REGIT_WEB3_MEMPOOL_SPACE_MAX_TXIDS")?
            .parse()
            .map_err(|_| Error::Configuration)?,
    )?;
    let txid = Txid::parse(&live::required("REGIT_WEB3_MEMPOOL_SPACE_TXID")?)?;
    let client = MempoolSpaceClient::connect(configuration()?).await?;
    let before = live::unix_seconds()?;
    let summary = client.get_mempool_summary().await?;
    check(
        summary.context(),
        &client,
        Operation::Summary,
        "mempool",
        before,
    )?;
    assert!(summary.value().transaction_count() > 0);
    assert!(summary.value().virtual_bytes() > 0);
    roundtrip(&summary)?;
    println!(
        "summary count={} vbytes={} fee_sat={} bins={} retrieved={}",
        summary.value().transaction_count(),
        summary.value().virtual_bytes(),
        summary.value().total_fee().raw(),
        summary.value().histogram().len(),
        summary.context().retrieved_at().unix_seconds()
    );
    let before = live::unix_seconds()?;
    let recent = client.get_recent_transactions().await?;
    check(
        recent.context(),
        &client,
        Operation::RecentTransactions,
        "mempool-recent",
        before,
    )?;
    assert!(!recent.value().transactions().is_empty());
    assert!(recent.value().transactions().len() <= 10);
    roundtrip(&recent)?;
    println!(
        "recent count={} retrieved={}",
        recent.value().transactions().len(),
        recent.context().retrieved_at().unix_seconds()
    );
    let before = live::unix_seconds()?;
    let ids = client.get_mempool_txids(limit).await?;
    check(
        ids.context(),
        &client,
        Operation::TransactionIds { limit },
        "mempool-txids",
        before,
    )?;
    assert!(!ids.value().txids().is_empty());
    assert!(ids.value().txids().len() <= limit.get() as usize);
    roundtrip(&ids)?;
    println!(
        "full_txids count={} cap={} retrieved={}",
        ids.value().txids().len(),
        limit.get(),
        ids.context().retrieved_at().unix_seconds()
    );
    let before = live::unix_seconds()?;
    let fees = client.get_recommended_fees().await?;
    check(
        fees.context(),
        &client,
        Operation::RecommendedFees,
        "fees-recommended",
        before,
    )?;
    live::print_and_roundtrip(&fees)?;
    qualify_transactions(&client, txid).await
}

async fn qualify_transactions(
    client: &MempoolSpaceClient,
    txid: Txid,
) -> Result<(), Box<dyn std::error::Error>> {
    let before = live::unix_seconds()?;
    let transaction = client.get_transaction(txid).await?;
    assert_eq!(transaction.value().body().txid(), txid);
    assert_eq!(transaction.context().network(), client.config().network());
    assert_eq!(
        transaction.context().source().provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(transaction.context().source().method(), "tx-with-hex");
    assert!(
        (before..=live::unix_seconds()?)
            .contains(&transaction.context().retrieved_at().unix_seconds())
    );
    roundtrip(&transaction)?;
    println!(
        "transaction txid={} inputs={} outputs={} retrieved={}",
        transaction.value().body().txid(),
        transaction.value().body().inputs().len(),
        transaction.value().body().outputs().len(),
        transaction.context().retrieved_at().unix_seconds()
    );
    let before = live::unix_seconds()?;
    let status = client.get_transaction_status(txid).await?;
    assert!(matches!(status.value(), TransactionStatus::Confirmed(_)));
    assert_eq!(status.context().network(), client.config().network());
    assert_eq!(
        status.context().source().provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(status.context().source().method(), "tx-status");
    assert!(
        (before..=live::unix_seconds()?).contains(&status.context().retrieved_at().unix_seconds())
    );
    live::print_and_roundtrip(&status)?;
    Ok(())
}
