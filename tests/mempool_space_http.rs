// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Real loopback qualification for bounded mempool-space HTTP operations.
#![cfg(feature = "mempool-space-http")]

use std::time::Duration;

use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        bitcoin::{Network, NetworkId, TransactionStatus, Txid},
        mempool_space::TransactionLimit,
    },
    error::{Error, ProviderError},
    providers::mempool_space::{MempoolSpaceClient, MempoolSpaceHttpConfig, MempoolSpaceReader},
};
use serde_json::json;

#[path = "support/esplora.rs"]
mod support;
use support::{Fixture, Reply};

const SUMMARY: &str = r#"{"count":2,"vsize":1000,"total_fee":9007199254740993,"fee_histogram":[[3.004267500000000001,100],[0.25,400]]}"#;
const FEES: &str = r#"{"fastestFee":0.100000000000000001,"halfHourFee":2,"hourFee":1,"economyFee":0,"minimumFee":3}"#;
const TXID: &str = "14396c6a212fce4a794501b5840c718a700daa9e462427fed22a7f38d5197b1e";
fn genesis() -> Reply {
    Reply::ok(Network::Mainnet.genesis_hash().to_string())
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
async fn configured(
    fixture: &Fixture,
    timeout: Duration,
    cap: usize,
    retries: u8,
) -> Result<MempoolSpaceClient, Error> {
    MempoolSpaceClient::connect(MempoolSpaceHttpConfig::new(
        NetworkId::new(Network::Mainnet, "bitcoin-fixture")?,
        HttpConfig::new(
            RpcEndpoint::new(fixture.endpoint())?.with_header("x-api-key", "secret-key")?,
            RpcLimits::new(timeout, timeout, cap, retries)?,
            "mempool-fixture",
        )?,
    ))
    .await
}
async fn client(fixture: &Fixture) -> Result<MempoolSpaceClient, Error> {
    configured(fixture, Duration::from_secs(2), 16 * 1024, 0).await
}

#[test]
fn establishment_checks_runtime_before_http_client_setup() -> Result<(), Error> {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let config = MempoolSpaceHttpConfig::new(
        NetworkId::new(Network::Mainnet, "bitcoin-fixture")?,
        HttpConfig::new(
            RpcEndpoint::new("http://127.0.0.1:1/api")?,
            RpcLimits::new(Duration::from_secs(1), Duration::from_secs(1), 1024, 0)?,
            "mempool-fixture",
        )?,
    );
    let mut future = Box::pin(MempoolSpaceClient::connect(config));
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}

#[tokio::test]
async fn establishment_deadline_covers_delegated_genesis_read()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![Reply::delayed(
        Network::Mainnet.genesis_hash().to_string(),
        Duration::from_millis(150),
    )])
    .await?;
    assert_eq!(
        configured(&fixture, Duration::from_millis(100), 1024, 0)
            .await
            .err(),
        Some(Error::Timeout)
    );
    assert_eq!(fixture.requests()?.len(), 1);
    Ok(())
}
#[tokio::test]
async fn all_provider_reads_preserve_exact_units_source_and_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let recent = format!(r#"[{{"txid":"{TXID}","fee":123,"vsize":152.25,"value":1000}}]"#);
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::ok(SUMMARY),
        genesis(),
        Reply::ok(recent),
        genesis(),
        Reply::ok(format!(r#"["{TXID}"]"#)),
        genesis(),
        Reply::ok(FEES),
    ])
    .await?;
    let client = client(&fixture).await?;
    let summary = MempoolSpaceReader::get_mempool_summary(&client).await?;
    assert_eq!(summary.value().total_fee().raw(), 9_007_199_254_740_993);
    assert_eq!(
        summary.value().histogram()[0]
            .lower_rate()
            .value()
            .canonical(),
        "3.004267500000000001"
    );
    assert_eq!(summary.context().source().provider_id(), "mempool-fixture");
    assert_eq!(summary.context().source().method(), "mempool");
    assert_eq!(summary.context().network(), client.config().network());
    assert_eq!(
        serde_json::from_str::<
            regit_web3::domain::mempool_space::Observation<
                regit_web3::domain::mempool_space::MempoolSummary,
            >,
        >(&serde_json::to_string(&summary)?)?,
        summary
    );
    let recent = client.get_recent_transactions().await?;
    assert_eq!(
        recent.value().transactions()[0]
            .virtual_size()
            .value()
            .canonical(),
        "152.25"
    );
    assert_eq!(recent.context().source().method(), "mempool-recent");
    let ids = client.get_mempool_txids(TransactionLimit::new(1)?).await?;
    assert_eq!(ids.value().txids(), &[Txid::parse(TXID)?]);
    assert_eq!(ids.context().source().method(), "mempool-txids");
    let fees = client.get_recommended_fees().await?;
    assert_eq!(
        fees.value().fastest().value().canonical(),
        "0.100000000000000001"
    );
    assert_eq!(fees.context().source().method(), "fees-recommended");
    let requests = fixture.requests()?;
    for (request, path) in requests.iter().zip([
        "block-height/0",
        "block-height/0",
        "mempool",
        "block-height/0",
        "mempool/recent",
        "block-height/0",
        "mempool/txids",
        "block-height/0",
        "v1/fees/recommended",
    ]) {
        assert!(request.starts_with(&format!("GET /api/{path} HTTP/1.1")));
        assert!(request.contains("x-api-key: secret-key"));
    }
    assert!(!format!("{client:?}").contains("secret-key"));
    Ok(())
}
#[tokio::test]
async fn compatible_transaction_and_status_delegate_without_losing_source()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::ok(include_str!("fixtures/bitcoin/segwit.hex")),
        Reply::ok(include_str!("fixtures/bitcoin/segwit.json")),
        genesis(),
        Reply::ok(r#"{"confirmed":false}"#),
    ])
    .await?;
    let client = client(&fixture).await?;
    let transaction = MempoolSpaceReader::get_transaction(&client, Txid::parse(TXID)?).await?;
    assert_eq!(transaction.value().body().txid(), Txid::parse(TXID)?);
    assert_eq!(
        transaction.context().source().provider_id(),
        "mempool-fixture"
    );
    assert_eq!(transaction.context().source().method(), "tx-with-hex");
    let status = MempoolSpaceReader::get_transaction_status(&client, Txid::parse(TXID)?).await?;
    assert_eq!(*status.value(), TransactionStatus::Unconfirmed);
    assert_eq!(status.context().source().provider_id(), "mempool-fixture");
    assert_eq!(status.context().source().method(), "tx-status");
    assert_eq!(fixture.requests()?.len(), 6);
    Ok(())
}
#[tokio::test]
async fn malformed_fee_fields_never_round_default_or_overwrite()
-> Result<(), Box<dyn std::error::Error>> {
    for body in [
        FEES.replace("\"fastestFee\":0.100000000000000001", "\"fastestFee\":null"),
        FEES.replace("0.100000000000000001", "-1"),
        FEES.replace("0.100000000000000001", "\"1\""),
        FEES.replace(
            "0.100000000000000001",
            r#"{"$serde_json::private::Number":"1"}"#,
        ),
        FEES.replace(
            "\"fastestFee\":0.100000000000000001",
            "\"fastestFee\":1,\"fastestFee\":2",
        ),
        r#"{"halfHourFee":1,"hourFee":1,"economyFee":1,"minimumFee":1}"#.to_owned(),
    ] {
        let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok(body)]).await?;
        assert_eq!(
            client(&fixture).await?.get_recommended_fees().await,
            Err(invalid())
        );
    }
    Ok(())
}
#[tokio::test]
async fn malformed_summary_bins_and_totals_fail_explicitly()
-> Result<(), Box<dyn std::error::Error>> {
    for body in [
        SUMMARY.replace("\"count\":2", "\"count\":2,\"count\":3"),
        SUMMARY.replace("\"count\":2", "\"count\":0"),
        SUMMARY.replace("9007199254740993", "18446744073709551616"),
        SUMMARY.replace("3.004267500000000001", "0.1"),
        SUMMARY.replace("3.004267500000000001", "-1"),
        SUMMARY.replace(
            "3.004267500000000001",
            r#"{"$serde_json::private::Number":"5"}"#,
        ),
        SUMMARY.replace("0.25,400", "0.25,1001"),
        SUMMARY.replace("0.25,400", "0.25,400,1"),
        SUMMARY.replace("\"total_fee\":9007199254740993", "\"total_fee\":null"),
    ] {
        let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok(body)]).await?;
        assert_eq!(
            client(&fixture).await?.get_mempool_summary().await,
            Err(invalid())
        );
    }
    Ok(())
}
#[tokio::test]
async fn recent_and_full_id_collections_fail_wholly_at_identity_and_item_limits()
-> Result<(), Box<dyn std::error::Error>> {
    let recent = json!({"txid":TXID,"fee":1,"vsize":123,"value":1});
    for values in [
        vec![recent.clone(); 2],
        vec![recent.clone(); 11],
        vec![json!({"txid":TXID,"fee":1,"vsize":0,"value":1})],
        vec![json!({"txid":TXID,"fee":1,"vsize":1,"value":2_100_000_000_000_000_u64})],
    ] {
        let fixture = Fixture::start(vec![
            genesis(),
            genesis(),
            Reply::ok(serde_json::to_string(&values)?),
        ])
        .await?;
        assert_eq!(
            client(&fixture).await?.get_recent_transactions().await,
            Err(invalid())
        );
    }
    for body in [
        json!([TXID, TXID]),
        json!([TXID, format!("{:064x}", 1)]),
        json!(["not-a-txid"]),
        json!([null]),
    ] {
        let fixture =
            Fixture::start(vec![genesis(), genesis(), Reply::ok(body.to_string())]).await?;
        assert_eq!(
            client(&fixture)
                .await?
                .get_mempool_txids(TransactionLimit::new(1)?)
                .await,
            Err(invalid())
        );
        assert!(
            fixture
                .requests()?
                .last()
                .is_some_and(|r| r.starts_with("GET /api/mempool/txids HTTP/1.1"))
        );
    }
    Ok(())
}
#[tokio::test]
async fn empty_results_remain_empty_without_misreporting_unavailable()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::ok(r#"{"count":0,"vsize":0,"total_fee":0,"fee_histogram":[]}"#),
        genesis(),
        Reply::ok("[]"),
        genesis(),
        Reply::ok("[]"),
    ])
    .await?;
    let client = client(&fixture).await?;
    assert_eq!(
        client
            .get_mempool_summary()
            .await?
            .value()
            .transaction_count(),
        0
    );
    assert!(
        client
            .get_recent_transactions()
            .await?
            .value()
            .transactions()
            .is_empty()
    );
    assert!(
        client
            .get_mempool_txids(TransactionLimit::new(1)?)
            .await?
            .value()
            .txids()
            .is_empty()
    );
    Ok(())
}
#[tokio::test]
async fn changed_genesis_stops_provider_and_reused_transaction_reads()
-> Result<(), Box<dyn std::error::Error>> {
    for transaction in [false, true] {
        let fixture = Fixture::start(vec![
            genesis(),
            Reply::ok(Network::Testnet4.genesis_hash().to_string()),
        ])
        .await?;
        let client = client(&fixture).await?;
        let error = if transaction {
            client.get_transaction(Txid::parse(TXID)?).await.err()
        } else {
            client.get_mempool_summary().await.err()
        };
        assert_eq!(error, Some(Error::Provider(ProviderError::ChainMismatch)));
        assert_eq!(fixture.requests()?.len(), 2);
    }
    Ok(())
}
#[tokio::test]
async fn safe_retry_retains_exact_path_and_provider_errors_keep_private_bodies()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::status(429, "private-provider-body"),
        Reply::ok(FEES),
    ])
    .await?;
    let retry_client = configured(&fixture, Duration::from_secs(2), 16 * 1024, 1).await?;
    retry_client.get_recommended_fees().await?;
    let requests = fixture.requests()?;
    assert_eq!(requests[2], requests[3]);
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::status(403, "private-provider-body"),
    ])
    .await?;
    let error = client(&fixture)
        .await?
        .get_mempool_summary()
        .await
        .err()
        .ok_or(Error::UnavailableData)?;
    assert_eq!(error, Error::Provider(ProviderError::HttpStatus));
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error)?,
    ] {
        assert!(!rendered.contains("private-provider-body"));
    }
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::status(404, "private-provider-body"),
    ])
    .await?;
    assert_eq!(
        client(&fixture)
            .await?
            .get_transaction_status(Txid::parse(TXID)?)
            .await,
        Err(Error::UnavailableData)
    );
    Ok(())
}
#[tokio::test]
async fn total_budget_covers_genesis_and_body_and_body_size_is_bounded()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis(),
        Reply::delayed(
            Network::Mainnet.genesis_hash().to_string(),
            Duration::from_millis(60),
        ),
        Reply::delayed(FEES, Duration::from_millis(60)),
    ])
    .await?;
    let client = configured(&fixture, Duration::from_millis(100), 16 * 1024, 0).await?;
    assert_eq!(client.get_recommended_fees().await, Err(Error::Timeout));
    let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok(" ".repeat(1000))]).await?;
    let client = configured(&fixture, Duration::from_secs(2), 100, 0).await?;
    assert_eq!(
        client.get_mempool_summary().await,
        Err(Error::Provider(ProviderError::ResponseTooLarge))
    );
    Ok(())
}
