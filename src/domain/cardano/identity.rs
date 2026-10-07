// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, str::FromStr};

use pallas_addresses::{
    Address as PallasAddress,
    byron::{AddrAttrProperty, AddrType, ByronAddress},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, ValidationError};

/// Cardano's address-network tag and exact network-layer magic.
///
/// Shelley addresses alone distinguish mainnet from testnets, not one testnet
/// from another. Technical network identity includes both fields.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkIdFields")]
pub struct NetworkId {
    network_tag: u8,
    network_magic: u32,
}

impl NetworkId {
    /// Validates mainnet discrimination or a configurable testnet magic.
    ///
    /// # Errors
    /// Rejects reserved address tags and inconsistent mainnet discrimination.
    pub fn new(network_tag: u8, network_magic: u32) -> Result<Self, Error> {
        if !(matches!((network_tag, network_magic), (1, 764_824_073))
            || network_tag == 0 && network_magic != 764_824_073)
        {
            return Err(ValidationError::InvalidCardanoNetwork.into());
        }
        Ok(Self {
            network_tag,
            network_magic,
        })
    }

    /// Returns mainnet's documented address tag and network magic.
    #[must_use]
    pub const fn mainnet() -> Self {
        Self {
            network_tag: 1,
            network_magic: 764_824_073,
        }
    }

    /// Returns pre-production's documented address tag and network magic.
    #[must_use]
    pub const fn preprod() -> Self {
        Self {
            network_tag: 0,
            network_magic: 1,
        }
    }

    /// Returns preview's documented address tag and network magic.
    #[must_use]
    pub const fn preview() -> Self {
        Self {
            network_tag: 0,
            network_magic: 2,
        }
    }

    /// Returns the address header's mainnet/testnet discriminator.
    #[must_use]
    pub const fn network_tag(self) -> u8 {
        self.network_tag
    }

    /// Returns the exact expected network-layer magic.
    #[must_use]
    pub const fn network_magic(self) -> u32 {
        self.network_magic
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkIdFields {
    network_tag: u8,
    network_magic: u32,
}

impl TryFrom<NetworkIdFields> for NetworkId {
    type Error = Error;
    fn try_from(fields: NetworkIdFields) -> Result<Self, Error> {
        Self::new(fields.network_tag, fields.network_magic)
    }
}

/// An explicit technical network identity and independent display alias.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct Network {
    identity: NetworkId,
    alias: String,
}

impl Network {
    /// Records a validated identity and bounded non-secret display label.
    ///
    /// # Errors
    /// Rejects aliases outside the shared label contract.
    pub fn new(identity: NetworkId, alias: impl Into<String>) -> Result<Self, Error> {
        let alias = alias.into();
        if !super::super::valid_label(&alias, 64) {
            return Err(ValidationError::InvalidNetworkAlias.into());
        }
        Ok(Self { identity, alias })
    }

    /// Returns technical identity independently of the display alias.
    #[must_use]
    pub const fn identity(&self) -> NetworkId {
        self.identity
    }

    /// Returns the retained display alias.
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
    fn try_from(fields: NetworkFields) -> Result<Self, Error> {
        Self::new(fields.identity, fields.alias)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct AddressValue {
    encoded: String,
    bytes: Vec<u8>,
    network_tag: u8,
    byron_magic: Option<u32>,
}

fn address_error() -> Error {
    ValidationError::InvalidCardanoAddress.into()
}

fn validate_byron(value: &ByronAddress) -> Result<Option<u32>, Error> {
    let payload = value.payload.0.as_ref();
    if crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC).checksum(payload) != value.crc {
        return Err(address_error());
    }
    let decoded = value.decode().map_err(|_| address_error())?;
    if matches!(decoded.addrtype, AddrType::Other(_)) {
        return Err(address_error());
    }
    let encoded = pallas_codec::minicbor::to_vec(&decoded).map_err(|_| address_error())?;
    if encoded != payload {
        return Err(address_error());
    }
    let mut magic = None;
    let mut previous_key = None;
    for attribute in decoded.attributes.iter() {
        let key = match attribute {
            AddrAttrProperty::AddrDistr(_) => 0_u8,
            AddrAttrProperty::DerivationPath(_) => 1,
            AddrAttrProperty::NetworkTag(_) => 2,
        };
        if previous_key.is_some_and(|previous| previous >= key) {
            return Err(address_error());
        }
        previous_key = Some(key);
        match attribute {
            AddrAttrProperty::NetworkTag(bytes) => {
                if magic.is_some() {
                    return Err(address_error());
                }
                let mut reader = pallas_codec::minicbor::Decoder::new(bytes.as_ref());
                let parsed = reader.u32().map_err(|_| address_error())?;
                let canonical =
                    pallas_codec::minicbor::to_vec(parsed).map_err(|_| address_error())?;
                if reader.position() != bytes.len()
                    || canonical.as_slice() != &bytes[..]
                    || parsed == 764_824_073
                {
                    return Err(address_error());
                }
                magic = Some(parsed);
            }
            AddrAttrProperty::DerivationPath(bytes) => {
                let mut reader = pallas_codec::minicbor::Decoder::new(bytes.as_ref());
                let ciphertext = reader.bytes().map_err(|_| address_error())?;
                let mut writer = pallas_codec::minicbor::Encoder::new(Vec::new());
                writer.bytes(ciphertext).map_err(|_| address_error())?;
                if reader.position() != bytes.len() || writer.into_writer().as_slice() != &bytes[..]
                {
                    return Err(address_error());
                }
            }
            AddrAttrProperty::AddrDistr(_) => return Err(address_error()),
        }
    }
    Ok(magic)
}

fn from_parsed(address: PallasAddress, stake: bool) -> Result<AddressValue, Error> {
    let bytes = address.to_vec();
    let (encoded, network_tag, byron_magic) = match address {
        PallasAddress::Byron(value) if !stake => {
            let magic = validate_byron(&value)?;
            (value.to_base58(), u8::from(magic.is_none()), magic)
        }
        PallasAddress::Shelley(value) if !stake => {
            let tag = match value.network() {
                pallas_addresses::Network::Mainnet => 1,
                pallas_addresses::Network::Testnet => 0,
                pallas_addresses::Network::Other(_) => return Err(address_error()),
            };
            (value.to_bech32().map_err(|_| address_error())?, tag, None)
        }
        PallasAddress::Stake(value) if stake => {
            let tag = match value.network() {
                pallas_addresses::Network::Mainnet => 1,
                pallas_addresses::Network::Testnet => 0,
                pallas_addresses::Network::Other(_) => return Err(address_error()),
            };
            (value.to_bech32().map_err(|_| address_error())?, tag, None)
        }
        _ => return Err(address_error()),
    };
    Ok(AddressValue {
        encoded,
        bytes,
        network_tag,
        byron_magic,
    })
}

fn parse_address(value: &str, stake: bool) -> Result<AddressValue, Error> {
    if value.is_empty() || value.len() > 1024 {
        return Err(address_error());
    }
    if let Ok(parsed) = PallasAddress::from_bech32(value) {
        let address = from_parsed(parsed, stake)?;
        if !address.encoded.eq_ignore_ascii_case(value) {
            return Err(address_error());
        }
        return Ok(address);
    }
    if stake {
        return Err(address_error());
    }
    let parsed = ByronAddress::from_base58(value).map_err(|_| address_error())?;
    let address = from_parsed(PallasAddress::Byron(parsed), false)?;
    if address.encoded != value {
        return Err(address_error());
    }
    Ok(address)
}

macro_rules! address_type {
    ($name:ident, $stake:literal, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub struct $name(AddressValue);

        impl $name {
            /// Parses conventional encoding, validates structure and checksum.
            ///
            /// # Errors
            /// Rejects malformed, noncanonical or inappropriate address roles.
            pub fn parse(value: &str) -> Result<Self, Error> {
                parse_address(value, $stake).map(Self)
            }

            /// Parses exact address bytes through the maintained Cardano codec.
            ///
            /// # Errors
            /// Rejects malformed, excessive, noncanonical or wrong-role bytes.
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
                if bytes.is_empty() || bytes.len() > 512 {
                    return Err(address_error());
                }
                let parsed = PallasAddress::from_bytes(bytes).map_err(|_| address_error())?;
                let value = from_parsed(parsed, $stake)?;
                if value.bytes != bytes {
                    return Err(address_error());
                }
                Self::parse(&value.encoded)
            }

            /// Returns exact address bytes without proving ownership.
            #[must_use]
            pub fn bytes(&self) -> &[u8] {
                &self.0.bytes
            }

            /// Returns the canonical conventional encoding.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0.encoded
            }

            /// Returns the encoded mainnet/testnet address discriminator.
            #[must_use]
            pub const fn network_tag(&self) -> u8 {
                self.0.network_tag
            }

            /// Checks encoded network discrimination against the expected identity.
            ///
            /// A Shelley address carries no magic, so compatibility does not prove
            /// that the address belongs to one particular testnet.
            #[must_use]
            pub fn is_compatible_with(&self, network: NetworkId) -> bool {
                self.0.network_tag == network.network_tag()
                    && self
                        .0
                        .byron_magic
                        .is_none_or(|magic| magic == network.network_magic())
            }
        }

        impl FromStr for $name {
            type Err = Error;
            fn from_str(value: &str) -> Result<Self, Error> {
                Self::parse(value)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

address_type!(
    PaymentAddress,
    false,
    "A checksum-valid Shelley or Byron payment address. Byron validation supports canonical, uniquely ordered derivation-path and network-magic attributes; unsupported attribute profiles are rejected."
);
address_type!(
    StakeAddress,
    true,
    "A checksum-valid Shelley stake/reward address."
);

macro_rules! hash_type {
    ($name:ident, $size:literal, $reason:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; $size]);
        impl $name {
            /// Parses exact-width unprefixed hexadecimal.
            ///
            /// # Errors
            /// Rejects prefixes, invalid characters and incorrect width.
            pub fn parse(value: &str) -> Result<Self, Error> {
                if value.len() != $size * 2 {
                    return Err(ValidationError::$reason.into());
                }
                const_hex::decode_to_array(value)
                    .map(Self)
                    .map_err(|_| ValidationError::$reason.into())
            }
            /// Records exact bytes without verifying a cryptographic preimage.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; $size]) -> Self {
                Self(bytes)
            }
            /// Returns exact bytes.
            #[must_use]
            pub const fn bytes(self) -> [u8; $size] {
                self.0
            }
        }
        impl FromStr for $name {
            type Err = Error;
            fn from_str(value: &str) -> Result<Self, Error> {
                Self::parse(value)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&const_hex::encode(self.0))
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
    Hash,
    32,
    InvalidCardanoHash,
    "An encoding-valid Cardano transaction, block or datum hash."
);
hash_type!(
    PolicyId,
    28,
    InvalidCardanoAsset,
    "An encoding-valid 28-byte native-asset policy identifier."
);
hash_type!(
    ScriptHash,
    28,
    InvalidCardanoHash,
    "An encoding-valid 28-byte Cardano script hash."
);

/// The exact 0–32 bytes of a Cardano native-asset name.
///
/// Names need not be UTF-8. Identity uses the bytes, never a display name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetName(Vec<u8>);

impl AssetName {
    /// Records an asset name's bounded exact bytes.
    ///
    /// # Errors
    /// Rejects names exceeding 32 bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > 32 {
            return Err(ValidationError::InvalidCardanoAsset.into());
        }
        Ok(Self(bytes))
    }
    /// Parses an unprefixed hex name, including an empty name.
    ///
    /// # Errors
    /// Rejects invalid, odd-length or excessive hexadecimal.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() > 64
            || !value.len().is_multiple_of(2)
            || value.starts_with("0x")
            || value.starts_with("0X")
        {
            return Err(ValidationError::InvalidCardanoAsset.into());
        }
        Self::from_bytes(
            const_hex::decode(value)
                .map_err(|_| Error::from(ValidationError::InvalidCardanoAsset))?,
        )
    }
    /// Returns exact bytes without a UTF-8 interpretation.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for AssetName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&const_hex::encode(&self.0))
    }
}
impl Serialize for AssetName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for AssetName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Native ADA or exact policy/name identity for a native asset.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssetKind {
    /// ADA base units (lovelace).
    Native,
    /// A policy identifier and exact byte name.
    Token {
        /// The native-asset policy identifier.
        policy_id: PolicyId,
        /// The exact native-asset name bytes.
        asset_name: AssetName,
    },
}

/// Technical network and asset identity without display metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetId {
    network: NetworkId,
    asset: AssetKind,
}

impl AssetId {
    /// Returns native ADA identity for the supplied network.
    #[must_use]
    pub const fn native(network: NetworkId) -> Self {
        Self {
            network,
            asset: AssetKind::Native,
        }
    }
    /// Records exact token identity.
    #[must_use]
    pub fn token(network: NetworkId, policy_id: PolicyId, asset_name: AssetName) -> Self {
        Self {
            network,
            asset: AssetKind::Token {
                policy_id,
                asset_name,
            },
        }
    }
    /// Parses a provider unit: lovelace or policy hex concatenated with name hex.
    ///
    /// # Errors
    /// Rejects malformed policy/name encoding without retaining the input.
    pub fn from_unit(network: NetworkId, unit: &str) -> Result<Self, Error> {
        if unit == "lovelace" {
            return Ok(Self::native(network));
        }
        if unit.len() < 56 || unit.len() > 120 || !unit.is_ascii() {
            return Err(ValidationError::InvalidCardanoAsset.into());
        }
        Ok(Self::token(
            network,
            PolicyId::parse(&unit[..56])?,
            AssetName::parse(&unit[56..])?,
        ))
    }
    /// Returns technical network identity.
    #[must_use]
    pub const fn network(&self) -> NetworkId {
        self.network
    }
    /// Returns the exact native/token identity.
    #[must_use]
    pub const fn asset(&self) -> &AssetKind {
        &self.asset
    }
}
