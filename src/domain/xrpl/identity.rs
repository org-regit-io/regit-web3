// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, ValidationError};

fn invalid_address() -> Error {
    ValidationError::InvalidXrplAddress.into()
}

fn encode(bytes: &[u8]) -> String {
    bs58::encode(bytes)
        .with_alphabet(bs58::Alphabet::RIPPLE)
        .with_check()
        .into_string()
}

fn decode(text: &str, maximum: usize) -> Result<Vec<u8>, Error> {
    if text.is_empty() || text.len() > maximum {
        return Err(invalid_address());
    }
    let bytes = bs58::decode(text)
        .with_alphabet(bs58::Alphabet::RIPPLE)
        .with_check(None)
        .into_vec()
        .map_err(|_| invalid_address())?;
    if encode(&bytes) != text {
        return Err(invalid_address());
    }
    Ok(bytes)
}

macro_rules! string_serde {
    ($name:ident) => {
        impl FromStr for $name {
            type Err = Error;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

/// A checksum-valid canonical classic account address, independent of network.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Address([u8; 20]);

impl Address {
    /// Parses the classic version byte, 20-byte account ID and `Base58Check` checksum.
    ///
    /// # Errors
    /// Rejects seeds, public keys, other prefixes, invalid widths and checksums.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let bytes = decode(text, 35)?;
        if bytes.len() != 21 || bytes.first() != Some(&0) {
            return Err(invalid_address());
        }
        let account = bytes[1..].try_into().map_err(|_| invalid_address())?;
        Ok(Self(account))
    }

    /// Constructs an account identity from bytes, without proving ownership.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// Returns the exact account ID.
    #[must_use]
    pub const fn bytes(self) -> [u8; 20] {
        self.0
    }
}
impl fmt::Display for Address {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes = [0; 21];
        bytes[1..].copy_from_slice(&self.0);
        formatter.write_str(&encode(&bytes))
    }
}
string_serde!(Address);

/// An X-address retaining its account, optional tag and main/test category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAddress {
    address: Address,
    tag: Option<u32>,
    test: bool,
}

/// The main/test encoding category carried by an X-address.
///
/// It does not identify a particular test network or custom production chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XAddressCategory {
    /// Main-network X-address encoding.
    Main,
    /// Test-network X-address encoding.
    Test,
}
impl XAddress {
    /// Records the category and optional tag; tag zero differs from no tag.
    #[must_use]
    pub const fn new(address: Address, tag: Option<u32>, test: bool) -> Self {
        Self { address, tag, test }
    }

    /// Decodes a checksummed X-address including reserved bytes and tag flag.
    ///
    /// # Errors
    /// Rejects invalid prefixes, widths, flags and nonzero reserved bytes.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let bytes = decode(text, 50)?;
        if bytes.len() != 31 {
            return Err(invalid_address());
        }
        let test_category = match &bytes[..2] {
            [0x05, 0x44] => false,
            [0x04, 0x93] => true,
            _ => return Err(invalid_address()),
        };
        let address = Address::from_bytes(bytes[2..22].try_into().map_err(|_| invalid_address())?);
        if bytes[27..].iter().any(|byte| *byte != 0) {
            return Err(invalid_address());
        }
        let raw_tag = u32::from_le_bytes(bytes[23..27].try_into().map_err(|_| invalid_address())?);
        let tag = match bytes[22] {
            0 if raw_tag == 0 => None,
            1 => Some(raw_tag),
            _ => return Err(invalid_address()),
        };
        Ok(Self {
            address,
            tag,
            test: test_category,
        })
    }

    /// Returns the classic account.
    #[must_use]
    pub const fn address(self) -> Address {
        self.address
    }
    /// Returns the destination tag, preserving explicit zero.
    #[must_use]
    pub const fn tag(self) -> Option<u32> {
        self.tag
    }
    /// Reports the test category, which does not distinguish testnet and devnet.
    #[must_use]
    pub const fn is_test(self) -> bool {
        self.test
    }
}
impl fmt::Display for XAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes = [0; 31];
        bytes[..2].copy_from_slice(if self.test {
            &[0x04, 0x93]
        } else {
            &[0x05, 0x44]
        });
        bytes[2..22].copy_from_slice(&self.address.bytes());
        if let Some(tag) = self.tag {
            bytes[22] = 1;
            bytes[23..27].copy_from_slice(&tag.to_le_bytes());
        }
        formatter.write_str(&encode(&bytes))
    }
}
string_serde!(XAddress);

/// The explicit server-reported XRP Ledger network ID.
///
/// This ID is not an independently verified genesis or validator-set identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NetworkId(u32);
impl NetworkId {
    /// Records an explicit mainnet, testnet, devnet or custom network number.
    #[must_use]
    pub const fn new(number: u32) -> Self {
        Self(number)
    }
    /// Returns the network number; mainnet uses zero.
    #[must_use]
    pub const fn number(self) -> u32 {
        self.0
    }
}

/// Technical network number with a separate display alias.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct Network {
    identity: NetworkId,
    alias: String,
}
impl Network {
    /// Records the explicit network ID and bounded alias.
    ///
    /// # Errors
    /// Rejects empty aliases or aliases outside the shared label contract.
    pub fn new(identity: NetworkId, alias: impl Into<String>) -> Result<Self, Error> {
        let alias = alias.into();
        if !crate::domain::valid_label(&alias, 64) {
            return Err(ValidationError::InvalidNetworkAlias.into());
        }
        Ok(Self { identity, alias })
    }
    /// Returns the technical network number.
    #[must_use]
    pub const fn identity(&self) -> NetworkId {
        self.identity
    }
    /// Returns the independent display alias.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkFields {
    identity: NetworkId,
    alias: String,
}
impl TryFrom<NetworkFields> for Network {
    type Error = Error;
    fn try_from(value: NetworkFields) -> Result<Self, Error> {
        Self::new(value.identity, value.alias)
    }
}

/// A 32-byte transaction or ledger hash in canonical uppercase hexadecimal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hash([u8; 32]);
impl Hash {
    /// Parses exactly 64 hexadecimal characters without a prefix.
    ///
    /// # Errors
    /// Rejects invalid encoding or width with a fixed diagnostic.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.len() != 64 {
            return Err(ValidationError::InvalidXrplHash.into());
        }
        const_hex::decode_to_array(text)
            .map(Self)
            .map_err(|_| ValidationError::InvalidXrplHash.into())
    }
    /// Constructs an identity from exact bytes without cryptographic verification.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Returns the exact bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
impl fmt::Display for Hash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&const_hex::encode_upper(self.0))
    }
}
string_serde!(Hash);

/// An issued-currency identifier retaining its exact 160-bit protocol identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Currency([u8; 20]);
impl Currency {
    /// Parses a permitted case-sensitive three-character code or 40-digit hex.
    ///
    /// Equivalent standard codes and their hexadecimal form share identity.
    ///
    /// # Errors
    /// Rejects native XRP identities, malformed codes and invalid widths.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut bytes = [0; 20];
        if text.len() == 3 && text.bytes().all(currency_character) {
            bytes[12..15].copy_from_slice(text.as_bytes());
        } else if text.len() == 40 {
            bytes = const_hex::decode_to_array(text)
                .map_err(|_| Error::from(ValidationError::InvalidXrplCurrency))?;
        } else {
            return Err(ValidationError::InvalidXrplCurrency.into());
        }
        let mut xrp = [0; 20];
        xrp[12..15].copy_from_slice(b"XRP");
        if bytes == [0; 20] || bytes == xrp {
            return Err(ValidationError::InvalidXrplCurrency.into());
        }
        Ok(Self(bytes))
    }
    /// Returns the exact currency bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 20] {
        self.0
    }
}
fn currency_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"?!@#$%^&*<>(){}[]|".contains(&byte)
}
impl fmt::Display for Currency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0[..12]
            .iter()
            .chain(&self.0[15..])
            .all(|byte| *byte == 0)
            && self.0[12..15].iter().all(|byte| currency_character(*byte))
        {
            match std::str::from_utf8(&self.0[12..15]) {
                Ok(code) => formatter.write_str(code),
                Err(_) => Err(fmt::Error),
            }
        } else {
            formatter.write_str(&const_hex::encode_upper(self.0))
        }
    }
}
string_serde!(Currency);
