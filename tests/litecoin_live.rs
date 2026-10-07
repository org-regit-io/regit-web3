// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in keyless mainnet read qualification; ordinary tests stay offline.
#![cfg(feature = "litecoin-http")]
use regit_web3::{
    chains::litecoin::{BlockCypherClient, BlockCypherConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::litecoin::*,
    error::Error,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{fmt::Debug, time::Duration};
fn input(suffix: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(std::env::var(format!("REGIT_WEB3_LITECOIN_{suffix}"))?)
}
fn record<T: Serialize + DeserializeOwned + Eq + Debug>(
    name: &str,
    value: &T,
) -> Result<(), serde_json::Error> {
    let json = serde_json::to_string(value)?;
    assert_eq!(&serde_json::from_str::<T>(&json)?, value);
    println!("{name}={json}");
    Ok(())
}
async fn pace() {
    // Caller-owned qualification pacing for the documented keyless free tier.
    tokio::time::sleep(Duration::from_millis(1100)).await;
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit BlockCypher URL/provider/mainnet genesis/alias/public address/transaction/page capacities; read only"]
async fn litecoin_five_operations_live() -> Result<(), Box<dyn std::error::Error>> {
    let identity = NetworkId::new(Network::Mainnet, input("NETWORK_ALIAS")?)?;
    if BlockHash::parse(&input("GENESIS_HASH")?)? != *identity.genesis_hash() {
        return Err(Error::Configuration.into());
    }
    let address = Address::parse(&input("ADDRESS")?, Network::Mainnet)?;
    let txid = Txid::parse(&input("TXID")?)?;
    let minimum = input("HISTORY_MINIMUM")?.parse::<u16>()?;
    let history_capacity = input("HISTORY_CAPACITY")?.parse::<u32>()?;
    let transaction_capacity = input("TRANSACTION_CAPACITY")?.parse::<u32>()?;
    let request = HistoryRequest::new(None, minimum, history_capacity)?;
    let http = HttpConfig::new(
        RpcEndpoint::new(&input("URL")?)?,
        RpcLimits::new(
            Duration::from_secs(15),
            Duration::from_secs(45),
            16 * 1024 * 1024,
            0,
        )?,
        input("PROVIDER_ID")?,
    )?;
    let reader =
        BlockCypherClient::connect(BlockCypherConfig::new(identity.clone(), http)?).await?;
    pace().await;
    let balance = reader.get_address_balance(address.clone()).await?;
    assert_eq!(balance.context().network(), &identity);
    assert_eq!(balance.value().address(), &address);
    assert!(balance.value().data().confirmed.raw() > 0);
    record("balance", &balance)?;
    pace().await;
    let history = reader.get_address_history(address.clone(), request).await?;
    assert_eq!(history.context().network(), &identity);
    assert!(!history.value().confirmed().is_empty());
    record("history", &history)?;
    if let Some(before) = history.value().next_before_height() {
        pace().await;
        let page = reader
            .get_address_history(
                address,
                HistoryRequest::new(Some(before), minimum, history_capacity)?,
            )
            .await?;
        assert_eq!(page.value().request().before_height(), Some(before));
        assert_eq!(page.context().network(), &identity);
        record("history_continuation", &page)?;
    }
    pace().await;
    let fees = reader.get_fee_estimates().await?;
    assert_eq!(fees.context().network(), &identity);
    assert!(fees.value().tip_height > 0);
    record("fee_buckets", &fees)?;
    pace().await;
    let transaction = reader.get_transaction(txid, transaction_capacity).await?;
    assert_eq!(transaction.context().network(), &identity);
    assert_eq!(transaction.value().data().status.txid(), txid);
    assert!(!transaction.value().data().inputs.is_empty());
    assert!(!transaction.value().data().outputs.is_empty());
    record("indexed_transaction", &transaction)?;
    pace().await;
    let status = reader.get_transaction_status(txid).await?;
    assert_eq!(status.context().network(), &identity);
    assert_eq!(status.value().txid(), txid);
    assert!(status.value().inclusion().is_some());
    record("transaction_status", &status)?;
    println!(
        "qualified=5_methods; history_continuation_is_a_separate_point_in_time; raw_identity_consensus_signature_validation=not_performed"
    );
    Ok(())
}
