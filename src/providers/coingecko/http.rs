// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_time::{SystemTime, UNIX_EPOCH};

use super::{CoinGeckoReader, wire};
use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        coingecko::{
            HistoricalChart, HistoryRequest, MarketsPage, MarketsRequest, Prices, PricesRequest,
            SearchQuery, SearchResults,
        },
        market::{ItemLimit, Observation},
    },
    error::Error,
    transport::{HttpClient, OperationBudget},
};
use std::fmt;

/// Explicit `CoinGecko` API credential header variant; no base URL is inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiTier {
    /// Uses the documented `x-cg-demo-api-key` header.
    Demo,
    /// Uses the documented `x-cg-pro-api-key` header.
    Pro,
}
/// Caller-supplied API base, header credential and collection capacity.
/// The base URL includes `/api/v3`. No environment, URL or plan is assumed.
#[derive(Clone, Debug)]
pub struct CoinGeckoHttpConfig {
    http: HttpConfig,
    tier: Option<ApiTier>,
    items: ItemLimit,
}
impl CoinGeckoHttpConfig {
    /// Adds the explicit Demo/Pro credential as a sensitive header.
    /// # Errors
    /// Rejects an empty credential or invalid header/configuration.
    pub fn new(
        http: &HttpConfig,
        tier: ApiTier,
        api_key: &str,
        items: ItemLimit,
    ) -> Result<Self, Error> {
        if api_key.is_empty() || has_provider_credentials(http) {
            return Err(Error::Configuration);
        }
        let name = match tier {
            ApiTier::Demo => "x-cg-demo-api-key",
            ApiTier::Pro => "x-cg-pro-api-key",
        };
        let endpoint = http.endpoint().clone().with_header(name, api_key)?;
        Ok(Self {
            http: HttpConfig::new(endpoint, http.limits(), http.provider_id())?,
            tier: Some(tier),
            items,
        })
    }
    /// Selects anonymous public API access without injecting a provider key.
    ///
    /// Public access and restrictions depend on the configured source. This
    /// method does not retry with credentials or switch endpoints implicitly.
    ///
    /// # Errors
    /// Rejects preexisting Demo/Pro credential headers or query parameters.
    pub fn anonymous(http: HttpConfig, items: ItemLimit) -> Result<Self, Error> {
        if has_provider_credentials(&http) {
            return Err(Error::Configuration);
        }
        Ok(Self {
            http,
            tier: None,
            items,
        })
    }
    /// Returns redacted explicit endpoint, limits and provider attribution.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
    /// Returns the selected credential tier, or `None` for explicit public access.
    #[must_use]
    pub const fn tier(&self) -> Option<ApiTier> {
        self.tier
    }
    /// Returns the caller's maximum records per collection; exceeding it fails.
    #[must_use]
    pub const fn item_limit(&self) -> ItemLimit {
        self.items
    }
}

fn has_provider_credentials(http: &HttpConfig) -> bool {
    let endpoint = http.endpoint();
    endpoint.headers.contains_key("x-cg-demo-api-key")
        || endpoint.headers.contains_key("x-cg-pro-api-key")
        || endpoint
            .url
            .query_pairs()
            .any(|(name, _)| matches!(name.as_ref(), "x_cg_demo_api_key" | "x_cg_pro_api_key"))
}
/// Bounded `CoinGecko` reader of typed data, without automatic pagination.
/// Reads use one operation deadline across retries, body reads and decoding.
/// On native targets, the caller supplies a Tokio runtime with networking and time enabled.
pub struct CoinGeckoClient {
    config: CoinGeckoHttpConfig,
    http: HttpClient,
}
impl fmt::Debug for CoinGeckoClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoinGeckoClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl CoinGeckoClient {
    /// Constructs a reader from explicit settings; no request is made at construction.
    /// # Errors
    /// Returns fixed configuration failures if the outgoing HTTP client cannot build.
    pub fn new(config: CoinGeckoHttpConfig) -> Result<Self, Error> {
        let http = HttpClient::new(config.http_config())?;
        Ok(Self { config, http })
    }
    /// Returns validated configuration with redacted endpoint and credentials.
    #[must_use]
    pub const fn config(&self) -> &CoinGeckoHttpConfig {
        &self.config
    }
    fn observation<T>(&self, value: T, method: &str) -> Result<Observation<T>, Error> {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        Ok(Observation::new(
            value,
            Source::new(
                self.config.http.provider_id(),
                method,
                env!("CARGO_PKG_VERSION"),
            )?,
            Timestamp::from_unix_seconds(seconds),
        ))
    }
    /// Searches listings without choosing an identity from their symbols.
    /// # Errors
    /// Returns bounded fixed provider, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn search(&self, query: SearchQuery) -> Result<Observation<SearchResults>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                let bytes = self
                    .http
                    .read(&["search"], &[("query", query.as_str())], None, &budget)
                    .await?
                    .into_success()?;
                self.observation(wire::search(&bytes, query, self.config.items)?, "search")
            })
            .await
    }
    /// Reads exact prices for every requested ID/currency pair.
    /// Missing IDs/currencies and explicit null prices stay distinct; data time is
    /// the provider's `last_updated_at`, separate from retrieval time.
    /// # Errors
    /// Returns bounded fixed provider, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn prices(&self, request: PricesRequest) -> Result<Observation<Prices>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                let ids = request
                    .ids()
                    .iter()
                    .map(crate::domain::market::Identifier::as_str)
                    .collect::<Vec<_>>()
                    .join(",");
                let currencies = request
                    .currencies()
                    .iter()
                    .map(crate::domain::coingecko::Currency::as_str)
                    .collect::<Vec<_>>()
                    .join(",");
                let bytes = self
                    .http
                    .read(
                        &["simple", "price"],
                        &[
                            ("ids", &ids),
                            ("vs_currencies", &currencies),
                            ("include_last_updated_at", "true"),
                            ("precision", "full"),
                        ],
                        None,
                        &budget,
                    )
                    .await?
                    .into_success()?;
                self.observation(
                    wire::prices(&bytes, request, self.config.items)?,
                    "simple-price",
                )
            })
            .await
    }
    /// Reads one explicit market page in market-cap-descending order.
    /// A full page reports possible additional results; no next page is gathered.
    /// # Errors
    /// Returns bounded fixed provider, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn markets(
        &self,
        request: MarketsRequest,
    ) -> Result<Observation<MarketsPage>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                let page = request.page().to_string();
                let count = request.count().to_string();
                let ids = request.ids().map(|ids| {
                    ids.iter()
                        .map(crate::domain::market::Identifier::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                });
                let mut query = vec![
                    ("vs_currency", request.currency().as_str()),
                    ("page", &page),
                    ("per_page", &count),
                    ("order", "market_cap_desc"),
                    ("sparkline", "false"),
                    ("precision", "full"),
                ];
                if let Some(ids) = &ids {
                    query.push(("ids", ids));
                }
                let bytes = self
                    .http
                    .read(&["coins", "markets"], &query, None, &budget)
                    .await?
                    .into_success()?;
                self.observation(
                    wire::markets(&bytes, request, self.config.items)?,
                    "coins-markets",
                )
            })
            .await
    }
    /// Reads independent exact historical prices, capitalization and volume series.
    /// The provider selects sampling granularity; plan restrictions fail explicitly.
    /// # Errors
    /// Returns bounded fixed provider, unavailable-data, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn history(
        &self,
        request: HistoryRequest,
    ) -> Result<Observation<HistoricalChart>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                let from = request.from_unix_seconds().to_string();
                let to = request.to_unix_seconds().to_string();
                let response = self
                    .http
                    .read(
                        &["coins", request.id().as_str(), "market_chart", "range"],
                        &[
                            ("vs_currency", request.currency().as_str()),
                            ("from", &from),
                            ("to", &to),
                            ("precision", "full"),
                        ],
                        None,
                        &budget,
                    )
                    .await?;
                if response.status.as_u16() == 404 {
                    return Err(Error::UnavailableData);
                }
                self.observation(
                    wire::history(&response.into_success()?, request, self.config.items)?,
                    "market-chart-range",
                )
            })
            .await
    }
}
impl CoinGeckoReader for CoinGeckoClient {
    async fn search(&self, v: SearchQuery) -> Result<Observation<SearchResults>, Error> {
        Self::search(self, v).await
    }
    async fn prices(&self, v: PricesRequest) -> Result<Observation<Prices>, Error> {
        Self::prices(self, v).await
    }
    async fn markets(&self, v: MarketsRequest) -> Result<Observation<MarketsPage>, Error> {
        Self::markets(self, v).await
    }
    async fn history(&self, v: HistoryRequest) -> Result<Observation<HistoricalChart>, Error> {
        Self::history(self, v).await
    }
}
