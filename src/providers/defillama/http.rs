// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_time::{SystemTime, UNIX_EPOCH};

use super::{DefiLlamaReader, wire};
use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        defillama::{
            Analytics, AnalyticsMetric, AnalyticsRequest, ProtocolHistory, ProtocolTvl, ProviderId,
            StablecoinHistory, StablecoinScope, Stablecoins, TvlHistory, TvlScope, YieldHistory,
            YieldPools,
        },
        market::{ItemLimit, Observation},
    },
    error::Error,
    transport::{HttpClient, OperationBudget},
};
use std::fmt;
/// Explicit independent TVL/analytics, yields and stablecoin HTTP configurations.
/// Caller bases may target dedicated public hosts or corresponding authenticated
/// proxies; credentials and any path prefixes are supplied explicitly per base.
/// No endpoints, credentials or host fallbacks are inferred.
#[derive(Clone, Debug)]
pub struct DefiLlamaHttpConfig {
    tvl: HttpConfig,
    yields: HttpConfig,
    stablecoins: HttpConfig,
    items: ItemLimit,
}
impl DefiLlamaHttpConfig {
    /// Records validated independent source configurations and collection capacity.
    /// The current provider pool/protocol responses can approach the supported
    /// 16 MiB body ceiling; growth beyond caller byte/item limits fails explicitly.
    #[must_use]
    pub const fn new(
        tvl: HttpConfig,
        yields: HttpConfig,
        stablecoins: HttpConfig,
        items: ItemLimit,
    ) -> Self {
        Self {
            tvl,
            yields,
            stablecoins,
            items,
        }
    }
    /// Returns the explicit TVL and analytics configuration.
    #[must_use]
    pub const fn tvl(&self) -> &HttpConfig {
        &self.tvl
    }
    /// Returns the independently configured yield source.
    #[must_use]
    pub const fn yields(&self) -> &HttpConfig {
        &self.yields
    }
    /// Returns the independently configured stablecoin source.
    #[must_use]
    pub const fn stablecoins(&self) -> &HttpConfig {
        &self.stablecoins
    }
    /// Returns the caller's collection bound; responses are never truncated.
    #[must_use]
    pub const fn item_limit(&self) -> ItemLimit {
        self.items
    }
}
/// Bounded outgoing reader of exact attributed provider observations.
/// Each read uses its selected source's explicit limits and one total deadline.
/// On native targets, the caller supplies a Tokio runtime with networking and time enabled.
pub struct DefiLlamaClient {
    config: DefiLlamaHttpConfig,
    tvl: HttpClient,
    yields: HttpClient,
    stablecoins: HttpClient,
}
impl fmt::Debug for DefiLlamaClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefiLlamaClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl DefiLlamaClient {
    /// Constructs the independent bounded readers without making requests.
    /// # Errors
    /// Returns a fixed configuration failure if an HTTP client cannot build.
    pub fn new(config: DefiLlamaHttpConfig) -> Result<Self, Error> {
        Ok(Self {
            tvl: HttpClient::new(config.tvl())?,
            yields: HttpClient::new(config.yields())?,
            stablecoins: HttpClient::new(config.stablecoins())?,
            config,
        })
    }
    /// Returns source configurations with redacted endpoints and credentials.
    #[must_use]
    pub const fn config(&self) -> &DefiLlamaHttpConfig {
        &self.config
    }
    fn observation<T>(http: &HttpConfig, method: &str, value: T) -> Result<Observation<T>, Error> {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        Ok(Observation::new(
            value,
            Source::new(http.provider_id(), method, env!("CARGO_PKG_VERSION"))?,
            Timestamp::from_unix_seconds(seconds),
        ))
    }
    async fn read(
        http: &HttpClient,
        budget: &OperationBudget,
        path: &[&str],
        query: &[(&str, &str)],
        resource: bool,
    ) -> Result<Vec<u8>, Error> {
        let response = http.read(path, query, None, budget).await?;
        if resource && response.status.as_u16() == 404 {
            return Err(Error::UnavailableData);
        }
        response.into_success()
    }
    /// Reads one protocol's exact current TVL valuation in USD.
    /// No provider data timestamp exists in this scalar response.
    /// # Errors
    /// Returns fixed provider, unavailable-data, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn protocol_tvl(
        &self,
        protocol: ProviderId,
    ) -> Result<Observation<ProtocolTvl>, Error> {
        let budget = OperationBudget::new(self.config.tvl.limits())?;
        budget
            .run(async {
                let bytes =
                    Self::read(&self.tvl, &budget, &["tvl", protocol.as_str()], &[], true).await?;
                Self::observation(
                    &self.config.tvl,
                    "tvl",
                    wire::protocol_tvl(&bytes, protocol)?,
                )
            })
            .await
    }
    /// Reads total USD TVL history and current provider-labelled USD components.
    /// Overlapping reporting components are retained and never summed.
    /// # Errors
    /// Returns fixed provider, unavailable-data, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn protocol_history(
        &self,
        protocol: ProviderId,
    ) -> Result<Observation<ProtocolHistory>, Error> {
        let budget = OperationBudget::new(self.config.tvl.limits())?;
        budget
            .run(async {
                let bytes = Self::read(
                    &self.tvl,
                    &budget,
                    &["protocol", protocol.as_str()],
                    &[],
                    true,
                )
                .await?;
                Self::observation(
                    &self.config.tvl,
                    "protocol",
                    wire::protocol_history(&bytes, protocol, self.config.items)?,
                )
            })
            .await
    }
    /// Reads aggregate/chain-labelled USD TVL excluding provider-defined liquid
    /// staking and double-counted TVL, without establishing an on-chain snapshot.
    /// # Errors
    /// Returns fixed provider, unavailable-data, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn tvl_history(&self, scope: TvlScope) -> Result<Observation<TvlHistory>, Error> {
        let budget = OperationBudget::new(self.config.tvl.limits())?;
        budget
            .run(async {
                let mut path = vec!["v2", "historicalChainTvl"];
                if let TvlScope::Chain(chain) = &scope {
                    path.push(chain.as_str());
                }
                let bytes = Self::read(&self.tvl, &budget, &path, &[], true).await?;
                Self::observation(
                    &self.config.tvl,
                    "historical-chain-tvl",
                    wire::tvl_history(&bytes, scope, self.config.items)?,
                )
            })
            .await
    }
    /// Reads every returned pool with exact USD TVL and signed percent APYs.
    /// Missing APY components remain absent. Large catalogues fail at explicit
    /// byte/item bounds instead of returning a silently shortened catalogue.
    /// # Errors
    /// Returns fixed bounded provider, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn yield_pools(&self) -> Result<Observation<YieldPools>, Error> {
        let budget = OperationBudget::new(self.config.yields.limits())?;
        budget
            .run(async {
                let bytes = Self::read(&self.yields, &budget, &["pools"], &[], false).await?;
                Self::observation(
                    &self.config.yields,
                    "pools",
                    wire::yield_pools(&bytes, self.config.items)?,
                )
            })
            .await
    }
    /// Reads one explicit pool's provider UTC timestamps, USD TVL and percent APYs.
    /// # Errors
    /// Returns fixed provider, unavailable-data, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn yield_history(
        &self,
        pool: ProviderId,
    ) -> Result<Observation<YieldHistory>, Error> {
        let budget = OperationBudget::new(self.config.yields.limits())?;
        budget
            .run(async {
                let bytes =
                    Self::read(&self.yields, &budget, &["chart", pool.as_str()], &[], true).await?;
                Self::observation(
                    &self.config.yields,
                    "yield-chart",
                    wire::yield_history(&bytes, pool, self.config.items)?,
                )
            })
            .await
    }
    /// Reads catalogue peg-unit circulating amounts and separately reported USD
    /// prices. Null prices are never substituted by an assumed successful peg.
    /// # Errors
    /// Returns fixed bounded provider, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn stablecoins(&self) -> Result<Observation<Stablecoins>, Error> {
        let budget = OperationBudget::new(self.config.stablecoins.limits())?;
        budget
            .run(async {
                let bytes = Self::read(
                    &self.stablecoins,
                    &budget,
                    &["stablecoins"],
                    &[("includePrices", "true")],
                    false,
                )
                .await?;
                Self::observation(
                    &self.config.stablecoins,
                    "stablecoins",
                    wire::stablecoins(&bytes, self.config.items)?,
                )
            })
            .await
    }
    /// Reads an explicit aggregate/chain/asset historical scope with independent
    /// peg-denominated circulation and USD valuations at provider Unix seconds.
    /// # Errors
    /// Returns fixed provider, unavailable-data, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn stablecoin_history(
        &self,
        scope: StablecoinScope,
    ) -> Result<Observation<StablecoinHistory>, Error> {
        let budget = OperationBudget::new(self.config.stablecoins.limits())?;
        budget
            .run(async {
                let chain = scope
                    .chain()
                    .as_ref()
                    .map_or("all", crate::domain::market::Label::as_str);
                let asset = scope.asset().map(|id| id.get().to_string());
                let query = asset
                    .as_ref()
                    .map(|id| vec![("stablecoin", id.as_str())])
                    .unwrap_or_default();
                let bytes = Self::read(
                    &self.stablecoins,
                    &budget,
                    &["stablecoincharts", chain],
                    &query,
                    true,
                )
                .await?;
                Self::observation(
                    &self.config.stablecoins,
                    "stablecoin-charts",
                    wire::stablecoin_history(&bytes, scope, self.config.items)?,
                )
            })
            .await
    }
    /// Reads one protocol's exact USD DEX-volume, fee or revenue summary with
    /// distinct aggregation horizons and explicitly retained historical daily data.
    /// # Errors
    /// Returns fixed provider, unavailable-data, configuration and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn analytics(
        &self,
        request: AnalyticsRequest,
    ) -> Result<Observation<Analytics>, Error> {
        let budget = OperationBudget::new(self.config.tvl.limits())?;
        budget
            .run(async {
                let (module, metric) = match request.metric() {
                    AnalyticsMetric::DexVolume => ("dexs", "dailyVolume"),
                    AnalyticsMetric::Fees => ("fees", "dailyFees"),
                    AnalyticsMetric::Revenue => ("fees", "dailyRevenue"),
                    AnalyticsMetric::HoldersRevenue => ("fees", "dailyHoldersRevenue"),
                };
                let bytes = Self::read(
                    &self.tvl,
                    &budget,
                    &["summary", module, request.protocol().as_str()],
                    &[
                        ("excludeTotalDataChart", "false"),
                        ("excludeTotalDataChartBreakdown", "true"),
                        ("dataType", metric),
                    ],
                    true,
                )
                .await?;
                Self::observation(
                    &self.config.tvl,
                    "analytics-summary",
                    wire::analytics(&bytes, request, self.config.items)?,
                )
            })
            .await
    }
}
impl DefiLlamaReader for DefiLlamaClient {
    async fn protocol_tvl(&self, p: ProviderId) -> Result<Observation<ProtocolTvl>, Error> {
        Self::protocol_tvl(self, p).await
    }
    async fn protocol_history(&self, p: ProviderId) -> Result<Observation<ProtocolHistory>, Error> {
        Self::protocol_history(self, p).await
    }
    async fn tvl_history(&self, p: TvlScope) -> Result<Observation<TvlHistory>, Error> {
        Self::tvl_history(self, p).await
    }
    async fn yield_pools(&self) -> Result<Observation<YieldPools>, Error> {
        Self::yield_pools(self).await
    }
    async fn yield_history(&self, p: ProviderId) -> Result<Observation<YieldHistory>, Error> {
        Self::yield_history(self, p).await
    }
    async fn stablecoins(&self) -> Result<Observation<Stablecoins>, Error> {
        Self::stablecoins(self).await
    }
    async fn stablecoin_history(
        &self,
        p: StablecoinScope,
    ) -> Result<Observation<StablecoinHistory>, Error> {
        Self::stablecoin_history(self, p).await
    }
    async fn analytics(&self, p: AnalyticsRequest) -> Result<Observation<Analytics>, Error> {
        Self::analytics(self, p).await
    }
}
