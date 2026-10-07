// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{AddressPolicy, BlockHash, NetworkPolicy};
use crate::error::{Error, ValidationError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Standard Dogecoin networks from current Core network parameters.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    /// The public production chain.
    Mainnet,
    /// The public Core test network.
    Testnet,
    /// Core regression testing; overlapping prefixes only establish compatibility.
    Regtest,
}
impl Network {
    /// Returns the complete standard Core genesis identity.
    #[must_use]
    pub fn genesis_hash(self) -> BlockHash {
        NetworkPolicy::genesis_hash(self)
    }
    fn prefixes(self) -> (u8, u8) {
        match self {
            Self::Mainnet => (30, 22),
            Self::Testnet => (113, 196),
            Self::Regtest => (111, 196),
        }
    }
}
impl NetworkPolicy for Network {
    fn genesis_hash(self) -> BlockHash {
        BlockHash::from_display_bytes(match self {
            Self::Mainnet => [
                0x1a, 0x91, 0xe3, 0xda, 0xce, 0x36, 0xe2, 0xbe, 0x3b, 0xf0, 0x30, 0xa6, 0x56, 0x79,
                0xfe, 0x82, 0x1a, 0xa1, 0xd6, 0xef, 0x92, 0xe7, 0xc9, 0x90, 0x2e, 0xb3, 0x18, 0x18,
                0x2c, 0x35, 0x56, 0x91,
            ],
            Self::Testnet => [
                0xbb, 0x0a, 0x78, 0x26, 0x46, 0x37, 0x40, 0x6b, 0x63, 0x60, 0xaa, 0xd9, 0x26, 0x28,
                0x4d, 0x54, 0x4d, 0x70, 0x49, 0xf4, 0x51, 0x89, 0xdb, 0x56, 0x64, 0xf3, 0xc4, 0xd0,
                0x73, 0x50, 0x55, 0x9e,
            ],
            Self::Regtest => [
                0x3d, 0x21, 0x60, 0xa3, 0xb5, 0xdc, 0x4a, 0x9d, 0x62, 0xe7, 0xe6, 0x6a, 0x29, 0x5f,
                0x70, 0x31, 0x3a, 0xc8, 0x08, 0x44, 0x0e, 0xf7, 0x40, 0x0d, 0x6c, 0x07, 0x72, 0x17,
                0x1c, 0xe9, 0x73, 0xa5,
            ],
        })
    }
    fn blockcypher_name(self) -> Option<&'static str> {
        match self {
            Self::Mainnet => Some("DOGE.main"),
            Self::Testnet | Self::Regtest => None,
        }
    }
}
impl crate::domain::indexed_utxo::sealed::Network for Network {}

/// Supported ordinary Dogecoin address encoding category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressKind {
    /// Core public-key-hash `Base58Check` address.
    PubkeyHash,
    /// Core script-hash `Base58Check` address.
    ScriptHash,
}

/// A maintained-checksum Dogecoin address qualified by an explicit compatible network.
/// Dogecoin has no `SegWit` address form; foreign Bech32 encodings are rejected.
/// Shared test/regression prefixes cannot prove which remote chain serves an address.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AddressFields", into = "AddressFields")]
pub struct Address {
    network: Network,
    encoded: String,
    kind: AddressKind,
}
impl Address {
    /// Validates payload width, maintained checksum, canonical encoding and network parameters.
    /// # Errors
    /// Rejects unsupported/invalid encodings or incompatible network parameters with fixed errors.
    pub fn parse(value: &str, network: Network) -> Result<Self, Error> {
        let invalid = || Error::Validation(ValidationError::InvalidDogecoinAddress);
        if value.is_empty() || value.len() > 90 || value.chars().any(char::is_whitespace) {
            return Err(invalid());
        }
        let bytes = bs58::decode(value)
            .with_check(None)
            .into_vec()
            .map_err(|_| invalid())?;
        if bytes.len() != 21 || bs58::encode(&bytes).with_check().into_string() != value {
            return Err(invalid());
        }
        let (public, script) = network.prefixes();
        let kind = if bytes[0] == public {
            AddressKind::PubkeyHash
        } else if bytes[0] == script {
            AddressKind::ScriptHash
        } else {
            return Err(ValidationError::NetworkMismatch.into());
        };
        Ok(Self {
            network,
            encoded: bs58::encode(bytes).with_check().into_string(),
            kind,
        })
    }
    /// Returns the caller-qualified compatible network.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
    /// Returns the validated ordinary encoding category.
    #[must_use]
    pub const fn kind(&self) -> AddressKind {
        self.kind
    }
}
impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.encoded)
    }
}
impl AddressPolicy for Address {
    type Network = Network;
    const ATOMIC_UNIT: &'static str = "koinu";
    const TRANSACTION_MAXIMUM: u64 = 1_000_000_000_000_000_000;
    const SUPPORTS_WITNESS: bool = false;
    fn parse(value: &str, network: Network) -> Result<Self, Error> {
        Self::parse(value, network)
    }
    fn network(&self) -> Network {
        self.network
    }
}
impl crate::domain::indexed_utxo::sealed::Address for Address {}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AddressFields {
    network: Network,
    encoded: String,
}
impl TryFrom<AddressFields> for Address {
    type Error = Error;
    fn try_from(v: AddressFields) -> Result<Self, Error> {
        Self::parse(&v.encoded, v.network)
    }
}
impl From<Address> for AddressFields {
    fn from(v: Address) -> Self {
        Self {
            network: v.network,
            encoded: v.encoded,
        }
    }
}
