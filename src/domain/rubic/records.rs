// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Asset, Chain, Identifier, Limits, QuoteRequest, Text};
use crate::{
    domain::{Address, Amount, ExactDecimal, market::NonnegativeDecimal},
    error::Error,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Actual source catalogue row; optional IDs do not become fabricated chain IDs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainInfo {
    /// Source alias.
    pub alias: Identifier,
    /// Actual optional provider number, not a universal family identity.
    pub provider_id: Option<u64>,
    /// Exact source family label.
    pub source_type: Identifier,
    /// Source testnet category.
    pub testnet: bool,
    /// Source reports proxy contracts are available.
    pub proxy_available: bool,
    /// Bounded cross-chain provider labels in source order.
    pub cross_chain_providers: Vec<Identifier>,
    /// Bounded on-chain provider labels in source order.
    pub on_chain_providers: Vec<Identifier>,
}
impl ChainInfo {
    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.provider_id == Some(0)
            || self.cross_chain_providers.len() > 256
            || self.on_chain_providers.len() > 256
        {
            return Err(super::invalid_quote());
        }
        for v in [&self.cross_chain_providers, &self.on_chain_providers] {
            let mut ids = BTreeSet::new();
            if v.iter().any(|p| !ids.insert(p)) {
                return Err(super::invalid_quote());
            }
        }
        Ok(())
    }
    /// Checks exact catalogue identity against a caller qualification.
    /// This compares source declarations and does not prove genesis.
    /// # Errors
    /// Rejects alias, provider number, family or testnet disagreement.
    pub fn matches(&self, chain: &Chain) -> Result<(), Error> {
        if &self.alias != chain.alias()
            || self.provider_id != chain.provider_id()
            || self.source_type.as_str() != chain.family().source_type()
            || self.testnet != chain.testnet()
        {
            return Err(super::invalid_identity());
        }
        Ok(())
    }
}
/// Complete bounded source catalogue, with no implicit continuation or truncation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<ChainInfo>", into = "Vec<ChainInfo>")]
pub struct Chains(Vec<ChainInfo>);
impl Chains {
    /// Checks up to 512 uniquely aliased source rows and nested capacities.
    /// # Errors
    /// Rejects duplicates, malformed rows and collection excess.
    pub fn new(v: Vec<ChainInfo>) -> Result<Self, Error> {
        let mut ids = BTreeSet::new();
        if v.len() > 512 || v.iter().any(|c| !ids.insert(c.alias.clone())) {
            return Err(super::invalid_quote());
        }
        for c in &v {
            c.validate()?;
        }
        Ok(Self(v))
    }
    /// Returns all source rows in original order.
    #[must_use]
    pub fn entries(&self) -> &[ChainInfo] {
        &self.0
    }
}
impl TryFrom<Vec<ChainInfo>> for Chains {
    type Error = Error;
    fn try_from(v: Vec<ChainInfo>) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Chains> for Vec<ChainInfo> {
    fn from(v: Chains) -> Self {
        v.0
    }
}

/// Exact qualified source token plus descriptive metadata; precision is explicit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Token {
    /// Family-qualified exact asset and actual precision.
    pub asset: Asset,
    /// Source display symbol, unrelated to identity.
    pub symbol: Text,
    /// Source display name, unrelated to identity.
    pub name: Text,
    /// Optional exact nonnegative USD price; no default.
    pub price_usd: Option<NonnegativeDecimal>,
}
/// Source amount at this position, independent of caller gross and other legs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TokenAmountFields")]
pub struct TokenAmount {
    token: Token,
    amount: Amount,
}
impl TokenAmount {
    /// Checks precise token/base-unit agreement without amount conservation assumptions.
    /// # Errors
    /// Rejects incompatible precision.
    pub fn new(token: Token, amount: Amount) -> Result<Self, Error> {
        if amount.decimals() != Some(token.asset.decimals()) {
            return Err(super::invalid_quote());
        }
        Ok(Self { token, amount })
    }
    /// Returns exact qualified token metadata.
    #[must_use]
    pub const fn token(&self) -> &Token {
        &self.token
    }
    /// Returns the exact source-position amount.
    #[must_use]
    pub const fn amount(&self) -> Amount {
        self.amount
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenAmountFields {
    token: Token,
    amount: Amount,
}
impl TryFrom<TokenAmountFields> for TokenAmount {
    type Error = Error;
    fn try_from(v: TokenAmountFields) -> Result<Self, Error> {
        Self::new(v.token, v.amount)
    }
}

/// Exact source estimate facts; minimum is source-reported, not guaranteed execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EstimateData {
    /// Expected destination base units and explicit precision.
    pub output: Amount,
    /// Source minimum destination base units and explicit precision.
    pub minimum_output: Amount,
    /// Optional source USD output.
    pub output_usd: Option<NonnegativeDecimal>,
    /// Optional source minimum USD output.
    pub minimum_output_usd: Option<NonnegativeDecimal>,
    /// Exact source duration in minutes, without a local expiry claim.
    pub duration_minutes: NonnegativeDecimal,
    /// Exact source output slippage fraction, not percent/basis-point confusion.
    pub slippage_fraction: NonnegativeDecimal,
    /// Optional signed price impact as actually reported.
    pub price_impact: Option<ExactDecimal>,
    /// Optional exact intermediate raw amount with unavailable precision.
    pub intermediate_raw: Option<Amount>,
}
/// Immutable validated source estimate with no invented precision or expiry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EstimateData", into = "EstimateData")]
pub struct Estimate(EstimateData);
impl Estimate {
    /// Checks known equal destination precision, ordered bounds and raw intermediate units.
    /// # Errors
    /// Rejects contradictory precision, inverted amounts or invalid intermediate precision.
    pub fn new(v: EstimateData) -> Result<Self, Error> {
        if v.output.decimals().is_none()
            || v.output.decimals() != v.minimum_output.decimals()
            || v.minimum_output.raw() > v.output.raw()
            || v.intermediate_raw.is_some_and(|a| a.decimals().is_some())
        {
            return Err(super::invalid_quote());
        }
        Ok(Self(v))
    }
    /// Returns every exact source estimate fact.
    #[must_use]
    pub const fn data(&self) -> &EstimateData {
        &self.0
    }
}
impl TryFrom<EstimateData> for Estimate {
    type Error = Error;
    fn try_from(v: EstimateData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Estimate> for EstimateData {
    fn from(v: Estimate) -> Self {
        v.0
    }
}

/// Optional source gas components in raw gas/wei units; no atomicity assertion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GasFees {
    /// Optional raw source gas units.
    pub gas_limit: Option<Amount>,
    /// Optional raw source wei per gas.
    pub gas_price: Option<Amount>,
    /// Optional raw source base fee.
    pub base_fee: Option<Amount>,
    /// Optional raw source maximum wei per gas.
    pub max_fee_per_gas: Option<Amount>,
    /// Optional raw source maximum priority wei per gas.
    pub max_priority_fee_per_gas: Option<Amount>,
    /// Optional source total native fee with actual native precision.
    pub total: Option<Amount>,
    /// Optional source USD gas cost.
    pub total_usd: Option<NonnegativeDecimal>,
}
/// Source fixed fee with exact native base units and optional USD facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedFee {
    /// Exact native token fee, with explicit precision.
    pub amount: Amount,
    /// Optional exact source USD amount.
    pub usd: Option<NonnegativeDecimal>,
}
/// Attributed fee facts, distinct from caller limits or proven payable amounts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fees {
    /// Source native fee token, without symbol-based identification.
    pub native_token: Token,
    /// Exact optional gas components.
    pub gas: GasFees,
    /// Source fixed protocol fee.
    pub protocol: FixedFee,
    /// Source fixed provider fee.
    pub provider: FixedFee,
    /// Exact source fee percentage; 0.4 means 0.4 percent, not fraction 0.4.
    pub percent: NonnegativeDecimal,
    /// Optional source token to which the percentage applies.
    pub percent_token: Option<Token>,
}
impl Fees {
    pub(super) fn validate(&self, source: &Asset) -> Result<(), Error> {
        let decimals = Some(self.native_token.asset.decimals());
        if self.native_token.asset.chain() != source.chain()
            || !matches!(
                self.native_token.asset.identifier(),
                super::AssetIdentifier::Native
            )
            || self.protocol.amount.decimals() != decimals
            || self.provider.amount.decimals() != decimals
            || self.gas.total.is_some_and(|a| a.decimals() != decimals)
            || [
                self.gas.gas_limit,
                self.gas.gas_price,
                self.gas.base_fee,
                self.gas.max_fee_per_gas,
                self.gas.max_priority_fee_per_gas,
            ]
            .iter()
            .flatten()
            .any(|a| a.decimals().is_some())
        {
            return Err(super::invalid_quote());
        }
        Ok(())
    }
}
/// Actual source route-leg category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LegKind {
    /// Source on-chain route step.
    OnChain,
    /// Source cross-chain route step.
    CrossChain,
}
/// Source-selected route leg; ordered token amounts are not inferred conservation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LegFields")]
pub struct Leg {
    kind: LegKind,
    provider: Identifier,
    path: Vec<TokenAmount>,
}
impl Leg {
    /// Checks a nonempty path of at most 128 source token records.
    /// # Errors
    /// Rejects empty/excessive paths and contradictory on-chain family declarations.
    pub fn new(kind: LegKind, provider: Identifier, path: Vec<TokenAmount>) -> Result<Self, Error> {
        if path.is_empty()
            || path.len() > 128
            || kind == LegKind::OnChain
                && path
                    .iter()
                    .any(|t| t.token.asset.chain() != path[0].token.asset.chain())
        {
            return Err(super::invalid_quote());
        }
        Ok(Self {
            kind,
            provider,
            path,
        })
    }
    /// Returns actual source step kind.
    #[must_use]
    pub const fn kind(&self) -> LegKind {
        self.kind
    }
    /// Returns actual source provider label.
    #[must_use]
    pub const fn provider(&self) -> &Identifier {
        &self.provider
    }
    /// Returns source path order and each exact amount.
    #[must_use]
    pub fn path(&self) -> &[TokenAmount] {
        &self.path
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegFields {
    kind: LegKind,
    provider: Identifier,
    path: Vec<TokenAmount>,
}
impl TryFrom<LegFields> for Leg {
    type Error = Error;
    fn try_from(v: LegFields) -> Result<Self, Error> {
        Self::new(v.kind, v.provider, v.path)
    }
}
/// Actual source swap category, without an execution guarantee.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SwapKind {
    /// Source declares one-chain swap.
    OnChain,
    /// Source declares cross-chain swap.
    CrossChain,
}
/// A source warning's fixed numeric category; arbitrary diagnostic text is discarded.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Warning {
    /// Actual optional numeric source code.
    pub code: Option<u64>,
}

/// Complete immutable source route plus original caller quote choices.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteData {
    /// Exact original caller request.
    pub request: QuoteRequest,
    /// Exact source-selected route continuation ID.
    pub id: Identifier,
    /// Source's position-specific input, which can be net of source fees.
    pub input: TokenAmount,
    /// Source destination metadata.
    pub destination: Token,
    /// Source-selected provider.
    pub provider: Identifier,
    /// Source swap category.
    pub kind: SwapKind,
    /// Exact source estimate.
    pub estimate: Estimate,
    /// Exact source fee facts.
    pub fees: Fees,
    /// Ordered source route legs.
    pub legs: Vec<Leg>,
    /// Source warning numeric categories; no retained remote reason text.
    pub warnings: Vec<Warning>,
    /// Source declares Rubic contracts are used; deployment authenticity is unverified.
    pub use_rubic_contract: bool,
    /// Optional exact EVM allowance suggestion; no approval is performed.
    pub approval_address: Option<Address>,
    /// Optional exact EVM Permit2 allowance suggestion; no approval is performed.
    pub permit2_address: Option<Address>,
}
/// Immutable direct-only source route; private/deposit responses are not normalized here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QuoteData", into = "QuoteData")]
pub struct Quote(QuoteData);
impl Quote {
    /// Checks complete asset/precision, explicit filters and nested request capacities.
    /// No equality is invented between gross input, source input and leg amounts.
    /// # Errors
    /// Rejects contradictory identity, source slippage or violated caller filters.
    pub fn new(v: QuoteData) -> Result<Self, Error> {
        let r = v.request.data();
        if v.input.token.asset != r.source
            || v.destination.asset != r.destination
            || v.estimate.data().output.decimals() != Some(r.destination.decimals())
            || v.estimate.data().slippage_fraction.value()
                != &ExactDecimal::parse(&r.slippage.fraction())?
            || v.kind == SwapKind::OnChain && r.source.chain() != r.destination.chain()
            || v.kind == SwapKind::CrossChain && r.source.chain() == r.destination.chain()
            || r.preferred_provider
                .as_ref()
                .is_some_and(|p| p != &v.provider)
            || r.excluded_providers.contains(&v.provider)
            || v.legs
                .iter()
                .any(|l| r.excluded_providers.contains(l.provider()))
            || !matches!(r.source.chain().family(), super::Family::Evm { .. })
                && (v.approval_address.is_some() || v.permit2_address.is_some())
        {
            return Err(super::invalid_quote());
        }
        v.fees.validate(&r.source)?;
        validate_collections(&v.legs, &v.warnings, r.limits)?;
        Ok(Self(v))
    }
    /// Returns every exact source record and original caller choice.
    #[must_use]
    pub const fn data(&self) -> &QuoteData {
        &self.0
    }
}
impl TryFrom<QuoteData> for Quote {
    type Error = Error;
    fn try_from(v: QuoteData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Quote> for QuoteData {
    fn from(v: Quote) -> Self {
        v.0
    }
}
/// Fresh preparation facts, whose token metadata is unavailable when omitted by
/// the selected provider. The original complete quote remains a separate record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshQuoteData {
    /// Exact correlated original quote request.
    pub request: QuoteRequest,
    /// Exact correlated route continuation ID.
    pub id: Identifier,
    /// Actual fresh source-position input metadata, or unavailable absence.
    pub input: Option<TokenAmount>,
    /// Actual fresh destination metadata, or unavailable absence.
    pub destination: Option<Token>,
    /// Actual selected provider.
    pub provider: Identifier,
    /// Actual source swap category.
    pub kind: SwapKind,
    /// Fresh source estimate; precision is checked against the explicit request.
    pub estimate: Estimate,
    /// Fresh actual source fee facts.
    pub fees: Fees,
    /// Fresh ordered source route legs.
    pub legs: Vec<Leg>,
    /// Fresh source warning categories, without retained diagnostic text.
    pub warnings: Vec<Warning>,
    /// Source declaration, without deployment or execution verification.
    pub use_rubic_contract: bool,
    /// Actual optional fresh EVM allowance suggestion.
    pub approval_address: Option<Address>,
    /// Actual optional fresh Permit2 allowance suggestion.
    pub permit2_address: Option<Address>,
}
/// Immutable fresh preparation facts; missing metadata is never copied from the
/// earlier quote. Request echoes do not prove unsigned-payload semantics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FreshQuoteData", into = "FreshQuoteData")]
pub struct FreshQuote(FreshQuoteData);
impl FreshQuote {
    /// Checks actually available metadata, explicit identities, precision and bounds.
    /// # Errors
    /// Rejects contradictions and violated caller filters, without requiring
    /// providers to return optional fresh token metadata.
    pub fn new(v: FreshQuoteData) -> Result<Self, Error> {
        let r = v.request.data();
        if v.input.as_ref().is_some_and(|t| t.token.asset != r.source)
            || v.destination
                .as_ref()
                .is_some_and(|t| t.asset != r.destination)
            || v.estimate.data().output.decimals() != Some(r.destination.decimals())
            || v.estimate.data().slippage_fraction.value()
                != &ExactDecimal::parse(&r.slippage.fraction())?
            || v.kind == SwapKind::OnChain && r.source.chain() != r.destination.chain()
            || v.kind == SwapKind::CrossChain && r.source.chain() == r.destination.chain()
            || r.preferred_provider
                .as_ref()
                .is_some_and(|p| p != &v.provider)
            || r.excluded_providers.contains(&v.provider)
            || v.legs
                .iter()
                .any(|l| r.excluded_providers.contains(l.provider()))
            || !matches!(r.source.chain().family(), super::Family::Evm { .. })
                && (v.approval_address.is_some() || v.permit2_address.is_some())
        {
            return Err(super::invalid_quote());
        }
        v.fees.validate(&r.source)?;
        validate_collections(&v.legs, &v.warnings, r.limits)?;
        Ok(Self(v))
    }
    /// Returns fresh source facts and explicit metadata availability for review.
    #[must_use]
    pub const fn data(&self) -> &FreshQuoteData {
        &self.0
    }
}
impl TryFrom<FreshQuoteData> for FreshQuote {
    type Error = Error;
    fn try_from(v: FreshQuoteData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<FreshQuote> for FreshQuoteData {
    fn from(v: FreshQuote) -> Self {
        v.0
    }
}
pub(super) fn validate_collections(
    legs: &[Leg],
    warnings: &[Warning],
    limits: Limits,
) -> Result<(), Error> {
    if legs.is_empty()
        || legs.len() > usize::from(limits.legs())
        || legs
            .iter()
            .any(|l| l.path.len() > usize::from(limits.path_tokens()))
        || warnings.len() > usize::from(limits.warnings())
    {
        return Err(super::invalid_quote());
    }
    Ok(())
}
/// Complete bounded returned routes; no global completeness/optimality assertion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RoutesFields")]
pub struct Routes {
    request: QuoteRequest,
    routes: Vec<Quote>,
}
impl Routes {
    /// Checks exact common request, unique continuation IDs and caller route capacity.
    /// # Errors
    /// Rejects duplicates, inconsistent requests and overflow; empty remains empty.
    pub fn new(request: QuoteRequest, routes: Vec<Quote>) -> Result<Self, Error> {
        let mut ids = BTreeSet::new();
        if routes.len() > usize::from(request.data().limits.routes())
            || routes
                .iter()
                .any(|q| q.data().request != request || !ids.insert(q.data().id.clone()))
        {
            return Err(super::invalid_quote());
        }
        Ok(Self { request, routes })
    }
    /// Returns exact original caller choices.
    #[must_use]
    pub const fn request(&self) -> &QuoteRequest {
        &self.request
    }
    /// Returns all returned routes in source order, without truncation.
    #[must_use]
    pub fn routes(&self) -> &[Quote] {
        &self.routes
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutesFields {
    request: QuoteRequest,
    routes: Vec<Quote>,
}
impl TryFrom<RoutesFields> for Routes {
    type Error = Error;
    fn try_from(v: RoutesFields) -> Result<Self, Error> {
        Self::new(v.request, v.routes)
    }
}
