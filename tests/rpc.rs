// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Public EVM client contracts against deterministic loopback HTTP fixtures.

#![cfg(feature = "evm-http")]

#[path = "support/rpc_server.rs"]
mod rpc_server;

use std::time::{Duration, Instant};

use regit_web3::{
    chains::evm::EvmClient,
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::{BlockSelector, ChainId, NetworkId},
    error::{Error, ProviderError},
};
use serde_json::{Value, json};

use rpc_server::{Fixture, Framing, Reply};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn configured(
    endpoint: RpcEndpoint,
    expected: ChainId,
    max_bytes: usize,
    retries: u8,
    timeout: Duration,
) -> Result<EvmConfig, Error> {
    EvmConfig::new(
        NetworkId::new(expected, "fixture-network")?,
        endpoint,
        18,
        Some("NATIVE".to_owned()),
        BlockSelector::Latest,
        RpcLimits::new(
            Duration::from_millis(200).min(timeout),
            timeout,
            max_bytes,
            retries,
        )?,
        "fixture-provider",
    )
}

fn config(fixture: &Fixture, retries: u8) -> Result<EvmConfig, Error> {
    configured(
        RpcEndpoint::new(&fixture.endpoint)?,
        ChainId::from(1),
        4096,
        retries,
        Duration::from_secs(2),
    )
}

#[test]
fn connection_without_a_tokio_runtime_is_an_immediate_configuration_error() -> TestResult {
    let config = configured(
        RpcEndpoint::new("http://127.0.0.1:1/rpc")?,
        ChainId::from(1),
        4096,
        0,
        Duration::from_secs(2),
    )?;
    let mut connection = std::pin::pin!(EvmClient::connect(config));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(
        std::future::Future::poll(connection.as_mut(), &mut context),
        std::task::Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}

#[tokio::test]
async fn establishment_checks_exact_chain_ids_and_emits_a_typed_handshake() -> TestResult {
    let maximum = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    for (quantity, expected) in [
        ("0x0".to_owned(), "0"),
        ("0x1".to_owned(), "1"),
        ("0x20000000000001".to_owned(), "9007199254740993"),
        (format!("0x{}", "f".repeat(64)), maximum),
        (format!("0x{}", "F".repeat(64)), maximum),
    ] {
        let expected = ChainId::from_decimal(expected)?;
        let fixture = Fixture::start(vec![Reply::result(&json!(quantity))?]).await?;
        let client = EvmClient::connect(configured(
            RpcEndpoint::new(&fixture.endpoint)?,
            expected,
            4096,
            0,
            Duration::from_secs(2),
        )?)
        .await?;
        assert_eq!(client.chain_id(), expected);
        assert_eq!(client.config().network().chain_id(), expected);
        let requests = fixture.requests()?;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].target, "/rpc");
        assert!(requests[0].headers.starts_with("POST /rpc HTTP/1.1"));
        assert!(
            requests[0]
                .headers
                .to_ascii_lowercase()
                .contains("content-type: application/json")
        );
        assert_eq!(
            requests[0].body,
            json!({"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]})
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_different_chain_is_rejected_without_retry() -> TestResult {
    let fixture = Fixture::start(vec![Reply::result(&json!("0x2"))?]).await?;
    let error = EvmClient::connect(config(&fixture, 2)?).await.unwrap_err();
    assert_eq!(error, Error::Provider(ProviderError::ChainMismatch));
    assert_eq!(fixture.requests()?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn noncanonical_malformed_and_overflowing_chain_quantities_are_rejected() -> TestResult {
    for result in [
        json!(""),
        json!("1"),
        json!("0x"),
        json!("0x00"),
        json!("0x01"),
        json!("0X1"),
        json!("0x-1"),
        json!("0x1.0"),
        json!("0xg"),
        json!(" 0x1"),
        json!(format!("0x1{}", "0".repeat(64))),
        json!(1),
        json!([]),
        json!({}),
    ] {
        let fixture = Fixture::start(vec![Reply::result(&result)?]).await?;
        let error = EvmClient::connect(config(&fixture, 2)?).await.unwrap_err();
        assert_eq!(error, Error::Provider(ProviderError::InvalidResponse));
        assert_eq!(fixture.requests()?.len(), 1);
    }
    let fixture = Fixture::start(vec![Reply::result(&Value::Null)?]).await?;
    assert_eq!(
        EvmClient::connect(config(&fixture, 0)?).await.unwrap_err(),
        Error::Provider(ProviderError::InvalidResponse)
    );
    Ok(())
}

#[tokio::test]
async fn envelopes_require_version_matching_numeric_id_and_one_outcome() -> TestResult {
    for response in [
        json!({"jsonrpc":"1.0","id":1,"result":"0x1"}),
        json!({"id":1,"result":"0x1"}),
        json!({"jsonrpc":"2.0","result":"0x1"}),
        json!({"jsonrpc":"2.0","id":0,"result":"0x1"}),
        json!({"jsonrpc":"2.0","id":"1","result":"0x1"}),
        json!({"jsonrpc":"2.0","id":null,"result":"0x1"}),
        json!({"jsonrpc":"2.0","id":1}),
        json!({"jsonrpc":"2.0","id":1,"result":"0x1","error":null}),
        json!({"jsonrpc":"2.0","id":1,"result":null,"error":{"code":-32_000,"message":"fixture"}}),
        json!({"jsonrpc":"2.0","id":1,"error":null}),
        json!({"jsonrpc":"2.0","id":1,"error":{}}),
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32_000}}),
        json!({"jsonrpc":"2.0","id":1,"error":{"message":"fixture"}}),
        json!({"jsonrpc":"2.0","id":1,"error":{"code":"-32000","message":"fixture"}}),
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32_000,"message":1}}),
    ] {
        let mut reply = Reply::json(&response)?;
        reply.echo_id = false;
        let fixture = Fixture::start(vec![reply]).await?;
        assert_eq!(
            EvmClient::connect(config(&fixture, 2)?).await.unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 1);
    }
    Ok(())
}

#[tokio::test]
async fn raw_json_rpc_envelopes_reject_duplicate_keys_and_invalid_json() -> TestResult {
    for raw in [
        r#"{"jsonrpc":"2.0","id":1,"id":1,"result":"0x1"}"#,
        r#"{"jsonrpc":"2.0","id":1,"result":"0x1","result":"0x1"}"#,
        r#"{"jsonrpc":"2.0","jsonrpc":"2.0","id":1,"result":"0x1"}"#,
        "{invalid JSON}",
        "[]",
    ] {
        let fixture = Fixture::start(vec![Reply::raw(200, raw.as_bytes().to_vec())]).await?;
        assert_eq!(
            EvmClient::connect(config(&fixture, 2)?).await.unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 1);
    }
    Ok(())
}

#[tokio::test]
async fn valid_provider_errors_are_typed_without_retaining_provider_messages() -> TestResult {
    let response = json!({"jsonrpc":"2.0","id":1,"error":{
        "code":-32_000,"message":"Bearer fixture-body-token", "data":{"key":"fixture-data-token"}
    }});
    let fixture = Fixture::start(vec![Reply::json(&response)?]).await?;
    let error = EvmClient::connect(config(&fixture, 2)?).await.unwrap_err();
    assert_eq!(error, Error::Provider(ProviderError::Rpc));
    assert_eq!(fixture.requests()?.len(), 1);
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error)?,
    ] {
        assert!(!rendered.contains("fixture-body-token"));
        assert!(!rendered.contains("fixture-data-token"));
    }
    Ok(())
}

#[tokio::test]
async fn safe_handshake_retries_only_up_to_the_configured_additional_attempts() -> TestResult {
    for status in [429, 500, 503] {
        let fixture = Fixture::start(vec![
            Reply::raw(status, b"fixture failure".to_vec()),
            Reply::raw(status, b"fixture failure".to_vec()),
            Reply::result(&json!("0x1"))?,
        ])
        .await?;
        assert_eq!(
            EvmClient::connect(config(&fixture, 2)?).await?.chain_id(),
            ChainId::from(1)
        );
        let requests = fixture.requests()?;
        assert_eq!(requests.len(), 3);
        assert!(
            requests
                .iter()
                .all(|request| request.body["method"] == "eth_chainId")
        );
        assert!(requests.iter().all(|request| request.body["id"] == 1));
    }
    Ok(())
}

#[tokio::test]
async fn transport_failures_are_retried_only_within_the_explicit_limit() -> TestResult {
    let mut dropped = Reply::raw(200, Vec::new());
    dropped.close_connection = true;
    let fixture = Fixture::start(vec![dropped, Reply::result(&json!("0x1"))?]).await?;
    assert_eq!(
        EvmClient::connect(config(&fixture, 1)?).await?.chain_id(),
        ChainId::from(1)
    );
    assert_eq!(fixture.requests()?.len(), 2);

    let failures = (0..3)
        .map(|_| {
            let mut reply = Reply::raw(200, Vec::new());
            reply.close_connection = true;
            reply
        })
        .collect();
    let fixture = Fixture::start(failures).await?;
    assert_eq!(
        EvmClient::connect(config(&fixture, 2)?).await.unwrap_err(),
        Error::Provider(ProviderError::Transport)
    );
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test]
async fn exhausted_retries_and_nonretryable_http_failures_have_exact_counts() -> TestResult {
    for (status, retries, expected) in [
        (429, 0, ProviderError::RateLimited),
        (429, 2, ProviderError::RateLimited),
        (503, 0, ProviderError::HttpStatus),
        (503, 2, ProviderError::HttpStatus),
        (401, 2, ProviderError::HttpStatus),
        (403, 2, ProviderError::HttpStatus),
    ] {
        let retryable = status == 429 || status >= 500;
        let attempts = if retryable {
            usize::from(retries) + 1
        } else {
            1
        };
        let fixture = Fixture::start(
            (0..attempts)
                .map(|_| Reply::raw(status, b"fixture failure".to_vec()))
                .collect(),
        )
        .await?;
        assert_eq!(
            EvmClient::connect(config(&fixture, retries)?)
                .await
                .unwrap_err(),
            Error::Provider(expected)
        );
        assert_eq!(fixture.requests()?.len(), attempts);
    }
    Ok(())
}

#[tokio::test]
async fn oversized_bodies_are_bounded_with_length_close_delimiting_and_chunked_encoding()
-> TestResult {
    for mode in ["content_length", "close", "chunked"] {
        let mut reply = Reply::raw(200, vec![b'x'; 8192]);
        reply.framing = match mode {
            "content_length" => Framing::ContentLength,
            "chunked" => Framing::Chunked,
            _ => Framing::CloseDelimited,
        };
        let fixture = Fixture::start(vec![reply]).await?;
        let config = configured(
            RpcEndpoint::new(&fixture.endpoint)?,
            ChainId::from(1),
            128,
            2,
            Duration::from_secs(2),
        )?;
        assert_eq!(
            EvmClient::connect(config).await.unwrap_err(),
            Error::Provider(ProviderError::ResponseTooLarge)
        );
        assert_eq!(fixture.requests()?.len(), 1);
    }
    Ok(())
}

#[tokio::test]
async fn request_deadline_bounds_waiting_for_headers_and_receiving_the_body() -> TestResult {
    for slow_body in [false, true] {
        let mut reply = Reply::result(&json!("0x1"))?;
        if slow_body {
            reply.body_delay = Duration::from_secs(2);
            reply.framing = Framing::Chunked;
        } else {
            reply.delay = Duration::from_secs(2);
        }
        let fixture = Fixture::start(vec![reply]).await?;
        let config = configured(
            RpcEndpoint::new(&fixture.endpoint)?,
            ChainId::from(1),
            4096,
            2,
            Duration::from_millis(750),
        )?;
        let started = Instant::now();
        assert_eq!(
            EvmClient::connect(config).await.unwrap_err(),
            Error::Timeout
        );
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(fixture.requests()?.len(), 1);
    }
    Ok(())
}

#[tokio::test]
async fn all_retry_attempts_share_one_total_deadline() -> TestResult {
    // Each response fits a fresh budget, but their accumulated delays do not.
    // Leave setup/scheduling margin so the second attempt actually reaches the
    // fixture even while other binaries compete for the platform runner.
    let mut first = Reply::raw(503, Vec::new());
    first.delay = Duration::from_millis(600);
    let mut second = Reply::result(&json!("0x1"))?;
    second.delay = Duration::from_millis(1200);
    let fixture = Fixture::start(vec![first, second]).await?;
    let config = configured(
        RpcEndpoint::new(&fixture.endpoint)?,
        ChainId::from(1),
        4096,
        2,
        Duration::from_millis(1500),
    )?;
    assert_eq!(
        EvmClient::connect(config).await.unwrap_err(),
        Error::Timeout
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body, requests[1].body);
    Ok(())
}

#[tokio::test]
async fn redirects_are_not_followed_or_sent_credentials() -> TestResult {
    let destination = Fixture::start(vec![Reply::result(&json!("0x1"))?]).await?;
    let mut redirect = Reply::raw(302, Vec::new());
    redirect
        .headers
        .push(("Location".to_owned(), destination.endpoint.clone()));
    let origin = Fixture::start(vec![redirect]).await?;
    let endpoint = RpcEndpoint::new(&origin.endpoint)?
        .with_header("Authorization", "Bearer fixture-header-token")?;
    let config = configured(endpoint, ChainId::from(1), 4096, 2, Duration::from_secs(2))?;
    assert_eq!(
        EvmClient::connect(config).await.unwrap_err(),
        Error::Provider(ProviderError::HttpStatus)
    );
    assert_eq!(origin.requests()?.len(), 1);
    assert!(destination.requests()?.is_empty());
    Ok(())
}

#[tokio::test]
async fn endpoint_query_header_and_error_body_credentials_are_not_diagnostics() -> TestResult {
    let fixture =
        Fixture::start(vec![Reply::raw(401, b"Bearer fixture-body-token".to_vec())]).await?;
    let url = format!("{}?key=fixture-query-token", fixture.endpoint);
    let endpoint =
        RpcEndpoint::new(&url)?.with_header("Authorization", "Bearer fixture-header-token")?;
    let config = configured(endpoint, ChainId::from(1), 4096, 0, Duration::from_secs(2))?;
    assert!(!format!("{config:?}").contains("fixture-query-token"));
    assert!(!format!("{config:?}").contains("fixture-header-token"));
    let error = EvmClient::connect(config).await.unwrap_err();
    assert_eq!(error, Error::Provider(ProviderError::HttpStatus));
    let requests = fixture.requests()?;
    assert!(requests[0].target.contains("fixture-query-token"));
    assert!(requests[0].headers.contains("fixture-header-token"));
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error)?,
    ] {
        for sensitive in [
            "fixture-query-token",
            "fixture-header-token",
            "fixture-body-token",
        ] {
            assert!(!rendered.contains(sensitive));
        }
    }
    Ok(())
}

#[tokio::test]
async fn a_connected_client_debug_does_not_disclose_endpoint_or_credentials() -> TestResult {
    let fixture = Fixture::start(vec![Reply::result(&json!("0x1"))?]).await?;
    let url = format!("{}/fixture-path?key=fixture-query-token", fixture.endpoint);
    let endpoint =
        RpcEndpoint::new(&url)?.with_header("Authorization", "Bearer fixture-header-token")?;
    let client = EvmClient::connect(configured(
        endpoint,
        ChainId::from(1),
        4096,
        0,
        Duration::from_secs(2),
    )?)
    .await?;
    let debug = format!("{client:?}");
    for sensitive in [
        "fixture-path",
        "fixture-query-token",
        "fixture-header-token",
        &fixture.endpoint,
    ] {
        assert!(!debug.contains(sensitive));
    }
    Ok(())
}
