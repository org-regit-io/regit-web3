// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! HTTP read contracts independently exercised without an RPC consumer.

use std::{error::Error as StdError, io, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use super::super::{HttpClient, OperationBudget};
use crate::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    error::{Error, ProviderError},
};

fn http_config(
    endpoint: RpcEndpoint,
    maximum: usize,
    timeout: Duration,
) -> Result<HttpConfig, Error> {
    HttpConfig::new(
        endpoint,
        RpcLimits::new(timeout, timeout, maximum, 0)?,
        "fixture",
    )
}

async fn fixture(
    headers: &'static str,
    body: Vec<u8>,
    body_delay: Duration,
) -> io::Result<(String, JoinHandle<io::Result<String>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/api/v1/", listener.local_addr()?);
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let request = read_request(&mut stream).await?;
        stream.write_all(headers.as_bytes()).await?;
        tokio::time::sleep(body_delay).await;
        // An early body-limit or deadline failure may close this connection.
        let _body_written = stream.write_all(&body).await;
        Ok(request)
    });
    Ok((endpoint, task))
}

async fn read_request(stream: &mut TcpStream) -> io::Result<String> {
    let mut received = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !received.windows(4).any(|window| window == b"\r\n\r\n") {
        let count = stream.read(&mut buffer).await?;
        if count == 0 || received.len() > 16_384 {
            return Err(io::Error::other("fixture request headers invalid"));
        }
        received.extend_from_slice(&buffer[..count]);
    }
    String::from_utf8(received).map_err(|_| io::Error::other("fixture request is not UTF-8"))
}

async fn retry_fixture(
    replies: Vec<(u16, Duration)>,
) -> io::Result<(String, JoinHandle<io::Result<Vec<String>>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/api/", listener.local_addr()?);
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for (status, delay) in replies {
            let (mut stream, _) = listener.accept().await?;
            requests.push(read_request(&mut stream).await?);
            tokio::time::sleep(delay).await;
            let response = format!(
                "HTTP/1.1 {status} Fixture\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
            );
            let _response_written = stream.write_all(response.as_bytes()).await;
        }
        Ok(requests)
    });
    Ok((endpoint, task))
}

#[tokio::test]
async fn paths_and_queries_preserve_base_credentials_and_encode_dynamic_segments()
-> Result<(), Box<dyn StdError>> {
    let endpoint =
        RpcEndpoint::new("https://user:secret@example.invalid/api/v1/?key=endpoint-secret")?;
    let client = HttpClient::new(&http_config(endpoint, 1024, Duration::from_secs(1))?)?;
    let url = client.request_url(
        &["address", "../other/?#%", "https://elsewhere.invalid"],
        &[("asset", "token & value"), ("page", "2")],
    )?;
    assert_eq!(url.host_str(), Some("example.invalid"));
    assert_eq!(url.username(), "user");
    assert_eq!(url.password(), Some("secret"));
    assert_eq!(
        url.path(),
        "/api/v1/address/..%2Fother%2F%3F%23%25/https:%2F%2Felsewhere.invalid"
    );
    let pairs: Vec<_> = url.query_pairs().collect();
    assert_eq!(pairs[0], ("key".into(), "endpoint-secret".into()));
    assert_eq!(pairs[1], ("asset".into(), "token & value".into()));
    assert_eq!(pairs[2], ("page".into(), "2".into()));
    for segment in ["", ".", "..", "bad\nsegment", "bad\0segment"] {
        assert!(matches!(
            client.request_url(&[segment], &[]),
            Err(Error::Configuration)
        ));
    }
    Ok(())
}

#[tokio::test]
async fn get_requests_send_explicit_authentication_and_return_exact_decimal_bytes()
-> Result<(), Box<dyn StdError>> {
    let decimal = b"0.012345678900000001234567890".to_vec();
    let (endpoint, captured) = fixture(
        "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
        decimal.clone(),
        Duration::ZERO,
    )
    .await?;
    let endpoint = RpcEndpoint::new(&format!("{endpoint}?api_key=fixture-query-secret"))?
        .with_header("authorization", "Bearer fixture-header-secret")?;
    let config = http_config(endpoint, 1024, Duration::from_secs(1))?;
    let client = HttpClient::new(&config)?;
    let budget = OperationBudget::new(config.limits())?;
    let response = client
        .read(&["prices"], &[("asset", "a/b & c")], None, &budget)
        .await?;
    assert_eq!(response.into_success()?, decimal);
    let request = captured.await??;
    assert!(request.starts_with(
        "GET /api/v1/prices?api_key=fixture-query-secret&asset=a%2Fb+%26+c HTTP/1.1\r\n"
    ));
    assert!(request.contains("authorization: Bearer fixture-header-secret\r\n"));
    let exact: serde_json::Number = serde_json::from_slice(&decimal)?;
    assert_eq!(exact.as_str(), "0.012345678900000001234567890");
    Ok(())
}

#[tokio::test]
async fn get_body_bounds_apply_without_length_and_with_real_chunked_framing()
-> Result<(), Box<dyn StdError>> {
    for (headers, body) in [
        (
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
            b"12345".to_vec(),
        ),
        (
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            b"3\r\n123\r\n2\r\n45\r\n0\r\n\r\n".to_vec(),
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n",
            b"12345".to_vec(),
        ),
    ] {
        let (endpoint, captured) = fixture(headers, body, Duration::ZERO).await?;
        let config = http_config(RpcEndpoint::new(&endpoint)?, 4, Duration::from_secs(1))?;
        let client = HttpClient::new(&config)?;
        let budget = OperationBudget::new(config.limits())?;
        assert!(matches!(
            client.read(&[], &[], None, &budget).await,
            Err(Error::Provider(ProviderError::ResponseTooLarge))
        ));
        captured.await??;
    }
    Ok(())
}

#[tokio::test]
async fn get_consumption_respects_deadline_and_discards_remote_error_bodies()
-> Result<(), Box<dyn StdError>> {
    let (endpoint, captured) = fixture(
        "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n",
        b"0".to_vec(),
        Duration::from_secs(3),
    )
    .await?;
    let config = http_config(RpcEndpoint::new(&endpoint)?, 1024, Duration::from_secs(2))?;
    let client = HttpClient::new(&config)?;
    let budget = OperationBudget::new(config.limits())?;
    assert!(matches!(
        client.read(&[], &[], None, &budget).await,
        Err(Error::Timeout)
    ));
    assert!(captured.await??.starts_with("GET /api/v1/ "));

    let (endpoint, captured) = fixture(
        "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n",
        b"fixture-response-secret".to_vec(),
        Duration::ZERO,
    )
    .await?;
    let config = http_config(RpcEndpoint::new(&endpoint)?, 1, Duration::from_secs(1))?;
    let client = HttpClient::new(&config)?;
    let budget = OperationBudget::new(config.limits())?;
    let response = client.read(&[], &[], None, &budget).await?;
    assert_eq!(response.status.as_u16(), 404);
    assert!(response.body.is_empty());
    assert!(matches!(
        response.into_success(),
        Err(Error::Provider(ProviderError::HttpStatus))
    ));
    captured.await??;
    Ok(())
}

#[tokio::test]
async fn get_retries_retain_url_headers_and_bound_attempts_and_total_deadline()
-> Result<(), Box<dyn StdError>> {
    for (statuses, success) in [([429, 500, 200], true), ([503, 503, 503], false)] {
        let (endpoint, captured) = retry_fixture(
            statuses
                .into_iter()
                .map(|status| (status, Duration::ZERO))
                .collect(),
        )
        .await?;
        let endpoint = RpcEndpoint::new(&format!("{endpoint}?api_key=fixture-query-secret"))?
            .with_header("x-api-key", "fixture-header-secret")?;
        let config = HttpConfig::new(
            endpoint,
            RpcLimits::new(Duration::from_secs(1), Duration::from_secs(1), 1024, 2)?,
            "fixture",
        )?;
        let client = HttpClient::new(&config)?;
        let budget = OperationBudget::new(config.limits())?;
        let response = client
            .read(&["prices"], &[("asset", "x/y")], None, &budget)
            .await?;
        if success {
            assert_eq!(response.into_success()?, b"{}");
        } else {
            assert!(matches!(
                response.into_success(),
                Err(Error::Provider(ProviderError::HttpStatus))
            ));
        }
        let requests = captured.await??;
        assert_eq!(requests.len(), 3);
        assert!(requests.iter().all(|request| request == &requests[0]));
        assert!(requests[0].contains("api_key=fixture-query-secret&asset=x%2Fy"));
        assert!(requests[0].contains("x-api-key: fixture-header-secret\r\n"));
    }

    // Each attempt fits a fresh budget; together they exceed the original one.
    let (endpoint, captured) = retry_fixture(vec![
        (429, Duration::from_millis(1200)),
        (200, Duration::from_millis(1200)),
    ])
    .await?;
    let config = HttpConfig::new(
        RpcEndpoint::new(&endpoint)?,
        RpcLimits::new(Duration::from_secs(2), Duration::from_secs(2), 1024, 1)?,
        "fixture",
    )?;
    let client = HttpClient::new(&config)?;
    let budget = OperationBudget::new(config.limits())?;
    assert!(matches!(
        client.read(&[], &[], None, &budget).await,
        Err(Error::Timeout)
    ));
    let requests = captured.await??;
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
    assert!(requests[1].starts_with("GET /api/ "));
    Ok(())
}
