// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::invalid;
use crate::{
    domain::{Amount, U256, valid_label},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use std::{fmt, marker::PhantomData};

/// Family-specific standard network identity policy used by indexed observations.
#[doc(hidden)]
pub trait NetworkPolicy:
    super::sealed::Network + Copy + fmt::Debug + Eq + Serialize + DeserializeOwned
{
    /// Returns the complete genesis identity from the family's primary implementation.
    fn genesis_hash(self) -> BlockHash;
    /// Returns the documented `BlockCypher` mainnet name, or no supported backend network.
    fn blockcypher_name(self) -> Option<&'static str>;
}

/// Family-specific address validation and native atomic-unit policy.
#[doc(hidden)]
pub trait AddressPolicy:
    super::sealed::Address + Clone + fmt::Debug + fmt::Display + Eq + Serialize + DeserializeOwned
{
    /// The family's declared standard network.
    type Network: NetworkPolicy;
    /// The native atomic-unit name; both supported families use eight decimal digits.
    const ATOMIC_UNIT: &'static str;
    /// The current Core transaction money-range limit, not a lifetime-flow limit.
    const TRANSACTION_MAXIMUM: u64;
    /// Whether the family has ordinary segregated-witness transaction data.
    const SUPPORTS_WITNESS: bool;
    /// Parses a network-compatible address with maintained checksums.
    /// # Errors
    /// Rejects unsupported or invalid address encodings with fixed diagnostics.
    fn parse(value: &str, network: Self::Network) -> Result<Self, Error>;
    /// Returns the caller-qualified standard network.
    fn network(&self) -> Self::Network;
}

macro_rules! hash_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);
        impl $name {
            /// Records an exact displayed-hash byte sequence without cryptographic verification.
            #[must_use]
            pub const fn from_display_bytes(value: [u8; 32]) -> Self {
                Self(value)
            }
            /// Parses exactly 64 hexadecimal digits; prefixes and whitespace are rejected.
            /// # Errors
            /// Rejects malformed identifiers without retaining supplied input.
            pub fn parse(value: &str) -> Result<Self, Error> {
                let decoded = const_hex::decode_to_array(value).map_err(|_| invalid())?;
                if value.len() != 64 {
                    return Err(invalid());
                }
                Ok(Self(decoded))
            }
            /// Returns the exact displayed-hash bytes.
            #[must_use]
            pub const fn display_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&const_hex::encode(self.0))
            }
        }
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
            }
        }
    };
}
hash_type!(
    BlockHash,
    "An exact source-reported block hash; no proof-of-work or inclusion proof is implied."
);
hash_type!(
    Txid,
    "An exact source-reported transaction identifier; raw bytes are not decoded or independently hashed."
);

/// A complete standard genesis identity with separate caller-owned display metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields<N>", bound(deserialize = "N: NetworkPolicy"))]
pub struct NetworkId<N: NetworkPolicy> {
    network: N,
    genesis_hash: BlockHash,
    alias: String,
}
impl<N: NetworkPolicy> NetworkId<N> {
    /// Declares a standard expected network and bounded non-secret display alias.
    /// # Errors
    /// Rejects an invalid alias.
    pub fn new(network: N, alias: impl Into<String>) -> Result<Self, Error> {
        let alias = alias.into();
        if !valid_label(&alias, 64) {
            return Err(ValidationError::InvalidNetworkAlias.into());
        }
        Ok(Self {
            network,
            genesis_hash: network.genesis_hash(),
            alias,
        })
    }
    /// Returns the declared family-specific network.
    #[must_use]
    pub const fn network(&self) -> N {
        self.network
    }
    /// Returns the complete expected genesis identity.
    #[must_use]
    pub const fn genesis_hash(&self) -> &BlockHash {
        &self.genesis_hash
    }
    /// Returns caller-supplied display metadata.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "N: NetworkPolicy"))]
struct NetworkFields<N: NetworkPolicy> {
    network: N,
    genesis_hash: BlockHash,
    alias: String,
}
impl<N: NetworkPolicy> TryFrom<NetworkFields<N>> for NetworkId<N> {
    type Error = Error;
    fn try_from(value: NetworkFields<N>) -> Result<Self, Error> {
        let network = Self::new(value.network, value.alias)?;
        if network.genesis_hash != value.genesis_hash {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(network)
    }
}

/// An exact unsigned native atomic-unit quantity associated with its address family.
/// Lifetime funding/spending values are not restricted to a transaction money limit.
#[derive(Debug, Eq, PartialEq)]
pub struct AtomicAmount<A> {
    raw: u64,
    family: PhantomData<fn() -> A>,
}
impl<A> Copy for AtomicAmount<A> {}
impl<A> Clone for AtomicAmount<A> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<A: AddressPolicy> AtomicAmount<A> {
    /// Records exact unsigned atomic units without floating point.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self {
            raw,
            family: PhantomData,
        }
    }
    /// Parses canonical unsigned decimal units within the source's 64-bit range.
    /// # Errors
    /// Rejects signs, whitespace, leading zeroes and overflow.
    pub fn from_decimal(value: &str) -> Result<Self, Error> {
        if value.is_empty()
            || !value.bytes().all(|b| b.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(invalid());
        }
        value.parse().map(Self::new).map_err(|_| invalid())
    }
    /// Returns the exact atomic integer.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.raw
    }
    /// Returns the exact amount with the family's eight native decimal digits.
    #[must_use]
    pub fn amount(self) -> Amount {
        Amount::new(U256::from(self.raw), Some(8))
    }
    /// Returns the family's explicit native atomic-unit name.
    #[must_use]
    pub const fn unit() -> &'static str {
        A::ATOMIC_UNIT
    }
}
impl<A: AddressPolicy> Serialize for AtomicAmount<A> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.raw.to_string())
    }
}
impl<'de, A: AddressPolicy> Deserialize<'de> for AtomicAmount<A> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::from_decimal(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// An exact signed unconfirmed native-unit delta, distinct from a confirmed balance.
#[derive(Debug, Eq, PartialEq)]
pub struct MempoolDelta<A> {
    raw: i128,
    family: PhantomData<fn() -> A>,
}
impl<A> Copy for MempoolDelta<A> {}
impl<A> Clone for MempoolDelta<A> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<A: AddressPolicy> MempoolDelta<A> {
    /// Records a signed delta whose magnitude fits the source's unsigned 64-bit range.
    /// # Errors
    /// Rejects excessive magnitude.
    pub fn new(raw: i128) -> Result<Self, Error> {
        if raw.unsigned_abs() > u128::from(u64::MAX) {
            return Err(invalid());
        }
        Ok(Self {
            raw,
            family: PhantomData,
        })
    }
    /// Parses canonical signed integer units; negative zero is rejected.
    /// # Errors
    /// Rejects invalid notation, fractional units and excessive magnitude.
    pub fn from_decimal(value: &str) -> Result<Self, Error> {
        let magnitude = value.strip_prefix('-').unwrap_or(value);
        if magnitude.is_empty()
            || !magnitude.bytes().all(|b| b.is_ascii_digit())
            || (magnitude.len() > 1 && magnitude.starts_with('0'))
            || value == "-0"
        {
            return Err(invalid());
        }
        Self::new(value.parse().map_err(|_| invalid())?)
    }
    /// Returns the exact signed atomic-unit delta.
    #[must_use]
    pub const fn raw(&self) -> i128 {
        self.raw
    }
}
impl<A: AddressPolicy> Serialize for MempoolDelta<A> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.raw.to_string())
    }
}
impl<'de, A: AddressPolicy> Deserialize<'de> for MempoolDelta<A> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::from_decimal(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Bounded opaque source bytes, scripts or raw transaction payloads.
/// Encoding validity and resource bounds do not establish script or consensus validity.
#[derive(Clone, Eq, PartialEq)]
pub struct Bytes(Vec<u8>);
impl Bytes {
    /// The local per-value byte bound, independent of family consensus limits.
    pub const MAXIMUM_BYTES: usize = 1_048_576;
    /// Records bounded bytes without decoding any transaction or script dialect.
    /// # Errors
    /// Rejects an excessive resource size.
    pub fn new(value: Vec<u8>) -> Result<Self, Error> {
        if value.len() > Self::MAXIMUM_BYTES {
            return Err(ValidationError::InvalidUtxoBytes.into());
        }
        Ok(Self(value))
    }
    /// Parses bounded even-width hexadecimal; explicit empty bytes remain empty.
    /// # Errors
    /// Rejects prefixes, invalid encoding and excessive resource size.
    pub fn from_hex(value: &str) -> Result<Self, Error> {
        if value.len() > Self::MAXIMUM_BYTES * 2
            || !value.len().is_multiple_of(2)
            || !value.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ValidationError::InvalidUtxoBytes.into());
        }
        Self::new(
            const_hex::decode(value)
                .map_err(|_| Error::Validation(ValidationError::InvalidUtxoBytes))?,
        )
    }
    /// Returns the exact source byte sequence.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bytes")
            .field("length", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&const_hex::encode(&self.0))
    }
}
impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::from_hex(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Bounded printable source metadata; timestamps remain source text rather than inferred clocks.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceText(String);
impl SourceText {
    /// Records at most 128 printable UTF-8 bytes without retaining them in diagnostics.
    /// # Errors
    /// Rejects control characters and excessive size.
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.len() > 128 || value.chars().any(char::is_control) {
            return Err(invalid());
        }
        Ok(Self(value))
    }
    /// Returns exact source text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SourceText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceText")
            .field("bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl TryFrom<String> for SourceText {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<SourceText> for String {
    fn from(value: SourceText) -> Self {
        value.0
    }
}
