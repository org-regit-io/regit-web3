// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::market_wire::{List, Map, Number, check_limit, decode, invalid, required_option};
use crate::{
    domain::{
        Timestamp,
        defillama::{
            Analytics, AnalyticsRequest, AnalyticsTotals, BreakdownValue, ProtocolHistory,
            ProtocolTvl, ProviderId, Stablecoin, StablecoinHistory, StablecoinId, StablecoinPoint,
            StablecoinScope, Stablecoins, TvlHistory, TvlScope, UsdPoint, YieldHistory, YieldPoint,
            YieldPool, YieldPools, YieldRates, YieldSymbol,
        },
        market::{ItemLimit, Label, UtcDateTime},
    },
    error::Error,
};
use serde::Deserialize;
pub(super) fn protocol_tvl(bytes: &[u8], protocol: ProviderId) -> Result<ProtocolTvl, Error> {
    Ok(ProtocolTvl::new(
        protocol,
        decode::<Number>(bytes)?.nonnegative()?,
    ))
}
fn reporting(values: Map<Number>, limit: ItemLimit) -> Result<Vec<BreakdownValue>, Error> {
    check_limit(values.0.len(), limit)?;
    values
        .0
        .into_iter()
        .map(|(key, value)| {
            Ok(BreakdownValue::new(
                Label::new(&key).map_err(|_| invalid())?,
                value.nonnegative()?,
            ))
        })
        .collect()
}
fn points(values: List<TvlFields>, limit: ItemLimit) -> Result<Vec<UsdPoint>, Error> {
    check_limit(values.0.len(), limit)?;
    values
        .0
        .into_iter()
        .map(|v| {
            Ok(UsdPoint::new(
                Timestamp::from_unix_seconds(v.date),
                v.value.nonnegative()?,
            ))
        })
        .collect()
}
#[derive(Deserialize)]
struct TvlFields {
    date: u64,
    #[serde(rename = "totalLiquidityUSD", alias = "tvl")]
    value: Number,
}
pub(super) fn protocol_history(
    bytes: &[u8],
    protocol: ProviderId,
    limit: ItemLimit,
) -> Result<ProtocolHistory, Error> {
    let v: ProtocolFields = decode(bytes)?;
    ProtocolHistory::new(
        protocol,
        Label::new(&v.id).map_err(|_| invalid())?,
        Label::new(&v.name).map_err(|_| invalid())?,
        points(v.tvl, limit)?,
        reporting(v.current_chain_tvls, limit)?,
    )
    .map_err(|_| invalid())
}
#[derive(Deserialize)]
struct ProtocolFields {
    id: String,
    name: String,
    tvl: List<TvlFields>,
    #[serde(rename = "currentChainTvls")]
    current_chain_tvls: Map<Number>,
}
pub(super) fn tvl_history(
    bytes: &[u8],
    scope: TvlScope,
    limit: ItemLimit,
) -> Result<TvlHistory, Error> {
    TvlHistory::new(scope, points(decode(bytes)?, limit)?).map_err(|_| invalid())
}
#[derive(Deserialize)]
struct YieldEnvelope<T> {
    status: String,
    data: List<T>,
}
#[derive(Deserialize)]
struct PoolFields {
    pool: String,
    chain: String,
    project: String,
    symbol: String,
    #[serde(rename = "tvlUsd")]
    tvl_usd: Number,
    #[serde(deserialize_with = "required_option")]
    apy: Option<Number>,
    #[serde(rename = "apyBase", deserialize_with = "required_option")]
    apy_base: Option<Number>,
    #[serde(rename = "apyReward", deserialize_with = "required_option")]
    apy_reward: Option<Number>,
}
fn rates(apy: Option<Number>, base: Option<Number>, reward: Option<Number>) -> YieldRates {
    YieldRates::new(apy.map(|v| v.0), base.map(|v| v.0), reward.map(|v| v.0))
}
pub(super) fn yield_pools(bytes: &[u8], limit: ItemLimit) -> Result<YieldPools, Error> {
    let v: YieldEnvelope<PoolFields> = decode(bytes)?;
    if v.status != "success" {
        return Err(invalid());
    }
    check_limit(v.data.0.len(), limit)?;
    let pools = v
        .data
        .0
        .into_iter()
        .map(|p| {
            Ok(YieldPool::new(
                ProviderId::parse(&p.pool).map_err(|_| invalid())?,
                Label::new(&p.chain).map_err(|_| invalid())?,
                ProviderId::parse(&p.project).map_err(|_| invalid())?,
                YieldSymbol::new(&p.symbol).map_err(|_| invalid())?,
                p.tvl_usd.nonnegative()?,
                rates(p.apy, p.apy_base, p.apy_reward),
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    YieldPools::new(pools).map_err(|_| invalid())
}
#[derive(Deserialize)]
struct YieldPointFields {
    timestamp: String,
    #[serde(rename = "tvlUsd")]
    tvl_usd: Number,
    #[serde(deserialize_with = "required_option")]
    apy: Option<Number>,
    #[serde(rename = "apyBase", deserialize_with = "required_option")]
    apy_base: Option<Number>,
    #[serde(rename = "apyReward", deserialize_with = "required_option")]
    apy_reward: Option<Number>,
}
pub(super) fn yield_history(
    bytes: &[u8],
    pool: ProviderId,
    limit: ItemLimit,
) -> Result<YieldHistory, Error> {
    let v: YieldEnvelope<YieldPointFields> = decode(bytes)?;
    if v.status != "success" {
        return Err(invalid());
    }
    check_limit(v.data.0.len(), limit)?;
    let points = v
        .data
        .0
        .into_iter()
        .map(|p| {
            Ok(YieldPoint::new(
                UtcDateTime::parse(&p.timestamp).map_err(|_| invalid())?,
                p.tvl_usd.nonnegative()?,
                rates(p.apy, p.apy_base, p.apy_reward),
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    YieldHistory::new(pool, points).map_err(|_| invalid())
}
#[derive(Deserialize)]
struct StableEnvelope {
    #[serde(rename = "peggedAssets")]
    assets: List<StableFields>,
}
#[derive(Deserialize)]
struct StableFields {
    id: String,
    name: String,
    symbol: String,
    #[serde(rename = "pegType")]
    peg_type: String,
    circulating: Map<Number>,
    #[serde(deserialize_with = "required_option")]
    price: Option<Number>,
    chains: List<String>,
}
fn canonical_integer<T: std::str::FromStr>(text: &str) -> Result<T, Error> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }
    text.parse().map_err(|_| invalid())
}
pub(super) fn stablecoins(bytes: &[u8], limit: ItemLimit) -> Result<Stablecoins, Error> {
    let v: StableEnvelope = decode(bytes)?;
    check_limit(v.assets.0.len(), limit)?;
    let assets = v
        .assets
        .0
        .into_iter()
        .map(|a| {
            check_limit(a.chains.0.len(), limit)?;
            let chains = a
                .chains
                .0
                .into_iter()
                .map(|c| Label::new(&c).map_err(|_| invalid()))
                .collect::<Result<Vec<_>, Error>>()?;
            Stablecoin::new(
                StablecoinId::new(canonical_integer(&a.id)?).map_err(|_| invalid())?,
                Label::new(&a.name).map_err(|_| invalid())?,
                Label::new(&a.symbol).map_err(|_| invalid())?,
                Label::new(&a.peg_type).map_err(|_| invalid())?,
                reporting(a.circulating, limit)?,
                a.price.map(Number::nonnegative).transpose()?,
                chains,
            )
            .map_err(|_| invalid())
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Stablecoins::new(assets).map_err(|_| invalid())
}
#[derive(Deserialize)]
struct StablePointFields {
    date: String,
    #[serde(rename = "totalCirculating")]
    circulating: Map<Number>,
    #[serde(rename = "totalCirculatingUSD")]
    circulating_usd: Map<Number>,
}
pub(super) fn stablecoin_history(
    bytes: &[u8],
    scope: StablecoinScope,
    limit: ItemLimit,
) -> Result<StablecoinHistory, Error> {
    let v: List<StablePointFields> = decode(bytes)?;
    check_limit(v.0.len(), limit)?;
    let points =
        v.0.into_iter()
            .map(|p| {
                StablecoinPoint::new(
                    Timestamp::from_unix_seconds(canonical_integer(&p.date)?),
                    reporting(p.circulating, limit)?,
                    reporting(p.circulating_usd, limit)?,
                )
                .map_err(|_| invalid())
            })
            .collect::<Result<Vec<_>, Error>>()?;
    StablecoinHistory::new(scope, points).map_err(|_| invalid())
}
#[derive(Deserialize)]
struct AnalyticsFields {
    slug: String,
    name: String,
    #[serde(rename = "total24h", deserialize_with = "required_option")]
    day: Option<Number>,
    #[serde(rename = "total7d", deserialize_with = "required_option")]
    week: Option<Number>,
    #[serde(rename = "totalAllTime", deserialize_with = "required_option")]
    all_time: Option<Number>,
    #[serde(rename = "totalDataChart")]
    daily: List<(u64, Number)>,
}
pub(super) fn analytics(
    bytes: &[u8],
    request: AnalyticsRequest,
    limit: ItemLimit,
) -> Result<Analytics, Error> {
    let v: AnalyticsFields = decode(bytes)?;
    if v.slug != request.protocol().as_str() {
        return Err(invalid());
    }
    check_limit(v.daily.0.len(), limit)?;
    let daily = v
        .daily
        .0
        .into_iter()
        .map(|(date, value)| {
            Ok(UsdPoint::new(
                Timestamp::from_unix_seconds(date),
                value.nonnegative()?,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Analytics::new(
        request,
        Label::new(&v.name).map_err(|_| invalid())?,
        AnalyticsTotals::new(
            v.day.map(Number::nonnegative).transpose()?,
            v.week.map(Number::nonnegative).transpose()?,
            v.all_time.map(Number::nonnegative).transpose()?,
        ),
        daily,
    )
    .map_err(|_| invalid())
}
