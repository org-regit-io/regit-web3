// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in mainnet read/quote qualification through all eight Rust APIs.
//! Inputs belong to this ignored harness. Independent source observations do not
//! establish a historical hash pin, external inclusion or signed execution.
//! Current quote input resolution is unreported; exact caller input is retained.
#![cfg(feature = "thorchain-http")]
#[path = "support/live.rs"]
mod live;
use regit_web3::{
    chains::thorchain::{ThorchainClient, ThorchainHttpConfig},
    domain::thorchain::{
        AccountPrefix, Address, Asset, ChainAddress, CollectionLimit, Context, Network,
        Observation, Operation, ProtocolAmount, QuoteInputResolution, SwapParameters, SwapRequest,
        Txid,
    },
    error::Error,
};
use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;
fn configuration() -> Result<ThorchainHttpConfig, Error> {
    let prefix = match live::required("REGIT_WEB3_THORCHAIN_ACCOUNT_PREFIX")?.as_str() {
        "thor" => AccountPrefix::Thor,
        "tthor" => AccountPrefix::Tthor,
        "sthor" => AccountPrefix::Sthor,
        "cthor" => AccountPrefix::Cthor,
        _ => return Err(Error::Configuration),
    };
    Ok(ThorchainHttpConfig::new(
        Network::new(
            &live::required("REGIT_WEB3_THORCHAIN_CHAIN_ID")?,
            prefix,
            &live::required("REGIT_WEB3_THORCHAIN_NETWORK_ALIAS")?,
        )?,
        live::http_config("REGIT_WEB3_THORCHAIN")?,
    ))
}
fn check(
    context: &Context,
    client: &ThorchainClient,
    operation: &Operation,
    method: &str,
    before: u64,
) -> Result<(), Error> {
    assert_eq!(context.schema_version(), 1);
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
fn summary<T>(value: &Observation<T>, count: usize) -> Result<(), serde_json::Error>
where
    T: Serialize,
    Observation<T>: DeserializeOwned + PartialEq + Debug,
{
    let wire = serde_json::to_vec(value)?;
    assert_eq!(&serde_json::from_slice::<Observation<T>>(&wire)?, value);
    println!(
        "{} complete items; {}",
        count,
        serde_json::to_string(value.context())?
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit THORNode base/provider/network/account/asset/amount/destination/transaction/limit inputs; read and quote only"]
async fn thorchain_eight_operations_live() -> Result<(), Box<dyn std::error::Error>> {
    let account = Address::parse(&live::required("REGIT_WEB3_THORCHAIN_ACCOUNT")?)?;
    let from = Asset::parse(&live::required("REGIT_WEB3_THORCHAIN_FROM_ASSET")?)?;
    let to = Asset::parse(&live::required("REGIT_WEB3_THORCHAIN_TO_ASSET")?)?;
    let amount = ProtocolAmount::from_decimal(&live::required("REGIT_WEB3_THORCHAIN_AMOUNT")?)?;
    let destination = ChainAddress::new(
        to.chain().clone(),
        &live::required("REGIT_WEB3_THORCHAIN_DESTINATION")?,
    )?;
    let request = SwapRequest::new(
        from.clone(),
        to.clone(),
        amount,
        SwapParameters {
            destination: Some(destination),
            ..SwapParameters::default()
        },
    )?;
    let limit = CollectionLimit::new(
        live::required("REGIT_WEB3_THORCHAIN_MAX_ITEMS")?
            .parse()
            .map_err(|_| Error::Configuration)?,
    )?;
    let txid = Txid::parse(&live::required("REGIT_WEB3_THORCHAIN_TXID")?)?;
    let c = ThorchainClient::connect(configuration()?).await?;
    let before = live::unix_seconds()?;
    let v = c.get_rune_balance(account.clone()).await?;
    check(
        v.context(),
        &c,
        &Operation::RuneBalance {
            address: account.clone(),
        },
        "bank-balance",
        before,
    )?;
    assert_eq!(v.value().address(), &account);
    assert_eq!(v.value().amount().amount().decimals(), Some(8));
    live::print_and_roundtrip(&v)?;
    let before = live::unix_seconds()?;
    let v = c.get_pool(from.clone()).await?;
    check(
        v.context(),
        &c,
        &Operation::Pool {
            asset: from.clone(),
        },
        "pool",
        before,
    )?;
    assert_eq!(v.value().asset(), &from);
    live::print_and_roundtrip(&v)?;
    let before = live::unix_seconds()?;
    let v = c.get_pools(limit).await?;
    check(
        v.context(),
        &c,
        &Operation::Pools { limit },
        "pools",
        before,
    )?;
    assert!(v.value().items().iter().any(|p| p.asset() == &from));
    summary(&v, v.value().items().len())?;
    let before = live::unix_seconds()?;
    let v = c.get_network().await?;
    check(v.context(), &c, &Operation::Network, "network", before)?;
    live::print_and_roundtrip(&v)?;
    let before = live::unix_seconds()?;
    let v = c.get_swap_quote(request.clone()).await?;
    check(
        v.context(),
        &c,
        &Operation::SwapQuote {
            request: Box::new(request.clone()),
        },
        "quote-swap",
        before,
    )?;
    assert_eq!(v.value().request(), &request);
    assert_eq!(v.value().data().fees.asset, to);
    assert_eq!(
        v.value().data().input_resolution,
        QuoteInputResolution::Unreported
    );
    assert!(v.value().data().expiry.unix_seconds() > live::unix_seconds()?);
    assert!(v.value().data().memo.is_some());
    live::print_and_roundtrip(&v)?;
    qualify_state(&c, limit, txid).await?;
    Ok(())
}
async fn qualify_state(
    c: &ThorchainClient,
    limit: CollectionLimit,
    txid: Txid,
) -> Result<(), Box<dyn std::error::Error>> {
    let before = live::unix_seconds()?;
    let v = c.get_inbound_addresses(limit).await?;
    check(
        v.context(),
        c,
        &Operation::InboundAddresses { limit },
        "inbound-addresses",
        before,
    )?;
    assert!(!v.value().items().is_empty());
    summary(&v, v.value().items().len())?;
    for entry in v.value().items() {
        println!("inbound {}", serde_json::to_string(entry)?);
    }
    let before = live::unix_seconds()?;
    let v = c.get_last_blocks(limit).await?;
    check(
        v.context(),
        c,
        &Operation::LastBlocks { limit },
        "lastblock",
        before,
    )?;
    assert!(v.value().items().iter().all(|v| v.thorchain() > 0));
    live::print_and_roundtrip(&v)?;
    let before = live::unix_seconds()?;
    let v = c.get_transaction_status(txid.clone(), limit).await?;
    check(
        v.context(),
        c,
        &Operation::TransactionStatus {
            txid: txid.clone(),
            limit,
        },
        "tx-status",
        before,
    )?;
    assert_eq!(v.value().query_id(), &txid);
    assert!(
        v.value()
            .transaction()
            .ok_or(Error::UnavailableData)?
            .data()
            .id
            .same_transaction(&txid)
    );
    assert!(v.value().planned_outbounds().is_some());
    assert!(v.value().outbounds().is_some());
    live::print_and_roundtrip(&v)?;
    Ok(())
}
