// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, ValidationError};

use super::{Address, Amount, Asset, NetworkId};

/// An explicitly supplied Unix timestamp in whole seconds since the UTC epoch.
///
/// Construction never reads the system clock. Block and retrieval timestamps
/// are separate facts; no relationship between their clocks is assumed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Constructs a timestamp from explicitly supplied whole Unix seconds.
    #[must_use]
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    /// Returns whole seconds since the UTC Unix epoch.
    #[must_use]
    pub const fn unix_seconds(self) -> u64 {
        self.0
    }
}

/// A validated 32-byte EVM block hash with canonical lowercase serialization.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BlockHash([u8; 32]);

impl BlockHash {
    /// Parses a `0x`-prefixed, exactly 64-digit hexadecimal hash.
    ///
    /// # Errors
    ///
    /// Rejects invalid length, hexadecimal, or prefix without echoing input.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let Some(payload) = value.strip_prefix("0x") else {
            return Err(ValidationError::InvalidBlockHash.into());
        };
        if payload.len() != 64 || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidBlockHash.into());
        }
        const_hex::decode_to_array(payload)
            .map(Self)
            .map_err(|_| ValidationError::InvalidBlockHash.into())
    }

    /// Constructs a hash from its exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact 32-byte hash.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

impl FromStr for BlockHash {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for BlockHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&const_hex::encode_prefixed(self.0))
    }
}

impl Serialize for BlockHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BlockHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// The requested state-read anchor, retained separately from the resolved block.
///
/// Serialized as a snake-case `kind` and, for explicit identities, a `value`.
/// Pending state is deliberately unsupported for balance observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum BlockSelector {
    /// The most recent block reported by the source.
    Latest,
    /// The source's safe block tag.
    Safe,
    /// The source's finalized block tag.
    Finalized,
    /// An explicit block height.
    Number(u64),
    /// An explicit block hash.
    Hash(BlockHash),
}

/// The resolved block identity and its separately reported timestamp.
///
/// Constructing a block context records caller-supplied facts; it does not
/// independently verify existence or canonicality of the block.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockContext {
    number: u64,
    hash: BlockHash,
    timestamp: Timestamp,
}

impl BlockContext {
    /// Records an explicit resolved height, hash, and block timestamp.
    #[must_use]
    pub const fn new(number: u64, hash: BlockHash, timestamp: Timestamp) -> Self {
        Self {
            number,
            hash,
            timestamp,
        }
    }

    /// Returns the resolved block height.
    #[must_use]
    pub const fn number(&self) -> u64 {
        self.number
    }

    /// Returns the resolved block hash.
    #[must_use]
    pub const fn hash(&self) -> &BlockHash {
        &self.hash
    }

    /// Returns the block timestamp, independently of retrieval time.
    #[must_use]
    pub const fn timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

/// Source-reported finality, separately recorded from confirmation count.
///
/// These labels do not independently establish finality. A requested safe or
/// finalized selector alone does not assign a finality result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Finality {
    /// Finality has not been assessed or reported.
    Unknown,
    /// The source reported safe-block context.
    Safe,
    /// The source reported finalized-block context.
    Finalized,
}

/// Non-secret attribution labels for the provider, method, and integration.
///
/// Each label must contain 1–128 ASCII letters, digits, `-`, `_`, `.`, or `+`.
/// URLs, whitespace, header syntax, and control characters are rejected. Label
/// validation is lexical; callers must supply non-secret labels rather than
/// credentials. An identifier alone does not authenticate a source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SourceFields")]
pub struct Source {
    provider_id: String,
    method: String,
    integration_version: String,
}

impl Source {
    /// Constructs explicit non-secret attribution labels.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or lexically invalid labels. Errors contain no
    /// supplied labels.
    pub fn new(
        provider_id: impl Into<String>,
        method: impl Into<String>,
        integration_version: impl Into<String>,
    ) -> Result<Self, Error> {
        let source = Self {
            provider_id: provider_id.into(),
            method: method.into(),
            integration_version: integration_version.into(),
        };
        if [
            &source.provider_id,
            &source.method,
            &source.integration_version,
        ]
        .iter()
        .any(|value| !super::valid_label(value, 128))
        {
            return Err(ValidationError::InvalidSourceLabel.into());
        }
        Ok(source)
    }

    /// Returns the provider's non-secret attribution identifier.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns the recorded retrieval method label.
    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns the integration version label.
    #[must_use]
    pub fn integration_version(&self) -> &str {
        &self.integration_version
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFields {
    provider_id: String,
    method: String,
    integration_version: String,
}

impl TryFrom<SourceFields> for Source {
    type Error = Error;

    fn try_from(fields: SourceFields) -> Result<Self, Self::Error> {
        Self::new(
            fields.provider_id,
            fields.method,
            fields.integration_version,
        )
    }
}

/// The supported observation operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Retrieval of an EVM address's native asset balance.
    NativeBalance,
}

/// Versioned, explicit context for a native-balance observation.
///
/// Schema version `1` requires a resolved block. Explicit number and hash
/// selectors must match that block. Timestamp and source fields are supplied
/// by the caller; finality initially remains unknown and confirmations absent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields")]
pub struct ObservationContext {
    schema_version: u16,
    operation: Operation,
    network: NetworkId,
    requested_selector: BlockSelector,
    block: BlockContext,
    source: Source,
    retrieved_at: Timestamp,
    finality: Finality,
    confirmations: Option<u64>,
}

impl ObservationContext {
    /// Constructs schema-version-1 native-balance context from explicit facts.
    ///
    /// # Errors
    ///
    /// Rejects an explicit number or hash selector that differs from the
    /// resolved block. Tagged selectors do not infer finality.
    pub fn new(
        network: NetworkId,
        requested_selector: BlockSelector,
        block: BlockContext,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        let mismatched = match requested_selector {
            BlockSelector::Number(number) => number != block.number(),
            BlockSelector::Hash(hash) => hash != *block.hash(),
            BlockSelector::Latest | BlockSelector::Safe | BlockSelector::Finalized => false,
        };
        if mismatched {
            return Err(ValidationError::BlockMismatch.into());
        }
        Ok(Self {
            schema_version: 1,
            operation: Operation::NativeBalance,
            network,
            requested_selector,
            block,
            source,
            retrieved_at,
            finality: Finality::Unknown,
            confirmations: None,
        })
    }

    /// Records explicitly supplied source finality and optional confirmations.
    ///
    /// No finality is inferred from the confirmation count; zero confirmations
    /// and an unknown confirmation count remain distinct.
    #[must_use]
    pub const fn with_finality(mut self, finality: Finality, confirmations: Option<u64>) -> Self {
        self.finality = finality;
        self.confirmations = confirmations;
        self
    }

    /// Returns the supported observation schema version, currently `1`.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    /// Returns the native-balance operation identity.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns the expected chain identity and retained display alias.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }

    /// Returns the selector originally requested.
    #[must_use]
    pub const fn requested_selector(&self) -> &BlockSelector {
        &self.requested_selector
    }

    /// Returns the required resolved block context.
    #[must_use]
    pub const fn block(&self) -> &BlockContext {
        &self.block
    }

    /// Returns explicit attribution labels.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Returns the caller-supplied retrieval timestamp in Unix seconds.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }

    /// Returns the separately recorded source-reported finality.
    #[must_use]
    pub const fn finality(&self) -> Finality {
        self.finality
    }

    /// Returns optional source-reported confirmations without inferring finality.
    #[must_use]
    pub const fn confirmations(&self) -> Option<u64> {
        self.confirmations
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFields {
    schema_version: u16,
    operation: Operation,
    network: NetworkId,
    requested_selector: BlockSelector,
    block: BlockContext,
    source: Source,
    retrieved_at: Timestamp,
    finality: Finality,
    #[serde(deserialize_with = "super::deserialize_optional")]
    confirmations: Option<u64>,
}

impl TryFrom<ContextFields> for ObservationContext {
    type Error = Error;

    fn try_from(fields: ContextFields) -> Result<Self, Self::Error> {
        if fields.schema_version != 1 {
            return Err(ValidationError::UnsupportedSchemaVersion.into());
        }
        let Operation::NativeBalance = fields.operation;
        Self::new(
            fields.network,
            fields.requested_selector,
            fields.block,
            fields.source,
            fields.retrieved_at,
        )
        .map(|context| context.with_finality(fields.finality, fields.confirmations))
    }
}

/// A native balance with validated address, asset identity, and exact amount.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BalanceFields")]
pub struct Balance {
    address: Address,
    asset: Asset,
    amount: Amount,
}

impl Balance {
    /// Constructs a native balance without any implicit decimal precision.
    ///
    /// # Errors
    ///
    /// Rejects unknown amount precision or precision differing from the asset.
    pub fn new(address: Address, asset: Asset, amount: Amount) -> Result<Self, Error> {
        if amount.decimals() != Some(asset.decimals()) {
            return Err(ValidationError::DecimalMismatch.into());
        }
        Ok(Self {
            address,
            asset,
            amount,
        })
    }

    /// Returns the validated EVM address.
    #[must_use]
    pub const fn address(&self) -> &Address {
        &self.address
    }

    /// Returns the native asset identity and explicitly configured metadata.
    #[must_use]
    pub const fn asset(&self) -> &Asset {
        &self.asset
    }

    /// Returns the exact native base-unit amount.
    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.amount
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BalanceFields {
    address: Address,
    asset: Asset,
    amount: Amount,
}

impl TryFrom<BalanceFields> for Balance {
    type Error = Error;

    fn try_from(fields: BalanceFields) -> Result<Self, Self::Error> {
        Self::new(fields.address, fields.asset, fields.amount)
    }
}

/// A value recorded with explicit, versioned source and block context.
///
/// Native balance is the currently constructible value. Its block anchor is
/// required and its chain identity must agree with the balance's chain identity.
/// Display aliases may differ and are retained independently. Construction
/// records supplied observations, without
/// independently verifying a source or making a network request.
///
/// Serialization flattens context fields alongside `value`. Deserialization
/// is currently implemented for [`Balance`] and preserves constructor checks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation<T> {
    #[serde(flatten)]
    context: ObservationContext,
    value: T,
}

impl<T> Observation<T> {
    /// Returns the value and its exact domain identity.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns the required observation context.
    #[must_use]
    pub const fn context(&self) -> &ObservationContext {
        &self.context
    }
}

impl Observation<Balance> {
    /// Records a native balance with a required matching observation context.
    ///
    /// # Errors
    ///
    /// Rejects a context chain identity differing from the asset's chain
    /// identity. Display aliases do not determine compatibility. Block/selector
    /// and precision invariants have already
    /// been validated by the context and balance constructors.
    pub fn native_balance(value: Balance, context: ObservationContext) -> Result<Self, Error> {
        if value.asset().network().chain_id() != context.network().chain_id() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(Self { context, value })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BalanceObservationFields {
    schema_version: u16,
    operation: Operation,
    network: NetworkId,
    requested_selector: BlockSelector,
    block: BlockContext,
    source: Source,
    retrieved_at: Timestamp,
    finality: Finality,
    #[serde(deserialize_with = "super::deserialize_optional")]
    confirmations: Option<u64>,
    value: Balance,
}

impl<'de> Deserialize<'de> for Observation<Balance> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = BalanceObservationFields::deserialize(deserializer)?;
        let context = ObservationContext::try_from(ContextFields {
            schema_version: fields.schema_version,
            operation: fields.operation,
            network: fields.network,
            requested_selector: fields.requested_selector,
            block: fields.block,
            source: fields.source,
            retrieved_at: fields.retrieved_at,
            finality: fields.finality,
            confirmations: fields.confirmations,
        })
        .map_err(serde::de::Error::custom)?;
        Self::native_balance(fields.value, context).map_err(serde::de::Error::custom)
    }
}
