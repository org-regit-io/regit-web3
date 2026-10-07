// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit opt-in `DefiLlama` qualification through the typed Rust public API.
//! Dataset reads are independent observations; no network or time input belongs
//! to the library. Large pool/protocol responses are bounded, never truncated.
#![cfg(feature = "defillama-http")]
#[path = "support/live.rs"]
mod live;
use regit_web3::{
    config::{HttpConfig, RpcLimits},
    domain::{
        defillama::{
            AnalyticsMetric, AnalyticsRequest, ProviderId, StablecoinId, StablecoinScope, TvlScope,
        },
        market::{ItemLimit, Label, Observation},
    },
    error::Error,
    providers::defillama::{DefiLlamaClient, DefiLlamaHttpConfig},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{fmt::Debug, time::Duration};
fn http(prefix: &str) -> Result<HttpConfig, Error> {
    let http = live::http_config(prefix)?;
    HttpConfig::new(
        http.endpoint()
            .clone()
            .with_header("user-agent", "regit-web3-qualification")?,
        RpcLimits::new(
            Duration::from_secs(5),
            Duration::from_secs(45),
            16 * 1024 * 1024,
            2,
        )?,
        http.provider_id(),
    )
}
fn number(name: &str) -> Result<u32, Error> {
    live::required(name)?
        .parse()
        .map_err(|_| Error::Configuration)
}
fn check<T>(
    v: &Observation<T>,
    source: &HttpConfig,
    method: &str,
    before: u64,
) -> Result<(), Error> {
    assert_eq!(v.source().provider_id(), source.provider_id());
    assert_eq!(v.source().method(), method);
    assert_eq!(v.source().integration_version(), env!("CARGO_PKG_VERSION"));
    assert!((before..=live::unix_seconds()?).contains(&v.retrieved_at().unix_seconds()));
    Ok(())
}
fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(
    v: &T,
) -> Result<(), serde_json::Error> {
    let bytes = serde_json::to_vec(v)?;
    assert_eq!(&serde_json::from_slice::<T>(&bytes)?, v);
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit independent source URLs/labels, protocol, pool, chain, stablecoin and analytics inputs"]
async fn defillama_provider_reads_live() -> Result<(), Box<dyn std::error::Error>> {
    let config = DefiLlamaHttpConfig::new(
        http("REGIT_WEB3_DEFILLAMA_TVL")?,
        http("REGIT_WEB3_DEFILLAMA_YIELDS")?,
        http("REGIT_WEB3_DEFILLAMA_STABLECOINS")?,
        ItemLimit::new(number("REGIT_WEB3_DEFILLAMA_MAX_ITEMS")?)?,
    );
    let protocol = ProviderId::parse(&live::required("REGIT_WEB3_DEFILLAMA_PROTOCOL")?)?;
    let pool = ProviderId::parse(&live::required("REGIT_WEB3_DEFILLAMA_POOL")?)?;
    let chain = Label::new(&live::required("REGIT_WEB3_DEFILLAMA_CHAIN")?)?;
    let stable_id = StablecoinId::new(number("REGIT_WEB3_DEFILLAMA_STABLECOIN_ID")?)?;
    let dex = ProviderId::parse(&live::required("REGIT_WEB3_DEFILLAMA_DEX_PROTOCOL")?)?;
    let fees = ProviderId::parse(&live::required("REGIT_WEB3_DEFILLAMA_FEES_PROTOCOL")?)?;
    let c = DefiLlamaClient::new(config)?;
    let before = live::unix_seconds()?;
    let v = c.protocol_tvl(protocol.clone()).await?;
    check(&v, c.config().tvl(), "tvl", before)?;
    assert_eq!(v.value().protocol(), &protocol);
    live::print_and_roundtrip(&v)?;
    let before = live::unix_seconds()?;
    let v = c.protocol_history(protocol.clone()).await?;
    check(&v, c.config().tvl(), "protocol", before)?;
    assert_eq!(v.value().protocol(), &protocol);
    assert!(!v.value().points().is_empty());
    roundtrip(&v)?;
    println!(
        "protocol history: {} points, {} reporting components, retrieval {}",
        v.value().points().len(),
        v.value().current_breakdowns().len(),
        v.retrieved_at().unix_seconds()
    );
    let scope = TvlScope::Chain(chain);
    let before = live::unix_seconds()?;
    let v = c.tvl_history(scope.clone()).await?;
    check(&v, c.config().tvl(), "historical-chain-tvl", before)?;
    assert_eq!(v.value().scope(), &scope);
    assert!(!v.value().points().is_empty());
    roundtrip(&v)?;
    println!(
        "chain TVL: {} points, retrieval {}",
        v.value().points().len(),
        v.retrieved_at().unix_seconds()
    );
    let before = live::unix_seconds()?;
    let v = c.yield_pools().await?;
    check(&v, c.config().yields(), "pools", before)?;
    assert!(v.value().pools().iter().any(|p| p.pool() == &pool));
    roundtrip(&v)?;
    println!(
        "yield catalogue: {} pools, retrieval {}",
        v.value().pools().len(),
        v.retrieved_at().unix_seconds()
    );
    let before = live::unix_seconds()?;
    let v = c.yield_history(pool.clone()).await?;
    check(&v, c.config().yields(), "yield-chart", before)?;
    assert_eq!(v.value().pool(), &pool);
    assert!(!v.value().points().is_empty());
    roundtrip(&v)?;
    println!(
        "yield history: {} points, retrieval {}",
        v.value().points().len(),
        v.retrieved_at().unix_seconds()
    );
    let before = live::unix_seconds()?;
    let v = c.stablecoins().await?;
    check(&v, c.config().stablecoins(), "stablecoins", before)?;
    assert!(v.value().assets().iter().any(|a| a.id() == stable_id));
    roundtrip(&v)?;
    println!(
        "stablecoin catalogue: {} assets, retrieval {}",
        v.value().assets().len(),
        v.retrieved_at().unix_seconds()
    );
    let scope = StablecoinScope::new(None, Some(stable_id));
    let before = live::unix_seconds()?;
    let v = c.stablecoin_history(scope.clone()).await?;
    check(&v, c.config().stablecoins(), "stablecoin-charts", before)?;
    assert_eq!(v.value().scope(), &scope);
    assert!(!v.value().points().is_empty());
    roundtrip(&v)?;
    println!(
        "stablecoin history: {} points, retrieval {}",
        v.value().points().len(),
        v.retrieved_at().unix_seconds()
    );
    qualify_analytics(&c, dex, fees).await?;
    Ok(())
}

async fn qualify_analytics(
    c: &DefiLlamaClient,
    dex: ProviderId,
    fees: ProviderId,
) -> Result<(), Box<dyn std::error::Error>> {
    for (protocol, metric) in [
        (dex, AnalyticsMetric::DexVolume),
        (fees.clone(), AnalyticsMetric::Fees),
        (fees.clone(), AnalyticsMetric::Revenue),
        (fees, AnalyticsMetric::HoldersRevenue),
    ] {
        let request = AnalyticsRequest::new(protocol, metric);
        let before = live::unix_seconds()?;
        let v = c.analytics(request.clone()).await?;
        check(&v, c.config().tvl(), "analytics-summary", before)?;
        assert_eq!(v.value().request(), &request);
        assert!(!v.value().daily().is_empty());
        roundtrip(&v)?;
        println!(
            "analytics {:?}: {} points, retrieval {}",
            metric,
            v.value().daily().len(),
            v.retrieved_at().unix_seconds()
        );
    }
    Ok(())
}
