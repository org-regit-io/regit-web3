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
