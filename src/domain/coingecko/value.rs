// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Currency, HistoryRequest, MarketsRequest, PricesRequest, SearchQuery};
use crate::{
    domain::{
        Timestamp,
        market::{Identifier, Label, NonnegativeDecimal, UnixMilliseconds, UtcDateTime, unique},
    },
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// Price availability for one explicitly requested ID/currency pair.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "value", rename_all = "snake_case")]
pub enum PriceAvailability {
    /// The provider supplied an exact nonnegative price.
    Present(NonnegativeDecimal),
    /// The provider explicitly supplied null for this price.
    NotReported,
    /// The requested listing ID was absent from the response.
    AssetUnavailable,
    /// The listing existed but the requested currency field was absent.
    CurrencyUnavailable,
}

/// A search candidate with provider ID and separate display metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchCoin {
    id: Identifier,
    name: Label,
    symbol: Label,

    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    market_cap_rank: Option<u64>,
}
impl SearchCoin {
    /// Records explicitly supplied validated field values.
    #[must_use]
    pub const fn new(
        id: Identifier,
        name: Label,
        symbol: Label,
        market_cap_rank: Option<u64>,
    ) -> Self {
        Self {
            id,
            name,
            symbol,
            market_cap_rank,
        }
    }
    /// `CoinGecko` listing ID.
    #[must_use]
    pub const fn id(&self) -> &Identifier {
        &self.id
    }
    /// Provider display name.
    #[must_use]
    pub const fn name(&self) -> &Label {
        &self.name
    }
    /// Provider display symbol; not an identity.
    #[must_use]
    pub const fn symbol(&self) -> &Label {
        &self.symbol
    }
    /// Explicit optional provider market-cap rank.
    #[must_use]
    pub const fn market_cap_rank(&self) -> &Option<u64> {
        &self.market_cap_rank
    }
}

/// One requested listing/currency pair with explicit data availability.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceQuote {
    id: Identifier,
    currency: Currency,
    availability: PriceAvailability,

    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    last_updated_at: Option<Timestamp>,
}
impl PriceQuote {
    /// Records explicitly supplied validated field values.
    #[must_use]
    pub const fn new(
        id: Identifier,
        currency: Currency,
        availability: PriceAvailability,
        last_updated_at: Option<Timestamp>,
    ) -> Self {
        Self {
            id,
            currency,
            availability,
            last_updated_at,
        }
    }
    /// Requested listing ID.
    #[must_use]
    pub const fn id(&self) -> &Identifier {
        &self.id
    }
    /// Requested quote currency.
    #[must_use]
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }
    /// Value or explicit missing/null availability.
    #[must_use]
    pub const fn availability(&self) -> &PriceAvailability {
        &self.availability
    }
    /// Provider data time in Unix seconds, if supplied.
    #[must_use]
    pub const fn last_updated_at(&self) -> &Option<Timestamp> {
        &self.last_updated_at
    }
}

/// An exact market record; currency applies to price, capitalization and volume.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoinMarket {
    id: Identifier,
    name: Label,
    symbol: Label,
    currency: Currency,
    values: MarketValues,

    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    market_cap_rank: Option<u64>,

    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    last_updated: Option<UtcDateTime>,
}
impl CoinMarket {
    /// Records explicitly supplied validated field values.
    #[must_use]
    pub const fn new(
        id: Identifier,
        name: Label,
        symbol: Label,
        currency: Currency,
        values: MarketValues,
        market_cap_rank: Option<u64>,
        last_updated: Option<UtcDateTime>,
    ) -> Self {
        Self {
            id,
            name,
            symbol,
            currency,
            values,
            market_cap_rank,
            last_updated,
        }
    }
    /// Listing ID.
    #[must_use]
    pub const fn id(&self) -> &Identifier {
        &self.id
    }
    /// Display name.
    #[must_use]
    pub const fn name(&self) -> &Label {
        &self.name
    }
    /// Display symbol.
    #[must_use]
    pub const fn symbol(&self) -> &Label {
        &self.symbol
    }
    /// Explicit quote currency.
    #[must_use]
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }
    /// Reported price; null remains absent.
    #[must_use]
    pub const fn current_price(&self) -> &Option<NonnegativeDecimal> {
        self.values.current_price()
    }
    /// Reported capitalization in the quote currency.
    #[must_use]
    pub const fn market_cap(&self) -> &Option<NonnegativeDecimal> {
        self.values.market_cap()
    }
    /// Reported 24-hour volume in the quote currency.
    #[must_use]
    pub const fn total_volume(&self) -> &Option<NonnegativeDecimal> {
        self.values.total_volume()
    }
    /// Provider rank, without an inferred value.
    #[must_use]
    pub const fn market_cap_rank(&self) -> &Option<u64> {
        &self.market_cap_rank
    }
    /// Provider UTC data timestamp; null remains absent.
    #[must_use]
    pub const fn last_updated(&self) -> &Option<UtcDateTime> {
        &self.last_updated
    }
}

/// One exact nonnegative sample at explicit Unix milliseconds.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartPoint {
    timestamp: UnixMilliseconds,
    value: NonnegativeDecimal,
}
impl ChartPoint {
    /// Records explicitly supplied validated field values.
    #[must_use]
    pub const fn new(timestamp: UnixMilliseconds, value: NonnegativeDecimal) -> Self {
        Self { timestamp, value }
    }
    /// Provider time in Unix milliseconds.
    #[must_use]
    pub const fn timestamp(&self) -> &UnixMilliseconds {
        &self.timestamp
    }
    /// Exact sampled value, in the enclosing series currency.
    #[must_use]
    pub const fn value(&self) -> &NonnegativeDecimal {
        &self.value
    }
}

/// Search candidates returned for one exact term; no candidate is selected.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SearchFields")]
pub struct SearchResults {
    query: SearchQuery,
    coins: Vec<SearchCoin>,
}
impl SearchResults {
    /// Requires a bounded candidate list without duplicate listing IDs.
    /// # Errors
    /// Rejects more than 100,000 candidates or duplicate IDs.
    pub fn new(query: SearchQuery, coins: Vec<SearchCoin>) -> Result<Self, Error> {
        if coins.len() > 100_000 || !unique(coins.iter().map(SearchCoin::id)) {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self { query, coins })
    }
    /// Returns the requested term.
    #[must_use]
    pub const fn query(&self) -> &SearchQuery {
        &self.query
    }
    /// Returns every returned candidate, in provider order.
    #[must_use]
    pub fn coins(&self) -> &[SearchCoin] {
        &self.coins
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchFields {
    query: SearchQuery,
    coins: Vec<SearchCoin>,
}
impl TryFrom<SearchFields> for SearchResults {
    type Error = Error;
    fn try_from(v: SearchFields) -> Result<Self, Error> {
        Self::new(v.query, v.coins)
    }
}

/// Exact prices for every requested ID/currency pair, including missing values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PriceFields")]
pub struct Prices {
    request: PricesRequest,
    quotes: Vec<PriceQuote>,
}
impl Prices {
    /// Requires exactly one quote per requested pair in caller matrix order.
    /// # Errors
    /// Rejects missing, extra, reordered or mismatched quote identities.
    pub fn new(request: PricesRequest, quotes: Vec<PriceQuote>) -> Result<Self, Error> {
        if quotes.len() != request.ids().len() * request.currencies().len()
            || request
                .ids()
                .iter()
                .flat_map(|id| {
                    request
                        .currencies()
                        .iter()
                        .map(move |currency| (id, currency))
                })
                .zip(&quotes)
                .any(|((id, currency), q)| id != q.id() || currency != q.currency())
        {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self { request, quotes })
    }
    /// Returns the exact request matrix.
    #[must_use]
    pub const fn request(&self) -> &PricesRequest {
        &self.request
    }
    /// Returns quotes in requested ID/currency matrix order.
    #[must_use]
    pub fn quotes(&self) -> &[PriceQuote] {
        &self.quotes
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PriceFields {
    request: PricesRequest,
    quotes: Vec<PriceQuote>,
}
impl TryFrom<PriceFields> for Prices {
    type Error = Error;
    fn try_from(v: PriceFields) -> Result<Self, Error> {
        Self::new(v.request, v.quotes)
    }
}

/// Honest page-size information, without an inferred exhaustive catalogue.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    /// Fewer records than requested were returned for this operation.
    ShortPage,
    /// A full page was returned, so more records may exist.
    MayHaveMore,
}
/// One explicit markets page; no additional pages are collected implicitly.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MarketsPageFields")]
pub struct MarketsPage {
    request: MarketsRequest,
    coins: Vec<CoinMarket>,
}
impl MarketsPage {
    /// Checks page count, listing uniqueness, selected IDs and quote currencies.
    /// # Errors
    /// Rejects a record outside the requested page contract.
    pub fn new(request: MarketsRequest, coins: Vec<CoinMarket>) -> Result<Self, Error> {
        if coins.len() > usize::from(request.count())
            || !unique(coins.iter().map(CoinMarket::id))
            || coins.iter().any(|v| {
                v.currency() != request.currency()
                    || request.ids().is_some_and(|ids| !ids.contains(v.id()))
            })
        {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self { request, coins })
    }
    /// Returns the original page request.
    #[must_use]
    pub const fn request(&self) -> &MarketsRequest {
        &self.request
    }
    /// Returns provider records in returned order.
    #[must_use]
    pub fn coins(&self) -> &[CoinMarket] {
        &self.coins
    }
    /// Returns observed page-size information without predicting future pages.
    #[must_use]
    pub fn status(&self) -> PageStatus {
        if self.coins.len() == usize::from(self.request.count()) {
            PageStatus::MayHaveMore
        } else {
            PageStatus::ShortPage
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketsPageFields {
    request: MarketsRequest,
    coins: Vec<CoinMarket>,
}
impl TryFrom<MarketsPageFields> for MarketsPage {
    type Error = Error;
    fn try_from(v: MarketsPageFields) -> Result<Self, Error> {
        Self::new(v.request, v.coins)
    }
}

/// Independent exact price, capitalization and 24-hour-volume historical series.
/// Timestamps are not artificially aligned across the three provider series.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryFields")]
pub struct HistoricalChart {
    request: HistoryRequest,
    prices: Vec<ChartPoint>,
    market_caps: Vec<ChartPoint>,
    total_volumes: Vec<ChartPoint>,
}
impl HistoricalChart {
    /// Requires bounded, strictly increasing samples inside the requested range.
    /// # Errors
    /// Rejects excessive, repeated, reordered or out-of-range data times.
    pub fn new(
        request: HistoryRequest,
        prices: Vec<ChartPoint>,
        market_caps: Vec<ChartPoint>,
        total_volumes: Vec<ChartPoint>,
    ) -> Result<Self, Error> {
        let from = request.from_unix_seconds() * 1000;
        let to = request.to_unix_seconds() * 1000;
        if [&prices, &market_caps, &total_volumes].iter().any(|v| {
            v.len() > 100_000
                || v.iter()
                    .any(|p| !(from..=to).contains(&p.timestamp().get()))
                || v.windows(2).any(|p| p[0].timestamp() >= p[1].timestamp())
        }) {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self {
            request,
            prices,
            market_caps,
            total_volumes,
        })
    }
    /// Returns ID, currency and requested range in Unix seconds.
    #[must_use]
    pub const fn request(&self) -> &HistoryRequest {
        &self.request
    }
    /// Returns exact prices in quote currency, at provider Unix milliseconds.
    #[must_use]
    pub fn prices(&self) -> &[ChartPoint] {
        &self.prices
    }
    /// Returns market capitalization in the explicit quote currency.
    #[must_use]
    pub fn market_caps(&self) -> &[ChartPoint] {
        &self.market_caps
    }
    /// Returns 24-hour trading volume in the explicit quote currency.
    #[must_use]
    pub fn total_volumes(&self) -> &[ChartPoint] {
        &self.total_volumes
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryFields {
    request: HistoryRequest,
    prices: Vec<ChartPoint>,
    market_caps: Vec<ChartPoint>,
    total_volumes: Vec<ChartPoint>,
}
impl TryFrom<HistoryFields> for HistoricalChart {
    type Error = Error;
    fn try_from(v: HistoryFields) -> Result<Self, Error> {
        Self::new(v.request, v.prices, v.market_caps, v.total_volumes)
    }
}

/// Related price, capitalization and trading-volume values in one quote currency.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketValues {
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    current_price: Option<NonnegativeDecimal>,

    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    market_cap: Option<NonnegativeDecimal>,

    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    total_volume: Option<NonnegativeDecimal>,
}
impl MarketValues {
    /// Records explicitly reported values; null remains distinct from zero.
    #[must_use]
    pub const fn new(
        current_price: Option<NonnegativeDecimal>,
        market_cap: Option<NonnegativeDecimal>,
        total_volume: Option<NonnegativeDecimal>,
    ) -> Self {
        Self {
            current_price,
            market_cap,
            total_volume,
        }
    }
    /// Returns reported price in quote currency.
    #[must_use]
    pub const fn current_price(&self) -> &Option<NonnegativeDecimal> {
        &self.current_price
    }
    /// Returns reported capitalization in quote currency.
    #[must_use]
    pub const fn market_cap(&self) -> &Option<NonnegativeDecimal> {
        &self.market_cap
    }
    /// Returns reported 24-hour trading volume in quote currency.
    #[must_use]
    pub const fn total_volume(&self) -> &Option<NonnegativeDecimal> {
        &self.total_volume
    }
}
