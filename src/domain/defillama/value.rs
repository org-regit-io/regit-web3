// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{AnalyticsRequest, StablecoinId, StablecoinScope, TvlScope};
use crate::{
    domain::{
        ExactDecimal, Timestamp,
        market::{Identifier, Label, NonnegativeDecimal, UtcDateTime, unique},
    },
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// Current provider USD TVL for an explicitly requested protocol.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolTvl {
    protocol: Identifier,
    usd: NonnegativeDecimal,
}
impl ProtocolTvl {
    /// Records explicitly supplied, validated field values.
    #[must_use]
    pub const fn new(protocol: Identifier, usd: NonnegativeDecimal) -> Self {
        Self { protocol, usd }
    }
    /// Requested protocol slug.
    #[must_use]
    pub const fn protocol(&self) -> &Identifier {
        &self.protocol
    }
    /// Exact TVL valuation in USD, without a supplied data timestamp.
    #[must_use]
    pub const fn usd(&self) -> &NonnegativeDecimal {
        &self.usd
    }
}

/// One exact USD-valued sample at an explicit provider Unix-second data time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsdPoint {
    date: Timestamp,
    usd: NonnegativeDecimal,
}
impl UsdPoint {
    /// Records explicitly supplied, validated field values.
    #[must_use]
    pub const fn new(date: Timestamp, usd: NonnegativeDecimal) -> Self {
        Self { date, usd }
    }
    /// Provider data time in Unix seconds.
    #[must_use]
    pub const fn date(&self) -> &Timestamp {
        &self.date
    }
    /// Exact sampled USD value; the enclosing operation defines its metric.
    #[must_use]
    pub const fn usd(&self) -> &NonnegativeDecimal {
        &self.usd
    }
}

/// One labelled provider reporting component; labels may include staking/borrowed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BreakdownValue {
    label: Label,
    value: NonnegativeDecimal,
}
impl BreakdownValue {
    /// Records explicitly supplied, validated field values.
    #[must_use]
    pub const fn new(label: Label, value: NonnegativeDecimal) -> Self {
        Self { label, value }
    }
    /// Provider reporting key, without inferred chain semantics.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }
    /// Exact value in the enclosing record unit.
    #[must_use]
    pub const fn value(&self) -> &NonnegativeDecimal {
        &self.value
    }
}

/// Exact percentage APYs, with explicit missing components and signed values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YieldRates {
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    apy: Option<ExactDecimal>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    base: Option<ExactDecimal>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    reward: Option<ExactDecimal>,
}
impl YieldRates {
    /// Records explicitly supplied, validated field values.
    #[must_use]
    pub const fn new(
        apy: Option<ExactDecimal>,
        base: Option<ExactDecimal>,
        reward: Option<ExactDecimal>,
    ) -> Self {
        Self { apy, base, reward }
    }
    /// Total APY in percent, not a fractional multiplier.
    #[must_use]
    pub const fn apy(&self) -> &Option<ExactDecimal> {
        &self.apy
    }
    /// Base APY in percent, if reported.
    #[must_use]
    pub const fn base(&self) -> &Option<ExactDecimal> {
        &self.base
    }
    /// Reward APY in percent, if reported.
    #[must_use]
    pub const fn reward(&self) -> &Option<ExactDecimal> {
        &self.reward
    }
}

/// One provider yield pool with exact USD TVL and percentage APYs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YieldPool {
    pool: Identifier,
    chain: Label,
    project: Identifier,
    symbol: YieldSymbol,
    tvl_usd: NonnegativeDecimal,
    rates: YieldRates,
}
impl YieldPool {
    /// Records explicitly supplied, validated field values.
    #[must_use]
    pub const fn new(
        pool: Identifier,
        chain: Label,
        project: Identifier,
        symbol: YieldSymbol,
        tvl_usd: NonnegativeDecimal,
        rates: YieldRates,
    ) -> Self {
        Self {
            pool,
            chain,
            project,
            symbol,
            tvl_usd,
            rates,
        }
    }
    /// Provider pool ID, not necessarily an on-chain account.
    #[must_use]
    pub const fn pool(&self) -> &Identifier {
        &self.pool
    }
    /// Provider chain label.
    #[must_use]
    pub const fn chain(&self) -> &Label {
        &self.chain
    }
    /// Provider project slug.
    #[must_use]
    pub const fn project(&self) -> &Identifier {
        &self.project
    }
    /// Provider display token symbols.
    #[must_use]
    pub const fn symbol(&self) -> &YieldSymbol {
        &self.symbol
    }
    /// Exact pool TVL in USD.
    #[must_use]
    pub const fn tvl_usd(&self) -> &NonnegativeDecimal {
        &self.tvl_usd
    }
    /// Exact signed APY percentages with missing components.
    #[must_use]
    pub const fn rates(&self) -> &YieldRates {
        &self.rates
    }
}

/// One provider yield history sample with an explicit UTC data time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YieldPoint {
    date: UtcDateTime,
    tvl_usd: NonnegativeDecimal,
    rates: YieldRates,
}
impl YieldPoint {
    /// Records explicitly supplied, validated field values.
    #[must_use]
    pub const fn new(date: UtcDateTime, tvl_usd: NonnegativeDecimal, rates: YieldRates) -> Self {
        Self {
            date,
            tvl_usd,
            rates,
        }
    }
    /// Provider UTC data timestamp, preserving fractional precision.
    #[must_use]
    pub const fn date(&self) -> &UtcDateTime {
        &self.date
    }
    /// Exact USD TVL.
    #[must_use]
    pub const fn tvl_usd(&self) -> &NonnegativeDecimal {
        &self.tvl_usd
    }
    /// Exact APY percentages; null components remain absent.
    #[must_use]
    pub const fn rates(&self) -> &YieldRates {
        &self.rates
    }
}

/// Exact USD provider totals at distinct reported aggregation horizons.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsTotals {
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    day: Option<NonnegativeDecimal>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    week: Option<NonnegativeDecimal>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    all_time: Option<NonnegativeDecimal>,
}
impl AnalyticsTotals {
    /// Records explicitly supplied, validated field values.
    #[must_use]
    pub const fn new(
        day: Option<NonnegativeDecimal>,
        week: Option<NonnegativeDecimal>,
        all_time: Option<NonnegativeDecimal>,
    ) -> Self {
        Self {
            day,
            week,
            all_time,
        }
    }
    /// Reported 24-hour USD aggregate, if available.
    #[must_use]
    pub const fn day(&self) -> &Option<NonnegativeDecimal> {
        &self.day
    }
    /// Reported seven-day USD aggregate, if available.
    #[must_use]
    pub const fn week(&self) -> &Option<NonnegativeDecimal> {
        &self.week
    }
    /// Reported all-time USD aggregate, if available.
    #[must_use]
    pub const fn all_time(&self) -> &Option<NonnegativeDecimal> {
        &self.all_time
    }
}

fn bounded(count: usize) -> Result<(), Error> {
    if count > 100_000 {
        Err(ValidationError::InvalidMarketRecord.into())
    } else {
        Ok(())
    }
}
fn ordered(points: &[UsdPoint]) -> Result<(), Error> {
    bounded(points.len())?;
    if points.windows(2).any(|p| p[0].date() >= p[1].date()) {
        return Err(ValidationError::InvalidMarketRecord.into());
    }
    Ok(())
}
fn breakdowns(values: &[BreakdownValue]) -> Result<(), Error> {
    bounded(values.len())?;
    if !unique(values.iter().map(|v| v.label().as_str())) {
        return Err(ValidationError::InvalidMarketRecord.into());
    }
    Ok(())
}

/// Protocol identity, total USD TVL history and current labelled USD breakdowns.
/// Reporting labels may include overlapping categories; they are never summed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ProtocolFields")]
pub struct ProtocolHistory {
    protocol: Identifier,
    provider_id: Label,
    name: Label,
    points: Vec<UsdPoint>,
    current_breakdowns: Vec<BreakdownValue>,
}
impl ProtocolHistory {
    /// Requires strictly ordered history and unique current reporting labels.
    /// # Errors
    /// Rejects excessive/duplicate reporting components or duplicate/reordered dates.
    pub fn new(
        protocol: Identifier,
        provider_id: Label,
        name: Label,
        points: Vec<UsdPoint>,
        current_breakdowns: Vec<BreakdownValue>,
    ) -> Result<Self, Error> {
        ordered(&points)?;
        breakdowns(&current_breakdowns)?;
        Ok(Self {
            protocol,
            provider_id,
            name,
            points,
            current_breakdowns,
        })
    }
    /// Returns the explicitly requested protocol slug.
    #[must_use]
    pub const fn protocol(&self) -> &Identifier {
        &self.protocol
    }
    /// Returns the provider ID, which can differ from the requested slug.
    #[must_use]
    pub const fn provider_id(&self) -> &Label {
        &self.provider_id
    }
    /// Returns separate display metadata.
    #[must_use]
    pub const fn name(&self) -> &Label {
        &self.name
    }
    /// Returns total exact USD TVL samples, without token breakdown data.
    #[must_use]
    pub fn points(&self) -> &[UsdPoint] {
        &self.points
    }
    /// Returns current provider reporting components in USD, without summing them.
    #[must_use]
    pub fn current_breakdowns(&self) -> &[BreakdownValue] {
        &self.current_breakdowns
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtocolFields {
    protocol: Identifier,
    provider_id: Label,
    name: Label,
    points: Vec<UsdPoint>,
    current_breakdowns: Vec<BreakdownValue>,
}
impl TryFrom<ProtocolFields> for ProtocolHistory {
    type Error = Error;
    fn try_from(v: ProtocolFields) -> Result<Self, Error> {
        Self::new(
            v.protocol,
            v.provider_id,
            v.name,
            v.points,
            v.current_breakdowns,
        )
    }
}

/// Provider aggregate or chain-specific USD TVL history under the selected scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TvlFields")]
pub struct TvlHistory {
    scope: TvlScope,
    points: Vec<UsdPoint>,
}
impl TvlHistory {
    /// Requires bounded, strictly increasing provider data times.
    /// # Errors
    /// Rejects excessive, duplicate or reordered samples.
    pub fn new(scope: TvlScope, points: Vec<UsdPoint>) -> Result<Self, Error> {
        ordered(&points)?;
        Ok(Self { scope, points })
    }
    /// Returns the exact selected aggregate/chain methodology scope.
    #[must_use]
    pub const fn scope(&self) -> &TvlScope {
        &self.scope
    }
    /// Returns provider data times and exact USD TVL values.
    #[must_use]
    pub fn points(&self) -> &[UsdPoint] {
        &self.points
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TvlFields {
    scope: TvlScope,
    points: Vec<UsdPoint>,
}
impl TryFrom<TvlFields> for TvlHistory {
    type Error = Error;
    fn try_from(v: TvlFields) -> Result<Self, Error> {
        Self::new(v.scope, v.points)
    }
}

/// A bounded yield catalogue; complete response data is never silently truncated.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "YieldPoolsFields")]
pub struct YieldPools {
    pools: Vec<YieldPool>,
}
impl YieldPools {
    /// Requires bounded pool records without duplicate provider IDs.
    /// # Errors
    /// Rejects excessive or repeated pools.
    pub fn new(pools: Vec<YieldPool>) -> Result<Self, Error> {
        bounded(pools.len())?;
        if !unique(pools.iter().map(YieldPool::pool)) {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self { pools })
    }
    /// Returns every returned pool with independent USD/percent units.
    #[must_use]
    pub fn pools(&self) -> &[YieldPool] {
        &self.pools
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct YieldPoolsFields {
    pools: Vec<YieldPool>,
}
impl TryFrom<YieldPoolsFields> for YieldPools {
    type Error = Error;
    fn try_from(v: YieldPoolsFields) -> Result<Self, Error> {
        Self::new(v.pools)
    }
}
/// Historical APYs and USD TVL for one explicitly selected provider pool.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "YieldHistoryFields")]
pub struct YieldHistory {
    pool: Identifier,
    points: Vec<YieldPoint>,
}
impl YieldHistory {
    /// Requires bounded samples with strictly increasing UTC provider data times.
    /// # Errors
    /// Rejects excessive, duplicate or reordered samples.
    pub fn new(pool: Identifier, points: Vec<YieldPoint>) -> Result<Self, Error> {
        bounded(points.len())?;
        if points
            .windows(2)
            .any(|p| p[0].date().chronological_key() >= p[1].date().chronological_key())
        {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self { pool, points })
    }
    /// Returns the exact requested provider pool ID.
    #[must_use]
    pub const fn pool(&self) -> &Identifier {
        &self.pool
    }
    /// Returns supplied UTC data timestamps, exact USD TVL and percent APYs.
    #[must_use]
    pub fn points(&self) -> &[YieldPoint] {
        &self.points
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct YieldHistoryFields {
    pool: Identifier,
    points: Vec<YieldPoint>,
}
impl TryFrom<YieldHistoryFields> for YieldHistory {
    type Error = Error;
    fn try_from(v: YieldHistoryFields) -> Result<Self, Error> {
        Self::new(v.pool, v.points)
    }
}

/// A catalogue asset with peg-denominated circulation and separately reported USD price.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StablecoinFields")]
pub struct Stablecoin {
    id: StablecoinId,
    name: Label,
    symbol: Label,
    peg_type: Label,
    circulating: Vec<BreakdownValue>,
    price_usd: Option<NonnegativeDecimal>,
    chains: Vec<Label>,
}
impl Stablecoin {
    /// Requires unique peg reporting labels and bounded chain metadata.
    /// # Errors
    /// Rejects duplicate/excessive circulation components or chain labels.
    pub fn new(
        id: StablecoinId,
        name: Label,
        symbol: Label,
        peg_type: Label,
        circulating: Vec<BreakdownValue>,
        price_usd: Option<NonnegativeDecimal>,
        chains: Vec<Label>,
    ) -> Result<Self, Error> {
        breakdowns(&circulating)?;
        bounded(chains.len())?;
        if !unique(chains.iter().map(Label::as_str)) {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self {
            id,
            name,
            symbol,
            peg_type,
            circulating,
            price_usd,
            chains,
        })
    }
    /// Returns provider asset ID, not its display symbol.
    #[must_use]
    pub const fn id(&self) -> StablecoinId {
        self.id
    }
    /// Returns provider display name.
    #[must_use]
    pub const fn name(&self) -> &Label {
        &self.name
    }
    /// Returns provider display symbol.
    #[must_use]
    pub const fn symbol(&self) -> &Label {
        &self.symbol
    }
    /// Returns peg type without assuming a successful one-to-one peg.
    #[must_use]
    pub const fn peg_type(&self) -> &Label {
        &self.peg_type
    }
    /// Returns circulation values in their explicitly labelled peg denominations.
    #[must_use]
    pub fn circulating(&self) -> &[BreakdownValue] {
        &self.circulating
    }
    /// Returns reported USD price; absent/null price never becomes the nominal peg.
    #[must_use]
    pub const fn price_usd(&self) -> &Option<NonnegativeDecimal> {
        &self.price_usd
    }
    /// Returns separate provider chain metadata without verified chain identity.
    #[must_use]
    pub fn chains(&self) -> &[Label] {
        &self.chains
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StablecoinFields {
    id: StablecoinId,
    name: Label,
    symbol: Label,
    peg_type: Label,
    circulating: Vec<BreakdownValue>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    price_usd: Option<NonnegativeDecimal>,
    chains: Vec<Label>,
}
impl TryFrom<StablecoinFields> for Stablecoin {
    type Error = Error;
    fn try_from(v: StablecoinFields) -> Result<Self, Error> {
        Self::new(
            v.id,
            v.name,
            v.symbol,
            v.peg_type,
            v.circulating,
            v.price_usd,
            v.chains,
        )
    }
}
/// A bounded provider stablecoin catalogue without duplicate numeric identities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StablecoinsFields")]
pub struct Stablecoins {
    assets: Vec<Stablecoin>,
}
impl Stablecoins {
    /// Requires bounded unique catalogue entries.
    /// # Errors
    /// Rejects excessive or repeated provider IDs.
    pub fn new(assets: Vec<Stablecoin>) -> Result<Self, Error> {
        bounded(assets.len())?;
        if !unique(assets.iter().map(Stablecoin::id)) {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self { assets })
    }
    /// Returns every returned stablecoin record.
    #[must_use]
    pub fn assets(&self) -> &[Stablecoin] {
        &self.assets
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StablecoinsFields {
    assets: Vec<Stablecoin>,
}
impl TryFrom<StablecoinsFields> for Stablecoins {
    type Error = Error;
    fn try_from(v: StablecoinsFields) -> Result<Self, Error> {
        Self::new(v.assets)
    }
}

/// Stablecoin aggregate data retaining peg-unit circulation and USD valuation maps separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StablecoinPointFields")]
pub struct StablecoinPoint {
    date: Timestamp,
    circulating: Vec<BreakdownValue>,
    circulating_usd: Vec<BreakdownValue>,
}
impl StablecoinPoint {
    /// Requires unique labels in each independent unit map.
    /// # Errors
    /// Rejects excessive or duplicate reporting components.
    pub fn new(
        date: Timestamp,
        circulating: Vec<BreakdownValue>,
        circulating_usd: Vec<BreakdownValue>,
    ) -> Result<Self, Error> {
        breakdowns(&circulating)?;
        breakdowns(&circulating_usd)?;
        Ok(Self {
            date,
            circulating,
            circulating_usd,
        })
    }
    /// Returns provider data time in Unix seconds.
    #[must_use]
    pub const fn date(&self) -> Timestamp {
        self.date
    }
    /// Returns amounts in each explicitly labelled peg denomination.
    #[must_use]
    pub fn circulating(&self) -> &[BreakdownValue] {
        &self.circulating
    }
    /// Returns USD valuations keyed by peg denomination, separately from units.
    #[must_use]
    pub fn circulating_usd(&self) -> &[BreakdownValue] {
        &self.circulating_usd
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StablecoinPointFields {
    date: Timestamp,
    circulating: Vec<BreakdownValue>,
    circulating_usd: Vec<BreakdownValue>,
}
impl TryFrom<StablecoinPointFields> for StablecoinPoint {
    type Error = Error;
    fn try_from(v: StablecoinPointFields) -> Result<Self, Error> {
        Self::new(v.date, v.circulating, v.circulating_usd)
    }
}
/// Explicitly scoped stablecoin circulation/USD historical observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StablecoinHistoryFields")]
pub struct StablecoinHistory {
    scope: StablecoinScope,
    points: Vec<StablecoinPoint>,
}
impl StablecoinHistory {
    /// Requires bounded strictly increasing provider data times.
    /// # Errors
    /// Rejects excessive, duplicate or reordered sample times.
    pub fn new(scope: StablecoinScope, points: Vec<StablecoinPoint>) -> Result<Self, Error> {
        bounded(points.len())?;
        if points.windows(2).any(|p| p[0].date() >= p[1].date()) {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self { scope, points })
    }
    /// Returns exact requested aggregate, chain and stablecoin filters.
    #[must_use]
    pub const fn scope(&self) -> &StablecoinScope {
        &self.scope
    }
    /// Returns each independent peg-unit/USD-valued sample.
    #[must_use]
    pub fn points(&self) -> &[StablecoinPoint] {
        &self.points
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StablecoinHistoryFields {
    scope: StablecoinScope,
    points: Vec<StablecoinPoint>,
}
impl TryFrom<StablecoinHistoryFields> for StablecoinHistory {
    type Error = Error;
    fn try_from(v: StablecoinHistoryFields) -> Result<Self, Error> {
        Self::new(v.scope, v.points)
    }
}
/// One protocol's USD DEX volume, fee or revenue summary and daily history.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AnalyticsFields")]
pub struct Analytics {
    request: AnalyticsRequest,
    name: Label,
    totals: AnalyticsTotals,
    daily: Vec<UsdPoint>,
}
impl Analytics {
    /// Requires a bounded strictly increasing daily USD series.
    /// The metric and request are retained; units and source methodology are not inferred.
    /// # Errors
    /// Rejects excessive, duplicate or reordered daily times.
    pub fn new(
        request: AnalyticsRequest,
        name: Label,
        totals: AnalyticsTotals,
        daily: Vec<UsdPoint>,
    ) -> Result<Self, Error> {
        ordered(&daily)?;
        Ok(Self {
            request,
            name,
            totals,
            daily,
        })
    }
    /// Returns the exact protocol and selected analytics metric.
    #[must_use]
    pub const fn request(&self) -> &AnalyticsRequest {
        &self.request
    }
    /// Returns provider display name independently of requested identity.
    #[must_use]
    pub const fn name(&self) -> &Label {
        &self.name
    }
    /// Returns separately reported USD aggregation horizons.
    #[must_use]
    pub const fn totals(&self) -> &AnalyticsTotals {
        &self.totals
    }
    /// Returns exact daily USD values at provider Unix-second data times.
    #[must_use]
    pub fn daily(&self) -> &[UsdPoint] {
        &self.daily
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyticsFields {
    request: AnalyticsRequest,
    name: Label,
    totals: AnalyticsTotals,
    daily: Vec<UsdPoint>,
}
impl TryFrom<AnalyticsFields> for Analytics {
    type Error = Error;
    fn try_from(v: AnalyticsFields) -> Result<Self, Error> {
        Self::new(v.request, v.name, v.totals, v.daily)
    }
}

/// Bounded raw provider token-symbol metadata, without identity/path semantics.
/// UTF-8 text is retained exactly, including provider-supplied control characters.
/// JSON serialization and `Debug` escape controls; callers choose display policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct YieldSymbol(String);
impl YieldSymbol {
    /// Preserves nonempty provider metadata of at most 512 UTF-8 bytes exactly.
    /// # Errors
    /// Rejects empty or oversized symbol metadata.
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > 512 {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self(value.to_owned()))
    }
    /// Returns original untrusted metadata; this is not an identity or safe display label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for YieldSymbol {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::new(&value)
    }
}
impl From<YieldSymbol> for String {
    fn from(value: YieldSymbol) -> Self {
        value.0
    }
}
