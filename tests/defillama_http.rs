// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Public typed analytics operations over independent real loopback HTTP sources.
#![cfg(feature = "defillama-http")]
#[path = "support/market_server.rs"]
mod market_server;
use market_server::{Fixture, Reply};
use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        defillama::{
            AnalyticsMetric, AnalyticsRequest, ProviderId, StablecoinId, StablecoinScope, TvlScope,
        },
        market::{ItemLimit, Label},
    },
    error::{Error, ProviderError},
    providers::defillama::{DefiLlamaClient, DefiLlamaHttpConfig},
};
use std::{
    future::Future,
    task::{Context, Poll, Waker},
    time::Duration,
};
const KEY: &str = "explicit-local-secret";
fn config(
    url: &str,
    label: &str,
    bytes: usize,
    retries: u8,
    timeout: Duration,
) -> Result<HttpConfig, Error> {
    HttpConfig::new(
        RpcEndpoint::new(url)?.with_header("x-fixture-key", KEY)?,
        RpcLimits::new(timeout, timeout, bytes, retries)?,
        label,
    )
}
fn client(
    tvl: &str,
    yields: &str,
    stablecoins: &str,
    items: u32,
) -> Result<DefiLlamaClient, Error> {
    DefiLlamaClient::new(DefiLlamaHttpConfig::new(
        config(tvl, "tvl-source", 1024 * 1024, 0, Duration::from_secs(3))?,
        config(
            yields,
            "yield-source",
            1024 * 1024,
            0,
            Duration::from_secs(3),
        )?,
        config(
            stablecoins,
            "stable-source",
            1024 * 1024,
            0,
            Duration::from_secs(3),
        )?,
        ItemLimit::new(items)?,
    ))
}
fn one(url: &str, items: u32) -> Result<DefiLlamaClient, Error> {
    client(url, url, url, items)
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn pool() -> &'static str {
    r#"{"pool":"747c1d2a-c668-4682-b9f9-296708a3dd90","chain":"Ethereum","project":"lido","symbol":"STETH","tvlUsd":9007199254740993.000000000000000001,"apy":-0.000000000000000001,"apyBase":null,"apyReward":1.25e1,"ignored_metadata":true}"#
}
fn stable() -> &'static str {
    r#"{"id":"1","name":"Coin","symbol":"COIN","pegType":"peggedEUR","circulating":{"peggedEUR":9007199254740993.125},"price":null,"chains":["Ethereum"]}"#
}
fn analytics() -> &'static str {
    r#"{"slug":"aave","name":"Aave","total24h":9007199254740993.000000000000000001,"total7d":null,"totalAllTime":1.25e3,"totalDataChart":[[1000,1.000000000000000001],[2000,2]]}"#
}
fn id() -> Result<ProviderId, Error> {
    ProviderId::parse("aave")
}
#[tokio::test(flavor = "current_thread")]
async fn every_operation_preserves_exact_units_times_identities_and_independent_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let tvl=Fixture::start(vec![Reply::json("9007199254740993.125"),Reply::json(r#"{"id":"parent#aave","name":"Aave","tvl":[{"date":1,"totalLiquidityUSD":1.25e3}],"currentChainTvls":{"Ethereum":1000,"Ethereum-borrowed":100},"tokens":{"discarded_provider_metadata":true}}"#),Reply::json(r#"[{"date":1,"tvl":0},{"date":2,"tvl":9007199254740993.125}]"#)]).await?;
    let yields=Fixture::start(vec![Reply::json(&format!("{{\"status\":\"success\",\"data\":[{}]}}",pool())),Reply::json(r#"{"status":"success","data":[{"timestamp":"2026-10-07T00:00:00.000Z","tvlUsd":1.25e3,"apy":-1.25,"apyBase":null,"apyReward":null}]}"#)]).await?;
    let stables=Fixture::start(vec![Reply::json(&format!("{{\"peggedAssets\":[{}]}}",stable())),Reply::json(r#"[{"date":"1000","totalCirculating":{"peggedEUR":2},"totalCirculatingUSD":{"peggedEUR":2.25}}]"#)]).await?;
    let c = client(&tvl.endpoint, &yields.endpoint, &stables.endpoint, 100)?;
    let v = c.protocol_tvl(id()?).await?;
    assert_eq!(v.value().usd().value().canonical(), "9007199254740993.125");
    assert_eq!(v.source().provider_id(), "tvl-source");
    let v = c.protocol_history(id()?).await?;
    assert_eq!(v.value().provider_id().as_str(), "parent#aave");
    assert_eq!(v.value().points()[0].date().unix_seconds(), 1);
    assert_eq!(v.value().current_breakdowns().len(), 2);
    let scope = TvlScope::Chain(Label::new("OP Mainnet")?);
    let v = c.tvl_history(scope.clone()).await?;
    assert_eq!(v.value().scope(), &scope);
    assert_eq!(
        v.value().points()[1].usd().value().canonical(),
        "9007199254740993.125"
    );
    let v = c.yield_pools().await?;
    assert_eq!(v.source().provider_id(), "yield-source");
    assert_eq!(
        v.value().pools()[0]
            .rates()
            .apy()
            .as_ref()
            .unwrap()
            .canonical(),
        "-0.000000000000000001"
    );
    assert!(v.value().pools()[0].rates().base().is_none());
    assert_eq!(
        v.value().pools()[0]
            .rates()
            .reward()
            .as_ref()
            .unwrap()
            .canonical(),
        "12.5"
    );
    let pool_id = ProviderId::parse("747c1d2a-c668-4682-b9f9-296708a3dd90")?;
    let v = c.yield_history(pool_id.clone()).await?;
    assert_eq!(v.value().pool(), &pool_id);
    assert_eq!(
        v.value().points()[0].date().as_str(),
        "2026-10-07T00:00:00.000Z"
    );
    let v = c.stablecoins().await?;
    assert_eq!(v.source().provider_id(), "stable-source");
    assert_eq!(v.value().assets()[0].id().get(), 1);
    assert_eq!(
        v.value().assets()[0].circulating()[0].label().as_str(),
        "peggedEUR"
    );
    assert!(v.value().assets()[0].price_usd().is_none());
    let scope = StablecoinScope::new(Some(Label::new("Ethereum")?), Some(StablecoinId::new(1)?));
    let v = c.stablecoin_history(scope.clone()).await?;
    assert_eq!(v.value().scope(), &scope);
    assert_eq!(v.value().points()[0].date().unix_seconds(), 1000);
    assert_eq!(
        v.value().points()[0].circulating()[0]
            .value()
            .value()
            .canonical(),
        "2"
    );
    assert_eq!(
        v.value().points()[0].circulating_usd()[0]
            .value()
            .value()
            .canonical(),
        "2.25"
    );
    let r = tvl.requests()?;
    assert_eq!(r.len(), 3);
    assert!(r[0].contains("/tvl/aave "));
    assert!(r[1].contains("/protocol/aave "));
    assert!(r[2].contains("/v2/historicalChainTvl/OP%20Mainnet "));
    let r = yields.requests()?;
    assert_eq!(r.len(), 2);
    assert!(r[0].contains("/pools "));
    assert!(r[1].contains("/chart/747c1d2a-c668-4682-b9f9-296708a3dd90 "));
    let r = stables.requests()?;
    assert_eq!(r.len(), 2);
    assert!(r[0].contains("/stablecoins?includePrices=true "));
    assert!(r[1].contains("/stablecoincharts/Ethereum?stablecoin=1 "));
    assert!(!format!("{c:?}").contains(KEY));
    assert!(!format!("{c:?}").contains(&tvl.endpoint));
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn exact_scalar_alias_duplicates_and_signed_tvl_reject_malformed_responses()
-> Result<(), Box<dyn std::error::Error>> {
    for raw in [
        "null",
        "\"1\"",
        "-1",
        r#"{"$serde_json::private::Number":"1"}"#,
    ] {
        let f = Fixture::start(vec![Reply::json(raw)]).await?;
        assert_eq!(
            one(&f.endpoint, 100)?
                .protocol_tvl(id()?)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    for raw in [
        r#"[{"date":1,"tvl":1,"totalLiquidityUSD":2}]"#,
        r#"[{"date":1,"tvl":1},{"date":1,"tvl":2}]"#,
        r#"[{"date":1.5,"tvl":1}]"#,
        r#"[{"date":1,"tvl":-1}]"#,
    ] {
        let f = Fixture::start(vec![Reply::json(raw)]).await?;
        assert_eq!(
            one(&f.endpoint, 100)?
                .tvl_history(TvlScope::All)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    for raw in [
        r#"{"id":"aave","name":"Aave","tvl":[],"currentChainTvls":{"Ethereum":1,"Ethereum":2}}"#,
        r#"{"id":"aave","name":"Aave","tvl":[],"tvl":[],"currentChainTvls":{}}"#,
    ] {
        let f = Fixture::start(vec![Reply::json(raw)]).await?;
        assert_eq!(
            one(&f.endpoint, 100)?
                .protocol_history(id()?)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn pool_envelope_duplicate_fields_ids_and_item_caps_fail_without_truncation()
-> Result<(), Box<dyn std::error::Error>> {
    let p = pool();
    for (raw, cap) in [
        (format!("{{\"status\":\"failure\",\"data\":[{p}]}}"), 100),
        (
            format!("{{\"status\":\"success\",\"data\":[{p},{p}]}}"),
            100,
        ),
        (
            format!(
                "{{\"status\":\"success\",\"data\":[{p},{}]}}",
                p.replace("747c1d2a", "747c1d2b")
            ),
            1,
        ),
        (
            format!(
                "{{\"status\":\"success\",\"data\":[{}]}}",
                p.replace("\"tvlUsd\":", "\"tvlUsd\":1,\"tvlUsd\":")
            ),
            100,
        ),
        (
            format!(
                "{{\"status\":\"success\",\"data\":[{}]}}",
                p.replace("\"apyBase\":null,", "")
            ),
            100,
        ),
    ] {
        let f = Fixture::start(vec![Reply::json(&raw)]).await?;
        assert_eq!(
            one(&f.endpoint, cap)?.yield_pools().await.unwrap_err(),
            invalid()
        );
        assert_eq!(f.requests()?.len(), 1);
    }
    let f = Fixture::start(vec![Reply::json(r#"{"status":"success","data":[]}"#)]).await?;
    assert!(
        one(&f.endpoint, 100)?
            .yield_pools()
            .await?
            .value()
            .pools()
            .is_empty()
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn stablecoin_peg_maps_canonical_ids_dates_and_nullable_prices_are_checked()
-> Result<(), Box<dyn std::error::Error>> {
    let s = stable();
    for raw in [
        format!("{{\"peggedAssets\":[{s},{s}]}}"),
        format!(
            "{{\"peggedAssets\":[{}]}}",
            s.replace("\"id\":\"1\"", "\"id\":\"01\"")
        ),
        format!(
            "{{\"peggedAssets\":[{}]}}",
            s.replace(
                "\"circulating\":{\"peggedEUR\":",
                "\"circulating\":{\"peggedEUR\":1,\"peggedEUR\":"
            )
        ),
        format!(
            "{{\"peggedAssets\":[{}]}}",
            s.replace("\"price\":null,", "")
        ),
        format!(
            "{{\"peggedAssets\":[{}]}}",
            s.replace("\"price\":null", "\"price\":-1")
        ),
    ] {
        let f = Fixture::start(vec![Reply::json(&raw)]).await?;
        assert_eq!(
            one(&f.endpoint, 100)?.stablecoins().await.unwrap_err(),
            invalid()
        );
    }
    for raw in [
        r#"[{"date":"01","totalCirculating":{},"totalCirculatingUSD":{}}]"#,
        r#"[{"date":1,"totalCirculating":{},"totalCirculatingUSD":{}}]"#,
        r#"[{"date":"1","totalCirculating":{},"totalCirculatingUSD":{"peggedEUR":{"$serde_json::private::Number":"1"}}}]"#,
        r#"[{"date":"1","totalCirculating":{}}]"#,
    ] {
        let f = Fixture::start(vec![Reply::json(raw)]).await?;
        assert_eq!(
            one(&f.endpoint, 100)?
                .stablecoin_history(StablecoinScope::new(None, None))
                .await
                .unwrap_err(),
            invalid()
        );
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn yield_and_analytics_time_identity_and_component_fields_are_strict()
-> Result<(), Box<dyn std::error::Error>> {
    for raw in [
        r#"{"status":"success","data":[{"timestamp":"2026-02-30T00:00:00Z","tvlUsd":1,"apy":1,"apyBase":null,"apyReward":null}]}"#,
        r#"{"status":"success","data":[{"timestamp":"2026-10-07T00:00:00Z","tvlUsd":1,"apy":{"$serde_json::private::Number":"1"},"apyBase":null,"apyReward":null}]}"#,
    ] {
        let f = Fixture::start(vec![Reply::json(raw)]).await?;
        assert_eq!(
            one(&f.endpoint, 100)?
                .yield_history(ProviderId::parse("pool")?)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    for raw in [
        analytics().replace("\"slug\":\"aave\"", "\"slug\":\"uniswap\""),
        analytics().replace("[2000,2]", "[1000,2]"),
        analytics().replace("\"total7d\":null,", ""),
    ] {
        let f = Fixture::start(vec![Reply::json(&raw)]).await?;
        assert_eq!(
            one(&f.endpoint, 100)?
                .analytics(AnalyticsRequest::new(id()?, AnalyticsMetric::Fees))
                .await
                .unwrap_err(),
            invalid()
        );
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn selected_host_retry_budget_and_http_status_policy_do_not_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::start(vec![
        Reply::status(503),
        Reply::status(429),
        Reply::json("1"),
    ])
    .await?;
    let http = config(&f.endpoint, "fixed", 1024, 2, Duration::from_secs(3))?;
    let c = DefiLlamaClient::new(DefiLlamaHttpConfig::new(
        http.clone(),
        http.clone(),
        http,
        ItemLimit::new(100)?,
    ))?;
    c.protocol_tvl(id()?).await?;
    let r = f.requests()?;
    assert_eq!(r.len(), 3);
    assert_eq!(r[0], r[1]);
    assert_eq!(r[1], r[2]);
    let f = Fixture::start(vec![Reply::json("9007199254740993")]).await?;
    let http = config(&f.endpoint, "fixed", 4, 0, Duration::from_secs(3))?;
    let c = DefiLlamaClient::new(DefiLlamaHttpConfig::new(
        http.clone(),
        http.clone(),
        http,
        ItemLimit::new(1)?,
    ))?;
    assert_eq!(
        c.protocol_tvl(id()?).await.unwrap_err(),
        Error::Provider(ProviderError::ResponseTooLarge)
    );
    let mut reply = Reply::json("1");
    reply.delay = Duration::from_secs(3);
    let f = Fixture::start(vec![reply]).await?;
    let http = config(&f.endpoint, "fixed", 1024, 0, Duration::from_secs(2))?;
    let c = DefiLlamaClient::new(DefiLlamaHttpConfig::new(
        http.clone(),
        http.clone(),
        http,
        ItemLimit::new(1)?,
    ))?;
    assert_eq!(c.protocol_tvl(id()?).await.unwrap_err(), Error::Timeout);
    let requests = f.requests()?;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /api/v3/tvl/aave "));
    for (status, expected) in [
        (404, Error::UnavailableData),
        (401, Error::Provider(ProviderError::HttpStatus)),
        (429, Error::Provider(ProviderError::RateLimited)),
    ] {
        let f = Fixture::start(vec![Reply::status(status)]).await?;
        assert_eq!(
            one(&f.endpoint, 100)?
                .protocol_tvl(id()?)
                .await
                .unwrap_err(),
            expected
        );
    }
    let f = Fixture::start(vec![Reply::status(404)]).await?;
    assert_eq!(
        one(&f.endpoint, 100)?.yield_pools().await.unwrap_err(),
        Error::Provider(ProviderError::HttpStatus)
    );
    Ok(())
}
#[test]
fn runtime_selection_is_explicit_without_implicit_runtime_creation()
-> Result<(), Box<dyn std::error::Error>> {
    let c = one("http://example.invalid", 100)?;
    let future = c.protocol_tvl(id()?);
    let mut f = std::pin::pin!(future);
    assert!(matches!(
        f.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn analytics_metric_variants_keep_exact_values_and_documented_query_types()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::start(vec![
        Reply::json(analytics()),
        Reply::json(analytics()),
        Reply::json(analytics()),
        Reply::json(analytics()),
    ])
    .await?;
    let c = one(&f.endpoint, 100)?;
    for metric in [
        AnalyticsMetric::DexVolume,
        AnalyticsMetric::Fees,
        AnalyticsMetric::Revenue,
        AnalyticsMetric::HoldersRevenue,
    ] {
        let request = AnalyticsRequest::new(id()?, metric);
        let v = c.analytics(request.clone()).await?;
        assert_eq!(v.value().request(), &request);
        assert_eq!(
            v.value()
                .totals()
                .day()
                .as_ref()
                .unwrap()
                .value()
                .canonical(),
            "9007199254740993.000000000000000001"
        );
        assert!(v.value().totals().week().is_none());
        assert_eq!(
            v.value().daily()[0].usd().value().canonical(),
            "1.000000000000000001"
        );
    }
    let r = f.requests()?;
    for (i, (module, kind)) in [
        ("dexs", "dailyVolume"),
        ("fees", "dailyFees"),
        ("fees", "dailyRevenue"),
        ("fees", "dailyHoldersRevenue"),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(r[i].contains(&format!("/summary/{module}/aave?excludeTotalDataChart=false&excludeTotalDataChartBreakdown=true&dataType={kind}")));
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn raw_yield_symbol_metadata_preserves_provider_controls_without_changing_identity_policy()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = pool().replace("\"symbol\":\"STETH\"", r#""symbol":"\b8-ETH""#);
    let raw = format!("{{\"status\":\"success\",\"data\":[{raw}]}}");
    let f = Fixture::start(vec![Reply::json(&raw)]).await?;
    let v = one(&f.endpoint, 100)?.yield_pools().await?;
    let p = &v.value().pools()[0];
    assert_eq!(p.symbol().as_str(), "\u{0008}8-ETH");
    let encoded = serde_json::to_string(p)?;
    assert!(encoded.contains(r"\b8-ETH"));
    assert_eq!(
        serde_json::from_str::<regit_web3::domain::defillama::YieldPool>(&encoded)?,
        *p
    );
    assert!(!format!("{p:?}").contains('\u{0008}'));
    assert!(ProviderId::parse("invalid\u{0008}id").is_err());
    assert!(Label::new("invalid\u{0008}path").is_err());
    Ok(())
}
