// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, str::FromStr};

use ethaddr::Address as EvmAddress;
use ruint::aliases::U256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, ValidationError};

use super::amount::parse_uint;

/// An EVM chain identifier, retaining the full unsigned 256-bit range.
///
/// Serialization is a canonical decimal string, including for small values.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ChainId(U256);

impl ChainId {
    /// Constructs an EVM chain identifier without narrowing its numeric range.
    #[must_use]
    pub const fn new(value: U256) -> Self {
        Self(value)
    }

    /// Parses a canonical unsigned decimal EVM chain identifier.
    ///
    /// # Errors
    ///
    /// Returns a typed validation error for malformed or overflowing input.
    pub fn from_decimal(value: &str) -> Result<Self, Error> {
        parse_uint(value).map(Self)
    }

    /// Returns the exact EVM chain identifier.
    #[must_use]
    pub const fn value(self) -> U256 {
        self.0
    }
}

impl From<u64> for ChainId {
    fn from(value: u64) -> Self {
        Self(U256::from(value))
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for ChainId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ChainId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_decimal(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Expected EVM chain identity and a separate display alias.
///
/// The alias is caller-supplied display metadata, not an on-chain identifier.
/// Equality includes both fields; compare [`Self::chain_id`] for chain identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct NetworkId {
    chain_id: ChainId,
    alias: String,
}

impl NetworkId {
    /// Constructs a network identity with a bounded non-secret display alias.
    ///
    /// # Errors
    ///
    /// Rejects aliases outside 1–64 ASCII letters, digits, `-`, `_`, `.`, `+`.
    pub fn new(chain_id: ChainId, alias: impl Into<String>) -> Result<Self, Error> {
        let alias = alias.into();
        if !super::valid_label(&alias, 64) {
            return Err(ValidationError::InvalidNetworkAlias.into());
        }
        Ok(Self { chain_id, alias })
    }

    /// Returns the on-chain identity, independently of display metadata.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Returns the caller-supplied display alias.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkFields {
    chain_id: ChainId,
    alias: String,
}

impl TryFrom<NetworkFields> for NetworkId {
    type Error = Error;

    fn try_from(fields: NetworkFields) -> Result<Self, Self::Error> {
        Self::new(fields.chain_id, fields.alias)
    }
}

/// A validated 20-byte EVM address.
///
/// Parsing accepts all-lowercase and all-uppercase hexadecimal payloads. A
/// mixed-case payload must have a valid EIP-55 checksum. Serialization and
/// display use canonical lowercase hexadecimal with a `0x` prefix.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Address(EvmAddress);

impl Address {
    /// Parses a `0x`-prefixed, exactly 40-digit EVM address.
    ///
    /// # Errors
    ///
    /// Rejects invalid hexadecimal, length, prefix, or mixed-case checksum.
    /// The error never includes the supplied input.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let Some(payload) = value.strip_prefix("0x") else {
            return Err(ValidationError::InvalidAddress.into());
        };
        if payload.len() != 40 || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidAddress.into());
        }
        let lower = payload.bytes().any(|byte| byte.is_ascii_lowercase());
        let upper = payload.bytes().any(|byte| byte.is_ascii_uppercase());
        let address = if lower && upper {
            EvmAddress::from_str_checksum(value)
                .map_err(|_| Error::from(ValidationError::InvalidAddress))
        } else {
            EvmAddress::from_str(value).map_err(|_| Error::from(ValidationError::InvalidAddress))
        }?;
        Ok(Self(address))
    }

    /// Constructs an address from its exact bytes, with no textual ambiguity.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(EvmAddress(bytes))
    }

    /// Returns the exact 20-byte address.
    #[must_use]
    pub const fn bytes(self) -> [u8; 20] {
        self.0.0
    }
}

impl FromStr for Address {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#x}", self.0)
    }
}

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// The supported asset identity kind.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// The native asset of an EVM chain.
    Native,
}

/// On-chain asset identity, excluding symbols, aliases, and metadata origin.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct AssetId {
    chain_id: ChainId,
    kind: AssetKind,
}

impl AssetId {
    /// Returns the EVM chain identity.
    #[must_use]
    pub const fn chain_id(self) -> ChainId {
        self.chain_id
    }

    /// Returns the on-chain asset kind.
    #[must_use]
    pub const fn kind(self) -> AssetKind {
        self.kind
    }
}

/// Where native asset display metadata and decimal precision originated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataOrigin {
    /// Metadata was supplied explicitly by the caller, without an on-chain claim.
    CallerConfigured,
}

/// A native EVM asset with explicitly configured decimal precision.
///
/// A symbol is display metadata and does not determine identity. Compare
/// [`Self::identity`] for on-chain identity; structural equality retains every
/// field, including display metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AssetFields")]
pub struct Asset {
    network: NetworkId,
    kind: AssetKind,
    decimals: u8,
    symbol: Option<String>,
    metadata_origin: MetadataOrigin,
}

impl Asset {
    /// Constructs a native asset with caller-configured metadata.
    ///
    /// # Errors
    ///
    /// Rejects a present symbol outside 1–32 ASCII letters, digits, `-`, `_`,
    /// `.`, `+`. No decimal precision is inferred.
    pub fn native(network: NetworkId, decimals: u8, symbol: Option<String>) -> Result<Self, Error> {
        if symbol
            .as_ref()
            .is_some_and(|value| !super::valid_label(value, 32))
        {
            return Err(ValidationError::InvalidAssetSymbol.into());
        }
        Ok(Self {
            network,
            kind: AssetKind::Native,
            decimals,
            symbol,
            metadata_origin: MetadataOrigin::CallerConfigured,
        })
    }

    /// Returns on-chain identity, independent of aliases or display metadata.
    #[must_use]
    pub const fn identity(&self) -> AssetId {
        AssetId {
            chain_id: self.network.chain_id(),
            kind: self.kind,
        }
    }

    /// Returns the expected network and its display alias.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }

    /// Returns the native asset kind.
    #[must_use]
    pub const fn kind(&self) -> AssetKind {
        self.kind
    }

    /// Returns explicitly supplied native decimal precision.
    #[must_use]
    pub const fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Returns optional caller-supplied display metadata.
    #[must_use]
    pub fn symbol(&self) -> Option<&str> {
        self.symbol.as_deref()
    }

    /// Returns the origin of decimal precision and optional symbol.
    #[must_use]
    pub const fn metadata_origin(&self) -> MetadataOrigin {
        self.metadata_origin
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetFields {
    network: NetworkId,
    kind: AssetKind,
    decimals: u8,
    symbol: Option<String>,
    metadata_origin: MetadataOrigin,
}

impl TryFrom<AssetFields> for Asset {
    type Error = Error;

    fn try_from(fields: AssetFields) -> Result<Self, Self::Error> {
        let AssetKind::Native = fields.kind;
        let MetadataOrigin::CallerConfigured = fields.metadata_origin;
        Self::native(fields.network, fields.decimals, fields.symbol)
    }
}
