// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Actual loopback `QuoterV2` calls, captured-hash route comparison and total bounds.
#![cfg(feature = "uniswap-http")]
#[path = "support/rpc_server.rs"]
mod rpc_server;

use regit_web3::{
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::{
        Address, BlockSelector, ChainId, Finality, NetworkId, U256,
        evm::{FeeTerms, Quantity, ReadState},
        uniswap::{
            CallSettings, CallSettingsData, PreparedSwap, RouteOutcome, SlippageBps, SwapIntent,
            SwapIntentData, V3Deployment, V3Path, V3PathData, V3QuoteRequest, V3QuoteRequestData,
            V3RouteRequest, V3RouteRequestData,
        },
    },
    error::{Error, ProviderError},
    protocols::uniswap::{UniswapV3Client, V3QuoteReader},
};
use rpc_server::{Fixture, Framing, Reply};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn hash() -> String {
    format!("0x{}", "ab".repeat(32))
}
fn block() -> Value {
    json!({"hash":hash(),"number":"0x2a","timestamp":"0x64"})
}
fn config(f: &Fixture, timeout: Duration, bytes: usize) -> Result<EvmConfig, Error> {
    EvmConfig::new(
        NetworkId::new(ChainId::from(1), "fixture")?,
        RpcEndpoint::new(&f.endpoint)?,
        18,
        None,
        BlockSelector::Safe,
        RpcLimits::new(Duration::from_secs(1).min(timeout), timeout, bytes, 3)?,
        "uniswap-fixture",
    )
}
fn settings() -> Result<CallSettings, Error> {
    CallSettings::new(CallSettingsData {
        from: Address::from_bytes([7; 20]),
        nonce: 9,
        gas_limit: 500_000,
        fees: FeeTerms::Legacy {
            gas_price: Quantity::from(10),
        },
    })
}
fn path(fee: u32, hops: usize) -> Result<V3Path, Error> {
    V3Path::new(V3PathData {
        tokens: (0..=hops)
            .map(|i| Address::from_bytes([u8::try_from(i).unwrap_or(0) + 3; 20]))
            .collect(),
        fees: vec![fee; hops],
    })
}
fn request(fee: u32, hops: usize) -> Result<V3QuoteRequest, Error> {
    V3QuoteRequest::new(V3QuoteRequestData {
        deployment: V3Deployment::ethereum_mainnet()?,
        path: path(fee, hops)?,
        amount_in: Quantity::from(1_000),
        settings: settings()?,
    })
}
fn route(fees: &[u32]) -> Result<V3RouteRequest, Error> {
    V3RouteRequest::new(V3RouteRequestData {
        deployment: V3Deployment::ethereum_mainnet()?,
        paths: fees
            .iter()
            .map(|fee| path(*fee, 1))
            .collect::<Result<_, _>>()?,
        amount_in: Quantity::from(1_000),
        settings: settings()?,
    })
}
fn output(hops: usize, amount: u64) -> Vec<u8> {
    // Independent ABI fixture per IQuoterV2's four-field output schema.
    let mut bytes = Vec::new();
    for number in [
        amount,
        128,
        160 + 32 * u64::try_from(hops).unwrap_or(0),
        42_000,
        u64::try_from(hops).unwrap_or(0),
    ] {
        bytes.extend(U256::from(number).to_be_bytes::<32>());
    }
    for i in 0..hops {
        bytes.extend(U256::from(123 + i).to_be_bytes::<32>());
    }
    bytes.extend(U256::from(hops).to_be_bytes::<32>());
    for i in 0..hops {
        bytes.extend(U256::from(i + 3).to_be_bytes::<32>());
    }
    bytes
}
fn encoded_reply(bytes: &[u8]) -> Result<Reply, serde_json::Error> {
    Reply::result(&json!(const_hex::encode_prefixed(bytes)))
}
fn prefix() -> Result<Vec<Reply>, serde_json::Error> {
    Ok(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&json!("0x1"))?,
        Reply::result(&block())?,
        Reply::result(&json!("0x2c"))?,
    ])
}
fn call_replies(reply: Reply) -> Result<Vec<Reply>, serde_json::Error> {
    Ok(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&block())?,
        reply,
    ])
}
async fn client(
    replies: Vec<Reply>,
    timeout: Duration,
    bytes: usize,
) -> Result<(Fixture, UniswapV3Client), Box<dyn std::error::Error>> {
    let f = Fixture::start(replies).await?;
    let c = UniswapV3Client::connect(
        config(&f, timeout, bytes)?,
        V3Deployment::ethereum_mainnet()?,
    )
    .await?;
    Ok((f, c))
}
fn rpc_error(code: i64) -> Result<Reply, serde_json::Error> {
    Reply::json(
        &json!({"jsonrpc":"2.0","id":1,"error":{"code":code,"message":"SECRET_REMOTE_TEXT"}}),
    )
}

#[tokio::test]
async fn direct_quote_uses_actual_call_keeps_original_selector_and_never_writes() -> TestResult {
    let mut replies = prefix()?;
    replies.extend(call_replies(encoded_reply(&output(1, 9_999))?)?);
    let (f, c) = client(replies, Duration::from_secs(5), 4096).await?;
    let request = request(500, 1)?;
    let start = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let q = V3QuoteReader::quote_exact_input(&c, request.clone(), None).await?;
    assert_eq!(q.value().data().request, request);
    assert_eq!(q.value().data().amount_out, Quantity::from(9_999));
    assert_eq!(q.value().data().gas_estimate, Quantity::from(42_000));
    assert!(
        matches!(q.context().state(),ReadState::CanonicalHash {requested_selector:BlockSelector::Safe,block} if block.number()==42 && block.hash().to_string()==hash())
    );
    assert_eq!(q.context().finality(), Finality::Unknown);
    assert_eq!(q.context().confirmations(), None);
    assert_eq!(q.context().source().method(), "eth_call");
    assert_eq!(
        q.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!(q.context().retrieved_at().unix_seconds() >= start);
    assert_eq!(
        serde_json::from_value::<regit_web3::domain::uniswap::V3QuoteObservation>(
            serde_json::to_value(&q)?
        )?,
        q
    );
    let requests = f.requests()?;
    assert_eq!(requests.len(), 7);
    assert_eq!(requests[2].body["params"], json!(["safe", false]));
    assert_eq!(requests[3].body["method"], "eth_getTransactionCount");
    assert_eq!(requests[5].body["method"], "eth_getBlockByHash");
    assert_eq!(requests[6].body["method"], "eth_call");
    assert_eq!(
        requests[6].body["params"][1],
        json!({"blockHash":hash(),"requireCanonical":true})
    );
    assert_eq!(
        requests[6].body["params"][0]["data"],
        request.call()?.data().input.to_hex()
    );
    assert_eq!(requests[6].target, "/rpc");
    assert!(requests[6].headers.contains("application/json"));
    let p = PreparedSwap::new(SwapIntent::new(SwapIntentData {
        quote: q,
        recipient: Address::from_bytes([8; 20]),
        slippage: SlippageBps::new(100)?,
        deadline: regit_web3::domain::Timestamp::from_unix_seconds(200),
        settings: settings()?,
    })?)?;
    assert_eq!(p.unsigned().allowances().amount, Quantity::from(1_000));
    assert_eq!(f.requests()?.len(), 7);
    assert!(
        requests
            .iter()
            .all(|r| r.body["method"] != "eth_sendRawTransaction")
    );
    Ok(())
}
#[tokio::test]
async fn multihop_quote_keeps_each_source_price_and_tick_count() -> TestResult {
    let mut replies = prefix()?;
    replies.extend(call_replies(encoded_reply(&output(2, 99))?)?);
    let (_fixture, c) = client(replies, Duration::from_secs(5), 4096).await?;
    let q = c
        .quote_exact_input(request(3000, 2)?, Some(BlockSelector::Latest))
        .await?;
    assert_eq!(
        q.value().data().sqrt_price_x96_after,
        vec![Quantity::from(123), Quantity::from(124)]
    );
    assert_eq!(q.value().data().initialized_ticks_crossed, vec![3, 4]);
    Ok(())
}
#[tokio::test]
async fn provided_routes_share_hash_keep_revert_and_choose_highest_actual_output() -> TestResult {
    for all_reverted in [false, true] {
        let mut replies = prefix()?;
        for index in 0..3 {
            replies.extend(call_replies(if all_reverted || index == 1 {
                rpc_error(3)?
            } else {
                encoded_reply(&output(1, if index == 0 { 99 } else { 100 }))?
            })?);
        }
        let (f, c) = client(replies, Duration::from_secs(5), 4096).await?;
        let comparison =
            V3QuoteReader::compare_routes(&c, route(&[500, 3000, 10_000])?, None).await?;
        assert_eq!(
            comparison.best_index(),
            if all_reverted { None } else { Some(2) }
        );
        assert!(matches!(
            comparison.data().outcomes[1],
            RouteOutcome::Reverted { .. }
        ));
        assert_eq!(
            serde_json::from_value::<regit_web3::domain::uniswap::RouteComparison>(
                serde_json::to_value(&comparison)?
            )?,
            comparison
        );
        let requests = f.requests()?;
        assert_eq!(requests.len(), 13);
        for index in [6, 9, 12] {
            assert_eq!(requests[index].body["method"], "eth_call");
            assert_eq!(
                requests[index].body["params"][1],
                json!({"blockHash":hash(),"requireCanonical":true})
            );
        }
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.body["method"] == "eth_getBlockByNumber")
                .count(),
            1
        );
    }
    Ok(())
}
#[tokio::test]
async fn deployment_mismatch_is_local_before_connection_or_operation_reads() -> TestResult {
    let f = Fixture::start(vec![]).await?;
    let mut wrong = V3Deployment::ethereum_mainnet()?.data().clone();
    wrong.chain_id = ChainId::from(2);
    assert!(
        UniswapV3Client::connect(
            config(&f, Duration::from_secs(5), 4096)?,
            V3Deployment::new(wrong)?
        )
        .await
        .is_err()
    );
    assert!(f.requests()?.is_empty());
    let (f, c) = client(
        vec![Reply::result(&json!("0x1"))?],
        Duration::from_secs(5),
        4096,
    )
    .await?;
    let mut d = request(500, 1)?.data().clone();
    let mut deployment = d.deployment.data().clone();
    deployment.quoter_v2 = Address::from_bytes([13; 20]);
    d.deployment = V3Deployment::new(deployment)?;
    assert!(
        c.quote_exact_input(V3QuoteRequest::new(d)?, None)
            .await
            .is_err()
    );
    assert_eq!(f.requests()?.len(), 1);
    Ok(())
}
#[tokio::test]
async fn malformed_abi_offsets_lengths_widths_trailing_and_empty_are_not_quotes() -> TestResult {
    for variant in 0..8 {
        let mut bytes = output(1, 99);
        match variant {
            0 => bytes.clear(),
            1 => bytes[63] = 129,
            2 => bytes[95] = 160,
            3 => bytes[159] = 2,
            4 => bytes[160] = 1,
            5 => bytes[224] = 1,
            6 => bytes.push(0),
            _ => {
                bytes.truncate(bytes.len() - 1);
            }
        }
        let mut replies = prefix()?;
        replies.extend(call_replies(encoded_reply(&bytes)?)?);
        let (f, c) = client(replies, Duration::from_secs(5), 4096).await?;
        assert!(matches!(
            c.quote_exact_input(request(500, 1)?, None).await,
            Err(Error::Validation(_))
        ));
        assert_eq!(f.requests()?.len(), 7);
    }
    Ok(())
}
#[tokio::test]
async fn unknown_rpc_and_wrong_response_identity_abort_without_route_reclassification() -> TestResult
{
    for variant in 0..3 {
        let reply = match variant {
            0 => rpc_error(-32000)?,
            1 => {
                let mut r = Reply::json(
                    &json!({"jsonrpc":"2.0","id":"wrong","result":const_hex::encode_prefixed(output(1,99))}),
                )?;
                r.echo_id = false;
                r
            }
            _ => Reply::raw(
                200,
                br#"{"jsonrpc":"2.0","id":1,"result":"0x","result":"0x"}"#.to_vec(),
            ),
        };
        let mut replies = prefix()?;
        replies.extend(call_replies(reply)?);
        let (f, c) = client(replies, Duration::from_secs(5), 4096).await?;
        let error = c
            .compare_routes(route(&[500, 3000])?, None)
            .await
            .err()
            .ok_or("expected failure")?;
        assert!(matches!(
            error,
            Error::Provider(ProviderError::Rpc | ProviderError::InvalidResponse)
        ));
        assert!(!format!("{error:?}").contains("SECRET_REMOTE_TEXT"));
        assert_eq!(f.requests()?.len(), 7);
    }
    Ok(())
}
#[tokio::test]
async fn state_identity_mismatch_cannot_qualify_quote() -> TestResult {
    let mut replies = prefix()?;
    let mut bad = block();
    bad["number"] = json!("0x2b");
    replies.extend(call_replies(encoded_reply(&output(1, 99))?)?);
    replies[5] = Reply::result(&bad)?;
    let (f, c) = client(replies, Duration::from_secs(5), 4096).await?;
    assert!(c.quote_exact_input(request(500, 1)?, None).await.is_err());
    assert_eq!(f.requests()?.len(), 7);
    Ok(())
}
#[tokio::test]
async fn call_retries_keep_identical_deployment_path_and_captured_hash() -> TestResult {
    let mut replies = prefix()?;
    replies.extend(call_replies(Reply::raw(429, vec![]))?);
    replies.push(Reply::raw(503, vec![]));
    let mut final_reply = encoded_reply(&output(1, 99))?;
    final_reply.framing = Framing::CloseDelimited;
    replies.push(final_reply);
    let (f, c) = client(replies, Duration::from_secs(5), 4096).await?;
    c.quote_exact_input(request(500, 1)?, None).await?;
    let requests = f.requests()?;
    assert_eq!(requests.len(), 9);
    for retry in [7, 8] {
        assert_eq!(requests[retry].body["params"], requests[6].body["params"]);
        assert_eq!(requests[retry].body["method"], "eth_call");
    }
    Ok(())
}
#[tokio::test]
async fn response_body_limit_is_preserved_by_composed_backend() -> TestResult {
    let mut replies = prefix()?;
    let mut reply = Reply::raw(200, vec![b' '; 2048]);
    reply.framing = Framing::Chunked;
    replies.extend(call_replies(reply)?);
    let (f, c) = client(replies, Duration::from_secs(5), 1024).await?;
    assert!(matches!(
        c.quote_exact_input(request(500, 1)?, None).await,
        Err(Error::Provider(ProviderError::ResponseTooLarge))
    ));
    assert_eq!(f.requests()?.len(), 7);
    Ok(())
}
#[tokio::test]
async fn total_deadline_spans_all_routes_and_last_call_uses_frozen_hash() -> TestResult {
    let mut replies = prefix()?;
    for _ in 0..3 {
        let mut r = encoded_reply(&output(1, 99))?;
        r.delay = Duration::from_millis(1500);
        replies.extend(call_replies(r)?);
    }
    let (f, c) = client(replies, Duration::from_secs(4), 4096).await?;
    assert!(matches!(
        c.compare_routes(route(&[500, 3000, 10_000])?, None).await,
        Err(Error::Timeout)
    ));
    let requests = f.requests()?;
    assert_eq!(requests.len(), 13);
    assert_eq!(requests[12].body["method"], "eth_call");
    assert_eq!(
        requests[12].body["params"][1],
        json!({"blockHash":hash(),"requireCanonical":true})
    );
    assert_eq!(
        requests[12].body["params"][0]["data"],
        route(&[500, 3000, 10_000])?
            .candidate(2)?
            .call()?
            .data()
            .input
            .to_hex()
    );
    Ok(())
}
