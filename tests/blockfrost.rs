// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Deterministic public Blockfrost read contracts over loopback HTTP fixtures.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "blockfrost-http")]

use std::{
    future::Future,
    io,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        U256,
        cardano::{Network, NetworkId, Order, PageRequest, PageStatus, PaymentAddress},
    },
    error::{Error, ProviderError, ValidationError},
    providers::blockfrost::{BlockfrostClient, BlockfrostHttpConfig},
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

const ADDRESS: &str = "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x";
const TEST_ADDRESS: &str = "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgs68faae";
const CREDENTIAL: &str = "explicit-project-test-credential";

struct Reply {
    status: u16,
    body: String,
    body_delay: Duration,
}
impl Reply {
    fn json(value: &Value) -> Self {
        Self::raw(200, value.to_string())
    }
    fn raw(status: u16, body: String) -> Self {
        Self {
            status,
            body,
            body_delay: Duration::ZERO,
        }
    }
}

struct Fixture {
    endpoint: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<io::Result<()>>,
}
impl Fixture {
    async fn start(replies: Vec<Reply>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/api/v0/", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            for reply in replies {
                let (mut stream, _) = listener.accept().await?;
                let mut bytes = Vec::new();
                let mut buffer = [0_u8; 1024];
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    if bytes.len() > 16_384 {
                        return Err(io::Error::other("fixture headers too large"));
                    }
                    let count = stream.read(&mut buffer).await?;
                    if count == 0 {
                        return Err(io::ErrorKind::UnexpectedEof.into());
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                let headers = String::from_utf8(bytes)
                    .map_err(|_| io::Error::other("fixture invalid headers"))?;
                captured
                    .lock()
                    .map_err(|_| io::Error::other("fixture lock poisoned"))?
                    .push(headers);
                let header = format!(
                    "HTTP/1.1 {} Fixture\r\ncontent-length: {}\r\nconnection: close\r\ncontent-type: application/json\r\n\r\n",
                    reply.status,
                    reply.body.len()
                );
                stream.write_all(header.as_bytes()).await?;
                tokio::time::sleep(reply.body_delay).await;
                stream.write_all(reply.body.as_bytes()).await?;
                stream.shutdown().await?;
            }
            Ok(())
        });
        Ok(Self {
            endpoint,
            requests,
            task,
        })
    }
    fn requests(&self) -> io::Result<Vec<String>> {
        self.requests
            .lock()
            .map(|value| value.clone())
            .map_err(|_| io::Error::other("fixture lock poisoned"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn config(
    endpoint: &str,
    retries: u8,
    timeout: Duration,
    cap: usize,
) -> Result<BlockfrostHttpConfig, Error> {
    Ok(BlockfrostHttpConfig::new(
        Network::new(NetworkId::mainnet(), "fixture")?,
        HttpConfig::new(
            RpcEndpoint::new(endpoint)?.with_header("project_id", CREDENTIAL)?,
            RpcLimits::new(timeout, timeout, cap, retries)?,
            "local",
        )?,
    ))
}
fn standard_config(endpoint: &str) -> Result<BlockfrostHttpConfig, Error> {
    config(endpoint, 0, Duration::from_secs(3), 1024 * 1024)
}
fn genesis() -> Reply {
    Reply::json(&json!({"network_magic":764_824_073,"system_start":1_506_203_091}))
}
fn balance() -> Value {
    json!({"address":ADDRESS,"amount":[
        {"unit":"lovelace","quantity":"9007199254740993"},
        {"unit":format!("{}00ff","03".repeat(28)),"quantity":"18446744073709551616"}
    ],"type":"shelley","script":false})
}
fn utxo() -> Value {
    json!({"address":ADDRESS,"tx_hash":"01".repeat(32),"output_index":65535,
        "amount":[{"unit":"lovelace","quantity":u64::MAX.to_string()},
            {"unit":"03".repeat(28),"quantity":"7"}],
        "block":"02".repeat(32),"data_hash":null,"inline_datum":"19a6aa",
        "reference_script_hash":"04".repeat(28),"extra_compatible_field":true})
}
fn unix_now() -> Result<u64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

#[tokio::test(flavor = "current_thread")]
async fn verifies_genesis_each_read_and_preserves_exact_assets_and_attribution()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![genesis(), genesis(), Reply::json(&balance())]).await?;
    let client = BlockfrostClient::connect(standard_config(&fixture.endpoint)?).await?;
    let before = unix_now()?;
    let result = client.get_balance(PaymentAddress::parse(ADDRESS)?).await?;
    assert_eq!(
        result.value().native_amount().unwrap().raw(),
        U256::from(9_007_199_254_740_993_u64)
    );
    assert_eq!(result.value().native_amount().unwrap().decimals(), Some(6));
    assert_eq!(
        result.value().assets()[1].amount().raw(),
        U256::from(u64::MAX) + U256::from(1)
    );
    assert_eq!(result.value().assets()[1].amount().formatted(), None);
    assert_eq!(result.context().source().provider_id(), "local");
    assert_eq!(result.context().source().method(), "addresses");
    assert_eq!(
        result.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=unix_now()?).contains(&result.context().retrieved_at().unix_seconds()));
    assert_eq!(client.config().http_config().provider_id(), "local");
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("GET /api/v0/genesis HTTP/1.1"));
    assert!(requests[1].starts_with("GET /api/v0/genesis HTTP/1.1"));
    assert!(requests[2].starts_with(&format!("GET /api/v0/addresses/{ADDRESS} HTTP/1.1")));
    for request in requests {
        assert!(request.contains(&format!("project_id: {CREDENTIAL}")));
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn network_changes_are_provider_errors_and_wrong_caller_addresses_are_validation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![genesis(), Reply::json(&json!({"network_magic":1}))]).await?;
    let client = BlockfrostClient::connect(standard_config(&fixture.endpoint)?).await?;
    assert_eq!(
        client
            .get_balance(PaymentAddress::parse(TEST_ADDRESS)?)
            .await
            .unwrap_err(),
        Error::Validation(ValidationError::NetworkMismatch)
    );
    assert_eq!(fixture.requests()?.len(), 1);
    assert_eq!(
        client
            .get_balance(PaymentAddress::parse(ADDRESS)?)
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::ChainMismatch)
    );
    assert_eq!(fixture.requests()?.len(), 2);
    for raw in [
        r#"{"network_magic":null}"#,
        r#"{"network_magic":"764824073"}"#,
        r#"{"network_magic":764824073,"network_magic":764824073}"#,
        r#"{"network_magic":4294967296}"#,
        "{}",
    ] {
        let fixture = Fixture::start(vec![Reply::raw(200, raw.to_owned())]).await?;
        assert_eq!(
            BlockfrostClient::connect(standard_config(&fixture.endpoint)?)
                .await
                .unwrap_err(),
            invalid_response()
        );
        assert_eq!(fixture.requests()?.len(), 1);
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn unavailable_is_distinct_from_explicit_empty_success_for_both_reads()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::raw(404, "secret-provider-diagnostic".to_owned()),
        genesis(),
        Reply::json(&json!({"address":ADDRESS,"amount":[]})),
        genesis(),
        Reply::raw(404, "secret-provider-diagnostic".to_owned()),
        genesis(),
        Reply::json(&json!([])),
    ])
    .await?;
    let client = BlockfrostClient::connect(standard_config(&fixture.endpoint)?).await?;
    let address = PaymentAddress::parse(ADDRESS)?;
    assert_eq!(
        client.get_balance(address.clone()).await.unwrap_err(),
        Error::UnavailableData
    );
    let empty = client.get_balance(address.clone()).await?;
    assert!(empty.value().assets().is_empty());
    assert!(empty.value().native_amount().is_none());
    let page = PageRequest::new(1, 10, Order::Asc)?;
    assert_eq!(
        client.get_utxos(address.clone(), page).await.unwrap_err(),
        Error::UnavailableData
    );
    let empty = client.get_utxos(address, page).await?;
    assert!(empty.value().outputs().is_empty());
    assert_eq!(empty.value().status(), PageStatus::ShortPage);
    assert_eq!(fixture.requests()?.len(), 9);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_balances_are_terminal_without_lossy_quantity_or_duplicate_handling()
-> Result<(), Box<dyn std::error::Error>> {
    let mut invalid = Vec::new();
    for raw in [
        json!(1),
        json!("-1"),
        json!("01"),
        json!("1.0"),
        json!((U256::MAX.to_string() + "0")),
        json!(null),
    ] {
        let mut value = balance();
        value["amount"][0]["quantity"] = raw;
        invalid.push(value.to_string());
    }
    let mut value = balance();
    value["amount"][1] = value["amount"][0].clone();
    invalid.push(value.to_string());
    let mut value = balance();
    value["address"] = json!(TEST_ADDRESS);
    invalid.push(value.to_string());
    invalid.push(balance().to_string().replacen(
        r#""quantity":"9007199254740993""#,
        r#""quantity":"9007199254740993","quantity":"1""#,
        1,
    ));
    invalid.push(
        balance()
            .to_string()
            .replacen(r#""amount":["#, r#""amount":[],"amount":["#, 1),
    );
    for raw in invalid {
        let fixture = Fixture::start(vec![genesis(), genesis(), Reply::raw(200, raw)]).await?;
        let client = BlockfrostClient::connect(config(
            &fixture.endpoint,
            2,
            Duration::from_secs(3),
            1024 * 1024,
        )?)
        .await?;
        assert_eq!(
            client
                .get_balance(PaymentAddress::parse(ADDRESS)?)
                .await
                .unwrap_err(),
            invalid_response()
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn output_page_retains_request_creation_block_data_and_may_have_more()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![genesis(), genesis(), Reply::json(&json!([utxo()]))]).await?;
    let client = BlockfrostClient::connect(standard_config(&fixture.endpoint)?).await?;
    let page = PageRequest::new(2, 1, Order::Desc)?;
    let result = client
        .get_utxos(PaymentAddress::parse(ADDRESS)?, page)
        .await?;
    assert_eq!(result.value().requested_page(), page);
    assert_eq!(result.value().status(), PageStatus::MayHaveMore);
    let output = &result.value().outputs()[0];
    assert_eq!(output.output_index(), u16::MAX);
    assert_eq!(output.assets()[0].amount().raw(), U256::from(u64::MAX));
    assert_eq!(output.creation_block().to_string(), "02".repeat(32));
    assert_eq!(output.inline_datum().unwrap().bytes(), [0x19, 0xa6, 0xaa]);
    assert_eq!(
        output.reference_script_hash().unwrap().to_string(),
        "04".repeat(28)
    );
    assert_eq!(result.context().source().method(), "addresses-utxos");
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert!(requests[2].starts_with(&format!(
        "GET /api/v0/addresses/{ADDRESS}/utxos?page=2&count=1&order=desc HTTP/1.1"
    )));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn output_width_identity_nullable_and_duplicate_fields_are_checked()
-> Result<(), Box<dyn std::error::Error>> {
    let mut invalid = vec![json!([utxo(), utxo()]).to_string()];
    for (field, replacement) in [
        ("output_index", json!(65536)),
        ("tx_index", json!(0)),
        ("address", json!(TEST_ADDRESS)),
        ("tx_hash", json!("01")),
        ("block", json!(null)),
        ("reference_script_hash", json!("04")),
        ("inline_datum", json!("0x00")),
    ] {
        let mut value = utxo();
        value[field] = replacement;
        invalid.push(json!([value]).to_string());
    }
    for field in ["data_hash", "inline_datum", "reference_script_hash"] {
        let mut value = utxo();
        value.as_object_mut().unwrap().remove(field);
        invalid.push(json!([value]).to_string());
    }
    for raw in ["18446744073709551616", "0"] {
        let mut value = utxo();
        let index = usize::from(raw == "0");
        value["amount"][index]["quantity"] = json!(raw);
        invalid.push(json!([value]).to_string());
    }
    let mut value = utxo();
    value["amount"][1] = value["amount"][0].clone();
    invalid.push(json!([value]).to_string());
    invalid.push(json!([utxo()]).to_string().replacen(
        r#""output_index":65535"#,
        r#""output_index":65535,"output_index":0"#,
        1,
    ));
    for raw in invalid {
        let fixture = Fixture::start(vec![genesis(), genesis(), Reply::raw(200, raw)]).await?;
        let client = BlockfrostClient::connect(standard_config(&fixture.endpoint)?).await?;
        assert_eq!(
            client
                .get_utxos(
                    PaymentAddress::parse(ADDRESS)?,
                    PageRequest::new(1, 2, Order::Asc)?
                )
                .await
                .unwrap_err(),
            invalid_response()
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    // A response larger than the explicitly requested page is invalid.
    let mut second = utxo();
    second["output_index"] = json!(0);
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::json(&json!([utxo(), second])),
    ])
    .await?;
    let client = BlockfrostClient::connect(standard_config(&fixture.endpoint)?).await?;
    assert_eq!(
        client
            .get_utxos(
                PaymentAddress::parse(ADDRESS)?,
                PageRequest::new(1, 1, Order::Asc)?
            )
            .await
            .unwrap_err(),
        invalid_response()
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn transient_retries_retain_the_same_explicit_page_and_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::raw(500, "discard-secret".to_owned()),
        Reply::raw(429, "discard-secret".to_owned()),
        Reply::json(&json!([utxo()])),
    ])
    .await?;
    let client = BlockfrostClient::connect(config(
        &fixture.endpoint,
        2,
        Duration::from_secs(3),
        1024 * 1024,
    )?)
    .await?;
    client
        .get_utxos(
            PaymentAddress::parse(ADDRESS)?,
            PageRequest::new(2, 1, Order::Desc)?,
        )
        .await?;
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[2], requests[3]);
    assert_eq!(requests[3], requests[4]);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn one_deadline_covers_network_verification_and_body_reads()
-> Result<(), Box<dyn std::error::Error>> {
    let mut delayed_genesis = genesis();
    // Each body fits a fresh budget; their accumulated delays must time out.
    delayed_genesis.body_delay = Duration::from_millis(1200);
    let mut delayed_balance = Reply::json(&balance());
    delayed_balance.body_delay = Duration::from_millis(1200);
    let fixture = Fixture::start(vec![genesis(), delayed_genesis, delayed_balance]).await?;
    let client = BlockfrostClient::connect(config(
        &fixture.endpoint,
        0,
        Duration::from_secs(2),
        1024 * 1024,
    )?)
    .await?;
    assert_eq!(
        client
            .get_balance(PaymentAddress::parse(ADDRESS)?)
            .await
            .unwrap_err(),
        Error::Timeout
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert!(requests[2].starts_with(&format!("GET /api/v0/addresses/{ADDRESS} ")));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn bounded_response_and_fixed_diagnostics_never_expose_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![genesis(), genesis(), Reply::json(&balance())]).await?;
    let endpoint = format!("{}?access_token=private-query-test", fixture.endpoint);
    let configuration = config(&endpoint, 0, Duration::from_secs(3), 128)?;
    let debug = format!("{configuration:?}");
    assert!(!debug.contains(CREDENTIAL));
    assert!(!debug.contains("private-query-test"));
    assert!(!debug.contains(&fixture.endpoint));
    let client = BlockfrostClient::connect(configuration).await?;
    assert!(!format!("{client:?}").contains(CREDENTIAL));
    let error = client
        .get_balance(PaymentAddress::parse(ADDRESS)?)
        .await
        .unwrap_err();
    assert_eq!(error, Error::Provider(ProviderError::ResponseTooLarge));
    assert_eq!(error.to_string(), "provider response exceeds byte limit");
    assert!(fixture.requests()?[2].contains("?access_token=private-query-test"));
    Ok(())
}

#[test]
fn missing_runtime_returns_configuration_without_polling_a_transport()
-> Result<(), Box<dyn std::error::Error>> {
    let future = BlockfrostClient::connect(standard_config("http://example.invalid/api/v0")?);
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}
