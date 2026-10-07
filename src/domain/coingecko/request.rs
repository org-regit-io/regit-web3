// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::market::{Identifier, Label, unique},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// An explicit lowercase `CoinGecko` quote currency code.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Currency(String);
impl Currency {
    /// Validates a nonempty lowercase ASCII alphanumeric code of at most 32 bytes.
    /// Availability at the provider is not inferred from lexical validity.
    /// # Errors
    /// Rejects invalid lexical form.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > 32
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        {
            return Err(ValidationError::InvalidMarketIdentity.into());
        }
        Ok(Self(value.to_owned()))
    }
    /// Returns the exact currency code used in the request and value units.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Currency {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<Currency> for String {
    fn from(v: Currency) -> Self {
        v.0
    }
}

/// A caller-supplied search term; symbols never select an asset automatically.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Label", into = "Label")]
pub struct SearchQuery(Label);
impl SearchQuery {
    /// Records a nonempty term of at most 256 bytes without control characters.
    /// # Errors
    /// Rejects an invalid or excessive term.
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.len() > 256 {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self(Label::new(value)?))
    }
    /// Returns the original term for explicit query-string encoding.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl TryFrom<Label> for SearchQuery {
    type Error = Error;
    fn try_from(v: Label) -> Result<Self, Error> {
        Self::new(v.as_str())
    }
}
impl From<SearchQuery> for Label {
    fn from(v: SearchQuery) -> Self {
        v.0
    }
}

/// An exact ID/currency matrix requested in one simple-price operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PricesFields")]
pub struct PricesRequest {
    ids: Vec<Identifier>,
    currencies: Vec<Currency>,
}
impl PricesRequest {
    /// Requires 1–515 distinct coin IDs and 1–20 distinct quote currencies.
    /// # Errors
    /// Rejects empty, excessive or duplicate request entries.
    pub fn new(ids: Vec<Identifier>, currencies: Vec<Currency>) -> Result<Self, Error> {
        if !(1..=515).contains(&ids.len())
            || !(1..=20).contains(&currencies.len())
            || !unique(ids.iter())
            || !unique(currencies.iter())
        {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self { ids, currencies })
    }
    /// Returns the selected provider IDs in caller order.
    #[must_use]
    pub fn ids(&self) -> &[Identifier] {
        &self.ids
    }
    /// Returns quote currencies in caller order.
    #[must_use]
    pub fn currencies(&self) -> &[Currency] {
        &self.currencies
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PricesFields {
    ids: Vec<Identifier>,
    currencies: Vec<Currency>,
}
impl TryFrom<PricesFields> for PricesRequest {
    type Error = Error;
    fn try_from(v: PricesFields) -> Result<Self, Error> {
        Self::new(v.ids, v.currencies)
    }
}

/// One explicit markets page, ordered by the provider's market-cap-descending sort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MarketsFields")]
pub struct MarketsRequest {
    currency: Currency,
    page: u32,
    count: u16,
    ids: Option<Vec<Identifier>>,
}
impl MarketsRequest {
    /// Records a page number, count in 1–250 and optional distinct ID filter.
    /// `None` queries the provider catalogue; an empty filter is rejected.
    /// # Errors
    /// Rejects zero/excessive pages, counts or duplicate/excessive IDs.
    pub fn new(
        currency: Currency,
        page: u32,
        count: u16,
        ids: Option<Vec<Identifier>>,
    ) -> Result<Self, Error> {
        if page == 0
            || page > 1_000_000
            || !(1..=250).contains(&count)
            || ids
                .as_ref()
                .is_some_and(|v| !(1..=250).contains(&v.len()) || !unique(v.iter()))
        {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self {
            currency,
            page,
            count,
            ids,
        })
    }
    /// Returns the price/market-cap/volume currency.
    #[must_use]
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }
    /// Returns the explicitly requested provider page.
    #[must_use]
    pub const fn page(&self) -> u32 {
        self.page
    }
    /// Returns the explicitly requested item count.
    #[must_use]
    pub const fn count(&self) -> u16 {
        self.count
    }
    /// Returns the optional listing-ID filter.
    #[must_use]
    pub fn ids(&self) -> Option<&[Identifier]> {
        self.ids.as_deref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketsFields {
    currency: Currency,
    page: u32,
    count: u16,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    ids: Option<Vec<Identifier>>,
}
impl TryFrom<MarketsFields> for MarketsRequest {
    type Error = Error;
    fn try_from(v: MarketsFields) -> Result<Self, Error> {
        Self::new(v.currency, v.page, v.count, v.ids)
    }
}

/// A historical market-chart range, with provider-selected sampling granularity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryFields")]
pub struct HistoryRequest {
    id: Identifier,
    currency: Currency,
    from_unix_seconds: u64,
    to_unix_seconds: u64,
}
impl HistoryRequest {
    /// Requires an increasing explicit range whose milliseconds fit `u64`.
    /// Provider plan restrictions remain provider errors; no range is shortened.
    /// # Errors
    /// Rejects a reversed/empty or overflowing time range.
    pub fn new(id: Identifier, currency: Currency, from: u64, to: u64) -> Result<Self, Error> {
        if from >= to || to.checked_mul(1000).is_none() {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self {
            id,
            currency,
            from_unix_seconds: from,
            to_unix_seconds: to,
        })
    }
    /// Returns the exact listing ID.
    #[must_use]
    pub const fn id(&self) -> &Identifier {
        &self.id
    }
    /// Returns the currency of all three retained series.
    #[must_use]
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }
    /// Returns inclusive starting Unix seconds supplied to the provider.
    #[must_use]
    pub const fn from_unix_seconds(&self) -> u64 {
        self.from_unix_seconds
    }
    /// Returns inclusive ending Unix seconds supplied to the provider.
    #[must_use]
    pub const fn to_unix_seconds(&self) -> u64 {
        self.to_unix_seconds
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryFields {
    id: Identifier,
    currency: Currency,
    from_unix_seconds: u64,
    to_unix_seconds: u64,
}
impl TryFrom<HistoryFields> for HistoryRequest {
    type Error = Error;
    fn try_from(v: HistoryFields) -> Result<Self, Error> {
        Self::new(v.id, v.currency, v.from_unix_seconds, v.to_unix_seconds)
    }
}
