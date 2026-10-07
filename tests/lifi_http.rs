// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Real-loopback LI.FI quotes, unchanged continuation replay and progress mapping.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "lifi-http")]

use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{Amount, ExactDecimal, lifi::*},
    error::{Error, ProviderError},
    protocols::lifi::{LifiClient, LifiHttpConfig, LifiReader},
};
use serde_json::{Value, json};
use std::time::Duration;
#[path = "fixtures/lifi/server.rs"]
mod server;
use server::{Fixture, Reply};
const QUOTE: &str = include_str!("fixtures/lifi/quote.json");
const ROUTES: &str = include_str!("fixtures/lifi/routes.json");
const STATUS: &str = include_str!("fixtures/lifi/status.json");
const ACCOUNT: &str = "0x204dedcf79dbbb02359205f4f64ce2cbdd483906";
const NATIVE: &str = "0x0000000000000000000000000000000000000000";
const HASH: &str = "0xe1ffdcf09d5aa92a2d89b1b39db3f8cadf09428a296cce0d5e387595ac83d08f";
fn chain(id: u64) -> Result<Chain, Error> {
    Chain::new(id, Family::Evm)
}
fn request() -> Result<Request, Error> {
    let from = chain(42161)?;
    let to = chain(8453)?;
    Request::new(RequestData {
        from: Asset::new(from, NATIVE)?,
        to: Asset::new(to, NATIVE)?,
        amount: Amount::from_decimal("10000000000000000", None)?,
        from_account: Account::new(from, ACCOUNT)?,
        to_account: Some(Account::new(to, ACCOUNT)?),
        slippage: Slippage::new(ExactDecimal::parse("0.005")?)?,
        allow_switch_chain: false,
    })
}
fn query(to: u64) -> Result<StatusQuery, Error> {
    StatusQuery::new(
        chain(42161)?,
        chain(to)?,
        StatusSelector::SendingTransaction(TransactionId::new(chain(42161)?, HASH)?),
        None,
    )
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn configured(
    f: &Fixture,
    budget: Duration,
    bytes: usize,
    retries: u8,
    limits: Limits,
    key: &str,
) -> Result<LifiClient, Error> {
    LifiClient::new(LifiHttpConfig::new(
        HttpConfig::new(
            RpcEndpoint::new(&f.endpoint)?.with_header("x-lifi-api-key", key)?,
            RpcLimits::new(budget, budget, bytes, retries)?,
            "lifi-fixture",
        )?,
        ChainCatalogue::new(vec![chain(42161)?, chain(8453)?, chain(167_000)?])?,
        limits,
    )?)
}
fn client(f: &Fixture) -> Result<LifiClient, Error> {
    configured(
        f,
        Duration::from_secs(5),
        2 * 1024 * 1024,
        0,
        Limits::new(64, 256, 128)?,
        "secret-key",
    )
}

#[tokio::test]
async fn all_four_operations_use_real_paths_exact_source_values_and_review_snapshots()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        Reply::ok(QUOTE),
        Reply::ok(ROUTES),
        Reply::ok(QUOTE),
        Reply::ok(STATUS),
    ])
    .await?;
    let client = client(&fixture)?;
    let request = request()?;
    let quote = LifiReader::get_quote(&client, request.clone()).await?;
    assert_eq!(
        quote
            .step()
            .data()
            .action
            .data()
            .from_amount
            .raw()
            .to_string(),
        "10000000000000000"
    );
    assert_eq!(quote.origin().source.provider_id(), "lifi-fixture");
    assert_eq!(
        quote
            .step()
            .data()
            .estimate
            .as_ref()
            .ok_or(Error::UnavailableData)?
            .data()
            .expiry,
        Expiry::Unreported
    );
    assert_eq!(
        quote.step().data().included_steps[1]
            .data()
            .estimate
            .as_ref()
            .ok_or(Error::UnavailableData)?
            .data()
            .execution_seconds
            .canonical(),
        "45.6"
    );
    let routes = LifiReader::get_routes(&client, request.clone()).await?;
    assert_eq!(routes.routes().len(), 8);
    let unavailable = routes.unavailable().ok_or(Error::UnavailableData)?;
    assert_eq!(unavailable.filtered_paths.len(), 278);
    assert_eq!(unavailable.failed_paths.len(), 4);
    assert!(!unavailable.failures.is_empty());
    let prepared = LifiReader::prepare_step(&client, &quote).await?;
    assert_eq!(prepared.original_request(), &request);
    assert_eq!(prepared.selected_step(), quote.step());
    assert_eq!(prepared.selected_origin(), quote.origin());
    assert_eq!(prepared.payload().chain(), chain(42161)?);
    assert_eq!(
        serde_json::from_str::<PreparedStep>(&serde_json::to_string(&prepared)?)?,
        prepared
    );
    let status = LifiReader::get_status(&client, query(167_000)?).await?;
    assert_eq!(status.data().progress, Progress::Done);
    assert_eq!(status.receiving_role(), ReceivingRole::Destination);
    assert_eq!(
        status
            .data()
            .sending
            .as_ref()
            .ok_or(Error::UnavailableData)?
            .data()
            .amount
            .ok_or(Error::UnavailableData)?
            .raw()
            .to_string(),
        "129486280"
    );
    let receiving = status
        .data()
        .receiving
        .as_ref()
        .ok_or(Error::UnavailableData)?
        .data();
    assert_eq!(receiving.chain, chain(167_000)?);
    assert_ne!(
        status
            .data()
            .transfer_id
            .as_ref()
            .ok_or(Error::UnavailableData)?
            .as_str(),
        HASH
    );
    assert_eq!(
        serde_json::from_str::<Status>(&serde_json::to_string(&status)?)?,
        status
    );
    let captured = fixture.requests()?;
    assert_four_paths(&captured)?;
    assert!(!format!("{client:?}").contains("secret-key"));
    Ok(())
}
fn assert_four_paths(captured: &[server::Request]) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(captured.len(), 4);
    assert!(captured[0].target.starts_with("/v1/quote?"));
    assert!(captured[0].target.contains("slippage=0.005"));
    assert_eq!(captured[1].target, "/v1/advanced/routes");
    assert_eq!(captured[2].target, "/v1/advanced/stepTransaction");
    assert_eq!(captured[2].body, QUOTE.as_bytes());
    let body: Value = serde_json::from_slice(&captured[1].body)?;
    assert_eq!(body["fromAmount"], "10000000000000000");
    assert_eq!(body["options"]["executionType"], "transaction");
    assert_eq!(body["options"]["gasless"], false);
    assert_eq!(captured[3].target, format!("/v1/status?txHash={HASH}"));
    assert!(
        captured
            .iter()
            .all(|r| r.headers.contains("x-lifi-api-key: secret-key"))
    );
    Ok(())
}
#[tokio::test]
async fn private_continuation_replays_unchanged_but_is_not_debug_or_json_exported()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = format!(
        "{{\"privateContinuation\":{{\"marker\":\"private-continuation-value\"}},{}",
        QUOTE.strip_prefix('{').ok_or(Error::UnavailableData)?
    );
    let fixture = Fixture::start(vec![Reply::ok(&raw), Reply::ok(QUOTE)]).await?;
    let client = client(&fixture)?;
    let handle = client.get_quote(request()?).await?;
    client.prepare_step(&handle).await?;
    assert_eq!(fixture.requests()?[1].body, raw.as_bytes());
    assert!(!format!("{handle:?}").contains("private-continuation-value"));
    assert!(!serde_json::to_string(&handle)?.contains("privateContinuation"));
    Ok(())
}
#[tokio::test]
async fn exact_numeric_source_lexemes_and_request_slippage_never_use_floats()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = QUOTE
        .replace(
            "\"executionDuration\":46",
            "\"executionDuration\":9007199254740993.125000000000000001",
        )
        .replace("0.005", "0.005000000000000001");
    let mut r = request()?.data().clone();
    r.slippage = Slippage::new(ExactDecimal::parse("0.005000000000000001")?)?;
    let fixture = Fixture::start(vec![Reply::ok(raw)]).await?;
    let quote = client(&fixture)?.get_quote(Request::new(r)?).await?;
    assert_eq!(
        quote
            .step()
            .data()
            .estimate
            .as_ref()
            .ok_or(Error::UnavailableData)?
            .data()
            .execution_seconds
            .canonical(),
        "9007199254740993.125000000000000001"
    );
    assert!(
        fixture.requests()?[0]
            .target
            .contains("slippage=0.005000000000000001")
    );
    Ok(())
}
#[tokio::test]
async fn malformed_numbers_duplicates_and_private_number_objects_fail_wholly()
-> Result<(), Box<dyn std::error::Error>> {
    for raw in [
        QUOTE.replace(
            "\"executionDuration\":46",
            "\"executionDuration\":{\"$serde_json::private::Number\":\"46\"}",
        ),
        QUOTE.replace("\"executionDuration\":46", "\"executionDuration\":\"46\""),
        QUOTE.replace("\"executionDuration\":46", "\"executionDuration\":null"),
        QUOTE.replace("\"executionDuration\":46", "\"executionDuration\":-1"),
        QUOTE.replacen("\"chainId\":42161", "\"chainId\":42161,\"chainId\":8453", 1),
        QUOTE.replacen(
            "\"fromAmount\":\"10000000000000000\"",
            "\"fromAmount\":10000000000000000",
            1,
        ),
        format!(
            "{{\"continuation\":{{\"key\":1,\"key\":2}},{}",
            QUOTE.strip_prefix('{').ok_or(Error::UnavailableData)?
        ),
    ] {
        let f = Fixture::start(vec![Reply::ok(raw)]).await?;
        assert_eq!(client(&f)?.get_quote(request()?).await, Err(invalid()));
    }
    Ok(())
}
#[tokio::test]
async fn action_chain_token_accounts_and_minimum_mismatches_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let base: Value = serde_json::from_str(QUOTE)?;
    for (pointer, value) in [
        ("/action/fromChainId", json!(8453)),
        ("/action/toToken/chainId", json!(42161)),
        (
            "/action/fromAddress",
            json!("0x0000000000000000000000000000000000000001"),
        ),
        (
            "/action/toAddress",
            json!("0x0000000000000000000000000000000000000001"),
        ),
        ("/estimate/toAmountMin", json!("99999999999999999999")),
        ("/transactionRequest/chainId", json!(8453)),
        (
            "/transactionRequest/from",
            json!("0x0000000000000000000000000000000000000001"),
        ),
    ] {
        let mut raw = base.clone();
        *raw.pointer_mut(pointer).ok_or(Error::UnavailableData)? = value;
        let f = Fixture::start(vec![Reply::ok(raw.to_string())]).await?;
        assert_eq!(client(&f)?.get_quote(request()?).await, Err(invalid()));
    }
    Ok(())
}
#[tokio::test]
async fn catalogue_failures_are_explicit_and_unknown_ids_are_never_evm()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = QUOTE.replacen("\"chainId\":42161", "\"chainId\":9999999999999999", 1);
    let f = Fixture::start(vec![Reply::ok(raw)]).await?;
    assert_eq!(
        client(&f)?.get_quote(request()?).await,
        Err(Error::UnsupportedCapability)
    );
    let f = Fixture::start(vec![]).await?;
    let mut r = request()?.data().clone();
    r.to = Asset::new(chain(1)?, NATIVE)?;
    r.to_account = Some(Account::new(chain(1)?, ACCOUNT)?);
    assert_eq!(
        client(&f)?.get_quote(Request::new(r)?).await,
        Err(Error::UnsupportedCapability)
    );
    assert!(f.requests()?.is_empty());
    Ok(())
}
#[tokio::test]
async fn cross_family_payloads_preserve_exact_large_chain_numbers_and_source_encodings()
-> Result<(), Box<dyn std::error::Error>> {
    for (from, token, account, encoded, encoding) in [
        (
            Chain::new(9_270_000_000_000_000, Family::Move)?,
            "0x2::sui::SUI".to_owned(),
            format!("0x{}", "11".repeat(32)),
            "{\"sdkTransaction\":\"opaque source data\"}",
            PayloadEncoding::MoveSdkText,
        ),
        (
            Chain::new(1_151_111_081_099_710, Family::Solana)?,
            "11111111111111111111111111111111".to_owned(),
            "11111111111111111111111111111111".to_owned(),
            "AQID",
            PayloadEncoding::SolanaBase64,
        ),
    ] {
        let mut raw: Value = serde_json::from_str(QUOTE)?;
        raw["action"]["fromChainId"] = json!(from.id());
        raw["action"]["fromToken"]["chainId"] = json!(from.id());
        raw["action"]["fromToken"]["address"] = json!(token);
        raw["action"]["fromAddress"] = json!(account);
        raw["includedSteps"] = json!([]);
        raw["estimate"]["feeCosts"] = json!([]);
        raw["estimate"]["gasCosts"] = json!([]);
        raw["estimate"]["approvalAddress"] = Value::Null;
        raw["transactionRequest"] = json!({"data":encoded});
        let f = Fixture::start(vec![Reply::ok(raw.to_string())]).await?;
        let client = LifiClient::new(LifiHttpConfig::new(
            HttpConfig::new(
                RpcEndpoint::new(&f.endpoint)?,
                RpcLimits::new(
                    Duration::from_secs(5),
                    Duration::from_secs(5),
                    2 * 1024 * 1024,
                    0,
                )?,
                "lifi-cross-family-fixture",
            )?,
            ChainCatalogue::new(vec![from, chain(8453)?])?,
            Limits::new(64, 256, 128)?,
        )?)?;
        let mut fields = request()?.data().clone();
        fields.from = Asset::new(from, &token)?;
        fields.from_account = Account::new(from, &account)?;
        let quote = client.get_quote(Request::new(fields)?).await?;
        assert_eq!(
            quote
                .step()
                .data()
                .action
                .data()
                .from_token
                .data()
                .asset
                .chain(),
            from
        );
        let Some(Payload::Encoded(payload)) = &quote.step().data().payload else {
            return Err(Error::UnavailableData.into());
        };
        assert_eq!(payload.chain(), from);
        assert_eq!(payload.encoding(), encoding);
        assert_eq!(payload.text(), encoded);
        assert!(!f.requests()?[0].headers.contains("x-lifi-api-key"));
    }
    Ok(())
}
#[tokio::test]
async fn empty_source_routes_preserve_an_empty_result_and_absent_diagnostics()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::start(vec![Reply::ok("{\"routes\":[]}")]).await?;
    let routes = client(&f)?.get_routes(request()?).await?;
    assert!(routes.routes().is_empty());
    assert!(routes.unavailable().is_none());
    assert_eq!(routes.request(), &request()?);
    let mut raw: Value = serde_json::from_str(ROUTES)?;
    raw["routes"][0]["toToken"]["decimals"] = json!(17);
    let f = Fixture::start(vec![Reply::ok(raw.to_string())]).await?;
    assert_eq!(
        client(&f)?.get_routes(request()?).await.err(),
        Some(invalid())
    );
    Ok(())
}
#[tokio::test]
async fn caller_route_step_and_cost_limits_fail_without_silent_truncation()
-> Result<(), Box<dyn std::error::Error>> {
    for (body, routes) in [(ROUTES, true), (QUOTE, false)] {
        let f = Fixture::start(vec![Reply::ok(body)]).await?;
        let limited = configured(
            &f,
            Duration::from_secs(5),
            2 * 1024 * 1024,
            0,
            Limits::new(1, 1, 1)?,
            "secret-key",
        )?;
        let error = if routes {
            limited.get_routes(request()?).await.err()
        } else {
            limited.get_quote(request()?).await.err()
        };
        assert_eq!(error, Some(invalid()));
    }
    let mut raw: Value = serde_json::from_str(QUOTE)?;
    raw["estimate"]["feeCosts"] = json!([
        raw["estimate"]["feeCosts"][0].clone(),
        raw["estimate"]["feeCosts"][0].clone()
    ]);
    let f = Fixture::start(vec![Reply::ok(raw.to_string())]).await?;
    let limited = configured(
        &f,
        Duration::from_secs(5),
        2 * 1024 * 1024,
        0,
        Limits::new(64, 256, 1)?,
        "secret-key",
    )?;
    assert_eq!(limited.get_quote(request()?).await, Err(invalid()));
    Ok(())
}
#[tokio::test]
async fn preparation_rejects_another_authority_or_key_before_network()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::start(vec![Reply::ok(QUOTE)]).await?;
    let selected = client(&f)?.get_quote(request()?).await?;
    let another = Fixture::start(vec![]).await?;
    assert_eq!(
        client(&another)?.prepare_step(&selected).await,
        Err(Error::Configuration)
    );
    assert!(another.requests()?.is_empty());
    let wrong_key = configured(
        &f,
        Duration::from_secs(5),
        2 * 1024 * 1024,
        0,
        Limits::new(64, 256, 128)?,
        "another-key",
    )?;
    assert_eq!(
        wrong_key.prepare_step(&selected).await,
        Err(Error::Configuration)
    );
    assert_eq!(f.requests()?.len(), 1);
    Ok(())
}
#[tokio::test]
async fn fresh_preparation_retains_old_snapshot_but_rejects_rewritten_selection()
-> Result<(), Box<dyn std::error::Error>> {
    for pointer in ["/id", "/tool", "/action/fromAmount", "/action/slippage"] {
        let mut raw: Value = serde_json::from_str(QUOTE)?;
        let value = if pointer == "/action/slippage" {
            json!(0.01)
        } else if pointer == "/action/fromAmount" {
            json!("10000000000000001")
        } else {
            json!("different")
        };
        *raw.pointer_mut(pointer).ok_or(Error::UnavailableData)? = value;
        let f = Fixture::start(vec![Reply::ok(QUOTE), Reply::ok(raw.to_string())]).await?;
        let c = client(&f)?;
        let h = c.get_quote(request()?).await?;
        assert_eq!(c.prepare_step(&h).await, Err(invalid()));
    }
    let mut fresh: Value = serde_json::from_str(QUOTE)?;
    fresh["estimate"]["toAmount"] = json!("9973849077309327");
    let f = Fixture::start(vec![Reply::ok(QUOTE), Reply::ok(fresh.to_string())]).await?;
    let c = client(&f)?;
    let h = c.get_quote(request()?).await?;
    let p = c.prepare_step(&h).await?;
    assert_ne!(
        p.selected_step().data().estimate,
        p.refreshed_step().data().estimate
    );
    assert_eq!(p.selected_step(), h.step());
    Ok(())
}
#[tokio::test]
async fn actual_status_hash_chain_and_refund_roles_are_checked()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::start(vec![Reply::ok(STATUS)]).await?;
    assert_eq!(client(&f)?.get_status(query(8453)?).await, Err(invalid()));
    let mut raw: Value = serde_json::from_str(STATUS)?;
    raw["sending"]["txHash"] = json!(format!("0x{:064x}", 1));
    let f = Fixture::start(vec![Reply::ok(raw.to_string())]).await?;
    assert_eq!(
        client(&f)?.get_status(query(167_000)?).await,
        Err(invalid())
    );
    let mut refund: Value = serde_json::from_str(STATUS)?;
    refund["substatus"] = json!("REFUNDED");
    refund["receiving"] = refund["sending"].clone();
    let f = Fixture::start(vec![Reply::ok(refund.to_string())]).await?;
    let status = client(&f)?.get_status(query(167_000)?).await?;
    assert_eq!(status.receiving_role(), ReceivingRole::Refund);
    let f = Fixture::start(vec![Reply::ok("{\"status\":\"NOT_FOUND\"}")]).await?;
    let status = client(&f)?.get_status(query(8453)?).await?;
    assert!(status.data().sending.is_none());
    assert!(status.data().receiving.is_none());
    Ok(())
}
#[tokio::test]
async fn provider_transfer_lookup_uses_exact_id_and_rejects_a_different_reported_id()
-> Result<(), Box<dyn std::error::Error>> {
    let mut raw: Value = serde_json::from_str(STATUS)?;
    let id = TransferId::new(
        raw["transactionId"]
            .as_str()
            .ok_or(Error::UnavailableData)?,
    )?;
    let q = StatusQuery::new(
        chain(42161)?,
        chain(167_000)?,
        StatusSelector::ProviderTransfer(id.clone()),
        None,
    )?;
    let f = Fixture::start(vec![Reply::ok(STATUS)]).await?;
    let status = client(&f)?.get_status(q.clone()).await?;
    assert_eq!(status.data().transfer_id.as_ref(), Some(&id));
    assert_eq!(
        f.requests()?[0].target,
        format!("/v1/status?txHash={}", id.as_str())
    );
    raw["transactionId"] = json!(format!("0x{}", "11".repeat(32)));
    let f = Fixture::start(vec![Reply::ok(raw.to_string())]).await?;
    assert_eq!(client(&f)?.get_status(q.clone()).await, Err(invalid()));
    raw["transactionId"] = Value::Null;
    let f = Fixture::start(vec![Reply::ok(raw.to_string())]).await?;
    let status = client(&f)?.get_status(q).await?;
    assert!(status.data().transfer_id.is_none());
    Ok(())
}
#[tokio::test]
async fn safe_read_post_retries_keep_exact_body_and_errors_keep_private_source_bodies()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::start(vec![
        Reply::status(429, "private-provider-message"),
        Reply::ok(ROUTES),
    ])
    .await?;
    let c = configured(
        &f,
        Duration::from_secs(5),
        2 * 1024 * 1024,
        1,
        Limits::new(64, 256, 128)?,
        "secret-key",
    )?;
    c.get_routes(request()?).await?;
    let captured = f.requests()?;
    assert_eq!(captured[0], captured[1]);
    for status in [403, 500] {
        let f = Fixture::start(vec![Reply::status(status, "private-provider-message")]).await?;
        let error = client(&f)?
            .get_quote(request()?)
            .await
            .err()
            .ok_or(Error::UnavailableData)?;
        assert_eq!(error, Error::Provider(ProviderError::HttpStatus));
        for rendered in [
            error.to_string(),
            format!("{error:?}"),
            serde_json::to_string(&error)?,
        ] {
            assert!(!rendered.contains("private-provider-message"));
        }
    }
    Ok(())
}
#[tokio::test]
async fn one_operation_deadline_covers_retry_and_body_and_size_limit_is_real()
-> Result<(), Box<dyn std::error::Error>> {
    let mut unavailable = Reply::status(429, "private");
    unavailable.header_delay = Duration::from_secs(2);
    let mut slow = Reply::ok(QUOTE);
    slow.body_delay = Duration::from_secs(2);
    let f = Fixture::start(vec![unavailable, slow]).await?;
    let c = configured(
        &f,
        Duration::from_secs(3),
        2 * 1024 * 1024,
        1,
        Limits::new(64, 256, 128)?,
        "secret-key",
    )?;
    assert_eq!(c.get_quote(request()?).await, Err(Error::Timeout));
    assert_eq!(f.requests()?.len(), 2);
    let f = Fixture::start(vec![Reply::ok(QUOTE)]).await?;
    let c = configured(
        &f,
        Duration::from_secs(5),
        100,
        0,
        Limits::new(64, 256, 128)?,
        "secret-key",
    )?;
    assert_eq!(
        c.get_quote(request()?).await,
        Err(Error::Provider(ProviderError::ResponseTooLarge))
    );
    Ok(())
}
#[test]
fn backend_operations_are_send_and_require_runtime_before_first_request() -> Result<(), Error> {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    fn send<T: Future + Send>(_: T) {}
    let c = LifiClient::new(LifiHttpConfig::new(
        HttpConfig::new(
            RpcEndpoint::new("http://127.0.0.1:1/v1")?,
            RpcLimits::new(Duration::from_secs(1), Duration::from_secs(1), 1024, 0)?,
            "lifi-fixture",
        )?,
        ChainCatalogue::new(vec![chain(42161)?, chain(8453)?])?,
        Limits::new(1, 1, 1)?,
    )?)?;
    send(c.get_quote(request()?));
    send(c.get_routes(request()?));
    send(c.get_status(query(8453)?));
    let mut future = Box::pin(c.get_quote(request()?));
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}
