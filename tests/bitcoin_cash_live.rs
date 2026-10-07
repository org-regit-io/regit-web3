// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in read-only BCH qualification over certificate-verified TLS.
//! All network, identity, address, capacity and trust-anchor inputs belong to this
//! test harness. Ordinary tests do not use the network or require configuration.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "bitcoin-cash-electrum")]

use regit_web3::{
    chains::bitcoin_cash::{ElectrumClient, ElectrumConfig, ElectrumEndpoint, TlsTrustRoots},
    config::RpcLimits,
    domain::bitcoin_cash::*,
    error::Error,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    env,
    fmt::Debug,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn required(suffix: &str) -> Result<String, Error> {
    env::var(format!("REGIT_WEB3_BITCOIN_CASH_{suffix}")).map_err(|_| Error::Configuration)
}
fn seconds() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| Error::Configuration)
}
fn print<T: Serialize + DeserializeOwned + PartialEq + Debug>(
    value: &T,
) -> Result<(), serde_json::Error> {
    let encoded = serde_json::to_string(value)?;
    assert_eq!(&serde_json::from_str::<T>(&encoded)?, value);
    println!("{encoded}");
    Ok(())
}
fn context(
    context: &Context,
    network: &NetworkIdentity,
    provider: &str,
    operation: &Operation,
    before: u64,
    after: u64,
) -> Result<(), Error> {
    assert_eq!(context.schema_version(), 1);
    assert_eq!(context.network(), network);
    assert_eq!(context.operation(), operation);
    assert_eq!(context.source().provider_id(), provider);
    let method = match operation {
        Operation::AddressBalance { .. } => "address-balance",
        Operation::AddressHistory { .. } => "address-history",
        Operation::FeeEstimate { .. } => "estimate-fee",
        Operation::TransactionStatus { .. } => "transaction-status",
        Operation::Transaction { .. } => "transaction",
        Operation::UnspentOutputs { .. } => "list-unspent",
    };
    assert_eq!(context.source().method(), method);
    assert_eq!(
        context.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=after).contains(&context.retrieved_at().unix_seconds()));
    let metadata = context.protocol_metadata().ok_or(Error::UnavailableData)?;
    assert_eq!(metadata.version.as_str(), "1.6");
    assert!(metadata.cash_tokens);
    Ok(())
}
struct Inputs {
    config: ElectrumConfig,
    address: Address,
    txid: Txid,
    limit: CollectionLimit,
    range: HistoryRange,
    target: FeeTarget,
}
fn inputs() -> Result<Inputs, Box<dyn std::error::Error>> {
    let network = NetworkIdentity::mainnet(required("NETWORK_ALIAS")?)?;
    assert_eq!(
        network.genesis_hash(),
        BlockHash::parse(&required("GENESIS_HASH")?)?
    );
    assert_eq!(
        network.fork_checkpoint(),
        ForkCheckpoint::new(
            required("FORK_HEIGHT")?.parse()?,
            BlockHash::parse(&required("FORK_HASH")?)?
        )?
    );
    let provider = required("PROVIDER_ID")?;
    let endpoint = ElectrumEndpoint::new(
        required("HOST")?,
        required("PORT")?.parse()?,
        required("SERVER_NAME")?,
    )?;
    let roots = TlsTrustRoots::new(vec![std::fs::read(required("ROOT_DER_PATH")?)?])?;
    let limits = RpcLimits::new(
        Duration::from_secs(10),
        Duration::from_secs(30),
        4 * 1024 * 1024,
        1,
    )?;
    let config = ElectrumConfig::new(network.clone(), endpoint, roots, limits, provider)?;
    let address = Address::parse(&required("ADDRESS")?, network.namespace())?;
    let txid = Txid::parse(&required("TXID")?)?;
    let limit = CollectionLimit::new(required("CAPACITY")?.parse()?)?;
    let range = HistoryRange::new(
        required("HISTORY_FROM")?.parse()?,
        HistoryUpperBound::Height {
            height: required("HISTORY_TO")?.parse()?,
        },
        limit,
    )?;
    let target = FeeTarget::new(required("FEE_TARGET")?.parse()?)?;
    Ok(Inputs {
        config,
        address,
        txid,
        limit,
        range,
        target,
    })
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit BCH TLS server, trust-anchor DER and public mainnet read inputs"]
async fn bitcoin_cash_six_source_reads_live() -> Result<(), Box<dyn std::error::Error>> {
    let Inputs {
        config,
        address,
        txid,
        limit,
        range,
        target,
    } = inputs()?;
    let network = config.network().clone();
    let provider = config.provider_id().to_owned();
    let filter = TokenFilter::IncludeTokens;
    let client = ElectrumClient::connect(config).await?;
    let before = seconds()?;
    let balance = client.get_address_balance(address.clone(), filter).await?;
    context(
        balance.context(),
        &network,
        &provider,
        &Operation::AddressBalance {
            address: address.clone(),
            token_filter: filter,
        },
        before,
        seconds()?,
    )?;
    assert!(balance.value().confirmed.raw() > 0);
    print(&balance)?;
    let before = seconds()?;
    let history = client.get_address_history(address.clone(), range).await?;
    context(
        history.context(),
        &network,
        &provider,
        &Operation::AddressHistory {
            address: address.clone(),
            range,
        },
        before,
        seconds()?,
    )?;
    assert!(!history.value().entries().is_empty());
    print(&history)?;
    let before = seconds()?;
    let fee = client.get_fee_estimate(target).await?;
    context(
        fee.context(),
        &network,
        &provider,
        &Operation::FeeEstimate { target },
        before,
        seconds()?,
    )?;
    assert!(fee.value().bch_per_kilobyte().is_some());
    print(&fee)?;
    let before = seconds()?;
    let status = client.get_transaction_status(txid).await?;
    context(
        status.context(),
        &network,
        &provider,
        &Operation::TransactionStatus { txid },
        before,
        seconds()?,
    )?;
    assert!(status.value().inclusion().is_some());
    print(&status)?;
    let before = seconds()?;
    let transaction = client.get_transaction(txid, limit).await?;
    context(
        transaction.context(),
        &network,
        &provider,
        &Operation::Transaction { txid, limit },
        before,
        seconds()?,
    )?;
    assert_eq!(transaction.value().data().raw.txid(), txid);
    assert!(!transaction.value().data().outputs.is_empty());
    print(&transaction)?;
    let before = seconds()?;
    let unspent = client
        .get_unspent_outputs(address.clone(), filter, limit)
        .await?;
    context(
        unspent.context(),
        &network,
        &provider,
        &Operation::UnspentOutputs {
            address,
            token_filter: filter,
            limit,
        },
        before,
        seconds()?,
    )?;
    assert!(!unspent.value().outputs().is_empty());
    print(&unspent)?;
    Ok(())
}
