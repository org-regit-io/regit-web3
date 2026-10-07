// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Browser execution of bounded RPC/provider reads and one-shot fixture submissions.

#![cfg(test)]
#![cfg(all(target_arch = "wasm32", target_os = "unknown", feature = "evm-http"))]

use std::{future::Future, time::Duration};

use regit_web3::{
    chains::evm::EvmClient,
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::{
        Address, BlockSelector, ChainId, Finality, NetworkId,
        evm::{Data, SignedSubmission},
    },
    error::{Error, ProviderError, SubmissionFailure},
};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

// Fetch is the deterministic fixture boundary. Request, Response, streams,
// abort signals, timers and the Rust library execute in the real browser.
#[wasm_bindgen(inline_js = r#"
let fixture;
const originalFetch = globalThis.fetch;
const encoder = new TextEncoder();
function delay(milliseconds, signal) {
    return new Promise((resolve, reject) => {
        if (signal.aborted) {
            reject(new DOMException('Fixture aborted', 'AbortError'));
            return;
        }
        const abort = () => {
            clearTimeout(timer);
            reject(new DOMException('Fixture aborted', 'AbortError'));
        };
        const timer = setTimeout(() => {
            signal.removeEventListener('abort', abort);
            resolve();
        }, milliseconds);
        signal.addEventListener('abort', abort, { once: true });
    });
}
export function installFixture(replies) {
    fixture = { replies: JSON.parse(replies), requests: [] };
    globalThis.fetch = async (input, options) => {
        const active = fixture;
        const request = input instanceof Request ? input : new Request(input, options);
        const record = {
            request, method: request.method, url: request.url,
            redirect: request.redirect, credentials: request.credentials,
            cache: request.cache, referrerPolicy: request.referrerPolicy,
            mode: request.mode, headers: Object.fromEntries(request.headers),
            rawBody: null, body: null, streamCanceled: false
        };
        active.requests.push(record);
        record.rawBody = await request.clone().text();
        if (record.rawBody) record.body = JSON.parse(record.rawBody);
        const reply = active.replies.shift();
        if (!reply) throw new Error('Unexpected fixture request');
        await delay(reply.header_delay_ms || 0, request.signal);
        if (reply.reject) throw new TypeError('Fixture transport rejected');
        let stopped = false;
        const chunks = reply.chunks || [{ text: reply.body || '', delay_ms: 0 }];
        const stream = new ReadableStream({
            start(controller) {
                (async () => {
                    try {
                        for (const chunk of chunks) {
                            await delay(chunk.delay_ms || 0, request.signal);
                            if (stopped) return;
                            controller.enqueue(encoder.encode(chunk.text));
                        }
                        if (!stopped) controller.close();
                    } catch (error) {
                        if (!stopped) controller.error(error);
                    }
                })();
            },
            cancel() { stopped = true; record.streamCanceled = true; }
        });
        return new Response(stream, {
            status: reply.status || 200,
            headers: reply.headers || { 'content-type': 'application/json' }
        });
    };
}
export function fixtureSnapshot() {
    return JSON.stringify(fixture.requests.map(({ request, ...record }) => ({
        ...record, aborted: request.signal.aborted
    })));
}
export function restoreFixture() { globalThis.fetch = originalFetch; }
export function fixtureWait(milliseconds) {
    return new Promise(resolve => setTimeout(resolve, milliseconds));
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = installFixture)]
    fn install_fixture(replies: &str);
    #[wasm_bindgen(js_name = fixtureSnapshot)]
    fn fixture_snapshot() -> String;
    #[wasm_bindgen(js_name = restoreFixture)]
    fn restore_fixture();
    #[wasm_bindgen(js_name = fixtureWait)]
    fn fixture_wait(milliseconds: u32) -> js_sys::Promise;
}

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ADDRESS: &str = "0x0000000000000000000000000000000000000001";
const SIGNED: &str = "0xf86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";

struct Fixture;
impl Fixture {
    fn new(replies: &[Value]) -> Result<Self, serde_json::Error> {
        install_fixture(&serde_json::to_string(replies)?);
        Ok(Self)
    }
    fn requests() -> Result<Vec<Value>, serde_json::Error> {
        serde_json::from_str(&fixture_snapshot())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        restore_fixture();
    }
}

fn result(value: Value) -> Value {
    let mut envelope = json!({"jsonrpc":"2.0","id":1});
    envelope["result"] = value;
    json!({"body":envelope.to_string()})
}
fn block_hash() -> String {
    format!("0x{}", "ab".repeat(32))
}
fn block() -> Value {
    result(json!({"number":"0x2a","hash":block_hash(),"timestamp":"0x6553f100"}))
}
fn config(
    bytes: usize,
    retries: u8,
    connect: Duration,
    total: Duration,
) -> Result<EvmConfig, Error> {
    EvmConfig::new(
        NetworkId::new(ChainId::from(1), "browser-fixture")?,
        RpcEndpoint::new("https://fixture.invalid/rpc?api-key=FAKE_QUERY_CREDENTIAL")?
            .with_header("authorization", "Bearer FAKE_HEADER_CREDENTIAL")?,
        18,
        Some("ETH".to_owned()),
        BlockSelector::Safe,
        RpcLimits::new(connect, total, bytes, retries)?,
        "browser-fixture",
    )
}
fn ordinary_config(bytes: usize, retries: u8) -> Result<EvmConfig, Error> {
    config(
        bytes,
        retries,
        Duration::from_secs(1),
        Duration::from_secs(3),
    )
}
async fn wait(milliseconds: u32) -> TestResult {
    fixture_wait(milliseconds)
        .await
        .map_err(|_| "browser fixture timer rejected")?;
    Ok(())
}

#[wasm_bindgen_test]
async fn browser_balance_uses_exact_values_and_explicit_fetch_policy() -> TestResult {
    let _fixture = Fixture::new(&[
        result(json!("0x1")),
        result(json!("0x1")),
        block(),
        result(json!("0x20000000000001")),
    ])?;
    let client = EvmClient::connect(ordinary_config(4096, 0)?).await?;
    let observed = client
        .get_native_balance(Address::parse(ADDRESS)?, None)
        .await?;
    assert_eq!(
        observed.value().amount().raw().to_string(),
        "9007199254740993"
    );
    assert_eq!(
        observed.context().requested_selector(),
        &BlockSelector::Safe
    );
    assert_eq!(observed.context().block().hash().to_string(), block_hash());
    assert_eq!(observed.context().block().number(), 42);
    assert_eq!(observed.context().finality(), Finality::Unknown);
    assert_eq!(observed.context().confirmations(), None);
    assert!(observed.context().retrieved_at().unix_seconds() > 1_700_000_000);
    let requests = Fixture::requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2]["body"]["params"], json!(["safe", false]));
    assert_eq!(
        requests[3]["body"]["params"],
        json!([ADDRESS,{"blockHash":block_hash(),"requireCanonical":true}])
    );
    for request in requests {
        assert_eq!(request["method"], "POST");
        assert_eq!(request["redirect"], "error");
        assert_eq!(request["credentials"], "omit");
        assert_eq!(request["cache"], "no-store");
        assert_eq!(request["referrerPolicy"], "no-referrer");
        assert_eq!(request["mode"], "cors");
        assert_eq!(
            request["headers"]["authorization"],
            "Bearer FAKE_HEADER_CREDENTIAL"
        );
        assert_eq!(request["headers"]["content-type"], "application/json");
    }
    Ok(())
}

#[wasm_bindgen_test]
async fn transient_read_retries_keep_the_captured_canonical_request() -> TestResult {
    let _fixture = Fixture::new(&[
        result(json!("0x1")),
        result(json!("0x1")),
        block(),
        json!({"status":429,"headers":{"retry-after":"0"},"body":"FAKE_RATE_LIMIT_TEXT"}),
        json!({"status":503,"body":"FAKE_SERVER_TEXT"}),
        result(json!("0x20000000000001")),
    ])?;
    let client = EvmClient::connect(ordinary_config(4096, 2)?).await?;
    let observed = client
        .get_native_balance(Address::parse(ADDRESS)?, None)
        .await?;
    assert_eq!(observed.context().block().hash().to_string(), block_hash());
    let requests = Fixture::requests()?;
    assert_eq!(requests.len(), 6);
    assert_eq!(requests[3]["rawBody"], requests[4]["rawBody"]);
    assert_eq!(requests[4]["rawBody"], requests[5]["rawBody"]);
    assert_eq!(
        requests[5]["body"]["params"],
        json!([ADDRESS,{"blockHash":block_hash(),"requireCanonical":true}])
    );
    Ok(())
}

#[wasm_bindgen_test]
async fn unknown_length_stream_is_bounded_and_canceled() -> TestResult {
    let _fixture = Fixture::new(&[json!({"chunks":[
        {"text":"x".repeat(32)}, {"text":"x".repeat(33)}
    ]})])?;
    let error = EvmClient::connect(ordinary_config(64, 0)?).await.err();
    assert_eq!(
        error,
        Some(Error::Provider(ProviderError::ResponseTooLarge))
    );
    let requests = Fixture::requests()?;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["aborted"], true);
    Ok(())
}

#[wasm_bindgen_test]
async fn response_header_deadline_aborts_pending_fetch() -> TestResult {
    let mut reply = result(json!("0x1"));
    reply["header_delay_ms"] = json!(500);
    let _fixture = Fixture::new(&[reply])?;
    let error = EvmClient::connect(config(
        4096,
        0,
        Duration::from_millis(60),
        Duration::from_secs(2),
    )?)
    .await
    .err();
    assert_eq!(error, Some(Error::Timeout));
    let requests = Fixture::requests()?;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["aborted"], true);
    Ok(())
}

#[wasm_bindgen_test]
async fn total_read_deadline_includes_stream_consumption() -> TestResult {
    let slow_body = json!({"chunks":[
        {"text":"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":"},
        {"text":"\"0x1\"}","delay_ms":500}
    ]});
    let _fixture = Fixture::new(&[
        result(json!("0x1")),
        result(json!("0x1")),
        block(),
        slow_body,
    ])?;
    let client = EvmClient::connect(config(
        4096,
        0,
        Duration::from_millis(80),
        Duration::from_millis(180),
    )?)
    .await?;
    let error = client
        .get_native_balance(Address::parse(ADDRESS)?, None)
        .await
        .err();
    assert_eq!(error, Some(Error::Timeout));
    let requests = Fixture::requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[3]["aborted"], true);
    Ok(())
}

#[wasm_bindgen_test]
async fn sequential_reads_share_one_deadline_even_when_each_response_is_timely() -> TestResult {
    let delayed = |mut reply: Value| {
        reply["header_delay_ms"] = json!(500);
        reply
    };
    let _fixture = Fixture::new(&[
        result(json!("0x1")),
        delayed(result(json!("0x1"))),
        delayed(block()),
        delayed(result(json!("0x1"))),
    ])?;
    let client = EvmClient::connect(config(
        4096,
        0,
        Duration::from_millis(750),
        Duration::from_millis(1250),
    )?)
    .await?;
    let error = client
        .get_native_balance(Address::parse(ADDRESS)?, None)
        .await
        .err();
    assert_eq!(error, Some(Error::Timeout));
    let requests = Fixture::requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[3]["body"]["method"], "eth_getBalance");
    assert_eq!(requests[3]["aborted"], true);
    Ok(())
}

#[wasm_bindgen_test]
async fn exact_body_limit_succeeds_and_declared_oversize_aborts() -> TestResult {
    let reply = result(json!("0x1"));
    let bytes = reply["body"].as_str().ok_or("fixture body missing")?.len();
    let fixture = Fixture::new(&[reply])?;
    let client = EvmClient::connect(ordinary_config(bytes, 0)?).await?;
    assert_eq!(client.chain_id(), ChainId::from(1));
    assert_eq!(Fixture::requests()?.len(), 1);
    drop(fixture);

    let _fixture = Fixture::new(&[json!({
        "headers":{"content-length":"65"},"body":"small"
    })])?;
    let error = EvmClient::connect(ordinary_config(64, 0)?).await.err();
    assert_eq!(
        error,
        Some(Error::Provider(ProviderError::ResponseTooLarge))
    );
    assert_eq!(Fixture::requests()?[0]["aborted"], true);
    Ok(())
}

#[wasm_bindgen_test]
async fn dropping_a_polled_operation_aborts_its_fetch_without_detached_work() -> TestResult {
    struct WakeProbe(std::sync::atomic::AtomicUsize);
    impl std::task::Wake for WakeProbe {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let mut reply = result(json!("0x1"));
    reply["header_delay_ms"] = json!(500);
    let _fixture = Fixture::new(&[reply])?;
    let probe = std::sync::Arc::new(WakeProbe(std::sync::atomic::AtomicUsize::new(0)));
    let waker = std::task::Waker::from(probe.clone());
    let mut context = std::task::Context::from_waker(&waker);
    let mut operation = Box::pin(EvmClient::connect(ordinary_config(4096, 0)?));
    assert!(operation.as_mut().poll(&mut context).is_pending());
    assert!(std::sync::Arc::strong_count(&probe) > 2);
    drop(operation);
    wait(10).await?;
    assert_eq!(
        std::sync::Arc::strong_count(&probe),
        2,
        "canceled operation retains a task waker through an unsettled host promise"
    );
    assert!(probe.0.load(std::sync::atomic::Ordering::Relaxed) > 0);
    let requests = Fixture::requests()?;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["aborted"], true);
    Ok(())
}

#[wasm_bindgen_test]
async fn browser_controlled_headers_and_url_user_information_are_rejected() -> TestResult {
    let _fixture = Fixture::new(&[])?;
    let endpoint = RpcEndpoint::new("https://user:FAKE_PASSWORD@fixture.invalid")?;
    let with_endpoint = |endpoint| {
        EvmConfig::new(
            NetworkId::new(ChainId::from(1), "browser-fixture")?,
            endpoint,
            18,
            None,
            BlockSelector::Safe,
            RpcLimits::new(Duration::from_secs(1), Duration::from_secs(3), 4096, 0)?,
            "browser-fixture",
        )
    };
    assert_eq!(
        EvmClient::connect(with_endpoint(endpoint)?).await.err(),
        Some(Error::Configuration)
    );
    for name in [
        "cookie",
        "referer",
        "origin",
        "host",
        "user-agent",
        "content-length",
    ] {
        let endpoint = RpcEndpoint::new("https://fixture.invalid")?.with_header(name, "FAKE_VALUE");
        match endpoint {
            Ok(endpoint) => assert_eq!(
                EvmClient::connect(with_endpoint(endpoint)?).await.err(),
                Some(Error::Configuration)
            ),
            Err(error) => assert_eq!(error, Error::Configuration),
        }
    }
    let endpoint = RpcEndpoint::new("https://fixture.invalid")?
        .with_header("x-explicit-fixture", " FAKE_VALUE ")?;
    assert_eq!(
        EvmClient::connect(with_endpoint(endpoint)?).await.err(),
        Some(Error::Configuration)
    );
    assert!(Fixture::requests()?.is_empty());
    Ok(())
}

#[wasm_bindgen_test]
async fn remote_rpc_text_and_config_credentials_do_not_enter_errors() -> TestResult {
    let _fixture = Fixture::new(&[json!({"body":json!({
        "jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"FAKE_REMOTE_SECRET"}
    }).to_string()})])?;
    let configuration = ordinary_config(4096, 0)?;
    let debug = format!("{configuration:?}");
    let error = EvmClient::connect(configuration).await.err();
    assert_eq!(error, Some(Error::Provider(ProviderError::Rpc)));
    let diagnostics = format!("{debug} {error:?}");
    for secret in [
        "FAKE_REMOTE_SECRET",
        "FAKE_QUERY_CREDENTIAL",
        "FAKE_HEADER_CREDENTIAL",
    ] {
        assert!(!diagnostics.contains(secret));
    }
    assert_eq!(Fixture::requests()?.len(), 1);
    Ok(())
}

#[wasm_bindgen_test]
async fn explicit_fixture_submission_is_once_and_retains_unknown_outcome() -> TestResult {
    for (reply, cause) in [
        (
            json!({"status":429,"body":"FAKE_WRITE_FAILURE"}),
            SubmissionFailure::RateLimited,
        ),
        (json!({"reject":true}), SubmissionFailure::Transport),
        (
            json!({"chunks":[{"text":"{}","delay_ms":500}]}),
            SubmissionFailure::Timeout,
        ),
    ] {
        let _fixture = Fixture::new(&[result(json!("0x1")), result(json!("0x1")), reply])?;
        let client = EvmClient::connect(config(
            4096,
            3,
            Duration::from_millis(200),
            Duration::from_millis(300),
        )?)
        .await?;
        let submission = SignedSubmission::new(ChainId::from(1), Data::parse(SIGNED)?)?;
        let error = client.submit_signed(submission).await.err();
        assert_eq!(error, Some(Error::SubmissionOutcomeUnknown(cause)));
        let requests = Fixture::requests()?;
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[2]["body"]["method"], "eth_sendRawTransaction");
        assert_eq!(requests[2]["body"]["params"], json!([SIGNED]));
    }
    Ok(())
}

#[cfg(feature = "coingecko-http")]
#[wasm_bindgen_test]
async fn provider_get_preserves_query_headers_exact_price_and_separate_timestamps() -> TestResult {
    use regit_web3::{
        config::HttpConfig,
        domain::{
            coingecko::{CoinId, Currency, PriceAvailability, PricesRequest},
            market::ItemLimit,
        },
        providers::coingecko::{ApiTier, CoinGeckoClient, CoinGeckoHttpConfig},
    };

    let _fixture = Fixture::new(&[json!({
        "body": r#"{"bitcoin":{"usd":9007199254740993.000000000000000001,"last_updated_at":1700000000}}"#
    })])?;
    let http = HttpConfig::new(
        RpcEndpoint::new("https://fixture.invalid/api/v3")?
            .with_header("x-fixture-id", "market-price")?,
        RpcLimits::new(Duration::from_secs(1), Duration::from_secs(3), 4096, 0)?,
        "browser-market",
    )?;
    let client = CoinGeckoClient::new(CoinGeckoHttpConfig::new(
        &http,
        ApiTier::Demo,
        "FAKE_DEMO_KEY",
        ItemLimit::new(10)?,
    )?)?;
    let observed = client
        .prices(PricesRequest::new(
            vec![CoinId::parse("bitcoin")?],
            vec![Currency::parse("usd")?],
        )?)
        .await?;
    let quotes = observed.value().quotes();
    assert_eq!(quotes.len(), 1);
    assert_eq!(quotes[0].id().as_str(), "bitcoin");
    assert_eq!(quotes[0].currency().as_str(), "usd");
    let PriceAvailability::Present(price) = quotes[0].availability() else {
        return Err("provider fixture did not preserve a reported price".into());
    };
    assert_eq!(
        price.value().canonical(),
        "9007199254740993.000000000000000001"
    );
    assert_eq!(
        quotes[0]
            .last_updated_at()
            .map(regit_web3::domain::Timestamp::unix_seconds),
        Some(1_700_000_000)
    );
    assert!(observed.retrieved_at().unix_seconds() > 1_700_000_000);
    assert_eq!(observed.source().provider_id(), "browser-market");
    assert_eq!(observed.source().method(), "simple-price");
    let requests = Fixture::requests()?;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "GET");
    assert_eq!(
        requests[0]["url"],
        "https://fixture.invalid/api/v3/simple/price?ids=bitcoin&vs_currencies=usd&include_last_updated_at=true&precision=full"
    );
    assert_eq!(requests[0]["rawBody"], "");
    assert!(requests[0]["body"].is_null());
    assert_eq!(requests[0]["headers"]["x-cg-demo-api-key"], "FAKE_DEMO_KEY");
    assert_eq!(requests[0]["headers"]["x-fixture-id"], "market-price");
    assert_eq!(requests[0]["redirect"], "error");
    assert_eq!(requests[0]["credentials"], "omit");
    assert_eq!(requests[0]["cache"], "no-store");
    assert_eq!(requests[0]["referrerPolicy"], "no-referrer");
    assert_eq!(requests[0]["mode"], "cors");
    Ok(())
}
