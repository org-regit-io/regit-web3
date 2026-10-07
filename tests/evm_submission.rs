// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! One-shot explicit signed EVM submission over a real loopback HTTP exchange.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "evm-http")]
#[path = "support/rpc_server.rs"]
mod rpc_server;
use regit_web3::{
    chains::evm::{EvmClient, EvmSubmitter},
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::evm::{Data, ReadState, SignedSubmission, TransactionId},
    domain::{BlockSelector, ChainId, Finality, NetworkId},
    error::{Error, ProviderError, SubmissionFailure},
};
use rpc_server::{Fixture, Framing, Reply};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const SIGNED: &str = "0xf86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";
fn signed() -> Result<SignedSubmission, Error> {
    SignedSubmission::new(ChainId::from(1), Data::parse(SIGNED)?)
}
fn config(f: &Fixture, chain: u64, bytes: usize, timeout: Duration) -> Result<EvmConfig, Error> {
    EvmConfig::new(
        NetworkId::new(ChainId::from(chain), "fixture")?,
        RpcEndpoint::new(&f.endpoint)?,
        18,
        None,
        BlockSelector::Safe,
        RpcLimits::new(Duration::from_secs(1).min(timeout), timeout, bytes, 3)?,
        "evm-fixture",
    )
}
async fn client(reply: Reply) -> Result<(Fixture, EvmClient), Box<dyn std::error::Error>> {
    let f = Fixture::start(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&json!("0x1"))?,
        reply,
    ])
    .await?;
    let c = EvmClient::connect(config(&f, 1, 1024, Duration::from_secs(3))?).await?;
    Ok((f, c))
}

#[tokio::test]
async fn matching_computed_hash_acknowledges_one_dispatch_without_validation_claims() -> TestResult
{
    let payload = signed()?;
    let (f, c) = client(Reply::result(&json!(payload.transaction_id().to_string()))?).await?;
    let before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let o = EvmSubmitter::submit_signed(&c, payload.clone()).await?;
    assert_eq!(o.value().transaction_id, payload.transaction_id());
    assert_eq!(o.value().chain_id, ChainId::from(1));
    assert_eq!(o.context().state(), ReadState::Unanchored);
    assert_eq!(o.context().finality(), Finality::Unknown);
    assert_eq!(o.context().confirmations(), None);
    assert_eq!(o.context().source().method(), "eth_sendRawTransaction");
    assert_eq!(
        o.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!(o.context().retrieved_at().unix_seconds() >= before);
    let requests = f.requests()?;
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[2].body,
        json!({"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":[SIGNED]})
    );
    assert!(requests[2].headers.contains("application/json"));
    assert_eq!(requests[2].target, "/rpc");
    let restored = serde_json::from_value::<
        regit_web3::domain::evm::OperationObservation<
            regit_web3::domain::evm::SubmissionAcknowledgment,
        >,
    >(serde_json::to_value(&o)?)?;
    assert_eq!(restored, o);
    Ok(())
}

#[tokio::test]
async fn write_disconnect_rate_limit_http_failures_never_retry() -> TestResult {
    for status in [0, 429, 500, 503, 302] {
        let mut reply = Reply::raw(if status == 0 { 200 } else { status }, Vec::new());
        reply.close_connection = status == 0;
        if status == 302 {
            reply
                .headers
                .push(("Location".into(), "http://127.0.0.1:9/SECRET".into()));
        }
        let (f, c) = client(reply).await?;
        let error = c.submit_signed(signed()?).await.expect_err("write failure");
        assert_eq!(
            error,
            Error::SubmissionOutcomeUnknown(match status {
                0 => SubmissionFailure::Transport,
                429 => SubmissionFailure::RateLimited,
                _ => SubmissionFailure::HttpStatus,
            })
        );
        assert_eq!(f.requests()?.len(), 3);
        assert!(!format!("{error:?} {error}").contains("SECRET"));
    }
    Ok(())
}

#[tokio::test]
async fn malformed_null_wrong_identity_and_rpc_errors_retain_possible_outcome() -> TestResult {
    let id = signed()?.transaction_id().to_string();
    let bodies = vec![
        Vec::new(),
        b"{SECRET_MALFORMED".to_vec(),
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":Value::Null}))?,
        serde_json::to_vec(
            &json!({"jsonrpc":"2.0","id":1,"result":TransactionId::from_bytes([9;32]).to_string()}),
        )?,
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":2,"result":id}))?,
        format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"{id}\",\"result\":\"{id}\"}}")
            .into_bytes(),
        serde_json::to_vec(
            &json!({"jsonrpc":"2.0","id":1,"result":id,"error":{"code":-32000,"message":"SECRET_ALREADY_KNOWN"}}),
        )?,
        serde_json::to_vec(
            &json!({"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"SECRET_ALREADY_KNOWN"}}),
        )?,
    ];
    for (index, body) in bodies.into_iter().enumerate() {
        let (f, c) = client(Reply::raw(200, body)).await?;
        let error = c.submit_signed(signed()?).await.expect_err("bad reply");
        assert_eq!(
            error,
            Error::SubmissionOutcomeUnknown(if index == 7 {
                SubmissionFailure::Rpc
            } else {
                SubmissionFailure::InvalidResponse
            })
        );
        assert_eq!(f.requests()?.len(), 3);
        assert!(!format!("{error:?} {error}").contains("SECRET"));
    }
    Ok(())
}

#[tokio::test]
async fn every_response_framing_bounds_post_dispatch_bytes_without_retry() -> TestResult {
    for framing in [
        Framing::ContentLength,
        Framing::Chunked,
        Framing::CloseDelimited,
    ] {
        let mut reply = Reply::raw(200, vec![b' '; 1025]);
        reply.framing = framing;
        let (f, c) = client(reply).await?;
        assert_eq!(
            c.submit_signed(signed()?).await,
            Err(Error::SubmissionOutcomeUnknown(
                SubmissionFailure::ResponseTooLarge
            ))
        );
        assert_eq!(f.requests()?.len(), 3);
    }
    Ok(())
}

#[tokio::test]
async fn local_or_changed_chain_preflight_failure_dispatches_zero_writes() -> TestResult {
    let f = Fixture::start(vec![Reply::result(&json!("0x2"))?]).await?;
    let c = EvmClient::connect(config(&f, 2, 1024, Duration::from_secs(3))?).await?;
    assert_eq!(
        c.submit_signed(signed()?).await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    assert_eq!(f.requests()?.len(), 1);
    let f = Fixture::start(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&json!("0x2"))?,
    ])
    .await?;
    let c = EvmClient::connect(config(&f, 1, 1024, Duration::from_secs(3))?).await?;
    assert_eq!(
        c.submit_signed(signed()?).await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    assert_eq!(f.requests()?.len(), 2);
    Ok(())
}

#[tokio::test]
async fn total_deadline_preserves_preflight_and_dispatch_phases() -> TestResult {
    for after_dispatch in [false, true] {
        let mut verify = Reply::result(&json!("0x1"))?;
        let mut write = Reply::result(&json!(signed()?.transaction_id().to_string()))?;
        if after_dispatch {
            write.body_delay = Duration::from_secs(3);
        } else {
            verify.delay = Duration::from_secs(3);
        }
        let f = Fixture::start(vec![Reply::result(&json!("0x1"))?, verify, write]).await?;
        let c = EvmClient::connect(config(&f, 1, 1024, Duration::from_secs(2))?).await?;
        assert_eq!(
            c.submit_signed(signed()?).await,
            Err(if after_dispatch {
                Error::SubmissionOutcomeUnknown(SubmissionFailure::Timeout)
            } else {
                Error::Timeout
            })
        );
        let requests = f.requests()?;
        assert_eq!(requests.len(), if after_dispatch { 3 } else { 2 });
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.body["method"] == "eth_sendRawTransaction")
                .count(),
            usize::from(after_dispatch)
        );
    }
    Ok(())
}

#[tokio::test]
async fn cancellation_after_observed_dispatch_does_not_detach_or_retry_submission() -> TestResult {
    let mut reply = Reply::result(&json!(signed()?.transaction_id().to_string()))?;
    reply.delay = Duration::from_secs(6);
    let (f, c) = client(reply).await?;
    {
        let mut future = std::pin::pin!(c.submit_signed(signed()?));
        tokio::select! {
            result=&mut future=>return Err(format!("unexpected completion {result:?}").into()),
            result=tokio::time::timeout(Duration::from_secs(2),async{
                loop{if f.requests()?.iter().any(|r|r.body["method"]=="eth_sendRawTransaction"){break Ok::<_,std::io::Error>(());}
                    tokio::task::yield_now().await;}
            })=>result??,
        }
    }
    // Request arrival, rather than an arbitrary pre-cancellation sleep, proves
    // the write crossed the boundary before the future was dropped.
    assert_eq!(f.requests()?.len(), 3);
    Ok(())
}
