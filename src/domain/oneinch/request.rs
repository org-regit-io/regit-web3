// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::invalid_request;
use crate::{
    domain::{Address, ChainId, NetworkId, U256, evm::Quantity},
    error::Error,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// An explicitly EVM asset; native units have no token-contract address.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "contract",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AssetKind {
    /// The chain's native asset, encoded as a sentinel only at the provider boundary.
    Native,
    /// Exact ERC20 contract identity, without assumed decimals or symbol.
    Erc20(Address),
}
/// Exact EVM chain and native/ERC20 identity used by Classic Swap.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AssetFields")]
pub struct Asset {
    chain_id: ChainId,
    kind: AssetKind,
}
impl Asset {
    /// Records a caller-qualified EVM identity without inferring a family from a number.
    /// # Errors
    /// Rejects zero chain IDs and ERC20 zero/native-sentinel pseudo-contracts.
    pub fn new(chain_id: ChainId, kind: AssetKind) -> Result<Self, Error> {
        if chain_id.value().is_zero()
            || matches!(kind,AssetKind::Erc20(a) if a.bytes()==[0;20] || a.bytes()==[0xee;20])
        {
            return Err(invalid_request());
        }
        Ok(Self { chain_id, kind })
    }
    /// Returns exact EVM chain attribution.
    #[must_use]
    pub const fn chain_id(self) -> ChainId {
        self.chain_id
    }
    /// Returns family identity, independently of provider sentinel syntax.
    #[must_use]
    pub const fn kind(self) -> AssetKind {
        self.kind
    }
    /// Returns provider API encoding; the native sentinel is not a real ERC20 identity.
    #[must_use]
    pub fn provider_address(self) -> Address {
        match self.kind {
            AssetKind::Native => Address::from_bytes([0xee; 20]),
            AssetKind::Erc20(a) => a,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetFields {
    chain_id: ChainId,
    kind: AssetKind,
}
impl TryFrom<AssetFields> for Asset {
    type Error = Error;
    fn try_from(v: AssetFields) -> Result<Self, Error> {
        Self::new(v.chain_id, v.kind)
    }
}

/// A bounded liquidity-source identifier safe to use in a comma-separated filter.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProtocolId(String);
impl ProtocolId {
    /// Retains exact source ID syntax without interpreting a DEX deployment.
    /// # Errors
    /// Rejects empty, excessive, non-ASCII or separator/control-bearing identifiers.
    pub fn new(text: impl Into<String>) -> Result<Self, Error> {
        let text = text.into();
        if text.is_empty()
            || text.len() > 128
            || !text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        {
            return Err(invalid_request());
        }
        Ok(Self(text))
    }
    /// Returns exact ID, not contract-code attestation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for ProtocolId {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<ProtocolId> for String {
    fn from(v: ProtocolId) -> Self {
        v.0
    }
}
/// Bounded exact provider display text, preserving UTF-8 and JSON-escaped controls.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceText(String);
impl SourceText {
    /// Retains at most 4096 UTF-8 bytes; this text does not identify a token.
    /// # Errors
    /// Rejects excessive text without echoing its content.
    pub fn new(v: impl Into<String>) -> Result<Self, Error> {
        let v = v.into();
        if v.len() > 4096 {
            return Err(invalid_request());
        }
        Ok(Self(v))
    }
    /// Returns exact source display metadata.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for SourceText {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<SourceText> for String {
    fn from(v: SourceText) -> Self {
        v.0
    }
}

/// Explicit bounded provider collection ceilings; excess fails rather than truncates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LimitFields")]
pub struct Limits {
    route_items: u16,
    liquidity_sources: u16,
}
impl Limits {
    /// Records caller ceilings within global 4096 graph items and 512 source entries.
    /// # Errors
    /// Rejects zero or excessive limits.
    pub fn new(route_items: u16, liquidity_sources: u16) -> Result<Self, Error> {
        if !(1..=4096).contains(&route_items) || !(1..=512).contains(&liquidity_sources) {
            return Err(invalid_request());
        }
        Ok(Self {
            route_items,
            liquidity_sources,
        })
    }
    /// Returns maximum total graph groups, hops and protocol-share records.
    #[must_use]
    pub const fn route_items(self) -> u16 {
        self.route_items
    }
    /// Returns maximum complete liquidity catalogue entries.
    #[must_use]
    pub const fn liquidity_sources(self) -> u16 {
        self.liquidity_sources
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitFields {
    route_items: u16,
    liquidity_sources: u16,
}
impl TryFrom<LimitFields> for Limits {
    type Error = Error;
    fn try_from(v: LimitFields) -> Result<Self, Error> {
        Self::new(v.route_items, v.liquidity_sources)
    }
}

/// Explicit routing settings retained unchanged between selection and fresh preparation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteSettings {
    /// Permitted source IDs; empty means the source's documented default catalogue.
    #[serde(deserialize_with = "protocols")]
    pub protocols: Vec<ProtocolId>,
    /// Explicitly excluded source IDs.
    #[serde(deserialize_with = "protocols")]
    pub excluded_protocols: Vec<ProtocolId>,
    /// Caller-selected intermediary token identities.
    #[serde(deserialize_with = "connectors")]
    pub connector_tokens: Vec<Asset>,
    /// Optional explicit gas-price routing input in wei per gas.
    pub gas_price: Option<Quantity>,
    /// Optional explicit source complexity setting (0 through 3).
    pub complexity_level: Option<u8>,
    /// Optional explicit source split count (1 through 100).
    pub parts: Option<u8>,
    /// Optional explicit main-route split count (1 through 100).
    pub main_route_parts: Option<u8>,
    /// Optional explicit source estimation gas ceiling.
    pub gas_limit: Option<u64>,
}
fn protocols<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<ProtocolId>, D::Error> {
    super::bounded::<D, ProtocolId, 64>(d)
}
fn connectors<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Asset>, D::Error> {
    super::bounded::<D, Asset, 64>(d)
}
impl QuoteSettings {
    /// Creates explicit provider defaults with no endpoint, credential or RPC access.
    #[must_use]
    pub const fn provider_defaults() -> Self {
        Self {
            protocols: Vec::new(),
            excluded_protocols: Vec::new(),
            connector_tokens: Vec::new(),
            gas_price: None,
            complexity_level: None,
            parts: None,
            main_route_parts: None,
            gas_limit: None,
        }
    }
    fn validate(&self, chain: ChainId) -> Result<(), Error> {
        let included: HashSet<_> = self.protocols.iter().collect();
        let excluded: HashSet<_> = self.excluded_protocols.iter().collect();
        let connectors: HashSet<_> = self.connector_tokens.iter().collect();
        if self.protocols.len() > 64
            || self.excluded_protocols.len() > 64
            || self.connector_tokens.len() > 64
            || included.len() != self.protocols.len()
            || excluded.len() != self.excluded_protocols.len()
            || !included.is_disjoint(&excluded)
            || connectors.len() != self.connector_tokens.len()
            || self.connector_tokens.iter().any(|a| a.chain_id() != chain)
            || self.complexity_level.is_some_and(|n| n > 3)
            || self.parts.is_some_and(|n| n == 0 || n > 100)
            || self.main_route_parts.is_some_and(|n| n == 0 || n > 100)
            || self.gas_limit == Some(0)
        {
            return Err(invalid_request());
        }
        Ok(())
    }
}
/// Immutable exact-input request for EVM Classic Swap v6.1.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QuoteRequestFields")]
pub struct QuoteRequest {
    network: NetworkId,
    source: Asset,
    destination: Asset,
    amount: Quantity,
    settings: QuoteSettings,
}
impl QuoteRequest {
    /// Retains exact raw input units and routing settings without decimal inference.
    /// # Errors
    /// Rejects wrong-chain/same assets, zero inputs or invalid settings/collections.
    pub fn new(
        network: NetworkId,
        source: Asset,
        destination: Asset,
        amount: Quantity,
        settings: QuoteSettings,
    ) -> Result<Self, Error> {
        if source.chain_id() != network.chain_id()
            || destination.chain_id() != network.chain_id()
            || source == destination
            || amount.value().is_zero()
        {
            return Err(invalid_request());
        }
        settings.validate(network.chain_id())?;
        Ok(Self {
            network,
            source,
            destination,
            amount,
            settings,
        })
    }
    /// Returns exact expected EVM network, not a provider genesis proof.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns exact supplied source asset.
    #[must_use]
    pub const fn source(&self) -> Asset {
        self.source
    }
    /// Returns exact supplied destination asset.
    #[must_use]
    pub const fn destination(&self) -> Asset {
        self.destination
    }
    /// Returns exact raw source units, without inferred decimals.
    #[must_use]
    pub const fn amount(&self) -> Quantity {
        self.amount
    }
    /// Returns immutable routing inputs.
    #[must_use]
    pub const fn settings(&self) -> &QuoteSettings {
        &self.settings
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuoteRequestFields {
    network: NetworkId,
    source: Asset,
    destination: Asset,
    amount: Quantity,
    settings: QuoteSettings,
}
impl TryFrom<QuoteRequestFields> for QuoteRequest {
    type Error = Error;
    fn try_from(v: QuoteRequestFields) -> Result<Self, Error> {
        Self::new(v.network, v.source, v.destination, v.amount, v.settings)
    }
}

/// Caller slippage tolerance in basis points (100 basis points equals one percent).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct SlippageBps(u16);
impl SlippageBps {
    /// Records the API's explicit inclusive 0–50 percent supported range.
    /// # Errors
    /// Rejects more than 5000 basis points.
    pub fn new(v: u16) -> Result<Self, Error> {
        if v > 5000 {
            return Err(invalid_request());
        }
        Ok(Self(v))
    }
    /// Returns exact basis points.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
    /// Returns the exact percentage query representation, without floating point.
    #[must_use]
    pub fn percent(self) -> String {
        format!("{}.{:02}", self.0 / 100, self.0 % 100)
    }
}
impl TryFrom<u16> for SlippageBps {
    type Error = Error;
    fn try_from(v: u16) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<SlippageBps> for u16 {
    fn from(v: SlippageBps) -> Self {
        v.0
    }
}
/// Exactly one source swap-return policy; min-return and percentage are never both sent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ReturnPolicy {
    /// Explicit destination-unit minimum sent as `minReturn`.
    Minimum(Quantity),
    /// Explicit source-relative tolerance sent as `slippage` percent.
    Slippage(SlippageBps),
}
/// Explicit ordinary swap review policy; the library never silently supplies these values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwapSettings {
    /// Caller address executing the router call.
    pub from: Address,
    /// Explicit initiating EOA passed as `origin`.
    pub origin: Address,
    /// Explicit output recipient passed as `receiver`.
    pub recipient: Address,
    /// Caller-selected destination raw-unit floor for fresh result review.
    /// Opaque calldata does not prove this value is enforced by execution.
    pub output_floor: Quantity,
    /// Exactly one explicit source min-return/slippage choice.
    pub return_policy: ReturnPolicy,
    /// Caller-qualified router identity; source `tx.to` must match.
    pub router: Address,
    /// Caller-qualified spender identity, independently of router identity.
    pub spender: Address,
    /// Explicit exact native value expected in returned source transaction.
    pub native_value: Quantity,
    /// Caller explicitly chooses whether the provider skips on-chain estimation.
    pub disable_estimate: bool,
}
/// Original selected quote plus immutable explicit settings for a fresh source response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SwapRequestFields")]
pub struct SwapRequest {
    selection: super::Quote,
    settings: SwapSettings,
}
impl SwapRequest {
    /// Records original selection and caller review policy without signing or submitting.
    /// # Errors
    /// Rejects zero review identities/floor, inconsistent min-return or native value.
    pub fn new(selection: super::Quote, settings: SwapSettings) -> Result<Self, Error> {
        if [
            settings.from,
            settings.origin,
            settings.recipient,
            settings.router,
            settings.spender,
        ]
        .iter()
        .any(|a| a.bytes() == [0; 20] || a.bytes() == [0xee; 20])
            || settings.output_floor.value().is_zero()
            || settings.output_floor.value() > selection.data().destination_amount.value()
            || matches!(settings.return_policy,ReturnPolicy::Minimum(q)if q!=settings.output_floor)
        {
            return Err(invalid_request());
        }
        if selection.data().source_token.fee_on_transfer == Some(true)
            || selection.data().destination_token.fee_on_transfer == Some(true)
        {
            return Err(Error::UnsupportedCapability);
        }
        let native = if selection.request().source().kind() == AssetKind::Native {
            selection.request().amount()
        } else {
            Quantity::new(U256::ZERO)
        };
        if settings.native_value != native {
            return Err(invalid_request());
        }
        Ok(Self {
            selection,
            settings,
        })
    }
    /// Returns the original quote and its retrieval/route facts unchanged.
    #[must_use]
    pub const fn selection(&self) -> &super::Quote {
        &self.selection
    }
    /// Returns every explicit immutable review setting.
    #[must_use]
    pub const fn settings(&self) -> &SwapSettings {
        &self.settings
    }
    /// Returns original exact expected EVM network.
    #[must_use]
    pub fn network(&self) -> &NetworkId {
        self.selection.request().network()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwapRequestFields {
    selection: super::Quote,
    settings: SwapSettings,
}
impl TryFrom<SwapRequestFields> for SwapRequest {
    type Error = Error;
    fn try_from(v: SwapRequestFields) -> Result<Self, Error> {
        Self::new(v.selection, v.settings)
    }
}
