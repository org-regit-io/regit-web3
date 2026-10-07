// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure `CoinGecko` provider identities, requests and exact market data.
//!
//! Coin IDs identify `CoinGecko` listings. Symbols are display metadata and are
//! never used to resolve identities. Quote currencies remain explicit.

mod request;
mod value;
pub use request::{Currency, HistoryRequest, MarketsRequest, PricesRequest, SearchQuery};
pub use value::{
    ChartPoint, CoinMarket, HistoricalChart, MarketValues, MarketsPage, PageStatus,
    PriceAvailability, PriceQuote, Prices, SearchCoin, SearchResults,
};
/// A case-sensitive `CoinGecko` listing ID, separate from symbol and chain identity.
pub type CoinId = super::market::Identifier;
