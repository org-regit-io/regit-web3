// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{AddressPolicy, BlockHash, NetworkPolicy};
use crate::error::{Error, ValidationError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Standard Litecoin networks from current Core network parameters.
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
    fn prefixes(self) -> (u8, u8, u8) {
        match self {
            Self::Mainnet => (48, 50, 5),
            Self::Testnet | Self::Regtest => (111, 58, 196),
        }
    }
    fn hrp(self) -> &'static str {
        match self {
            Self::Mainnet => "ltc",
            Self::Testnet => "tltc",
            Self::Regtest => "rltc",
        }
    }
}
impl NetworkPolicy for Network {
    fn genesis_hash(self) -> BlockHash {
        BlockHash::from_display_bytes(match self {
            Self::Mainnet => [
                0x12, 0xa7, 0x65, 0xe3, 0x1f, 0xfd, 0x40, 0x59, 0xba, 0xda, 0x1e, 0x25, 0x19, 0x0f,
                0x6e, 0x98, 0xc9, 0x9d, 0x97, 0x14, 0xd3, 0x34, 0xef, 0xa4, 0x1a, 0x19, 0x5a, 0x7e,
                0x7e, 0x04, 0xbf, 0xe2,
            ],
            Self::Testnet => [
                0x49, 0x66, 0x62, 0x5a, 0x4b, 0x28, 0x51, 0xd9, 0xfd, 0xee, 0x13, 0x9e, 0x56, 0x21,
                0x1a, 0x0d, 0x88, 0x57, 0x5f, 0x59, 0xed, 0x81, 0x6f, 0xf5, 0xe6, 0xa6, 0x3d, 0xeb,
                0x4e, 0x3e, 0x29, 0xa0,
            ],
            Self::Regtest => [
                0x53, 0x08, 0x27, 0xf3, 0x8f, 0x93, 0xb4, 0x3e, 0xd1, 0x2a, 0xf0, 0xb3, 0xad, 0x25,
                0xa2, 0x88, 0xdc, 0x02, 0xed, 0x74, 0xd6, 0xd7, 0x85, 0x78, 0x62, 0xdf, 0x51, 0xfc,
                0x56, 0xc4, 0x16, 0xf9,
            ],
        })
    }
    fn blockcypher_name(self) -> Option<&'static str> {
        match self {
            Self::Mainnet => Some("LTC.main"),
            Self::Testnet | Self::Regtest => None,
        }
    }
}
impl crate::domain::indexed_utxo::sealed::Network for Network {}

/// Supported ordinary Litecoin address encoding category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressKind {
    /// Core public-key-hash `Base58Check` address.
    PubkeyHash,
    /// Core script-hash `Base58Check` address.
    ScriptHash,
    /// `BIP-173/BIP-350` witness-program address; this category does not prove spendability.
    Witness {
        /// Exact witness version in 0..=16.
        version: u8,
    },
}

/// A maintained-checksum Litecoin address qualified by an explicit compatible network.
/// Legacy Litecoin P2SH encodings normalize to the current Core prefix.
/// `MWEB` stealth addresses require separate semantics and are rejected by this type.
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
        let invalid = || Error::Validation(ValidationError::InvalidLitecoinAddress);
        if value.is_empty() || value.len() > 90 || value.chars().any(char::is_whitespace) {
            return Err(invalid());
        }
        if value.to_ascii_lowercase().starts_with("ltc1")
            || value.to_ascii_lowercase().starts_with("tltc1")
            || value.to_ascii_lowercase().starts_with("rltc1")
        {
            let (hrp, version, program) = bech32::segwit::decode(value).map_err(|_| invalid())?;
            if !hrp.as_str().eq_ignore_ascii_case(network.hrp()) {
                return Err(ValidationError::NetworkMismatch.into());
            }
            let encoded = bech32::segwit::encode(hrp, version, &program).map_err(|_| invalid())?;
            if encoded != value.to_ascii_lowercase() {
                return Err(invalid());
            }
            return Ok(Self {
                network,
                encoded,
                kind: AddressKind::Witness {
                    version: version.to_u8(),
                },
            });
        }
        let mut bytes = bs58::decode(value)
            .with_check(None)
            .into_vec()
            .map_err(|_| invalid())?;
        if bytes.len() != 21 || bs58::encode(&bytes).with_check().into_string() != value {
            return Err(invalid());
        }
        let (public, script, legacy_script) = network.prefixes();
        let kind = if bytes[0] == public {
            AddressKind::PubkeyHash
        } else if bytes[0] == script || bytes[0] == legacy_script {
            bytes[0] = script;
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
    const ATOMIC_UNIT: &'static str = "litoshi";
    const TRANSACTION_MAXIMUM: u64 = 8_400_000_000_000_000;
    const SUPPORTS_WITNESS: bool = true;
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
