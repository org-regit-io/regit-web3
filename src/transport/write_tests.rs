// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Reusable one-shot HTTP boundary, independently of any family decoder.

use std::time::Duration;

use super::{HttpClient, OperationBudget, submission_unknown};
use crate::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    error::{Error, SubmissionFailure},
};

#[path = "../../tests/support/rpc_server.rs"]
mod server;
use server::{Fixture, Framing, Reply};

type TestError = Box<dyn std::error::Error>;

fn config(endpoint: &str) -> Result<HttpConfig, Error> {
    HttpConfig::new(
        RpcEndpoint::new(endpoint)?,
        RpcLimits::new(Duration::from_secs(1), Duration::from_secs(1), 4096, 3)?,
        "fixture",
    )
}

#[tokio::test]
async fn write_preserves_explicit_path_query_headers_and_exact_body_once() -> Result<(), TestError>
{
    let fixture = Fixture::start(vec![Reply::result(&serde_json::json!({"value":"ok"}))?]).await?;
    let endpoint = RpcEndpoint::new(&format!("{}?key=fixture-query-secret", fixture.endpoint))?
        .with_header("authorization", "Bearer fixture-header-secret")?;
    let config = HttpConfig::new(endpoint, config(&fixture.endpoint)?.limits(), "fixture")?;
    let client = HttpClient::new(&config)?;
    let budget = OperationBudget::new(config.limits())?;
    let body = br#"{"payload":"exact bytes"}"#;
    let received = client
        .write_once(&["submit"], &[("asset", "a/b & c")], body, &budget)
        .await?;
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&received.into_success()?)?,
        serde_json::json!({"jsonrpc":"2.0","id":null,"result":{"value":"ok"}})
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].target,
        "/rpc/submit?key=fixture-query-secret&asset=a%2Fb+%26+c"
    );
    assert!(
        requests[0]
            .headers
            .contains("authorization: Bearer fixture-header-secret")
    );
    assert_eq!(
        requests[0].body,
        serde_json::from_slice::<serde_json::Value>(body)?
    );
    Ok(())
}

#[tokio::test]
async fn invalid_url_path_and_expired_budget_do_not_execute_requests() -> Result<(), TestError> {
    let fixture = Fixture::start(Vec::new()).await?;
    let config = config(&fixture.endpoint)?;
    let client = HttpClient::new(&config)?;
    let budget = OperationBudget::new(config.limits())?;
    assert!(matches!(
        client.write_once(&[".."], &[], b"{}", &budget).await,
        Err(Error::Configuration)
    ));
    let limits = RpcLimits::new(
        Duration::from_millis(10),
        Duration::from_millis(10),
        4096,
        3,
    )?;
    let expired = OperationBudget::new(limits)?;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(matches!(
        client.write_once(&[], &[], b"{}", &expired).await,
        Err(Error::Timeout)
    ));
    assert!(fixture.requests()?.is_empty());
    Ok(())
}

#[tokio::test]
async fn one_shot_limits_every_body_framing_without_retry_or_remote_diagnostics()
-> Result<(), TestError> {
    for framing in [
        Framing::ContentLength,
        Framing::CloseDelimited,
        Framing::Chunked,
    ] {
        let mut reply = Reply::raw(200, b"fixture-response-secret".to_vec());
        reply.framing = framing;
        let fixture = Fixture::start(vec![reply]).await?;
        let base = config(&fixture.endpoint)?;
        let config = HttpConfig::new(
            base.endpoint().clone(),
            RpcLimits::new(Duration::from_secs(1), Duration::from_secs(1), 4, 3)?,
            "fixture",
        )?;
        let client = HttpClient::new(&config)?;
        let budget = OperationBudget::new(config.limits())?;
        assert!(matches!(
            client.write_once(&[], &[], b"{}", &budget).await,
            Err(Error::SubmissionOutcomeUnknown(
                SubmissionFailure::ResponseTooLarge
            ))
        ));
        assert_eq!(fixture.requests()?.len(), 1);
    }
    assert_eq!(
        submission_unknown(Error::Timeout),
        Error::SubmissionOutcomeUnknown(SubmissionFailure::Timeout)
    );
    let safe = Error::SubmissionOutcomeUnknown(SubmissionFailure::InvalidResponse);
    assert_eq!(submission_unknown(safe), safe);
    Ok(())
}

#[cfg(feature = "blockfrost-http")]
async fn raw_once(
    status: u16,
    response: &'static [u8],
) -> Result<
    (
        String,
        tokio::task::JoinHandle<std::io::Result<(String, Vec<u8>)>>,
    ),
    TestError,
> {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/api/v0", listener.local_addr()?);
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut bytes = Vec::new();
        let mut buf = [0u8; 1024];
        let header_end = loop {
            if let Some(at) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                break at + 4;
            }
            if bytes.len() > 16384 {
                return Err(std::io::Error::other("header limit"));
            }
            let n = stream.read(&mut buf).await?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            bytes.extend_from_slice(&buf[..n]);
        };
        let headers =
            String::from_utf8(bytes[..header_end].to_vec()).map_err(std::io::Error::other)?;
        let length = headers
            .lines()
            .filter_map(|v| v.split_once(':'))
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .ok_or_else(|| std::io::Error::other("length"))?
            .1
            .trim()
            .parse::<usize>()
            .map_err(std::io::Error::other)?;
        if length > 65536 {
            return Err(std::io::Error::other("body limit"));
        }
        while bytes.len() < header_end + length {
            let n = stream.read(&mut buf).await?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            bytes.extend_from_slice(&buf[..n]);
        }
        let body = bytes[header_end..header_end + length].to_vec();
        let header = format!(
            "HTTP/1.1 {status} Fixture\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            response.len()
        );
        stream.write_all(header.as_bytes()).await?;
        stream.write_all(response).await?;
        stream.shutdown().await?;
        Ok((headers, body))
    });
    Ok((endpoint, task))
}

#[cfg(feature = "blockfrost-http")]
#[tokio::test]
async fn cbor_write_is_exact_fixed_mime_and_single_dispatch_under_read_retry_policy()
-> Result<(), TestError> {
    const BODY: &[u8] = &[0x84, 0xa1, 0, 0x81, 0x82, 0x58, 0x20, 0, 0xff];
    for status in [200, 429, 500] {
        let (endpoint, task) = raw_once(status, b"\"fixture-private-response\"").await?;
        let endpoint =
            RpcEndpoint::new(&endpoint)?.with_header("project_id", "fixture-project-secret")?;
        let config = HttpConfig::new(
            endpoint,
            RpcLimits::new(Duration::from_secs(2), Duration::from_secs(5), 4096, 3)?,
            "fixture",
        )?;
        let client = HttpClient::new(&config)?;
        let budget = OperationBudget::new(config.limits())?;
        let response = client
            .write_once_cbor(&["tx", "submit"], &[], BODY, &budget)
            .await?;
        if status == 200 {
            assert_eq!(response.into_success()?, b"\"fixture-private-response\"");
        } else {
            let error = response
                .into_success()
                .map_err(submission_unknown)
                .err()
                .ok_or("expected status failure")?;
            assert_eq!(
                error,
                Error::SubmissionOutcomeUnknown(if status == 429 {
                    SubmissionFailure::RateLimited
                } else {
                    SubmissionFailure::HttpStatus
                })
            );
            assert!(!format!("{error:?} {error}").contains("fixture-private-response"));
        }
        let (headers, body) = task.await??;
        assert!(headers.starts_with("POST /api/v0/tx/submit HTTP/1.1"));
        assert!(headers.contains("content-type: application/cbor"));
        assert!(!headers.contains("fixture-wrong-content-type"));
        assert!(headers.contains("project_id: fixture-project-secret"));
        assert_eq!(body, BODY);
    }
    Ok(())
}
