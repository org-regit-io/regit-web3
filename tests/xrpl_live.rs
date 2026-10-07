// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Opt-in read-only XRPL qualification through the public Rust API.
//!
//! The harness, not the library, reads explicitly required endpoint, network ID,
//! account and provider label from `REGIT_WEB3_XRPL_RPC_URL`,
//! `REGIT_WEB3_XRPL_NETWORK_ID`, `REGIT_WEB3_XRPL_ACCOUNT` and
//! `REGIT_WEB3_XRPL_PROVIDER_ID` and `REGIT_WEB3_XRPL_HISTORY_MIN_LEDGER`.
//! No signing, funding or submission is performed.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "xrpl-http")]

use std::{
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use regit_web3::{
    chains::xrpl::{XrplClient, XrplHttpConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::xrpl::{Address, HistoryRequest, LedgerRange, Network, NetworkId, PageRequest},
    error::Error,
};

fn required(name: &str) -> Result<String, Error> {
    env::var(name).map_err(|_| Error::Configuration)
}
fn now() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .map_err(|_| Error::Configuration)
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires an explicit read-only XRPL endpoint, network, account and provider label"]
async fn xrpl_reads_live() -> Result<(), Error> {
    let account = Address::parse(&required("REGIT_WEB3_XRPL_ACCOUNT")?)?;
    let network = Network::new(
        NetworkId::new(
            required("REGIT_WEB3_XRPL_NETWORK_ID")?
                .parse()
                .map_err(|_| Error::Configuration)?,
        ),
        "live",
    )?;
    let config = XrplHttpConfig::new(
        network.clone(),
        HttpConfig::new(
            RpcEndpoint::new(&required("REGIT_WEB3_XRPL_RPC_URL")?)?,
            RpcLimits::new(
                Duration::from_secs(10),
                Duration::from_secs(30),
                1_048_576,
                1,
            )?,
            required("REGIT_WEB3_XRPL_PROVIDER_ID")?,
        )?,
    );
    let before = now()?;
    let client = XrplClient::connect(config).await?;
    println!("XRPL live: network identity verified");
    let balance = client.get_account_balance(account, None).await?;
    println!("XRPL live: validated account balance read");
    let lines = client
        .get_trust_lines(
            account,
            PageRequest::new(
                10,
                balance
                    .context()
                    .ledger()
                    .and_then(regit_web3::domain::xrpl::Ledger::hash),
                None,
            )?,
        )
        .await?;
    println!("XRPL live: ledger-bound trustline page read");
    let fee = client.get_fee_estimate().await?;
    let maximum = balance
        .context()
        .ledger()
        .ok_or(Error::Configuration)?
        .index();
    qualify_transactions(&client, account, maximum, before).await?;
    let after = now()?;
    assert_eq!(balance.value().account(), account);
    assert_eq!(lines.value().account(), account);
    assert_eq!(balance.context().ledger(), lines.context().ledger());
    assert!(
        balance
            .context()
            .ledger()
            .is_some_and(regit_web3::domain::xrpl::Ledger::validated)
    );
    assert!(
        balance
            .context()
            .ledger()
            .and_then(regit_web3::domain::xrpl::Ledger::hash)
            .is_some()
    );
    assert_eq!(balance.context().network().identity(), network.identity());
    assert_eq!(fee.context().network().identity(), network.identity());
    assert!(
        !fee.context()
            .ledger()
            .is_some_and(regit_web3::domain::xrpl::Ledger::validated)
    );
    assert!(
        fee.context()
            .ledger()
            .and_then(regit_web3::domain::xrpl::Ledger::hash)
            .is_none()
    );
    assert_eq!(
        balance.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        balance.context().source().provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(lines.context().source().method(), "account_lines");
    assert_eq!(fee.context().source().method(), "fee");
    for observed in [balance.context(), lines.context(), fee.context()] {
        assert!((before..=after).contains(&observed.retrieved_at().unix_seconds()));
    }
    for value in [
        serde_json::to_string(&balance),
        serde_json::to_string(&lines),
        serde_json::to_string(&fee),
    ] {
        println!("{}", value.map_err(|_| Error::Configuration)?);
    }
    Ok(())
}

async fn qualify_transactions(
    client: &XrplClient,
    account: Address,
    maximum: u32,
    before: u64,
) -> Result<(), Error> {
    let minimum = required("REGIT_WEB3_XRPL_HISTORY_MIN_LEDGER")?
        .parse()
        .map_err(|_| Error::Configuration)?;
    let history = client
        .get_account_history(
            account,
            HistoryRequest::new(LedgerRange::new(minimum, maximum)?, 2, false, None)?,
        )
        .await?;
    assert_eq!(history.value().account(), account);
    assert!(history.context().ledger().is_none());
    let entry = history
        .value()
        .transactions()
        .first()
        .ok_or(Error::UnavailableData)?;
    let transaction = client.get_transaction(entry.hash()).await?;
    let status = client.get_transaction_status(entry.hash()).await?;
    assert_eq!(transaction.value().hash(), entry.hash());
    assert_eq!(transaction.value().payload(), entry.payload());
    assert_eq!(transaction.value().metadata(), entry.metadata());
    assert_eq!(status.value().hash(), entry.hash());
    assert!(status.value().validated());
    assert!(status.value().execution().is_some());
    assert_eq!(transaction.context().ledger(), status.context().ledger());
    assert_eq!(
        transaction
            .value()
            .ledger()
            .map(regit_web3::domain::xrpl::Ledger::index),
        entry.ledger().map(regit_web3::domain::xrpl::Ledger::index)
    );
    let after = now()?;
    for context in [history.context(), transaction.context(), status.context()] {
        assert!((before..=after).contains(&context.retrieved_at().unix_seconds()));
        assert_eq!(context.network(), client.config().network());
        assert_eq!(
            context.source().integration_version(),
            env!("CARGO_PKG_VERSION")
        );
    }
    for serialized in [
        serde_json::to_string(&history),
        serde_json::to_string(&transaction),
        serde_json::to_string(&status),
    ] {
        println!("{}", serialized.map_err(|_| Error::Configuration)?);
    }
    Ok(())
}
