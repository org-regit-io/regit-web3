// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Classic Swap v6.1 public HTTP behavior using actual bounded loopback exchanges.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "oneinch-http")]
#[path = "support/oneinch.rs"]
mod fixture;
#[path = "support/market_server.rs"]
mod market_server;
use fixture::{
    DST, FROM, ROUTER, SPENDER, SRC, asset, context, graph, network, prepared, quote, request,
    settings, spender, swap_request, transaction,
};
use market_server::{Fixture, Reply};
use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        ChainId, NetworkId,
        evm::Quantity,
        oneinch::{
            Limits, Method, Quote, QuoteRequest, ReturnPolicy, SlippageBps, StateOverrides,
            SwapRequest,
        },
    },
    error::{Error, ProviderError},
    protocols::oneinch::{ClassicSwapReader, OneinchClient, OneinchHttpConfig},
};
use std::{
    future::Future,
    task::{Context, Poll, Waker},
    time::Duration,
};
type TestError = Box<dyn std::error::Error>;
const QUOTE: &str = include_str!("fixtures/oneinch/quote.json");
const SWAP: &str = include_str!("fixtures/oneinch/swap.json");
const SPENDER_JSON: &str = r#"{"address":"0x5555555555555555555555555555555555555555"}"#;
const SOURCES: &str = r##"{"protocols":[{"id":"UNISWAP_V3","title":"Uniswap V3","img":"https://example.invalid/icon.svg","img_color":"#ff00ff"}]}"##;
const SECRET: &str = "fixture-private-bearer-secret";
fn client(
    endpoint: &str,
    limits: Limits,
    retries: u8,
    bytes: usize,
    total: Duration,
) -> Result<OneinchClient, Error> {
    let endpoint =
        RpcEndpoint::new(endpoint)?.with_header("Authorization", &format!("Bearer {SECRET}"))?;
    OneinchClient::new(OneinchHttpConfig::new(
        HttpConfig::new(
            endpoint,
            RpcLimits::new(total, total, bytes, retries)?,
            "fixture-oneinch",
        )?,
        network()?,
        limits,
    )?)
}
fn standard(endpoint: &str) -> Result<OneinchClient, Error> {
    client(
        endpoint,
        Limits::new(4096, 512)?,
        0,
        2 * 1024 * 1024,
        Duration::from_secs(5),
    )
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
#[tokio::test(flavor = "current_thread")]
async fn all_four_methods_preserve_source_identity_exact_graph_and_original_review()
-> Result<(), TestError> {
    let f = Fixture::start(vec![
        Reply::json(QUOTE),
        Reply::json(SOURCES),
        Reply::json(SPENDER_JSON),
        Reply::json(SPENDER_JSON),
        Reply::json(SWAP),
    ])
    .await?;
    let c = standard(&f.endpoint)?;
    let q = ClassicSwapReader::quote_exact_input(&c, request()?).await?;
    assert_eq!(q.data().destination_amount.value().to_string(), "1000");
    assert_eq!(
        q.data().estimated_gas.unwrap().value().to_string(),
        "9007199254740993"
    );
    assert_eq!(
        q.data().routes.groups()[0].hops[0].protocols[0]
            .percent
            .canonical(),
        "33.000000000000000001"
    );
    assert_eq!(q.data().state_overrides, StateOverrides::Null);
    assert_eq!(q.context().source.provider_id(), "fixture-oneinch");
    assert_eq!(q.context().method, Method::Quote);
    assert_eq!(
        c.get_liquidity_sources().await?.items()[0].id.as_str(),
        "UNISWAP_V3"
    );
    assert_eq!(c.get_spender().await?.address().to_string(), SPENDER);
    let p = c
        .prepare_swap(SwapRequest::new(q.clone(), settings()?)?)
        .await?;
    assert_eq!(p.request().selection(), &q);
    assert_eq!(
        p.fresh_quote()
            .data()
            .destination_amount
            .value()
            .to_string(),
        "950"
    );
    assert_eq!(
        p.fresh_quote().data().routes.groups()[0].hops[0].protocols[0]
            .percent
            .canonical(),
        "80"
    );
    assert_eq!(
        p.transaction().data().gas_price.value().to_string(),
        "9007199254740993"
    );
    assert_eq!(p.transaction().data().gas_used, None);
    assert_eq!(p.spender().context().method, Method::Spender);
    assert_eq!(
        serde_json::from_str::<regit_web3::domain::oneinch::PreparedSwap>(&serde_json::to_string(
            &p
        )?)?,
        p
    );
    let requests = f.requests()?;
    assert_eq!(requests.len(), 5);
    assert!(requests[0].starts_with("GET /api/v3/v6.1/1/quote?"));
    assert!(requests[1].starts_with("GET /api/v3/v6.1/1/liquidity-sources HTTP"));
    assert!(requests[2].starts_with("GET /api/v3/v6.1/1/approve/spender HTTP"));
    let swap = &requests[4];
    assert!(swap.contains("includeTokensInfo=true&includeProtocols=true&includeGas=true"));
    for (name, value) in [
        ("from", FROM),
        ("origin", "0x6666666666666666666666666666666666666666"),
        ("receiver", "0x7777777777777777777777777777777777777777"),
        ("minReturn", "900"),
        ("disableEstimate", "true"),
        ("allowPartialFill", "false"),
        ("usePermit2", "false"),
        ("createAccessList", "false"),
    ] {
        assert!(swap.contains(&format!("{name}={value}")));
    }
    assert!(!swap.contains("slippage="));
    for r in requests {
        assert!(r.contains(&format!("authorization: Bearer {SECRET}")));
        assert!(r.starts_with("GET "));
        assert!(!r.contains("permit="));
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn caller_routing_parameters_and_exclusive_slippage_are_frozen() -> Result<(), TestError> {
    let f = Fixture::start(vec![Reply::json(SPENDER_JSON), Reply::json(SWAP)]).await?;
    let c = standard(&f.endpoint)?;
    let old = quote(Method::Quote, 1000)?;
    let mut routing = old.request().settings().clone();
    routing.protocols = vec![regit_web3::domain::oneinch::ProtocolId::new("UNISWAP_V3")?];
    routing.connector_tokens = vec![asset(ROUTER)?];
    routing.gas_price = Some(Quantity::from_decimal("9007199254740993")?);
    routing.complexity_level = Some(3);
    routing.parts = Some(100);
    routing.main_route_parts = Some(2);
    routing.gas_limit = Some(500_000);
    let r = QuoteRequest::new(
        network()?,
        old.request().source(),
        old.request().destination(),
        old.request().amount(),
        routing,
    )?;
    let selected = Quote::new(r, old.data().clone(), context(Method::Quote)?)?;
    let mut s = settings()?;
    s.return_policy = ReturnPolicy::Slippage(SlippageBps::new(123)?);
    s.disable_estimate = false;
    c.prepare_swap(SwapRequest::new(selected, s.clone())?)
        .await?;
    let request = &f.requests()?[1];
    for q in [
        "protocols=UNISWAP_V3",
        "connectorTokens=0x4444444444444444444444444444444444444444",
        "gasPrice=9007199254740993",
        "complexityLevel=3",
        "parts=100",
        "mainRouteParts=2",
        "gasLimit=500000",
        "slippage=1.23",
        "disableEstimate=false",
    ] {
        assert!(request.contains(q));
    }
    assert!(!request.contains("minReturn="));
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn malformed_numbers_duplicate_fields_and_old_graph_shape_fail_terminally()
-> Result<(), TestError> {
    let cases = [
        QUOTE.replace(
            "33.000000000000000001",
            r#"{"$serde_json::private::Number":"33"}"#,
        ),
        QUOTE.replace(
            "\"dstAmount\":\"1000\"",
            "\"dstAmount\":\"1000\",\"dstAmount\":\"999\"",
        ),
        QUOTE.replace("9007199254740993", "9007199254740993.5"),
        QUOTE.replace("\"protocols\":[{\"token\"", "\"protocols\":[[{\"token\""),
        QUOTE.replace("\"fromTokenId\":0", "\"fromTokenId\":0,\"fromTokenId\":1"),
        QUOTE.replace(
            "\"srcToken\":{\"address\":\"0x1111111111111111111111111111111111111111\"",
            "\"srcToken\":{\"address\":\"0x2222222222222222222222222222222222222222\"",
        ),
        QUOTE.replace("33.000000000000000001", "101"),
        QUOTE.replace("\"dstAmount\":\"1000\"", "\"dstAmount\":null"),
    ];
    for body in cases {
        let f = Fixture::start(vec![Reply::json(&body)]).await?;
        let c = client(
            &f.endpoint,
            Limits::new(4096, 512)?,
            2,
            2 * 1024 * 1024,
            Duration::from_secs(5),
        )?;
        assert_eq!(c.quote_exact_input(request()?).await, Err(invalid()));
        assert_eq!(f.requests()?.len(), 1);
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn override_availability_is_explicit_and_never_used_as_simulation() -> Result<(), TestError> {
    for (replacement, expected) in [
        (",\"stateOverrides\":null", StateOverrides::Null),
        (",\"stateOverrides\":{}", StateOverrides::Empty),
        (
            ",\"stateOverrides\":{\"0x1\":{\"balance\":\"0x1\"}}",
            StateOverrides::PresentUninterpreted,
        ),
        ("", StateOverrides::Unreported),
    ] {
        let body = QUOTE.replace(",\"stateOverrides\":null", replacement);
        let f = Fixture::start(vec![Reply::json(&body)]).await?;
        assert_eq!(
            standard(&f.endpoint)?
                .quote_exact_input(request()?)
                .await?
                .data()
                .state_overrides,
            expected
        );
    }
    let body = QUOTE.replace("\"stateOverrides\":null", "\"stateOverrides\":[]");
    let f = Fixture::start(vec![Reply::json(&body)]).await?;
    assert_eq!(
        standard(&f.endpoint)?.quote_exact_input(request()?).await,
        Err(invalid())
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn scalar_transaction_conflicts_and_unsupported_access_list_are_rejected()
-> Result<(), TestError> {
    for body in [
        SWAP.replace(FROM, SRC),
        SWAP.replace(ROUTER, DST),
        SWAP.replace("\"value\":\"0\"", "\"value\":\"1\""),
        SWAP.replace("\"dstAmount\":\"950\"", "\"dstAmount\":\"899\""),
        SWAP.replace(
            "\"gas\":210000",
            "\"gas\":210000,\"accessList\":[{\"address\":\"0x1\"}]",
        ),
        SWAP.replace("\"isFoT\":false", "\"isFoT\":true"),
    ] {
        let f = Fixture::start(vec![Reply::json(SPENDER_JSON), Reply::json(&body)]).await?;
        let result = standard(&f.endpoint)?.prepare_swap(swap_request()?).await;
        assert!(matches!(
            result,
            Err(Error::Provider(ProviderError::InvalidResponse) | Error::UnsupportedCapability)
        ));
        assert_eq!(f.requests()?.len(), 2);
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn source_spender_conflict_stops_before_swap_payload_request() -> Result<(), TestError> {
    let f = Fixture::start(vec![Reply::json(&SPENDER_JSON.replace(SPENDER, SRC))]).await?;
    assert_eq!(
        standard(&f.endpoint)?.prepare_swap(swap_request()?).await,
        Err(invalid())
    );
    assert_eq!(f.requests()?.len(), 1);
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn whole_catalogue_graph_and_http_body_bounds_fail_without_truncation()
-> Result<(), TestError> {
    let f = Fixture::start(vec![Reply::json(QUOTE)]).await?;
    assert_eq!(
        client(
            &f.endpoint,
            Limits::new(2, 1)?,
            0,
            2 * 1024 * 1024,
            Duration::from_secs(5)
        )?
        .quote_exact_input(request()?)
        .await,
        Err(invalid())
    );
    let duplicate = SOURCES.replace(
        "]}",
        ",{\"id\":\"UNISWAP_V3\",\"title\":\"duplicate\",\"img\":\"\",\"img_color\":\"\"}]}",
    );
    let f = Fixture::start(vec![Reply::json(&duplicate)]).await?;
    assert_eq!(
        standard(&f.endpoint)?.get_liquidity_sources().await,
        Err(invalid())
    );
    let excess = duplicate.replace(
        "\"id\":\"UNISWAP_V3\",\"title\":\"duplicate\"",
        "\"id\":\"OTHER\",\"title\":\"second\"",
    );
    let f = Fixture::start(vec![Reply::json(&excess)]).await?;
    assert_eq!(
        client(
            &f.endpoint,
            Limits::new(4096, 1)?,
            0,
            2 * 1024 * 1024,
            Duration::from_secs(5)
        )?
        .get_liquidity_sources()
        .await,
        Err(invalid())
    );
    let f = Fixture::start(vec![Reply::json(QUOTE)]).await?;
    assert_eq!(
        client(
            &f.endpoint,
            Limits::new(4096, 512)?,
            0,
            128,
            Duration::from_secs(5)
        )?
        .quote_exact_input(request()?)
        .await,
        Err(Error::Provider(ProviderError::ResponseTooLarge))
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn retries_keep_identical_read_only_request_and_private_diagnostics() -> Result<(), TestError>
{
    let f = Fixture::start(vec![
        Reply::status(429),
        Reply::status(503),
        Reply::json(QUOTE),
    ])
    .await?;
    let c = client(
        &f.endpoint,
        Limits::new(4096, 512)?,
        2,
        2 * 1024 * 1024,
        Duration::from_secs(5),
    )?;
    c.quote_exact_input(request()?).await?;
    let requests = f.requests()?;
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(requests[1], requests[2]);
    assert!(!format!("{c:?}").contains(SECRET));
    for (status, expected) in [
        (401, Error::Provider(ProviderError::HttpStatus)),
        (429, Error::Provider(ProviderError::RateLimited)),
    ] {
        let f = Fixture::start(vec![Reply::status(status)]).await?;
        let error = standard(&f.endpoint)?
            .quote_exact_input(request()?)
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("private-provider-body"));
        assert!(!format!("{error:?} {error}").contains(SECRET));
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn total_deadline_covers_spender_plus_swap_body_instead_of_resetting() -> Result<(), TestError>
{
    let mut a = Reply::json(SPENDER_JSON);
    a.delay = Duration::from_secs(2);
    let mut b = Reply::json(SWAP);
    b.delay = Duration::from_secs(2);
    let f = Fixture::start(vec![a, b]).await?;
    let c = client(
        &f.endpoint,
        Limits::new(4096, 512)?,
        0,
        2 * 1024 * 1024,
        Duration::from_secs(3),
    )?;
    assert_eq!(c.prepare_swap(swap_request()?).await, Err(Error::Timeout));
    assert_eq!(f.requests()?.len(), 2);
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn caller_network_mismatch_fails_before_network_dispatch() -> Result<(), TestError> {
    let f = Fixture::start(vec![]).await?;
    let c = standard(&f.endpoint)?;
    let r = request()?;
    let wrong = QuoteRequest::new(
        NetworkId::new(ChainId::from(1_u64), "wrong-qualification")?,
        r.source(),
        r.destination(),
        r.amount(),
        r.settings().clone(),
    )?;
    assert_eq!(c.quote_exact_input(wrong).await, Err(Error::Configuration));
    assert!(f.requests()?.is_empty());
    Ok(())
}
#[test]
fn no_runtime_returns_fixed_configuration_and_capability_futures_are_send() -> Result<(), TestError>
{
    fn send<F: Send>(_: F) {}
    let c = standard("http://127.0.0.1:1/swap/")?;
    send(c.quote_exact_input(request()?));
    send(c.get_liquidity_sources());
    send(c.get_spender());
    send(c.prepare_swap(swap_request()?));
    let future = c.get_spender();
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    assert!(matches!(
        future.as_mut().poll(&mut cx),
        Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}
#[test]
fn credentials_bounds_and_existing_pure_vectors_are_explicit() -> Result<(), TestError> {
    let h = HttpConfig::new(
        RpcEndpoint::new("http://127.0.0.1:1/swap/")?,
        RpcLimits::new(
            Duration::from_secs(1),
            Duration::from_secs(3),
            2 * 1024 * 1024,
            0,
        )?,
        "fixture",
    )?;
    assert!(OneinchHttpConfig::new(h, network()?, Limits::new(1, 1)?).is_err());
    assert!(
        client(
            "http://127.0.0.1:1/swap/",
            Limits::new(1, 1)?,
            0,
            2 * 1024 * 1024 + 1,
            Duration::from_secs(3)
        )
        .is_err()
    );
    assert_eq!(graph()?.item_count(), 3);
    assert_eq!(prepared()?.transaction(), &transaction()?);
    assert_eq!(spender()?.address().to_string(), SPENDER);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn native_provider_sentinel_correlates_only_exact_native_call_value() -> Result<(), TestError>
{
    use regit_web3::domain::oneinch::{Asset, AssetKind, QuoteSettings};
    let native = Asset::new(ChainId::from(1_u64), AssetKind::Native)?;
    let request = QuoteRequest::new(
        network()?,
        native,
        asset(DST)?,
        100_u64.into(),
        QuoteSettings::provider_defaults(),
    )?;
    let quoted = QUOTE.replace(SRC, &native.provider_address().to_string());
    let swapped = SWAP
        .replace(SRC, &native.provider_address().to_string())
        .replace("\"value\":\"0\"", "\"value\":\"100\"");
    let fixture = Fixture::start(vec![
        Reply::json(&quoted),
        Reply::json(SPENDER_JSON),
        Reply::json(&swapped),
    ])
    .await?;
    let client = standard(&fixture.endpoint)?;
    let quote = client.quote_exact_input(request).await?;
    assert_eq!(quote.data().source_token.asset.kind(), AssetKind::Native);
    let mut settings = settings()?;
    settings.native_value = 100_u64.into();
    let preparation = client
        .prepare_swap(SwapRequest::new(quote, settings)?)
        .await?;
    assert_eq!(preparation.transaction().data().value, 100_u64.into());
    assert!(fixture.requests()?[0].contains("src=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn swap_read_retries_retain_original_actor_floor_and_routing_bytes() -> Result<(), TestError>
{
    let fixture = Fixture::start(vec![
        Reply::json(SPENDER_JSON),
        Reply::status(503),
        Reply::json(SWAP),
    ])
    .await?;
    let client = client(
        &fixture.endpoint,
        Limits::new(4096, 512)?,
        1,
        2 * 1024 * 1024,
        Duration::from_secs(5),
    )?;
    let request = swap_request()?;
    let preparation = client.prepare_swap(request.clone()).await?;
    assert_eq!(preparation.request(), &request);
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1], requests[2]);
    assert!(requests[1].contains("minReturn=900"));
    assert!(requests[1].starts_with("GET "));
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn document_and_global_graph_bounds_and_duplicates_are_terminal() -> Result<(), TestError> {
    let group = r#"{"token":"0x1111111111111111111111111111111111111111","hops":[]}"#;
    let empty = QUOTE
        .split_once("\"protocols\":")
        .ok_or("missing fixture graph")?
        .0;
    let oversize = format!("{empty}\"protocols\":[{}]}}", vec![group; 65].join(","));
    for body in [
        oversize,
        QUOTE.replace("\"dstToken\":", "\"dstToken\":null,\"dstToken\":"),
        QUOTE.replace("\"gas\":9007199254740993", "\"gas\":\"9007199254740993\""),
    ] {
        let fixture = Fixture::start(vec![Reply::json(&body)]).await?;
        let client = client(
            &fixture.endpoint,
            Limits::new(4096, 512)?,
            2,
            2 * 1024 * 1024,
            Duration::from_secs(5),
        )?;
        assert_eq!(client.quote_exact_input(request()?).await, Err(invalid()));
        assert_eq!(fixture.requests()?.len(), 1);
    }
    assert!(
        client(
            "http://127.0.0.1:1/swap/?permit=hidden",
            Limits::new(1, 1)?,
            0,
            1024,
            Duration::from_secs(3)
        )
        .is_err()
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn reported_quote_protocols_reject_observable_filter_violations_before_acceptance()
-> Result<(), TestError> {
    for (allowed, excluded) in [
        (vec![], vec!["UNISWAP_V3"]),
        (vec!["OTHER"], vec![]),
        (vec!["uniswap_v3"], vec![]),
    ] {
        let request = request()?;
        let mut routing = request.settings().clone();
        routing.protocols = allowed
            .iter()
            .map(|id| regit_web3::domain::oneinch::ProtocolId::new(*id))
            .collect::<Result<Vec<_>, Error>>()?;
        routing.excluded_protocols = excluded
            .iter()
            .map(|id| regit_web3::domain::oneinch::ProtocolId::new(*id))
            .collect::<Result<Vec<_>, Error>>()?;
        let request = QuoteRequest::new(
            network()?,
            request.source(),
            request.destination(),
            request.amount(),
            routing,
        )?;
        let fixture = Fixture::start(vec![Reply::json(QUOTE)]).await?;
        assert_eq!(
            standard(&fixture.endpoint)?
                .quote_exact_input(request)
                .await,
            Err(invalid())
        );
        assert_eq!(fixture.requests()?.len(), 1);
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn fresh_swap_protocols_must_still_obey_original_filters() -> Result<(), TestError> {
    for allowlist in [false, true] {
        let fixture = Fixture::start(vec![
            Reply::json(SPENDER_JSON),
            Reply::json(&SWAP.replace("UNISWAP_V3", "OTHER")),
        ])
        .await?;
        let old = quote(Method::Quote, 1000)?;
        let mut routing = old.request().settings().clone();
        if allowlist {
            routing.protocols = vec![regit_web3::domain::oneinch::ProtocolId::new("UNISWAP_V3")?];
        } else {
            routing.excluded_protocols =
                vec![regit_web3::domain::oneinch::ProtocolId::new("OTHER")?];
        }
        let request = QuoteRequest::new(
            network()?,
            old.request().source(),
            old.request().destination(),
            old.request().amount(),
            routing,
        )?;
        let selected = Quote::new(request, old.data().clone(), context(Method::Quote)?)?;
        assert_eq!(
            standard(&fixture.endpoint)?
                .prepare_swap(SwapRequest::new(selected, settings()?)?)
                .await,
            Err(invalid())
        );
        assert_eq!(fixture.requests()?.len(), 2);
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn permitted_quote_and_empty_graph_remain_honest_source_records() -> Result<(), TestError> {
    let request = request()?;
    let mut routing = request.settings().clone();
    routing.protocols = vec![regit_web3::domain::oneinch::ProtocolId::new("UNISWAP_V3")?];
    routing.excluded_protocols = vec![regit_web3::domain::oneinch::ProtocolId::new("OTHER")?];
    let request = QuoteRequest::new(
        network()?,
        request.source(),
        request.destination(),
        request.amount(),
        routing,
    )?;
    let empty = QUOTE
        .split_once("\"protocols\":")
        .ok_or("missing fixture graph")?
        .0;
    let empty = format!("{empty}\"protocols\":[]}}");
    let fixture = Fixture::start(vec![Reply::json(QUOTE), Reply::json(&empty)]).await?;
    let client = standard(&fixture.endpoint)?;
    assert_eq!(
        client
            .quote_exact_input(request.clone())
            .await?
            .data()
            .routes
            .item_count(),
        3
    );
    let quote = client.quote_exact_input(request).await?;
    assert!(quote.data().routes.groups().is_empty());
    assert!(quote.data().estimated_gas.is_none());
    assert_eq!(quote.data().destination_amount.value().to_string(), "1000");
    assert_eq!(
        serde_json::from_str::<Quote>(&serde_json::to_string(&quote)?)?,
        quote
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn source_transaction_unknown_envelope_facts_are_not_silently_discarded()
-> Result<(), TestError> {
    for addition in [
        "\"chainId\":2",
        "\"type\":4",
        "\"authorizationList\":[]",
        "\"blobVersionedHashes\":[]",
        "\"maxFeePerBlobGas\":\"1\"",
        "\"r\":\"0x1\"",
        "\"s\":\"0x2\"",
        "\"v\":27",
        "\"signature\":\"private-source-signature\"",
        "\"nonce\":0",
        "\"unknownEnvelopeProperty\":true",
    ] {
        let body = SWAP.replace("\"gas\":210000", &format!("\"gas\":210000,{addition}"));
        let fixture = Fixture::start(vec![Reply::json(SPENDER_JSON), Reply::json(&body)]).await?;
        let client = client(
            &fixture.endpoint,
            Limits::new(4096, 512)?,
            2,
            2 * 1024 * 1024,
            Duration::from_secs(5),
        )?;
        let error = client.prepare_swap(swap_request()?).await.unwrap_err();
        assert_eq!(error, invalid());
        assert_eq!(fixture.requests()?.len(), 2);
        assert!(!format!("{error:?} {error}").contains("private-source-signature"));
    }
    let fixture = Fixture::start(vec![Reply::json(SPENDER_JSON), Reply::json(SWAP)]).await?;
    let prepared = standard(&fixture.endpoint)?
        .prepare_swap(swap_request()?)
        .await?;
    assert_eq!(prepared.transaction().data().gas_used, None);
    // Separate supported optional-metadata vector, not an ordinary live capture
    // or proof that the backend ran an access-list simulation.
    let metadata = SWAP.replace(
        "\"gas\":210000",
        "\"gas\":210000,\"gasUsed\":210000,\"accessList\":[]",
    );
    let fixture = Fixture::start(vec![Reply::json(SPENDER_JSON), Reply::json(&metadata)]).await?;
    let prepared = standard(&fixture.endpoint)?
        .prepare_swap(swap_request()?)
        .await?;
    assert_eq!(
        prepared.transaction().data().gas_used,
        Some(210_000_u64.into())
    );
    Ok(())
}
