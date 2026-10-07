// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::market_wire::{List, Map, Number, check_limit, decode, invalid, required_option};
use crate::{
    domain::{
        Timestamp,
        coingecko::{
            ChartPoint, CoinMarket, HistoricalChart, HistoryRequest, MarketValues, MarketsPage,
            MarketsRequest, PriceAvailability, PriceQuote, Prices, PricesRequest, SearchCoin,
            SearchQuery, SearchResults,
        },
        market::{Identifier, ItemLimit, Label, UnixMilliseconds, UtcDateTime},
    },
    error::Error,
};
use serde::Deserialize;
use serde_json::value::RawValue;

pub(super) fn search(
    bytes: &[u8],
    query: SearchQuery,
    limit: ItemLimit,
) -> Result<SearchResults, Error> {
    let fields: SearchFields = decode(bytes)?;
    check_limit(fields.coins.0.len(), limit)?;
    let coins = fields
        .coins
        .0
        .into_iter()
        .map(|v| {
            Ok(SearchCoin::new(
                Identifier::parse(&v.id).map_err(|_| invalid())?,
                Label::new(&v.name).map_err(|_| invalid())?,
                Label::new(&v.symbol).map_err(|_| invalid())?,
                v.market_cap_rank,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    SearchResults::new(query, coins).map_err(|_| invalid())
}
#[derive(Deserialize)]
struct SearchFields {
    coins: List<SearchCoinFields>,
}
#[derive(Deserialize)]
struct SearchCoinFields {
    id: String,
    name: String,
    symbol: String,
    #[serde(deserialize_with = "required_option")]
    market_cap_rank: Option<u64>,
}

pub(super) fn prices(
    bytes: &[u8],
    request: PricesRequest,
    limit: ItemLimit,
) -> Result<Prices, Error> {
    check_limit(request.ids().len() * request.currencies().len(), limit)?;
    let fields: Map<Map<Box<RawValue>>> = decode(bytes)?;
    if fields
        .0
        .keys()
        .any(|id| !request.ids().iter().any(|v| v.as_str() == id))
    {
        return Err(invalid());
    }
    let mut quotes = Vec::new();
    for id in request.ids() {
        let asset = fields.0.get(id.as_str());
        let updated = asset
            .map(|a| {
                a.0.get("last_updated_at")
                    .ok_or_else(invalid)
                    .and_then(|v| decode::<Option<u64>>(v.get().as_bytes()))
            })
            .transpose()?
            .flatten()
            .map(Timestamp::from_unix_seconds);
        for currency in request.currencies() {
            let availability = match asset {
                None => PriceAvailability::AssetUnavailable,
                Some(a) => match a.0.get(currency.as_str()) {
                    None => PriceAvailability::CurrencyUnavailable,
                    Some(raw) => match decode::<Option<Number>>(raw.get().as_bytes())? {
                        Some(n) => PriceAvailability::Present(n.nonnegative()?),
                        None => PriceAvailability::NotReported,
                    },
                },
            };
            quotes.push(PriceQuote::new(
                id.clone(),
                currency.clone(),
                availability,
                updated,
            ));
        }
    }
    Prices::new(request, quotes).map_err(|_| invalid())
}

pub(super) fn markets(
    bytes: &[u8],
    request: MarketsRequest,
    limit: ItemLimit,
) -> Result<MarketsPage, Error> {
    let fields: List<MarketFields> = decode(bytes)?;
    check_limit(fields.0.len(), limit)?;
    let coins = fields
        .0
        .into_iter()
        .map(|v| {
            Ok(CoinMarket::new(
                Identifier::parse(&v.id).map_err(|_| invalid())?,
                Label::new(&v.name).map_err(|_| invalid())?,
                Label::new(&v.symbol).map_err(|_| invalid())?,
                request.currency().clone(),
                MarketValues::new(
                    v.current_price.map(Number::nonnegative).transpose()?,
                    v.market_cap.map(Number::nonnegative).transpose()?,
                    v.total_volume.map(Number::nonnegative).transpose()?,
                ),
                v.market_cap_rank,
                v.last_updated
                    .map(|s| UtcDateTime::parse(&s).map_err(|_| invalid()))
                    .transpose()?,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    MarketsPage::new(request, coins).map_err(|_| invalid())
}
#[derive(Deserialize)]
struct MarketFields {
    id: String,
    name: String,
    symbol: String,
    #[serde(deserialize_with = "required_option")]
    current_price: Option<Number>,
    #[serde(deserialize_with = "required_option")]
    market_cap: Option<Number>,
    #[serde(deserialize_with = "required_option")]
    total_volume: Option<Number>,
    #[serde(deserialize_with = "required_option")]
    market_cap_rank: Option<u64>,
    #[serde(deserialize_with = "required_option")]
    last_updated: Option<String>,
}
pub(super) fn history(
    bytes: &[u8],
    request: HistoryRequest,
    limit: ItemLimit,
) -> Result<HistoricalChart, Error> {
    let fields: HistoryFields = decode(bytes)?;
    let map = |values: List<(u64, Number)>| -> Result<Vec<ChartPoint>, Error> {
        check_limit(values.0.len(), limit)?;
        values
            .0
            .into_iter()
            .map(|(t, v)| Ok(ChartPoint::new(UnixMilliseconds::new(t), v.nonnegative()?)))
            .collect()
    };
    HistoricalChart::new(
        request,
        map(fields.prices)?,
        map(fields.market_caps)?,
        map(fields.total_volumes)?,
    )
    .map_err(|_| invalid())
}
#[derive(Deserialize)]
struct HistoryFields {
    prices: List<(u64, Number)>,
    market_caps: List<(u64, Number)>,
    total_volumes: List<(u64, Number)>,
}
