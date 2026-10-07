// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::market::{Identifier, Label},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};
/// Stablecoin catalogue identity, distinct from symbol, contract and peg currency.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct StablecoinId(u32);
impl StablecoinId {
    /// Constructs an explicit nonzero provider stablecoin ID.
    /// # Errors
    /// Rejects zero, which does not identify a catalogue asset.
    pub fn new(value: u32) -> Result<Self, Error> {
        if value == 0 {
            return Err(ValidationError::InvalidMarketIdentity.into());
        }
        Ok(Self(value))
    }
    /// Returns the provider's numeric identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl TryFrom<u32> for StablecoinId {
    type Error = Error;
    fn try_from(v: u32) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<StablecoinId> for u32 {
    fn from(v: StablecoinId) -> Self {
        v.0
    }
}
/// Scope of the provider's aggregate TVL history, without an inferred chain tip.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scope", content = "chain", rename_all = "snake_case")]
pub enum TvlScope {
    /// Aggregate TVL excluding the provider's liquid staking/double-counted TVL.
    All,
    /// TVL for an explicit provider chain label, under the same methodology.
    Chain(Label),
}
/// An explicit stablecoin history scope; omitted filters mean provider aggregates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StablecoinScope {
    chain: Option<Label>,
    asset: Option<StablecoinId>,
}
impl StablecoinScope {
    /// Records explicit optional chain and stablecoin-ID filters.
    #[must_use]
    pub const fn new(chain: Option<Label>, asset: Option<StablecoinId>) -> Self {
        Self { chain, asset }
    }
    /// Returns the explicit chain filter; `None` queries `all`.
    #[must_use]
    pub const fn chain(&self) -> &Option<Label> {
        &self.chain
    }
    /// Returns the explicit asset filter; `None` retains the aggregate.
    #[must_use]
    pub const fn asset(&self) -> Option<StablecoinId> {
        self.asset
    }
}
/// Supported documented USD analytics metric; values are not combined across metrics.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsMetric {
    /// DEX daily trading volume (`dailyVolume`).
    DexVolume,
    /// Protocol daily fees (`dailyFees`).
    Fees,
    /// Protocol daily revenue (`dailyRevenue`).
    Revenue,
    /// Daily revenue accruing to token holders (`dailyHoldersRevenue`).
    HoldersRevenue,
}
/// One explicit protocol's typed analytics summary and historical daily values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsRequest {
    protocol: Identifier,
    metric: AnalyticsMetric,
}
impl AnalyticsRequest {
    /// Records the exact protocol slug and requested metric.
    #[must_use]
    pub const fn new(protocol: Identifier, metric: AnalyticsMetric) -> Self {
        Self { protocol, metric }
    }
    /// Returns the requested protocol slug.
    #[must_use]
    pub const fn protocol(&self) -> &Identifier {
        &self.protocol
    }
    /// Returns the exact requested metric and corresponding endpoint semantics.
    #[must_use]
    pub const fn metric(&self) -> AnalyticsMetric {
        self.metric
    }
}
