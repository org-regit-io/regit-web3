// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! `CoinGecko` search, exact prices, explicit markets pages and historical charts.
//!
//! The pure capability requires no transport or runtime. `coingecko-http` adds
//! a bounded outgoing reader with explicit Demo/Pro header credentials and API
//! base configuration. Provider plan restrictions are never silently bypassed.

use crate::{
    domain::{
        coingecko::{
            HistoricalChart, HistoryRequest, MarketsPage, MarketsRequest, Prices, PricesRequest,
            SearchQuery, SearchResults,
        },
        market::Observation,
    },
    error::Error,
};
use std::future::Future;
#[cfg(feature = "coingecko-http")]
mod http;
#[cfg(feature = "coingecko-http")]
mod wire;
#[cfg(feature = "coingecko-http")]
pub use http::{ApiTier, CoinGeckoClient, CoinGeckoHttpConfig};

/// Replaceable reader of typed `CoinGecko` provider data.
/// Callers own endpoint selection, credentials and runtime where applicable.
pub trait CoinGeckoReader {
    /// Searches without selecting a candidate or treating a symbol as an identity.
    fn search(
        &self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<Observation<SearchResults>, Error>> + crate::future::MaybeSend;
    /// Returns every requested ID/currency pair with explicit missing/null values.
    fn prices(
        &self,
        request: PricesRequest,
    ) -> impl Future<Output = Result<Observation<Prices>, Error>> + crate::future::MaybeSend;
    /// Returns one explicitly requested market page, without implicit pagination.
    fn markets(
        &self,
        request: MarketsRequest,
    ) -> impl Future<Output = Result<Observation<MarketsPage>, Error>> + crate::future::MaybeSend;
    /// Returns exact historical price, capitalization and volume series.
    fn history(
        &self,
        request: HistoryRequest,
    ) -> impl Future<Output = Result<Observation<HistoricalChart>, Error>> + crate::future::MaybeSend;
}
