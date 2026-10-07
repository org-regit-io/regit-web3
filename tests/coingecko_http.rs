// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Public `CoinGecko` HTTP behavior using real ephemeral loopback exchanges.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "coingecko-http")]
#[path = "support/market_server.rs"]
mod market_server;
use market_server::{Fixture, Reply};
use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        coingecko::{
            CoinId, Currency, HistoryRequest, MarketsRequest, PageStatus, PriceAvailability,
            PricesRequest, SearchQuery,
        },
        market::ItemLimit,
    },
    error::{Error, ProviderError},
    providers::coingecko::{ApiTier, CoinGeckoClient, CoinGeckoHttpConfig},
};
use std::{
    future::Future,
    task::{Context, Poll, Waker},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const KEY: &str = "explicit-secret-fixture-key";
fn client(
    endpoint: &str,
    items: u32,
    retries: u8,
    bytes: usize,
    timeout: Duration,
    tier: ApiTier,
) -> Result<CoinGeckoClient, Error> {
    CoinGeckoClient::new(CoinGeckoHttpConfig::new(
        &HttpConfig::new(
            RpcEndpoint::new(endpoint)?,
            RpcLimits::new(timeout, timeout, bytes, retries)?,
            "fixture",
        )?,
        tier,
        KEY,
        ItemLimit::new(items)?,
    )?)
}
fn standard(endpoint: &str) -> Result<CoinGeckoClient, Error> {
    client(
        endpoint,
        100,
        0,
        1024 * 1024,
        Duration::from_secs(3),
        ApiTier::Demo,
    )
}
fn price_request() -> Result<PricesRequest, Error> {
    PricesRequest::new(
        vec![CoinId::parse("bitcoin")?],
        vec![Currency::parse("usd")?],
    )
}
fn history_request() -> Result<HistoryRequest, Error> {
    HistoryRequest::new(CoinId::parse("bitcoin")?, Currency::parse("usd")?, 1, 3)
}
fn market() -> &'static str {
    r#"[{"id":"bitcoin","name":"Bitcoin","symbol":"btc","current_price":9007199254740993.000000000000000001,"market_cap":null,"total_volume":1.25e3,"market_cap_rank":1,"last_updated":"2026-10-07T01:02:03.123Z","ignored_metadata":true}]"#
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
#[tokio::test(flavor = "current_thread")]
async fn all_required_operations_preserve_identity_exact_values_units_times_and_queries()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture=Fixture::start(vec![Reply::json(r#"{"coins":[{"id":"bitcoin","name":"Bitcoin","symbol":"BTC","market_cap_rank":1}],"exchanges":[]}"#),Reply::json(r#"{"bitcoin":{"usd":9007199254740993.000000000000000001,"last_updated_at":1700000000}}"#),Reply::json(market()),Reply::json(r#"{"prices":[[1000,1.000000000000000001],[3000,2]],"market_caps":[[2000,9007199254740993]],"total_volumes":[]}"#)]).await?;
    let c = standard(&fixture.endpoint)?;
    let before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let search = c.search(SearchQuery::new("BTC coin & token")?).await?;
    assert_eq!(search.value().coins()[0].id().as_str(), "bitcoin");
    let prices = c.prices(price_request()?).await?;
    let PriceAvailability::Present(price) = prices.value().quotes()[0].availability() else {
        return Err("missing exact price".into());
    };
    assert_eq!(
        price.value().canonical(),
        "9007199254740993.000000000000000001"
    );
    assert_eq!(
        prices.value().quotes()[0]
            .last_updated_at()
            .map(regit_web3::domain::Timestamp::unix_seconds),
        Some(1_700_000_000)
    );
    assert!(prices.retrieved_at().unix_seconds() >= before);
    assert_eq!(prices.source().provider_id(), "fixture");
    assert_eq!(prices.source().method(), "simple-price");
    let markets = c
        .markets(MarketsRequest::new(
            Currency::parse("usd")?,
            2,
            1,
            Some(vec![CoinId::parse("bitcoin")?]),
        )?)
        .await?;
    assert_eq!(markets.value().status(), PageStatus::MayHaveMore);
    assert_eq!(
        markets.value().coins()[0]
            .total_volume()
            .as_ref()
            .unwrap()
            .value()
            .canonical(),
        "1250"
    );
    assert!(markets.value().coins()[0].market_cap().is_none());
    let history = c.history(history_request()?).await?;
    assert_eq!(history.value().prices()[0].timestamp().get(), 1000);
    assert_eq!(
        history.value().prices()[0].value().value().canonical(),
        "1.000000000000000001"
    );
    assert_eq!(history.value().market_caps().len(), 1);
    assert!(history.value().total_volumes().is_empty());
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert!(requests[0].starts_with("GET /api/v3/search?query=BTC+coin+%26+token HTTP/1.1"));
    assert!(
        requests[1]
            .contains("ids=bitcoin&vs_currencies=usd&include_last_updated_at=true&precision=full")
    );
    assert!(requests[2].contains(
        "page=2&per_page=1&order=market_cap_desc&sparkline=false&precision=full&ids=bitcoin"
    ));
    assert!(
        requests[3].contains(
            "/coins/bitcoin/market_chart/range?vs_currency=usd&from=1&to=3&precision=full"
        )
    );
    for r in requests {
        assert!(r.contains(&format!("x-cg-demo-api-key: {KEY}")));
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn null_missing_currency_and_missing_asset_remain_distinct()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture=Fixture::start(vec![Reply::json(r#"{"bitcoin":{"usd":null,"last_updated_at":null},"ethereum":{"usd":0,"last_updated_at":0}}"#)]).await?;
    let c = standard(&fixture.endpoint)?;
    let result = c
        .prices(PricesRequest::new(
            vec![
                CoinId::parse("bitcoin")?,
                CoinId::parse("ethereum")?,
                CoinId::parse("solana")?,
            ],
            vec![Currency::parse("usd")?, Currency::parse("eur")?],
        )?)
        .await?;
    let q = result.value().quotes();
    assert_eq!(q[0].availability(), &PriceAvailability::NotReported);
    assert_eq!(q[1].availability(), &PriceAvailability::CurrencyUnavailable);
    assert!(matches!(q[2].availability(),PriceAvailability::Present(v) if v.value().is_zero()));
    assert_eq!(q[4].availability(), &PriceAvailability::AssetUnavailable);
    assert_eq!(q[0].last_updated_at(), &None);
    assert_eq!(
        q[2].last_updated_at()
            .map(regit_web3::domain::Timestamp::unix_seconds),
        Some(0)
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn lexical_numbers_duplicate_maps_and_missing_fields_are_terminal()
-> Result<(), Box<dyn std::error::Error>> {
    for raw in [
        r#"{"bitcoin":{"usd":1,"usd":2,"last_updated_at":1}}"#,
        r#"{"bitcoin":{"usd":1,"last_updated_at":1},"bitcoin":{"usd":2,"last_updated_at":1}}"#,
        r#"{"bitcoin":{"usd":{"$serde_json::private::Number":"1"},"last_updated_at":1}}"#,
        r#"{"bitcoin":{"usd":"1","last_updated_at":1}}"#,
        r#"{"bitcoin":{"usd":-1,"last_updated_at":1}}"#,
        r#"{"bitcoin":{"usd":1}}"#,
        r#"{"bitcoin":{"usd":1,"last_updated_at":1.5}}"#,
        r#"{"ethereum":{"usd":1,"last_updated_at":1}}"#,
        "null",
    ] {
        let fixture = Fixture::start(vec![Reply::json(raw)]).await?;
        let c = client(
            &fixture.endpoint,
            100,
            2,
            1024 * 1024,
            Duration::from_secs(3),
            ApiTier::Pro,
        )?;
        assert_eq!(
            c.prices(price_request()?).await.unwrap_err(),
            invalid(),
            "accepted {raw}"
        );
        assert_eq!(fixture.requests()?.len(), 1);
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn page_identity_and_history_time_bounds_reject_inconsistent_provider_data()
-> Result<(), Box<dyn std::error::Error>> {
    for raw in [
        market().replace("bitcoin", "ethereum"),
        market().replace(
            "\"current_price\":",
            "\"current_price\":0,\"current_price\":",
        ),
        market().replace("2026-10-07T01:02:03.123Z", "2026-02-31T01:02:03Z"),
    ] {
        let fixture = Fixture::start(vec![Reply::json(&raw)]).await?;
        let c = standard(&fixture.endpoint)?;
        assert_eq!(
            c.markets(MarketsRequest::new(
                Currency::parse("usd")?,
                1,
                1,
                Some(vec![CoinId::parse("bitcoin")?])
            )?)
            .await
            .unwrap_err(),
            invalid()
        );
    }
    for raw in [
        r#"{"prices":[[1000,1],[1000,2]],"market_caps":[],"total_volumes":[]}"#,
        r#"{"prices":[[999,1]],"market_caps":[],"total_volumes":[]}"#,
        r#"{"prices":[[3001,1]],"market_caps":[],"total_volumes":[]}"#,
        r#"{"prices":[[1000,{"$serde_json::private::Number":"1"}]],"market_caps":[],"total_volumes":[]}"#,
        r#"{"prices":[],"market_caps":[]}"#,
    ] {
        let fixture = Fixture::start(vec![Reply::json(raw)]).await?;
        assert_eq!(
            standard(&fixture.endpoint)?
                .history(history_request()?)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn duplicate_search_ids_and_explicit_item_caps_fail_without_truncation()
-> Result<(), Box<dyn std::error::Error>> {
    let coin = r#"{"id":"bitcoin","name":"Bitcoin","symbol":"BTC","market_cap_rank":null}"#;
    for (raw, cap) in [
        (format!("{{\"coins\":[{coin},{coin}]}}"), 100),
        (
            format!(
                "{{\"coins\":[{coin},{}]}}",
                coin.replace("bitcoin", "ethereum")
            ),
            1,
        ),
        (
            format!(
                "{{\"coins\":[{}]}}",
                coin.replace(
                    "\"id\":\"bitcoin\"",
                    "\"id\":\"bitcoin\",\"id\":\"ethereum\""
                )
            ),
            100,
        ),
    ] {
        let fixture = Fixture::start(vec![Reply::json(&raw)]).await?;
        let c = client(
            &fixture.endpoint,
            cap,
            0,
            1024 * 1024,
            Duration::from_secs(3),
            ApiTier::Demo,
        )?;
        assert_eq!(
            c.search(SearchQuery::new("btc")?).await.unwrap_err(),
            invalid()
        );
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn frozen_read_retries_keep_auth_and_limits_and_fixed_diagnostics()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        Reply::status(503),
        Reply::status(429),
        Reply::json(r#"{"bitcoin":{"usd":1,"last_updated_at":1}}"#),
    ])
    .await?;
    let c = client(
        &fixture.endpoint,
        100,
        2,
        1024 * 1024,
        Duration::from_secs(3),
        ApiTier::Pro,
    )?;
    c.prices(price_request()?).await?;
    let r = fixture.requests()?;
    assert_eq!(r.len(), 3);
    assert_eq!(r[0], r[1]);
    assert_eq!(r[1], r[2]);
    assert!(r[0].contains(&format!("x-cg-pro-api-key: {KEY}")));
    assert!(!format!("{c:?}").contains(KEY));
    assert!(!format!("{c:?}").contains(&fixture.endpoint));
    let fixture = Fixture::start(vec![Reply::json(market())]).await?;
    assert_eq!(
        client(
            &fixture.endpoint,
            100,
            0,
            16,
            Duration::from_secs(3),
            ApiTier::Demo
        )?
        .markets(MarketsRequest::new(Currency::parse("usd")?, 1, 1, None)?)
        .await
        .unwrap_err(),
        Error::Provider(ProviderError::ResponseTooLarge)
    );
    let mut delayed = Reply::json(r#"{"coins":[]}"#);
    delayed.delay = Duration::from_secs(3);
    let fixture = Fixture::start(vec![delayed]).await?;
    assert_eq!(
        client(
            &fixture.endpoint,
            100,
            0,
            1024,
            Duration::from_secs(2),
            ApiTier::Demo
        )?
        .search(SearchQuery::new("btc")?)
        .await
        .unwrap_err(),
        Error::Timeout
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /api/v3/search?query=btc "));
    for (status, error) in [
        (404, Error::UnavailableData),
        (401, Error::Provider(ProviderError::HttpStatus)),
        (429, Error::Provider(ProviderError::RateLimited)),
    ] {
        let fixture = Fixture::start(vec![Reply::status(status)]).await?;
        let result = standard(&fixture.endpoint)?
            .history(history_request()?)
            .await
            .unwrap_err();
        assert_eq!(result, error);
        assert!(!result.to_string().contains("private-provider-body"));
    }
    Ok(())
}
#[test]
fn no_runtime_and_invalid_credential_fail_explicitly() -> Result<(), Box<dyn std::error::Error>> {
    let c = standard("http://example.invalid/api/v3")?;
    let future = c.prices(price_request()?);
    let mut f = std::pin::pin!(future);
    assert!(matches!(
        f.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(Error::Configuration))
    ));
    let http = HttpConfig::new(
        RpcEndpoint::new("https://example.invalid/api/v3")?,
        RpcLimits::new(Duration::from_secs(1), Duration::from_secs(1), 1024, 0)?,
        "fixture",
    )?;
    assert!(CoinGeckoHttpConfig::new(&http, ApiTier::Demo, "", ItemLimit::new(1)?).is_err());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_anonymous_access_does_not_inject_or_accept_provider_keys()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![Reply::json(r#"{"coins":[]}"#)]).await?;
    let make = |endpoint: RpcEndpoint| {
        HttpConfig::new(
            endpoint,
            RpcLimits::new(Duration::from_secs(1), Duration::from_secs(2), 1024, 0)?,
            "fixture",
        )
    };
    let config = CoinGeckoHttpConfig::anonymous(
        make(RpcEndpoint::new(&fixture.endpoint)?)?,
        ItemLimit::new(10)?,
    )?;
    assert_eq!(config.tier(), None);
    CoinGeckoClient::new(config)?
        .search(SearchQuery::new("btc")?)
        .await?;
    let requests = fixture.requests()?;
    assert!(!requests[0].contains("x-cg-demo-api-key"));
    assert!(!requests[0].contains("x-cg-pro-api-key"));
    for endpoint in [
        RpcEndpoint::new(&fixture.endpoint)?.with_header("x-cg-demo-api-key", KEY)?,
        RpcEndpoint::new(&format!("{}?x_cg_pro_api_key=secret", fixture.endpoint))?,
    ] {
        assert!(
            CoinGeckoHttpConfig::anonymous(make(endpoint.clone())?, ItemLimit::new(10)?).is_err()
        );
        assert!(
            CoinGeckoHttpConfig::new(&make(endpoint)?, ApiTier::Pro, KEY, ItemLimit::new(10)?)
                .is_err()
        );
    }
    Ok(())
}
