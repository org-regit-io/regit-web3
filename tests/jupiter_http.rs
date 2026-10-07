// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Real loopback Jupiter V2 GETs, source bounds and Solana estimate composition.
#![cfg(feature = "jupiter-http")]
#[path = "support/rpc_server.rs"]
mod rpc_server;
#[path = "jupiter_support/mod.rs"]
mod support;
use regit_web3::{
    chains::solana::{SolanaClient, SolanaHttpConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        jupiter::{
            Observation, Operation, PreparedSwap, Quote, SwapIntent, SwapIntentData, mainnet,
        },
        solana::{Commitment, ExecutionOutcome, ReadOptions},
    },
    error::{Error, ProviderError},
    protocols::jupiter::{JupiterClient, JupiterHttpConfig, JupiterReader},
};
use rpc_server::{Fixture, Framing, Reply};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use support::{build_request, prepared, quote_request, settings};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn assert_send<T: Send>(_: T) {}
const ORDER: &str = include_str!("fixtures/jupiter/order_v2.json");
const BUILD: &str = include_str!("fixtures/jupiter/build_v2.json");
fn http(endpoint: &str, timeout: Duration, bytes: usize, retries: u8) -> Result<HttpConfig, Error> {
    HttpConfig::new(
        RpcEndpoint::new(endpoint)?,
        RpcLimits::new(Duration::from_secs(1).min(timeout), timeout, bytes, retries)?,
        "jupiter-fixture",
    )
}
fn client(f: &Fixture) -> Result<JupiterClient, Error> {
    JupiterClient::new(JupiterHttpConfig::new(
        http(&f.endpoint, Duration::from_secs(10), 2 * 1024 * 1024, 2)?,
        mainnet("mainnet")?,
        None,
    )?)
}
async fn quote_error(body: String) -> TestResult {
    let f = Fixture::start(vec![Reply::raw(200, body.into_bytes())]).await?;
    let error = client(&f)?
        .quote(quote_request()?)
        .await
        .err()
        .ok_or("missing failure")?;
    assert_eq!(error, Error::Provider(ProviderError::InvalidResponse));
    assert_eq!(f.requests()?.len(), 1);
    assert!(!format!("{error:?}").contains("SECRET"));
    Ok(())
}
async fn build_error(body: Value) -> TestResult {
    let f = Fixture::start(vec![Reply::json(&body)?]).await?;
    assert_eq!(
        client(&f)?.build(build_request()?).await.err(),
        Some(Error::Provider(ProviderError::InvalidResponse))
    );
    assert_eq!(f.requests()?.len(), 1);
    Ok(())
}
#[tokio::test]
async fn quote_only_actual_source_fixture_preserves_routes_fee_distinctions_and_context()
-> TestResult {
    let f = Fixture::start(vec![Reply::raw(200, ORDER.as_bytes().to_vec())]).await?;
    let c = client(&f)?;
    let before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let q = JupiterReader::quote(&c, quote_request()?).await?;
    assert_eq!(q.value().data().out_amount, 118_139);
    assert_eq!(q.value().data().other_amount_threshold, 116_957);
    assert_eq!(q.value().data().routes.len(), 3);
    assert_eq!(q.value().data().fees.total_fee_bps, Some(2));
    assert_eq!(q.value().data().fees.platform_fee_amount, None);
    assert_eq!(q.value().data().validity.last_valid_block_height, None);
    assert_eq!(q.context().operation(), Operation::OrderQuote);
    assert_eq!(q.context().source().method(), "swap_v2_order");
    assert_eq!(
        q.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!(q.context().retrieved_at().unix_seconds() >= before);
    assert_eq!(
        serde_json::from_value::<Observation<Quote>>(serde_json::to_value(&q)?)?,
        q
    );
    let requests = f.requests()?;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].headers.starts_with("GET "));
    assert!(!requests[0].target.contains("taker="));
    assert!(requests[0].target.contains("slippageBps=100"));
    assert!(requests[0].target.contains("excludeRouters=jupiterz"));
    Ok(())
}
#[tokio::test]
async fn actual_v2_build_compiles_v0_with_exact_fresh_threshold_height_hash_and_source_price()
-> TestResult {
    let f = Fixture::start(vec![Reply::raw(200, BUILD.as_bytes().to_vec())]).await?;
    let c = client(&f)?;
    let build = JupiterReader::build(&c, build_request()?).await?;
    assert_eq!(build.value().data().out_amount, 118_101);
    assert_eq!(build.value().data().other_amount_threshold, 116_920);
    assert_eq!(build.value().compute_unit_price()?, 1484);
    assert_eq!(build.value().data().last_valid_block_height, 432_148_912);
    assert_eq!(
        build.value().data().blockhash_fetched_at.seconds(),
        1_791_346_143
    );
    let p = PreparedSwap::new(SwapIntent::new(SwapIntentData {
        build: build.clone(),
        settings: settings(),
    })?)?;
    assert_eq!(
        p.unsigned().transaction().message().recent_blockhash(),
        build.value().data().blockhash
    );
    assert_eq!(p.unsigned().lifetime().last_valid_block_height, 432_148_912);
    assert_eq!(
        serde_json::from_value::<PreparedSwap>(serde_json::to_value(&p)?)?,
        p
    );
    let request = &f.requests()?[0];
    for parameter in [
        "transactionVersion=0",
        "payer=",
        "taker=",
        "wrapAndUnwrapSol=true",
        "maxAccounts=64",
        "blockhashSlotsToExpiry=150",
        "computeUnitPricePercentile=5000",
        "platformFeeBps=0",
        "forJitoBundle=false",
    ] {
        assert!(request.target.contains(parameter));
    }
    assert!(request.headers.starts_with("GET "));
    Ok(())
}
#[tokio::test]
async fn retries_freeze_exact_query_and_sensitive_key_without_fallback() -> TestResult {
    let f = Fixture::start(vec![
        Reply::raw(429, b"SECRET".to_vec()),
        Reply::raw(503, b"SECRET".to_vec()),
        Reply::raw(200, ORDER.as_bytes().to_vec()),
    ])
    .await?;
    let config = JupiterHttpConfig::new(
        http(&f.endpoint, Duration::from_secs(10), 2 * 1024 * 1024, 2)?,
        mainnet("mainnet")?,
        Some("SECRET_KEY"),
    )?;
    assert!(!format!("{config:?}").contains("SECRET_KEY"));
    let c = JupiterClient::new(config)?;
    assert!(c.config().authenticated());
    c.quote(quote_request()?).await?;
    let requests = f.requests()?;
    assert_eq!(requests.len(), 3);
    for r in &requests {
        assert_eq!(r.target, requests[0].target);
        assert!(r.headers.contains("x-api-key: SECRET_KEY"));
        assert!(r.headers.starts_with("GET "));
    }
    Ok(())
}
#[tokio::test]
async fn critical_and_unknown_duplicate_keys_are_rejected_before_normalization() -> TestResult {
    for body in [
        ORDER.replacen(
            "\"outAmount\":\"118139\"",
            "\"outAmount\":\"118139\",\"outAmount\":\"1\"",
            1,
        ),
        ORDER.replacen(
            "\"label\":\"PancakeSwap\"",
            "\"label\":\"PancakeSwap\",\"label\":\"SECRET\"",
            1,
        ),
        ORDER.replacen('{', "{\"unknown\":{\"a\":1,\"a\":2},", 1),
    ] {
        quote_error(body).await?;
    }
    Ok(())
}
#[tokio::test]
async fn source_quote_identity_and_quote_only_null_contract_are_strict() -> TestResult {
    for (field, value) in [
        ("inAmount", json!("01000000")),
        ("inputMint", json!(support::output()?)),
        ("swapMode", json!("ExactOut")),
        ("slippageBps", json!(101)),
        ("transaction", json!("SECRET_TX")),
        ("taker", json!(support::taker()?)),
        ("errorCode", json!(1)),
        ("otherAmountThreshold", json!("1")),
    ] {
        let mut v: Value = serde_json::from_str(ORDER)?;
        v[field] = value;
        quote_error(serde_json::to_string(&v)?).await?;
    }
    for field in ["transaction", "taker", "otherAmountThreshold", "routePlan"] {
        let mut v: Value = serde_json::from_str(ORDER)?;
        v.as_object_mut().ok_or("object")?.remove(field);
        quote_error(serde_json::to_string(&v)?).await?;
    }
    Ok(())
}
#[tokio::test]
async fn route_percentages_use_exact_lexical_numbers_and_reject_number_object_spoof() -> TestResult
{
    let mut v: Value = serde_json::from_str(ORDER)?;
    v["routePlan"][0]["percent"] = json!({"$serde_json::private::Number":"100"});
    quote_error(serde_json::to_string(&v)?).await?;
    let body = ORDER.replacen(
        "\"percent\":100",
        "\"percent\":99.9999999999999999999999",
        1,
    );
    let f = Fixture::start(vec![Reply::raw(200, body.into_bytes())]).await?;
    let q = client(&f)?.quote(quote_request()?).await?;
    assert_eq!(
        q.value().data().routes[0].data().percent.to_string(),
        "99.9999999999999999999999"
    );
    Ok(())
}
#[tokio::test]
async fn source_build_version_tip_compute_budget_and_real_expiry_are_validated() -> TestResult {
    for (field, value) in [
        ("transactionVersion", json!(1)),
        ("tipInstruction", json!({"SECRET":true})),
        ("blockhashWithMetadata", json!(null)),
        ("computeBudgetInstructions", json!([])),
    ] {
        let mut b: Value = serde_json::from_str(BUILD)?;
        b[field] = value;
        build_error(b).await?;
    }
    let mut b: Value = serde_json::from_str(BUILD)?;
    b["blockhashWithMetadata"]["fetchedAt"]["nanos_since_epoch"] = json!(1_000_000_000);
    build_error(b).await?;
    let mut b: Value = serde_json::from_str(BUILD)?;
    b["swapInstruction"]["data"] = json!("AB==");
    build_error(b).await?;
    Ok(())
}
#[tokio::test]
async fn every_nested_collection_is_bounded_and_source_table_null_means_no_tables() -> TestResult {
    let mut o: Value = serde_json::from_str(ORDER)?;
    o["routePlan"] = json!(vec![o["routePlan"][0].clone(); 129]);
    quote_error(serde_json::to_string(&o)?).await?;
    for (field, count) in [("setupInstructions", 65), ("otherInstructions", 65)] {
        let mut b: Value = serde_json::from_str(BUILD)?;
        b[field] = json!(vec![b["swapInstruction"].clone(); count]);
        build_error(b).await?;
    }
    let mut b: Value = serde_json::from_str(BUILD)?;
    b["swapInstruction"]["accounts"] =
        json!(vec![b["swapInstruction"]["accounts"][0].clone(); 257]);
    build_error(b).await?;
    let mut b: Value = serde_json::from_str(BUILD)?;
    let table = b["addressesByLookupTableAddress"]
        .as_object_mut()
        .ok_or("tables")?
        .values_mut()
        .next()
        .ok_or("table")?;
    *table = json!(vec![support::taker()?; 257]);
    build_error(b).await?;
    let mut b: Value = serde_json::from_str(BUILD)?;
    b["addressesByLookupTableAddress"] = Value::Null;
    let f = Fixture::start(vec![Reply::json(&b)?]).await?;
    assert!(
        client(&f)?
            .build(build_request()?)
            .await?
            .value()
            .data()
            .lookup_tables
            .is_empty()
    );
    Ok(())
}
#[tokio::test]
async fn body_framing_bound_status_and_remote_diagnostics_remain_safe() -> TestResult {
    for framing in [
        Framing::ContentLength,
        Framing::CloseDelimited,
        Framing::Chunked,
    ] {
        let mut reply = Reply::raw(200, ORDER.as_bytes().to_vec());
        reply.framing = framing;
        let f = Fixture::start(vec![reply]).await?;
        let c = JupiterClient::new(JupiterHttpConfig::new(
            http(&f.endpoint, Duration::from_secs(5), 128, 0)?,
            mainnet("mainnet")?,
            None,
        )?)?;
        assert_eq!(
            c.quote(quote_request()?).await.err(),
            Some(Error::Provider(ProviderError::ResponseTooLarge))
        );
    }
    for (status, error) in [
        (404, Error::UnavailableData),
        (400, Error::Provider(ProviderError::HttpStatus)),
        (401, Error::Provider(ProviderError::HttpStatus)),
        (429, Error::Provider(ProviderError::RateLimited)),
    ] {
        let f = Fixture::start(vec![Reply::raw(status, b"SECRET_DIAGNOSTIC".to_vec())]).await?;
        let c = JupiterClient::new(JupiterHttpConfig::new(
            http(&f.endpoint, Duration::from_secs(5), 1024, 0)?,
            mainnet("mainnet")?,
            None,
        )?)?;
        assert_eq!(c.quote(quote_request()?).await.err(), Some(error));
    }
    Ok(())
}
#[test]
fn configuration_rejects_hidden_key_query_and_unsupported_genesis_without_requests() -> TestResult {
    let h = http(
        "https://api.jup.ag/swap/v2?amount=1",
        Duration::from_secs(5),
        1024,
        0,
    )?;
    assert!(JupiterHttpConfig::new(h, mainnet("mainnet")?, None).is_err());
    let h = HttpConfig::new(
        RpcEndpoint::new("https://api.jup.ag/swap/v2")?.with_header("x-api-key", "SECRET")?,
        RpcLimits::new(Duration::from_secs(1), Duration::from_secs(5), 1024, 0)?,
        "fixture",
    )?;
    assert!(JupiterHttpConfig::new(h, mainnet("mainnet")?, None).is_err());
    let wrong = regit_web3::domain::solana::Network::new(
        regit_web3::domain::solana::Hash::from_bytes([7; 32]),
        "wrong",
    )?;
    assert!(
        JupiterHttpConfig::new(
            http(
                "https://api.jup.ag/swap/v2",
                Duration::from_secs(5),
                1024,
                0
            )?,
            wrong,
            None
        )
        .is_err()
    );
    Ok(())
}
#[tokio::test]
async fn estimate_uses_exact_bytes_and_keeps_actual_separate_fee_simulation_facts() -> TestResult {
    let p = prepared()?;
    let f=Fixture::start(vec![Reply::result(&json!(mainnet("mainnet")?.genesis_hash()))?,Reply::result(&json!(mainnet("mainnet")?.genesis_hash()))?,Reply::result(&json!({"context":{"slot":100},"value":5000}))?,Reply::result(&json!(mainnet("mainnet")?.genesis_hash()))?,Reply::result(&json!({"context":{"slot":101},"value":{"err":"AccountNotFound","logs":null,"unitsConsumed":null}}))?]).await?;
    let rpc = SolanaClient::connect(SolanaHttpConfig::new(
        mainnet("rpc")?,
        http(&f.endpoint, Duration::from_secs(10), 1024 * 1024, 0)?,
    ))
    .await?;
    let jupiter = client(&f)?;
    assert_send(jupiter.estimate_swap(
        &rpc,
        p.clone(),
        ReadOptions::new(Commitment::Confirmed, None),
    ));
    let estimate = jupiter
        .estimate_swap(
            &rpc,
            p.clone(),
            ReadOptions::new(Commitment::Confirmed, None),
        )
        .await?;
    assert_eq!(estimate.fee().value().lamports, Some(5000));
    assert_eq!(estimate.simulation().context().evaluation_slot(), Some(101));
    assert!(matches!(
        estimate.simulation().value().outcome(),
        ExecutionOutcome::Failed { .. }
    ));
    let r = f.requests()?;
    assert_eq!(r.len(), 5);
    assert_eq!(r[2].body["method"], "getFeeForMessage");
    assert_eq!(r[4].body["method"], "simulateTransaction");
    assert_eq!(r[4].body["params"][1]["sigVerify"], false);
    assert_eq!(r[4].body["params"][1]["replaceRecentBlockhash"], false);
    assert_eq!(r[4].body["params"][1]["encoding"], "base64");
    assert_eq!(estimate.prepared(), &p);
    Ok(())
}
#[tokio::test]
async fn one_outer_deadline_covers_fee_and_simulation_without_hash_refresh() -> TestResult {
    let mut fee = Reply::result(&json!({"context":{"slot":100},"value":5000}))?;
    fee.body_delay = Duration::from_millis(2500);
    let mut simulation =
        Reply::result(&json!({"context":{"slot":101},"value":{"err":null,"logs":null}}))?;
    simulation.body_delay = Duration::from_millis(2500);
    let genesis = json!(mainnet("mainnet")?.genesis_hash());
    let f = Fixture::start(vec![
        Reply::result(&genesis)?,
        Reply::result(&genesis)?,
        fee,
        Reply::result(&genesis)?,
        simulation,
    ])
    .await?;
    let rpc = SolanaClient::connect(SolanaHttpConfig::new(
        mainnet("rpc")?,
        http(&f.endpoint, Duration::from_secs(10), 1024 * 1024, 0)?,
    ))
    .await?;
    let jupiter = JupiterClient::new(JupiterHttpConfig::new(
        http(&f.endpoint, Duration::from_secs(4), 1024 * 1024, 0)?,
        mainnet("mainnet")?,
        None,
    )?)?;
    assert_eq!(
        jupiter
            .estimate_swap(
                &rpc,
                prepared()?,
                ReadOptions::new(Commitment::Confirmed, None)
            )
            .await
            .err(),
        Some(Error::Timeout)
    );
    let requests = f.requests()?;
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[4].body["method"], "simulateTransaction");
    assert_eq!(
        requests[4].body["params"][1]["replaceRecentBlockhash"],
        false
    );
    Ok(())
}
