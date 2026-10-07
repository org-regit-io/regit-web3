// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, str::FromStr};

use bitcoin::{Network as BitcoinNetwork, address::NetworkUnchecked};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    domain::valid_label,
    error::{Error, ValidationError},
};

/// A standard Bitcoin network, including distinct third and fourth testnets.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    /// Bitcoin mainnet.
    Mainnet,
    /// The third Bitcoin testnet.
    Testnet3,
    /// The fourth Bitcoin testnet.
    Testnet4,
    /// Standard Bitcoin signet.
    Signet,
    /// Bitcoin regression-test network.
    Regtest,
}

impl Network {
    pub(super) const fn primitive(self) -> BitcoinNetwork {
        match self {
            Self::Mainnet => BitcoinNetwork::Bitcoin,
            Self::Testnet3 => BitcoinNetwork::Testnet,
            Self::Testnet4 => BitcoinNetwork::Testnet4,
            Self::Signet => BitcoinNetwork::Signet,
            Self::Regtest => BitcoinNetwork::Regtest,
        }
    }

    /// Returns the complete standard network genesis hash from maintained primitives.
    #[must_use]
    pub fn genesis_hash(self) -> BlockHash {
        BlockHash(bitcoin::constants::genesis_block(self.primitive()).block_hash())
    }
}

/// Full standard network identity and an independent caller-owned display alias.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct NetworkId {
    network: Network,
    genesis_hash: BlockHash,
    alias: String,
}

impl NetworkId {
    /// Declares a standard expected network and validated non-secret alias.
    ///
    /// # Errors
    /// Rejects an invalid bounded display alias.
    pub fn new(network: Network, alias: impl Into<String>) -> Result<Self, Error> {
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

    /// Returns the declared standard network.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }

    /// Returns the full genesis identity, independent of the display alias.
    #[must_use]
    pub const fn genesis_hash(&self) -> &BlockHash {
        &self.genesis_hash
    }

    /// Returns caller-owned display metadata.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkFields {
    network: Network,
    genesis_hash: BlockHash,
    alias: String,
}

impl TryFrom<NetworkFields> for NetworkId {
    type Error = Error;
    fn try_from(fields: NetworkFields) -> Result<Self, Error> {
        let network = Self::new(fields.network, fields.alias)?;
        if network.genesis_hash != fields.genesis_hash {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(network)
    }
}

/// A Bitcoin address qualified by the caller's declared compatible network.
///
/// Testnet3, testnet4 and signet may share encodings. Qualification records the
/// caller's declaration; it does not prove which chain a remote source serves.
///
/// ```
/// use regit_web3::domain::bitcoin::{Address, Network, Satoshis};
///
/// # fn main() -> Result<(), regit_web3::error::Error> {
/// let address = Address::parse("1BoatSLRHtKNngkdXEeobR76b53LETtpyT", Network::Mainnet)?;
/// assert_eq!(address.network(), Network::Mainnet);
/// let value = Satoshis::from_decimal("9007199254740993")?;
/// assert_eq!(value.amount().formatted().as_deref(), Some("90071992.54740993"));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AddressFields", into = "AddressFields")]
pub struct Address {
    network: Network,
    address: bitcoin::Address,
}

impl Address {
    pub(super) fn from_script(script: &bitcoin::Script, network: Network) -> Option<Self> {
        bitcoin::Address::from_script(script, network.primitive())
            .ok()
            .map(|address| Self { network, address })
    }

    /// Parses and checks address encoding, checksum and network compatibility.
    ///
    /// # Errors
    /// Rejects malformed addresses or incompatible network prefixes with fixed diagnostics.
    pub fn parse(value: &str, network: Network) -> Result<Self, Error> {
        let address = bitcoin::Address::<NetworkUnchecked>::from_str(value)
            .map_err(|_| Error::Validation(ValidationError::InvalidBitcoinAddress))?
            .require_network(network.primitive())
            .map_err(|_| Error::Validation(ValidationError::BitcoinAddressNetworkMismatch))?;
        Ok(Self { network, address })
    }

    /// Returns the explicitly declared network, including overlapping test networks.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
}

impl fmt::Display for Address {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.address.fmt(formatter)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AddressFields {
    network: Network,
    encoded: String,
}

impl From<Address> for AddressFields {
    fn from(value: Address) -> Self {
        Self {
            network: value.network,
            encoded: value.to_string(),
        }
    }
}
impl TryFrom<AddressFields> for Address {
    type Error = Error;
    fn try_from(value: AddressFields) -> Result<Self, Error> {
        Self::parse(&value.encoded, value.network)
    }
}

macro_rules! hash_type {
    ($name:ident, $primitive:ty, $reason:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name($primitive);
        impl $name {
            /// Parses an exactly 64-digit hexadecimal identifier without a prefix.
            ///
            /// # Errors
            /// Rejects malformed width or encoding without retaining supplied input.
            pub fn parse(value: &str) -> Result<Self, Error> {
                if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(ValidationError::$reason.into());
                }
                <$primitive>::from_str(value)
                    .map(Self)
                    .map_err(|_| ValidationError::$reason.into())
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
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
hash_type!(
    BlockHash,
    bitcoin::BlockHash,
    InvalidBitcoinHash,
    "A complete Bitcoin block hash with canonical lowercase hexadecimal serialization."
);
hash_type!(
    Txid,
    bitcoin::Txid,
    InvalidBitcoinTxid,
    "A complete Bitcoin transaction identifier with canonical lowercase hexadecimal serialization."
);
hash_type!(
    Wtxid,
    bitcoin::Wtxid,
    InvalidBitcoinTxid,
    "A complete witness transaction identifier computed including witness data."
);

impl Txid {
    pub(super) const fn from_native(value: bitcoin::Txid) -> Self {
        Self(value)
    }
}

impl Wtxid {
    pub(super) const fn from_native(value: bitcoin::Wtxid) -> Self {
        Self(value)
    }
}
