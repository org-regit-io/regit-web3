// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in public `CoinGecko` qualification through the typed Rust API.
//! All configuration and range inputs belong to this test; the library does not
//! read environment variables. Each result is a separate provider observation.
#![cfg(feature = "coingecko-http")]
#[path = "support/live.rs"]
mod live;
use regit_web3::{
    domain::{
        coingecko::{
            CoinId, Currency, HistoryRequest, MarketsRequest, PriceAvailability, PricesRequest,
            SearchQuery,
        },
        market::{ItemLimit, Observation},
    },
    error::Error,
    providers::coingecko::{CoinGeckoClient, CoinGeckoHttpConfig},
};
fn number(name: &str) -> Result<u64, Error> {
    live::required(name)?
        .parse()
        .map_err(|_| Error::Configuration)
}
fn check<T>(result: &Observation<T>, method: &str, before: u64) -> Result<(), Error> {
    assert_eq!(
        result.source().provider_id(),
        live::required("REGIT_WEB3_COINGECKO_PROVIDER_ID")?
    );
    assert_eq!(result.source().method(), method);
    assert_eq!(
        result.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=live::unix_seconds()?).contains(&result.retrieved_at().unix_seconds()));
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit public API base, provider label, listing, currency, search and time range"]
async fn coingecko_public_reads_live() -> Result<(), Box<dyn std::error::Error>> {
    let items = u32::try_from(number("REGIT_WEB3_COINGECKO_MAX_ITEMS")?)
        .map_err(|_| Error::Configuration)?;
    let http = live::http_config("REGIT_WEB3_COINGECKO")?;
    let http = regit_web3::config::HttpConfig::new(
        http.endpoint()
            .clone()
            .with_header("user-agent", "regit-web3-qualification")?,
        http.limits(),
        http.provider_id(),
    )?;
    let config = CoinGeckoHttpConfig::anonymous(http, ItemLimit::new(items)?)?;
    let client = CoinGeckoClient::new(config)?;
    assert_eq!(client.config().tier(), None);
    let id = CoinId::parse(&live::required("REGIT_WEB3_COINGECKO_COIN_ID")?)?;
    let currency = Currency::parse(&live::required("REGIT_WEB3_COINGECKO_CURRENCY")?)?;
    let query = SearchQuery::new(&live::required("REGIT_WEB3_COINGECKO_SEARCH")?)?;
    let from = number("REGIT_WEB3_COINGECKO_FROM_UNIX_SECONDS")?;
    let to = number("REGIT_WEB3_COINGECKO_TO_UNIX_SECONDS")?;
    let before = live::unix_seconds()?;
    let search = client.search(query.clone()).await?;
    check(&search, "search", before)?;
    assert_eq!(search.value().query(), &query);
    assert!(search.value().coins().iter().any(|coin| coin.id() == &id));
    live::print_and_roundtrip(&search)?;
    let before = live::unix_seconds()?;
    let prices = client
        .prices(PricesRequest::new(
            vec![id.clone()],
            vec![currency.clone()],
        )?)
        .await?;
    check(&prices, "simple-price", before)?;
    assert_eq!(prices.value().quotes().len(), 1);
    assert!(matches!(
        prices.value().quotes()[0].availability(),
        PriceAvailability::Present(_)
    ));
    live::print_and_roundtrip(&prices)?;
    let before = live::unix_seconds()?;
    let markets = client
        .markets(MarketsRequest::new(
            currency.clone(),
            1,
            1,
            Some(vec![id.clone()]),
        )?)
        .await?;
    check(&markets, "coins-markets", before)?;
    assert_eq!(markets.value().coins().len(), 1);
    assert_eq!(markets.value().coins()[0].id(), &id);
    live::print_and_roundtrip(&markets)?;
    let request = HistoryRequest::new(id, currency, from, to)?;
    let before = live::unix_seconds()?;
    let history = client.history(request.clone()).await?;
    check(&history, "market-chart-range", before)?;
    assert_eq!(history.value().request(), &request);
    assert!(!history.value().prices().is_empty());
    live::print_and_roundtrip(&history)?;
    Ok(())
}
