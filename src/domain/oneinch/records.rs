// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Asset, Limits, ProtocolId, QuoteRequest, SourceText, invalid_record};
use crate::{
    domain::{ExactDecimal, NetworkId, Source, Timestamp, evm::Quantity},
    error::Error,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Actual API operation represented by provider attribution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Source aggregated quote and route graph.
    Quote,
    /// Fresh independent swap payload and estimate.
    Swap,
    /// Full bounded source liquidity catalogue.
    LiquiditySources,
    /// Source-reported router spender identity.
    Spender,
}
/// Retrieval attribution with explicit expected EVM network, without a genesis/block claim.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// Expected network selected in the source API path, not independently proved.
    pub network: NetworkId,
    /// Actual operation that produced these source facts.
    pub method: Method,
    /// Explicit provider ID/API method/integration version.
    pub source: Source,
    /// Actual retrieval instant in Unix seconds.
    pub retrieved_at: Timestamp,
}
/// Provider supplies no expiry for these Classic Swap responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expiry {
    /// No source TTL or expiry is reported; none is invented from retrieval time.
    Unreported,
}
/// Presence of propAMM estimation state overrides, without injecting them into RPC.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateOverrides {
    /// Response did not supply the field.
    Unreported,
    /// Source explicitly supplied null.
    Null,
    /// Source supplied an empty object.
    Empty,
    /// Source supplied an object outside this ordinary preparation's interpreted profile.
    /// It is not retained as arbitrary JSON or used to simulate execution.
    PresentUninterpreted,
}
/// Typed source token display information; asset identity remains separate from precision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Token {
    /// Exact EVM chain/native/ERC20 identity derived from the requested API path and token address.
    pub asset: Asset,
    /// Actual source-reported decimals; no caller-configured precision is substituted.
    pub decimals: u8,
    /// Actual source display symbol, not identity.
    pub symbol: SourceText,
    /// Actual source display name.
    pub name: SourceText,
    /// Source nullable fee-on-transfer classification; absence remains unreported.
    pub fee_on_transfer: Option<bool>,
}
/// Source-reported protocol share, retained exactly rather than binary floating point.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolShare {
    /// Exact liquidity-source ID, not a pool/deployment proof.
    pub name: ProtocolId,
    /// Source-reported percentage for this split.
    pub percent: ExactDecimal,
}
/// One actual v6.1 directed hop with reported IDs and exact shares.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hop {
    /// Exact destination token identity for this hop.
    pub destination: Asset,
    /// Actual source token ID; it is not assumed to index the outer group array.
    pub from_token_id: u32,
    /// Actual destination token ID; terminal destinations need not have outer groups.
    pub to_token_id: u32,
    /// Source-reported distribution percent.
    pub percent: ExactDecimal,
    /// Every reported liquidity split in order, at most 256.
    #[serde(deserialize_with = "protocol_shares")]
    pub protocols: Vec<ProtocolShare>,
}
fn protocol_shares<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<ProtocolShare>, D::Error> {
    super::bounded::<D, ProtocolShare, 256>(d)
}
/// One v6.1 source-token group; no old v6.0 triple-array interpretation is applied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenSwaps {
    /// Exact source token for these actual reported hops.
    pub token: Asset,
    /// Every reported hop in source order, at most 256.
    #[serde(deserialize_with = "hops")]
    pub hops: Vec<Hop>,
}
fn hops<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Hop>, D::Error> {
    super::bounded::<D, Hop, 256>(d)
}
/// Bounded v6.1 source route graph with actual address/ID correlations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "GraphFields")]
pub struct RouteGraph {
    network: NetworkId,
    groups: Vec<TokenSwaps>,
}
impl RouteGraph {
    /// Validates bounded source addresses/shares and consistent repeated token IDs.
    /// It does not infer percent conservation, array indices, topology or finality.
    /// # Errors
    /// Rejects excessive groups/items, wrong chain, conflicting source ID identities or bad shares.
    pub fn new(network: NetworkId, groups: Vec<TokenSwaps>) -> Result<Self, Error> {
        if groups.len() > 64 || network.chain_id().value().is_zero() {
            return Err(invalid_record());
        }
        let mut ids = HashMap::new();
        let mut count = groups.len();
        for group in &groups {
            if group.token.chain_id() != network.chain_id() || group.hops.len() > 256 {
                return Err(invalid_record());
            }
            count += group.hops.len();
            for hop in &group.hops {
                if hop.destination.chain_id() != network.chain_id() || hop.protocols.len() > 256 {
                    return Err(invalid_record());
                }
                share(&hop.percent)?;
                for (id, asset) in [
                    (hop.from_token_id, group.token),
                    (hop.to_token_id, hop.destination),
                ] {
                    if ids
                        .insert(id, asset)
                        .is_some_and(|previous| previous != asset)
                    {
                        return Err(invalid_record());
                    }
                }
                count += hop.protocols.len();
                for protocol in &hop.protocols {
                    share(&protocol.percent)?;
                }
            }
        }
        if count > 4096 {
            return Err(invalid_record());
        }
        Ok(Self { network, groups })
    }
    /// Returns caller-qualified path network; no source genesis proof exists.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns every actual source group in order.
    #[must_use]
    pub fn groups(&self) -> &[TokenSwaps] {
        &self.groups
    }
    /// Returns exact retained group/hop/share record count.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.groups.len()
            + self
                .groups
                .iter()
                .map(|g| g.hops.len() + g.hops.iter().map(|h| h.protocols.len()).sum::<usize>())
                .sum::<usize>()
    }
    /// Checks an explicit caller's stricter collection bound without truncation.
    /// # Errors
    /// Rejects graph size exceeding that supplied bound.
    pub fn check_limits(&self, limits: Limits) -> Result<(), Error> {
        if self.item_count() > usize::from(limits.route_items()) {
            return Err(invalid_record());
        }
        Ok(())
    }
}
fn share(v: &ExactDecimal) -> Result<(), Error> {
    if v.is_negative() || v > &ExactDecimal::parse("100")? {
        return Err(invalid_record());
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphFields {
    network: NetworkId,
    #[serde(deserialize_with = "groups")]
    groups: Vec<TokenSwaps>,
}
fn groups<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<TokenSwaps>, D::Error> {
    super::bounded::<D, TokenSwaps, 64>(d)
}
impl TryFrom<GraphFields> for RouteGraph {
    type Error = Error;
    fn try_from(v: GraphFields) -> Result<Self, Error> {
        Self::new(v.network, v.groups)
    }
}

/// Actual source quote amounts, display precision, graph and gas suggestion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteData {
    /// Actual source token information correlated with the original request.
    pub source_token: Token,
    /// Actual destination token information correlated with the original request.
    pub destination_token: Token,
    /// Exact expected destination raw units.
    pub destination_amount: Quantity,
    /// Actual v6.1 graph, including explicit empty graph when source supplies it.
    pub routes: RouteGraph,
    /// Actual source gas suggestion, absent/null when unavailable, never an execution proof.
    pub estimated_gas: Option<Quantity>,
    /// Explicit source expiry absence.
    pub expiry: Expiry,
    /// Honest source estimation-override availability.
    pub state_overrides: StateOverrides,
}
/// Immutable original request plus attributable source quote.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QuoteFields")]
pub struct Quote {
    request: QuoteRequest,
    data: QuoteData,
    context: Context,
}
impl Quote {
    /// Correlates actual source token identities and graph with the supplied request.
    /// # Errors
    /// Rejects wrong-network/method/token identity or reported protocol IDs outside
    /// caller filters. Empty graphs remain absent route evidence; their completeness
    /// and execution are not inferred. Fee-on-transfer metadata stays explicit.
    pub fn new(request: QuoteRequest, data: QuoteData, context: Context) -> Result<Self, Error> {
        if &context.network != request.network()
            || !matches!(context.method, Method::Quote | Method::Swap)
            || data.source_token.asset != request.source()
            || data.destination_token.asset != request.destination()
            || data.routes.network() != request.network()
        {
            return Err(invalid_record());
        }
        let settings = request.settings();
        if data
            .routes
            .groups()
            .iter()
            .flat_map(|group| &group.hops)
            .flat_map(|hop| &hop.protocols)
            .any(|share| {
                settings.excluded_protocols.contains(&share.name)
                    || (!settings.protocols.is_empty() && !settings.protocols.contains(&share.name))
            })
        {
            return Err(invalid_record());
        }
        Ok(Self {
            request,
            data,
            context,
        })
    }
    /// Returns immutable exact original caller values.
    #[must_use]
    pub const fn request(&self) -> &QuoteRequest {
        &self.request
    }
    /// Returns exact actual source facts without reinterpreting calldata.
    #[must_use]
    pub const fn data(&self) -> &QuoteData {
        &self.data
    }
    /// Returns actual retrieval attribution with expected network.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuoteFields {
    request: QuoteRequest,
    data: QuoteData,
    context: Context,
}
impl TryFrom<QuoteFields> for Quote {
    type Error = Error;
    fn try_from(v: QuoteFields) -> Result<Self, Error> {
        Self::new(v.request, v.data, v.context)
    }
}

/// One named source liquidity protocol, without fetching image URLs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiquiditySource {
    /// Exact provider source ID.
    pub id: ProtocolId,
    /// Exact display title.
    pub title: SourceText,
    /// Exact source image reference text; no image is fetched.
    pub image: SourceText,
    /// Exact alternate image reference text.
    pub image_color: SourceText,
}
/// Complete bounded source liquidity catalogue at its retrieval instant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SourcesFields")]
pub struct LiquiditySources {
    context: Context,
    items: Vec<LiquiditySource>,
}
impl LiquiditySources {
    /// Checks source method/network attribution and unique bounded source IDs.
    /// # Errors
    /// Rejects wrong method, duplicate IDs or more than 512 entries.
    pub fn new(context: Context, items: Vec<LiquiditySource>) -> Result<Self, Error> {
        let ids: HashSet<_> = items.iter().map(|v| &v.id).collect();
        if context.method != Method::LiquiditySources
            || context.network.chain_id().value().is_zero()
            || items.len() > 512
            || ids.len() != items.len()
        {
            return Err(invalid_record());
        }
        Ok(Self { context, items })
    }
    /// Returns actual source attribution.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns every reported catalogue entry without truncation.
    #[must_use]
    pub fn items(&self) -> &[LiquiditySource] {
        &self.items
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcesFields {
    context: Context,
    #[serde(deserialize_with = "sources")]
    items: Vec<LiquiditySource>,
}
fn sources<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<LiquiditySource>, D::Error> {
    super::bounded::<D, LiquiditySource, 512>(d)
}
impl TryFrom<SourcesFields> for LiquiditySources {
    type Error = Error;
    fn try_from(v: SourcesFields) -> Result<Self, Error> {
        Self::new(v.context, v.items)
    }
}
/// Exact source-reported spender plus retrieval attribution, without approving allowances.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SpenderFields")]
pub struct Spender {
    context: Context,
    address: crate::domain::Address,
}
impl Spender {
    /// Retains one actual source spender identity; no approval is encoded or submitted.
    /// # Errors
    /// Rejects wrong method, zero/native-sentinel pseudo-contract addresses.
    pub fn new(context: Context, address: crate::domain::Address) -> Result<Self, Error> {
        if context.method != Method::Spender
            || context.network.chain_id().value().is_zero()
            || (address.bytes() == [0; 20] || address.bytes() == [0xee; 20])
        {
            return Err(invalid_record());
        }
        Ok(Self { context, address })
    }
    /// Returns actual source attribution.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns exact source spender, not a contract attestation.
    #[must_use]
    pub const fn address(&self) -> crate::domain::Address {
        self.address
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpenderFields {
    context: Context,
    address: crate::domain::Address,
}
impl TryFrom<SpenderFields> for Spender {
    type Error = Error;
    fn try_from(v: SpenderFields) -> Result<Self, Error> {
        Self::new(v.context, v.address)
    }
}
