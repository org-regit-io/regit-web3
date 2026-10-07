// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::U256;
use crate::{
    domain::amount::parse_uint,
    error::{Error, ValidationError},
};

/// An exact EVM integer serialized as a canonical unsigned decimal string.
///
/// Units are declared by the enclosing field; this type never applies decimals.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Quantity(U256);
impl Quantity {
    /// Records an exact integer without narrowing its width.
    #[must_use]
    pub const fn new(value: U256) -> Self {
        Self(value)
    }
    /// Parses a canonical unsigned decimal integer.
    ///
    /// # Errors
    /// Rejects malformed or overflowing integers without echoing input.
    pub fn from_decimal(value: &str) -> Result<Self, Error> {
        parse_uint(value).map(Self)
    }
    /// Returns the exact unsigned value.
    #[must_use]
    pub const fn value(self) -> U256 {
        self.0
    }
}
impl From<u64> for Quantity {
    fn from(value: u64) -> Self {
        Self(U256::from(value))
    }
}
impl Serialize for Quantity {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::from_decimal(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// A 32-byte EVM transaction identifier, distinct from a block hash.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransactionId([u8; 32]);
impl TransactionId {
    /// Constructs an identifier from exact bytes; existence is not implied.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Parses an exactly 32-byte, `0x`-prefixed hexadecimal identifier.
    ///
    /// # Errors
    /// Rejects invalid prefix, hexadecimal or width without echoing input.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let payload = value
            .strip_prefix("0x")
            .ok_or(ValidationError::InvalidEvmTransactionId)?;
        if payload.len() != 64 || !payload.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidEvmTransactionId.into());
        }
        const_hex::decode_to_array(payload)
            .map(Self)
            .map_err(|_| ValidationError::InvalidEvmTransactionId.into())
    }
    /// Returns the exact identifier bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
impl FromStr for TransactionId {
    type Err = Error;
    fn from_str(v: &str) -> Result<Self, Error> {
        Self::parse(v)
    }
}
impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&const_hex::encode_prefixed(self.0))
    }
}
impl Serialize for TransactionId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for TransactionId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Bounded opaque EVM data, serialized as lowercase `0x`-prefixed hexadecimal.
///
/// The maximum is 1 MiB. Empty data is valid. Debug does not expose its content.
#[derive(Clone, Eq, PartialEq)]
pub struct Data(Vec<u8>);
impl Data {
    /// The maximum supported data length in bytes.
    pub const MAX_BYTES: usize = 1_048_576;
    /// Records bounded opaque bytes without interpreting their contents.
    ///
    /// # Errors
    /// Rejects values exceeding the explicit byte limit.
    pub fn new(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(ValidationError::InvalidEvmBytes.into());
        }
        Ok(Self(bytes))
    }
    /// Parses byte-aligned, bounded, `0x`-prefixed hexadecimal data.
    ///
    /// # Errors
    /// Rejects malformed or oversized input without echoing it.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let payload = value
            .strip_prefix("0x")
            .ok_or(ValidationError::InvalidEvmBytes)?;
        if !payload.len().is_multiple_of(2)
            || payload.len() > Self::MAX_BYTES * 2
            || !payload.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ValidationError::InvalidEvmBytes.into());
        }
        const_hex::decode(payload)
            .map(Self)
            .map_err(|_| ValidationError::InvalidEvmBytes.into())
    }
    /// Returns the explicit byte contents.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
    /// Returns the canonical hexadecimal representation.
    #[must_use]
    pub fn to_hex(&self) -> String {
        const_hex::encode_prefixed(&self.0)
    }
}
impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Data")
            .field("byte_length", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl Serialize for Data {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}
impl<'de> Deserialize<'de> for Data {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// An exact 32-byte protocol word, independent of block/transaction identity.
///
/// Storage keys, log topics, versioned blob hashes and historical state roots
/// use their enclosing field's meaning; this type does not imply a block hash.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Word([u8; 32]);
impl Word {
    /// Constructs a protocol word from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Parses exactly 32 bytes of `0x`-prefixed hexadecimal.
    ///
    /// # Errors
    /// Rejects invalid width, hexadecimal or prefix without echoing input.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let payload = value
            .strip_prefix("0x")
            .ok_or(ValidationError::InvalidEvmBytes)?;
        if payload.len() != 64 || !payload.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ValidationError::InvalidEvmBytes.into());
        }
        const_hex::decode_to_array(payload)
            .map(Self)
            .map_err(|_| ValidationError::InvalidEvmBytes.into())
    }
    /// Returns exact protocol bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
impl fmt::Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&const_hex::encode_prefixed(self.0))
    }
}
impl Serialize for Word {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for Word {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
