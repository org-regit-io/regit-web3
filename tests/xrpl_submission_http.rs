// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit XRPL submit-only exchanges with honest post-dispatch ambiguity.

#![cfg(feature = "xrpl-http")]

use std::time::Duration;

use regit_web3::{
    chains::xrpl::{XrplClient, XrplHttpConfig, XrplSubmitter},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::xrpl::{
        EngineResultClass, Hash, HexData, Network, NetworkId, Setting, SignedSubmission,
        SubmissionLedgerState,
    },
    error::{Error, ProviderError, SubmissionFailure, ValidationError},
};
use serde_json::{Value, json};

#[path = "support/rpc_server.rs"]
mod server;
use server::{Fixture, Framing, Reply};

type TestError = Box<dyn std::error::Error>;

fn config(endpoint: &str, timeout: Duration, maximum: usize) -> Result<XrplHttpConfig, Error> {
    Ok(XrplHttpConfig::new(
        Network::new(NetworkId::new(0), "fixture")?,
        HttpConfig::new(
            RpcEndpoint::new(endpoint)?,
            RpcLimits::new(timeout, timeout, maximum, 3)?,
            "local",
        )?,
    ))
}
fn standard(endpoint: &str) -> Result<XrplHttpConfig, Error> {
    config(endpoint, Duration::from_secs(2), 1_048_576)
}
fn submission() -> Result<SignedSubmission, Error> {
    Ok(SignedSubmission::new(
        NetworkId::new(0),
        HexData::parse("ABCD")?,
        true,
    ))
}
fn reply(mut value: Value) -> Result<Reply, TestError> {
    value
        .as_object_mut()
        .ok_or("fixture object required")?
        .insert("status".into(), json!("success"));
    let mut reply = Reply::json(&json!({"result":value}))?;
    reply.echo_id = false;
    Ok(reply)
}
fn network() -> Result<Reply, TestError> {
    reply(json!({"info":{"network_id":0}}))
}
fn response() -> Result<Value, Error> {
    Ok(json!({"engine_result":"tesSUCCESS","engine_result_code":0,
        "engine_result_message":"fixture-response-secret","tx_blob":"ABCD",
        "tx_json":{"hash":submission()?.hash(),"TransactionType":"Payment"},
        "accepted":true,"applied":true,"broadcast":true,"kept":true,"queued":false,
        "account_sequence_available":9,"account_sequence_next":9,"open_ledger_cost":"10",
        "validated_ledger_index":100}))
}
fn writes(fixture: &Fixture) -> Result<usize, TestError> {
    Ok(fixture
        .requests()?
        .iter()
        .filter(|r| r.body["method"] == "submit")
        .count())
}
async fn unknown_case(
    post: Reply,
    expected: SubmissionFailure,
    maximum: usize,
) -> Result<(), TestError> {
    let fixture = Fixture::start(vec![network()?, network()?, post]).await?;
    let client =
        XrplClient::connect(config(&fixture.endpoint, Duration::from_secs(2), maximum)?).await?;
    assert_eq!(
        client.submit_signed(submission()?).await,
        Err(Error::SubmissionOutcomeUnknown(expected))
    );
    assert_eq!(writes(&fixture)?, 1);
    Ok(())
}

#[tokio::test]
async fn submit_only_dispatches_explicit_bytes_once_and_retains_preliminary_facts()
-> Result<(), TestError> {
    let fixture = Fixture::start(vec![network()?, network()?, reply(response()?)?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    let request = submission()?;
    let observed = XrplSubmitter::submit_signed(&client, request.clone()).await?;
    assert_eq!(observed.value().hash(), request.hash());
    assert_eq!(
        observed.value().engine_result().class(),
        EngineResultClass::Success
    );
    assert_eq!(observed.value().handling().accepted(), Setting::Enabled);
    assert_eq!(observed.value().handling().applied(), Setting::Enabled);
    assert_eq!(observed.context().ledger(), None);
    assert_eq!(observed.context().source().method(), "submit");
    assert_eq!(
        observed.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        observed
            .value()
            .ledger_state()
            .map(SubmissionLedgerState::latest_validated_ledger_index),
        Some(100)
    );
    assert!(!format!("{observed:?}").contains("fixture-response-secret"));
    let captured = fixture.requests()?;
    assert_eq!(captured[2].target, "/rpc");
    assert!(captured[2].headers.starts_with("POST /rpc HTTP/1.1\r\n"));
    assert!(
        captured[2]
            .headers
            .contains("content-type: application/json\r\n")
    );
    assert_eq!(
        captured[2].body,
        json!({"method":"submit","params":[{"api_version":2,"tx_blob":"ABCD","fail_hard":true}]})
    );
    assert_eq!(writes(&fixture)?, 1);
    Ok(())
}

#[tokio::test]
async fn queued_and_server_rejected_results_never_claim_validated_execution()
-> Result<(), TestError> {
    for (code, accepted, queued) in [("terQUEUED", true, true), ("tefALREADY", false, false)] {
        let mut value = response()?;
        value["engine_result"] = json!(code);
        value["accepted"] = json!(accepted);
        value["applied"] = json!(false);
        value["broadcast"] = json!(false);
        value["kept"] = json!(queued);
        value["queued"] = json!(queued);
        for field in [
            "account_sequence_available",
            "account_sequence_next",
            "open_ledger_cost",
            "validated_ledger_index",
        ] {
            value
                .as_object_mut()
                .ok_or("fixture object required")?
                .remove(field);
        }
        // Source hash can be absent: echoed exact bytes provide the known ID.
        value["tx_json"]
            .as_object_mut()
            .ok_or("fixture object required")?
            .remove("hash");
        let fixture = Fixture::start(vec![network()?, network()?, reply(value)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        let request = SignedSubmission::new(NetworkId::new(0), HexData::parse("ABCD")?, false);
        let observed = client.submit_signed(request).await?;
        assert_eq!(
            observed.value().handling().accepted(),
            if accepted {
                Setting::Enabled
            } else {
                Setting::Disabled
            }
        );
        assert_eq!(
            observed.value().handling().queued(),
            if queued {
                Setting::Enabled
            } else {
                Setting::Disabled
            }
        );
        assert!(observed.value().ledger_state().is_none());
        assert!(observed.context().ledger().is_none());
        assert_eq!(writes(&fixture)?, 1);
        assert_eq!(fixture.requests()?[2].body["params"][0]["fail_hard"], false);
    }
    Ok(())
}

#[tokio::test]
async fn retryable_http_statuses_dispatch_one_write_even_with_read_retries() -> Result<(), TestError>
{
    for (status, cause) in [
        (429, SubmissionFailure::RateLimited),
        (503, SubmissionFailure::HttpStatus),
    ] {
        let fixture = Fixture::start_routed(move |request| {
            if request.body["method"] == "server_info" {
                network().map_err(|_| std::io::Error::other("fixture failure"))
            } else {
                Ok(Reply::raw(status, b"fixture-response-secret".to_vec()))
            }
        })
        .await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client.submit_signed(submission()?).await,
            Err(Error::SubmissionOutcomeUnknown(cause))
        );
        assert_eq!(writes(&fixture)?, 1);
    }
    Ok(())
}

#[tokio::test]
async fn disconnect_after_receiving_payload_is_unknown_and_never_retried() -> Result<(), TestError>
{
    let fixture = Fixture::start_routed(|request| {
        if request.body["method"] == "server_info" {
            network().map_err(|_| std::io::Error::other("fixture failure"))
        } else {
            let mut reply = Reply::raw(200, Vec::new());
            reply.close_connection = true;
            Ok(reply)
        }
    })
    .await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.submit_signed(submission()?).await,
        Err(Error::SubmissionOutcomeUnknown(
            SubmissionFailure::Transport
        ))
    );
    assert_eq!(writes(&fixture)?, 1);
    Ok(())
}

#[tokio::test]
async fn total_deadline_preserves_preflight_and_post_dispatch_failure_phase()
-> Result<(), TestError> {
    let mut delayed = network()?;
    delayed.delay = Duration::from_secs(2);
    let fixture = Fixture::start(vec![network()?, delayed]).await?;
    let client =
        XrplClient::connect(config(&fixture.endpoint, Duration::from_millis(750), 4096)?).await?;
    assert_eq!(
        client.submit_signed(submission()?).await,
        Err(Error::Timeout)
    );
    assert_eq!(writes(&fixture)?, 0);

    let mut preflight = network()?;
    // Each stage fits a fresh budget, while their total does not. Setup and
    // dispatch have enough margin to preserve the failure phase under load.
    preflight.delay = Duration::from_millis(500);
    let mut post = reply(response()?)?;
    post.body_delay = Duration::from_millis(1200);
    let fixture = Fixture::start(vec![network()?, preflight, post]).await?;
    let client = XrplClient::connect(config(
        &fixture.endpoint,
        Duration::from_millis(1500),
        4096,
    )?)
    .await?;
    assert_eq!(
        client.submit_signed(submission()?).await,
        Err(Error::SubmissionOutcomeUnknown(SubmissionFailure::Timeout))
    );
    assert_eq!(writes(&fixture)?, 1);
    Ok(())
}

#[tokio::test]
async fn changed_or_mismatched_expected_network_dispatches_zero_writes() -> Result<(), TestError> {
    let fixture = Fixture::start(vec![network()?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    let wrong = SignedSubmission::new(NetworkId::new(1025), HexData::parse("ABCD")?, true);
    assert_eq!(
        client.submit_signed(wrong).await,
        Err(Error::Validation(ValidationError::NetworkMismatch))
    );
    assert_eq!(fixture.requests()?.len(), 1);
    assert_eq!(writes(&fixture)?, 0);

    let fixture =
        Fixture::start(vec![network()?, reply(json!({"info":{"network_id":1}}))?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.submit_signed(submission()?).await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    assert_eq!(writes(&fixture)?, 0);
    Ok(())
}

#[tokio::test]
async fn body_limits_and_malformed_responses_remain_unknown_after_one_write()
-> Result<(), TestError> {
    for framing in [
        Framing::ContentLength,
        Framing::CloseDelimited,
        Framing::Chunked,
    ] {
        let mut post = reply(response()?)?;
        post.framing = framing;
        unknown_case(post, SubmissionFailure::ResponseTooLarge, 128).await?;
    }
    unknown_case(
        Reply::raw(200, b"not json fixture-secret".to_vec()),
        SubmissionFailure::InvalidResponse,
        4096,
    )
    .await?;
    let error = Reply::raw(
        200,
        serde_json::to_vec(&json!({"result":{"status":"error",
        "error":"internalSubmit","error_code":73,"error_message":"fixture-response-secret"}}))?,
    );
    unknown_case(error, SubmissionFailure::Rpc, 4096).await?;
    unknown_case(
        Reply::result(&json!({"ok":true}))?,
        SubmissionFailure::InvalidResponse,
        4096,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn identity_null_overflow_and_inconsistent_handling_are_unknown() -> Result<(), TestError> {
    for change in [
        json!({"tx_blob":"ABCE"}),
        json!({"tx_json":{"hash":Hash::from_bytes([1;32])}}),
        json!({"tx_json":{"hash":null}}),
        json!({"tx_json":null}),
        json!({"accepted":false}),
        json!({"applied":null}),
        json!({"engine_result":"tesUNKNOWN"}),
        json!({"engine_result_message":null}),
        json!({"engine_result_message":42}),
        json!({"engine_result_code":i64::MAX}),
        json!({"open_ledger_cost":"0.5"}),
        json!({"validated_ledger_index":0}),
        json!({"account_sequence_available":null}),
        json!({"account_sequence_next":u64::from(u32::MAX)+1}),
    ] {
        let mut value = response()?;
        for (key, data) in change.as_object().ok_or("fixture object required")? {
            value[key] = data.clone();
        }
        unknown_case(reply(value)?, SubmissionFailure::InvalidResponse, 4096).await?;
    }
    let mut incomplete = response()?;
    incomplete
        .as_object_mut()
        .ok_or("fixture object required")?
        .remove("open_ledger_cost");
    unknown_case(reply(incomplete)?, SubmissionFailure::InvalidResponse, 4096).await?;
    Ok(())
}

#[tokio::test]
async fn duplicate_critical_fields_and_unsupported_api_version_remain_unknown()
-> Result<(), TestError> {
    let mut value = response()?;
    value["status"] = json!("success");
    let encoded = serde_json::to_string(&json!({"result":value}))?;
    for duplicated in [
        encoded.replace("\"accepted\":true", "\"accepted\":true,\"accepted\":true"),
        encoded.replace(
            "\"status\":\"success\"",
            "\"status\":\"success\",\"status\":\"success\"",
        ),
        encoded.replace(
            "\"hash\":",
            &format!("\"hash\":\"{}\",\"hash\":", submission()?.hash()),
        ),
        encoded.replace(
            "\"engine_result_message\":",
            "\"engine_result_message\":\"ignored\",\"engine_result_message\":",
        ),
        encoded.replace(
            "\"tx_blob\":\"ABCD\"",
            "\"tx_blob\":\"ABCD\",\"tx_blob\":\"ABCD\"",
        ),
    ] {
        unknown_case(
            Reply::raw(200, duplicated.into_bytes()),
            SubmissionFailure::InvalidResponse,
            4096,
        )
        .await?;
    }
    let mut body: Value = serde_json::from_str(&encoded)?;
    body["api_version"] = json!(1);
    unknown_case(
        Reply::raw(200, serde_json::to_vec(&body)?),
        SubmissionFailure::InvalidResponse,
        4096,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn caller_cancellation_after_dispatch_does_not_spawn_a_retry() -> Result<(), TestError> {
    let reached = std::sync::Arc::new(tokio::sync::Notify::new());
    let captured = std::sync::Arc::clone(&reached);
    let fixture = Fixture::start_routed(move |request| {
        if request.body["method"] == "server_info" {
            network().map_err(|_| std::io::Error::other("fixture failure"))
        } else {
            captured.notify_one();
            let mut reply =
                reply(response().map_err(|_| std::io::Error::other("fixture failure"))?)
                    .map_err(|_| std::io::Error::other("fixture failure"))?;
            reply.body_delay = Duration::from_millis(100);
            Ok(reply)
        }
    })
    .await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    {
        let pending = client.submit_signed(submission()?);
        let mut pending = std::pin::pin!(pending);
        tokio::select! {
            completed = &mut pending => return Err(format!("unexpected result: {completed:?}").into()),
            () = reached.notified() => {}
        }
    }
    // The first dispatch was observed above. Any detached retry would signal
    // another request; observe the entire original operation budget instead of
    // assuming a short sleep was enough for a loaded scheduler to run it.
    assert!(
        tokio::time::timeout(Duration::from_secs(2), reached.notified())
            .await
            .is_err()
    );
    assert_eq!(writes(&fixture)?, 1);
    Ok(())
}
