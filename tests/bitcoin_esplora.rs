// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Public Bitcoin Esplora reads through deterministic real HTTP exchanges.

#![cfg(feature = "bitcoin-esplora")]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regit_web3::{
    chains::bitcoin::{EsploraClient, EsploraConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::bitcoin::{
        Address, AddressBalance, Completeness, HistoryCursor, Network, NetworkId, Observation,
        Operation, TransactionStatus, Txid,
    },
    error::{Error, ProviderError, ValidationError},
};
use serde_json::{Value, json};

#[path = "support/esplora.rs"]
mod support;
use support::{Fixture, Reply};

const ADDRESS: &str = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT";
fn genesis() -> Reply {
    Reply::ok(Network::Mainnet.genesis_hash().to_string())
}
fn address() -> Result<Address, Error> {
    Address::parse(ADDRESS, Network::Mainnet)
}
fn txid(index: u8) -> Result<Txid, Error> {
    Txid::parse(&format!("{index:064x}"))
}
fn config(
    endpoint: &str,
    retries: u8,
    timeout: Duration,
    bytes: usize,
) -> Result<EsploraConfig, Error> {
    let limits = RpcLimits::new(timeout, timeout, bytes, retries)?;
    Ok(EsploraConfig::new(
        NetworkId::new(Network::Mainnet, "bitcoin-main")?,
        HttpConfig::new(RpcEndpoint::new(endpoint)?, limits, "esplora-fixture")?,
    ))
}
async fn client(fixture: &Fixture) -> Result<EsploraClient, Error> {
    EsploraClient::connect(config(
        fixture.endpoint(),
        0,
        Duration::from_secs(2),
        64 * 1024,
    )?)
    .await
}
fn stats(funded: u64, spent: u64, mempool_funded: u64, mempool_spent: u64) -> String {
    json!({"address":ADDRESS,"chain_stats":{"funded_txo_sum":funded,"spent_txo_sum":spent,"tx_count":7},"mempool_stats":{"funded_txo_sum":mempool_funded,"spent_txo_sum":mempool_spent,"tx_count":3}}).to_string()
}
fn inclusion() -> Value {
    json!({"confirmed":true,"block_height":800_000,"block_hash":Network::Mainnet.genesis_hash().to_string(),"block_time":1_700_000_000})
}

#[tokio::test]
async fn establishment_and_each_balance_verify_genesis_and_preserve_exact_source_facts() {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::ok(stats(u64::MAX, 1, 0, u64::MAX)),
    ])
    .await
    .unwrap();
    let client = client(&fixture).await.unwrap();
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let observation = client
        .get_address_balance(address().unwrap())
        .await
        .unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert_eq!(observation.value().confirmed().raw(), u64::MAX - 1);
    assert_eq!(
        observation.value().mempool_delta().raw(),
        -i128::from(u64::MAX)
    );
    assert_eq!(observation.context().schema_version(), 1);
    assert_eq!(
        observation.context().source().provider_id(),
        "esplora-fixture"
    );
    assert_eq!(observation.context().source().method(), "address");
    assert_eq!(
        observation.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=after).contains(&observation.context().retrieved_at().unix_seconds()));
    assert_eq!(
        serde_json::from_value::<Observation<AddressBalance>>(
            serde_json::to_value(&observation).unwrap()
        )
        .unwrap(),
        observation
    );
    let requests = fixture.requests().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("GET /api/block-height/0 HTTP/1.1"));
    assert!(requests[1].starts_with("GET /api/block-height/0 HTTP/1.1"));
    assert!(requests[2].starts_with(&format!("GET /api/address/{ADDRESS} HTTP/1.1")));
}

#[tokio::test]
async fn remote_genesis_change_and_malformed_identity_stop_the_read() {
    for (remote, expected) in [
        (
            Network::Signet.genesis_hash().to_string(),
            Error::Provider(ProviderError::ChainMismatch),
        ),
        (
            "FAKE_REMOTE_SECRET".to_owned(),
            Error::Provider(ProviderError::InvalidResponse),
        ),
    ] {
        let fixture = Fixture::start(vec![genesis(), Reply::ok(remote)])
            .await
            .unwrap();
        let client = client(&fixture).await.unwrap();
        assert_eq!(
            client
                .get_address_balance(address().unwrap())
                .await
                .unwrap_err(),
            expected
        );
        assert_eq!(fixture.requests().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn caller_address_network_mismatch_does_not_issue_an_operation() {
    let fixture = Fixture::start(vec![genesis()]).await.unwrap();
    let client = client(&fixture).await.unwrap();
    let shared = Address::parse("mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn", Network::Signet).unwrap();
    assert_eq!(
        client.get_address_balance(shared).await.unwrap_err(),
        Error::Validation(ValidationError::NetworkMismatch)
    );
    assert_eq!(fixture.requests().unwrap().len(), 1);
}

#[tokio::test]
async fn zero_is_exact_and_bad_balance_records_are_not_defaulted_or_overwritten() {
    let mut wrong_address: Value = serde_json::from_str(&stats(2, 1, 0, 0)).unwrap();
    wrong_address["address"] = json!("1BitcoinEaterAddressDontSendf59kuE");
    for body in [
        "null".to_owned(),
        stats(0, 1, 0, 0),
        wrong_address.to_string(),
        stats(2, 1, 0, 0).replace(
            "\"funded_txo_sum\":2",
            "\"funded_txo_sum\":2,\"funded_txo_sum\":3",
        ),
        stats(2, 1, 0, 0).replace(
            "\"funded_txo_sum\":2",
            "\"funded_txo_sum\":18446744073709551616",
        ),
        stats(2, 1, 0, 0).replace("\"funded_txo_sum\":2", "\"funded_txo_sum\":1.5"),
    ] {
        let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok(body)])
            .await
            .unwrap();
        let client = client(&fixture).await.unwrap();
        assert_eq!(
            client
                .get_address_balance(address().unwrap())
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
    }
    let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok(stats(0, 0, 0, 0))])
        .await
        .unwrap();
    assert_eq!(
        client(&fixture)
            .await
            .unwrap()
            .get_address_balance(address().unwrap())
            .await
            .unwrap()
            .value()
            .confirmed()
            .raw(),
        0
    );
}

#[tokio::test]
async fn dynamic_fee_horizons_and_fractional_numbers_remain_exact() {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::ok(
            r#"{"1":0,"18446744073709551615":9007199254740993.1234567890123456789,"2":1.25e-3}"#,
        ),
    ])
    .await
    .unwrap();
    let value = client(&fixture)
        .await
        .unwrap()
        .get_fee_estimates()
        .await
        .unwrap();
    assert_eq!(
        value.value().rates()[&u64::MAX].canonical(),
        "9007199254740993.1234567890123456789"
    );
    assert_eq!(value.value().rates()[&2].canonical(), "0.00125");
    assert_eq!(value.value().rates()[&1].canonical(), "0");
    assert_eq!(value.context().operation(), &Operation::FeeEstimates);
    assert!(fixture.requests().unwrap()[2].starts_with("GET /api/fee-estimates HTTP/1.1"));
}

#[tokio::test]
async fn fee_duplicates_and_malformed_numbers_fail_before_map_overwrite() {
    for body in [
        r#"{"1":1,"1":2}"#,
        r#"{"01":1}"#,
        r#"{"0":1}"#,
        r#"{"1":-1}"#,
        r#"{"1":"1"}"#,
        r#"{"1":null}"#,
        r#"{"1":1e5000}"#,
        r#"{"1":{"$serde_json::private::Number":"1.25"}}"#,
        r#"{"1":{"$serde_json::private::Number":"1.25","extra":1}}"#,
        r#"{"1":{"nested":{"$serde_json::private::Number":"1.25"}}}"#,
        "null",
        "[]",
    ] {
        let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok(body)])
            .await
            .unwrap();
        assert_eq!(
            client(&fixture)
                .await
                .unwrap()
                .get_fee_estimates()
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
    }
}

#[tokio::test]
async fn transaction_absence_unconfirmed_and_complete_inclusion_are_distinct() {
    for (reply, expected) in [
        (Reply::status(404, "FAKE_REMOTE_SECRET"), None),
        (
            Reply::ok(r#"{"confirmed":false}"#),
            Some(TransactionStatus::Unconfirmed),
        ),
        (
            Reply::ok(inclusion().to_string()),
            Some(TransactionStatus::Confirmed(
                regit_web3::domain::bitcoin::BlockReference::new(
                    800_000,
                    Network::Mainnet.genesis_hash(),
                    regit_web3::domain::Timestamp::from_unix_seconds(1_700_000_000),
                ),
            )),
        ),
    ] {
        let fixture = Fixture::start(vec![genesis(), genesis(), reply])
            .await
            .unwrap();
        let requested = txid(1).unwrap();
        let result = client(&fixture)
            .await
            .unwrap()
            .get_transaction_status(requested)
            .await;
        if let Some(expected) = expected {
            let observation = result.unwrap();
            assert_eq!(*observation.value(), expected);
            assert_eq!(
                observation.context().operation(),
                &Operation::TransactionStatus { txid: requested }
            );
        } else {
            assert_eq!(result.unwrap_err(), Error::UnavailableData);
        }
    }
}

#[tokio::test]
async fn incomplete_inconsistent_and_duplicate_status_fields_are_rejected() {
    for body in [
        "null",
        r#"{"confirmed":true}"#,
        r#"{"confirmed":false,"block_height":7}"#,
        r#"{"confirmed":false,"confirmed":true}"#,
        r#"{"confirmed":true,"block_height":1,"block_hash":null,"block_time":1}"#,
        r#"{"confirmed":true,"block_height":1,"block_hash":"FAKE_SECRET","block_time":1}"#,
    ] {
        let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok(body)])
            .await
            .unwrap();
        assert_eq!(
            client(&fixture)
                .await
                .unwrap()
                .get_transaction_status(txid(1).unwrap())
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
    }
}

#[tokio::test]
async fn recent_history_accepts_50_mempool_plus_25_confirmed_and_retains_confirmed_cursor() {
    let entries = (1..=75).map(|index| json!({"txid":txid(index).unwrap().to_string(),"fee":u64::MAX,"status":if index <= 50 { json!({"confirmed":false}) } else { inclusion() },"vin":[],"vout":[]})).collect::<Vec<_>>();
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::ok(json!(entries).to_string()),
        genesis(),
        Reply::ok("[]"),
    ])
    .await
    .unwrap();
    let client = client(&fixture).await.unwrap();
    let page = client
        .get_address_history(address().unwrap(), HistoryCursor::Recent)
        .await
        .unwrap();
    assert_eq!(page.value().entries().len(), 75);
    assert_eq!(
        page.value().mempool_completeness(),
        Some(Completeness::MayHaveMore)
    );
    let cursor = page.value().next_cursor().unwrap();
    let next = client
        .get_address_history(address().unwrap(), cursor)
        .await
        .unwrap();
    assert_eq!(next.value().entries().len(), 0);
    assert_eq!(next.value().mempool_completeness(), None);
    assert!(fixture.requests().unwrap()[4].starts_with(&format!(
        "GET /api/address/{ADDRESS}/txs/chain/{} HTTP/1.1",
        txid(75).unwrap()
    )));
}

#[tokio::test]
async fn history_identity_duplicates_limits_and_cursor_agreement_are_enforced() {
    let entry = json!({"txid":txid(1).unwrap().to_string(),"fee":1,"status":{"confirmed":false}});
    for (cursor, body) in [(HistoryCursor::Recent, "null".to_owned()), (HistoryCursor::Recent, json!([entry.clone(),entry.clone()]).to_string()), (HistoryCursor::Confirmed { after: None }, json!([entry.clone()]).to_string()), (HistoryCursor::Recent, json!((1..=51).map(|index| json!({"txid":txid(index).unwrap().to_string(),"fee":0,"status":{"confirmed":false}})).collect::<Vec<_>>()).to_string()), (HistoryCursor::Confirmed { after: Some(txid(1).unwrap()) }, json!([{"txid":txid(1).unwrap().to_string(),"fee":0,"status":inclusion()}]).to_string())] {
        let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok(body)]).await.unwrap();
        assert_eq!(client(&fixture).await.unwrap().get_address_history(address().unwrap(), cursor).await.unwrap_err(), Error::Provider(ProviderError::InvalidResponse));
    }
}

#[tokio::test]
async fn safe_retries_reuse_exact_query_credentials_and_errors_are_redacted() {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::status(429, "FAKE_REMOTE_SECRET"),
        Reply::status(503, "FAKE_REMOTE_SECRET"),
        Reply::ok(stats(9, 2, 0, 0)),
    ])
    .await
    .unwrap();
    let endpoint = format!("{}?key=FAKE_QUERY_SECRET", fixture.endpoint());
    let limits = RpcLimits::new(Duration::from_secs(2), Duration::from_secs(2), 1024, 2).unwrap();
    let http = HttpConfig::new(
        RpcEndpoint::new(&endpoint)
            .unwrap()
            .with_header("authorization", "Bearer FAKE_HEADER_SECRET")
            .unwrap(),
        limits,
        "fixture",
    )
    .unwrap();
    let client = EsploraClient::connect(EsploraConfig::new(
        NetworkId::new(Network::Mainnet, "main").unwrap(),
        http,
    ))
    .await
    .unwrap();
    assert_eq!(
        client
            .get_address_balance(address().unwrap())
            .await
            .unwrap()
            .value()
            .confirmed()
            .raw(),
        7
    );
    let requests = fixture.requests().unwrap();
    assert_eq!(requests.len(), 5);
    for request in &requests[2..] {
        assert!(request.starts_with(&format!(
            "GET /api/address/{ADDRESS}?key=FAKE_QUERY_SECRET HTTP/1.1"
        )));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer fake_header_secret")
        );
    }
    let debug = format!("{client:?}");
    for secret in [
        "FAKE_QUERY_SECRET",
        "FAKE_HEADER_SECRET",
        "FAKE_REMOTE_SECRET",
        fixture.endpoint(),
    ] {
        assert!(!debug.contains(secret));
    }
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::status(401, "FAKE_REMOTE_SECRET"),
    ])
    .await
    .unwrap();
    let failure = self::client(&fixture)
        .await
        .unwrap()
        .get_address_balance(address().unwrap())
        .await
        .unwrap_err();
    assert_eq!(failure, Error::Provider(ProviderError::HttpStatus));
    assert!(!failure.to_string().contains("FAKE_REMOTE_SECRET"));
}

#[tokio::test]
async fn one_deadline_covers_genesis_and_response_body_consumption() {
    // Each response fits a fresh deadline; their combined delay must not.
    let fixture = Fixture::start(vec![
        genesis(),
        Reply::delayed(
            Network::Mainnet.genesis_hash().to_string(),
            Duration::from_millis(1200),
        ),
        Reply::delayed(stats(0, 0, 0, 0), Duration::from_millis(1200)),
    ])
    .await
    .unwrap();
    let client = EsploraClient::connect(
        config(fixture.endpoint(), 0, Duration::from_secs(2), 1024).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        client
            .get_address_balance(address().unwrap())
            .await
            .unwrap_err(),
        Error::Timeout
    );
    let requests = fixture.requests().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[2].starts_with(&format!("GET /api/address/{ADDRESS} ")));
}

#[tokio::test]
async fn response_bytes_are_bounded_and_malformed_remote_text_is_redacted() {
    let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok("X".repeat(129))])
        .await
        .unwrap();
    let client =
        EsploraClient::connect(config(fixture.endpoint(), 0, Duration::from_secs(2), 128).unwrap())
            .await
            .unwrap();
    assert_eq!(
        client.get_fee_estimates().await.unwrap_err(),
        Error::Provider(ProviderError::ResponseTooLarge)
    );
    let fixture = Fixture::start(vec![genesis(), genesis(), Reply::ok("FAKE_REMOTE_SECRET")])
        .await
        .unwrap();
    let error = self::client(&fixture)
        .await
        .unwrap()
        .get_fee_estimates()
        .await
        .unwrap_err();
    assert_eq!(error, Error::Provider(ProviderError::InvalidResponse));
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error).unwrap(),
    ] {
        assert!(!rendered.contains("FAKE_REMOTE_SECRET"));
    }
}
