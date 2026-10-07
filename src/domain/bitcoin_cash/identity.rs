// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use cashaddr::CashEnc;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::{
    domain::valid_label,
    error::{Error, ValidationError},
};

macro_rules! hash_identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);
        impl $name {
            /// Constructs from bytes in displayed hexadecimal order.
            #[must_use]
            pub const fn from_display_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
            /// Parses exactly 64 hexadecimal characters, without a prefix.
            /// # Errors
            /// Rejects malformed width or characters with a fixed diagnostic.
            pub fn parse(value: &str) -> Result<Self, Error> {
                if value.len() != 64 {
                    return Err(super::invalid());
                }
                const_hex::decode_to_array(value)
                    .map(Self)
                    .map_err(|_| super::invalid())
            }
            /// Returns bytes in displayed hexadecimal order.
            #[must_use]
            pub const fn display_bytes(self) -> [u8; 32] {
                self.0
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
                let value = String::deserialize(d)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}
hash_identity!(
    Txid,
    "A complete Bitcoin Cash transaction identifier in displayed hash order."
);
hash_identity!(
    BlockHash,
    "A complete Bitcoin Cash block identifier in displayed hash order."
);
hash_identity!(
    ScriptHash,
    "An Electrum-Cash SHA256 script hash in displayed reversed-byte order."
);
hash_identity!(
    TokenCategory,
    "A complete `CashToken` category identifier in displayed transaction-hash order."
);

pub(super) fn sha256d_display(value: &[u8]) -> [u8; 32] {
    let mut bytes: [u8; 32] = Sha256::digest(Sha256::digest(value)).into();
    bytes.reverse();
    bytes
}

/// `CashAddr` address namespaces, independent of a remote chain's identity.
/// Several distinct BCH test chains share the `bchtest` namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressNamespace {
    /// The `bitcoincash` prefix and legacy mainnet address versions.
    Mainnet,
    /// The `bchtest` prefix shared by BCH test chains.
    Testnet,
    /// The `bchreg` prefix and overlapping legacy test versions.
    Regtest,
}
impl AddressNamespace {
    /// Returns the complete `CashAddr` prefix.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Mainnet => "bitcoincash",
            Self::Testnet => "bchtest",
            Self::Regtest => "bchreg",
        }
    }
    const fn legacy_versions(self) -> (u8, u8) {
        match self {
            Self::Mainnet => (0, 5),
            Self::Testnet | Self::Regtest => (111, 196),
        }
    }
}

/// Supported BCH address script category; token awareness is retained separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressKind {
    /// A 20-byte public-key hash.
    PubkeyHash,
    /// A 20-byte or 32-byte script hash.
    ScriptHash,
}

/// A maintained-codec, canonical BCH address qualified by an explicit namespace.
///
/// Supports legacy `Base58Check`, P2PKH, P2SH20/P2SH32 and token-aware `CashAddr`
/// types. Prefixless `CashAddr` is interpreted only in the supplied namespace.
/// Mixed case, reserved/unsupported types and noncanonical padding are rejected.
/// Namespace compatibility does not authenticate the chain or prove spendability.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AddressFields", into = "AddressFields")]
pub struct Address {
    namespace: AddressNamespace,
    encoded: String,
    kind: AddressKind,
    token_aware: bool,
    hash: Vec<u8>,
}
impl Address {
    /// Validates a caller-supplied BCH address and normalizes to full lowercase `CashAddr`.
    /// # Errors
    /// Rejects incompatible namespace, malformed checksum, payload or encoding.
    pub fn parse(value: &str, namespace: AddressNamespace) -> Result<Self, Error> {
        let invalid = || Error::Validation(ValidationError::InvalidBitcoinCashAddress);
        if value.is_empty()
            || value.len() > 128
            || !value.is_ascii()
            || value
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err(invalid());
        }
        if !value.contains(':')
            && !matches!(
                value.as_bytes()[0],
                b'q' | b'p' | b'z' | b'r' | b'Q' | b'P' | b'Z' | b'R'
            )
        {
            let bytes = bs58::decode(value)
                .with_check(None)
                .into_vec()
                .map_err(|_| invalid())?;
            if bytes.len() != 21 || bs58::encode(&bytes).with_check().into_string() != value {
                return Err(invalid());
            }
            let (public, script) = namespace.legacy_versions();
            let kind = if bytes[0] == public {
                AddressKind::PubkeyHash
            } else if bytes[0] == script {
                AddressKind::ScriptHash
            } else {
                return Err(ValidationError::NetworkMismatch.into());
            };
            return Self::from_hash(namespace, kind, false, &bytes[1..]);
        }
        let upper = value.bytes().any(|b| b.is_ascii_uppercase());
        let lower = value.bytes().any(|b| b.is_ascii_lowercase());
        if upper && lower {
            return Err(invalid());
        }
        let normalized = value.to_ascii_lowercase();
        let full = if normalized.contains(':') {
            normalized
        } else {
            format!("{}:{normalized}", namespace.prefix())
        };
        let (prefix, payload_text) = full.split_once(':').ok_or_else(invalid)?;
        if prefix != namespace.prefix() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        // The released codec assumes a nonempty decoded payload after checksum.
        // Guard the only supported widths before calling it, including tiny
        // checksum-only inputs that could otherwise reach unchecked indexing.
        if !matches!(payload_text.len(), 42 | 61) {
            return Err(invalid());
        }
        let payload: cashaddr::Payload = full.parse().map_err(|_| invalid())?;
        let type_bits = payload.hash_type().numeric_value();
        let (kind, token_aware) = match type_bits {
            0 => (AddressKind::PubkeyHash, false),
            1 => (AddressKind::ScriptHash, false),
            2 => (AddressKind::PubkeyHash, true),
            3 => (AddressKind::ScriptHash, true),
            _ => return Err(invalid()),
        };
        let result = Self::from_hash(namespace, kind, token_aware, payload.as_ref())?;
        if result.encoded != full {
            return Err(invalid());
        }
        Ok(result)
    }
    /// Encodes supported BCH script hashes using the maintained `CashAddr` codec.
    /// # Errors
    /// Requires P2PKH20 or P2SH20/P2SH32 payload widths.
    pub fn from_hash(
        namespace: AddressNamespace,
        kind: AddressKind,
        token_aware: bool,
        hash: &[u8],
    ) -> Result<Self, Error> {
        let invalid = || Error::Validation(ValidationError::InvalidBitcoinCashAddress);
        if !matches!(
            (kind, hash.len()),
            (AddressKind::PubkeyHash, 20) | (AddressKind::ScriptHash, 20 | 32)
        ) {
            return Err(invalid());
        }
        let bits = u8::from(kind == AddressKind::ScriptHash) + 2 * u8::from(token_aware);
        let encoded = hash
            .encode(
                namespace.prefix(),
                cashaddr::HashType::try_from(bits).map_err(|_| invalid())?,
            )
            .map_err(|_| invalid())?;
        Ok(Self {
            namespace,
            encoded,
            kind,
            token_aware,
            hash: hash.to_vec(),
        })
    }
    /// Returns the caller-qualified address namespace.
    #[must_use]
    pub const fn namespace(&self) -> AddressNamespace {
        self.namespace
    }
    /// Returns the validated BCH script category.
    #[must_use]
    pub const fn kind(&self) -> AddressKind {
        self.kind
    }
    /// Reports whether the supplied encoding explicitly signals token awareness.
    #[must_use]
    pub const fn is_token_aware(&self) -> bool {
        self.token_aware
    }
    /// Returns the exact public-key or script hash payload.
    #[must_use]
    pub fn hash(&self) -> &[u8] {
        &self.hash
    }
    /// Derives the BCH output locking bytecode for this address category.
    #[must_use]
    pub fn locking_bytecode(&self) -> Vec<u8> {
        let (prefix, suffix): (&[u8], &[u8]) = match (self.kind, self.hash.len()) {
            (AddressKind::PubkeyHash, _) => (&[0x76, 0xa9, 0x14], &[0x88, 0xac]),
            (AddressKind::ScriptHash, 32) => (&[0xaa, 0x20], &[0x87]),
            (AddressKind::ScriptHash, _) => (&[0xa9, 0x14], &[0x87]),
        };
        [prefix, &self.hash, suffix].concat()
    }
    /// Computes the Electrum-Cash script hash with maintained `SHA256`.
    /// Token-aware and ordinary encodings of the same script yield the same hash.
    #[must_use]
    pub fn script_hash(&self) -> ScriptHash {
        let mut bytes: [u8; 32] = Sha256::digest(self.locking_bytecode()).into();
        bytes.reverse();
        ScriptHash::from_display_bytes(bytes)
    }
}
impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.encoded)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AddressFields {
    namespace: AddressNamespace,
    encoded: String,
}
impl TryFrom<AddressFields> for Address {
    type Error = Error;
    fn try_from(v: AddressFields) -> Result<Self, Error> {
        Self::parse(&v.encoded, v.namespace)
    }
}
impl From<Address> for AddressFields {
    fn from(v: Address) -> Self {
        Self {
            namespace: v.namespace,
            encoded: v.encoded,
        }
    }
}

/// A caller-expected nonzero-height header checkpoint, separate from genesis.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CheckpointFields")]
pub struct ForkCheckpoint {
    height: u32,
    hash: BlockHash,
}
impl ForkCheckpoint {
    /// Records an explicit expected header identity without authenticating its origin.
    /// # Errors
    /// Rejects zero height or all-zero hash.
    pub fn new(height: u32, hash: BlockHash) -> Result<Self, Error> {
        if height == 0 || hash.display_bytes() == [0; 32] {
            return Err(super::invalid());
        }
        Ok(Self { height, hash })
    }
    /// Returns the exact expected checkpoint height.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
    /// Returns the complete expected checkpoint hash.
    #[must_use]
    pub const fn hash(self) -> BlockHash {
        self.hash
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointFields {
    height: u32,
    hash: BlockHash,
}
impl TryFrom<CheckpointFields> for ForkCheckpoint {
    type Error = Error;
    fn try_from(v: CheckpointFields) -> Result<Self, Error> {
        Self::new(v.height, v.hash)
    }
}

/// Complete expected chain facts and an independent compatible address namespace.
/// Custom expectations are caller assertions, not authenticated BCH chain identities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct NetworkIdentity {
    namespace: AddressNamespace,
    genesis_hash: BlockHash,
    fork_checkpoint: ForkCheckpoint,
    alias: String,
}
impl NetworkIdentity {
    /// Records explicitly supplied namespace, genesis, fork checkpoint and display alias.
    /// # Errors
    /// Rejects zero genesis or an invalid non-secret alias; it does not infer a chain pairing.
    pub fn new(
        namespace: AddressNamespace,
        genesis_hash: BlockHash,
        fork_checkpoint: ForkCheckpoint,
        alias: impl Into<String>,
    ) -> Result<Self, Error> {
        let alias = alias.into();
        if genesis_hash.display_bytes() == [0; 32] || !valid_label(&alias, 128) {
            return Err(super::invalid());
        }
        Ok(Self {
            namespace,
            genesis_hash,
            fork_checkpoint,
            alias,
        })
    }
    /// Uses the full BCHN mainnet genesis and post-Axion fork checkpoint at height 661648.
    /// This fixed checkpoint distinguishes the expected BCH branch from BTC and BSV.
    /// # Errors
    /// Rejects an invalid caller display alias.
    pub fn mainnet(alias: impl Into<String>) -> Result<Self, Error> {
        Self::new(
            AddressNamespace::Mainnet,
            BlockHash::parse("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f")?,
            ForkCheckpoint::new(
                661_648,
                BlockHash::parse(
                    "0000000000000000029e471c41818d24b8b74c911071c4ef0b4a0509f9b5a8ce",
                )?,
            )?,
            alias,
        )
    }
    /// Returns address compatibility, separate from expected chain facts.
    #[must_use]
    pub const fn namespace(&self) -> AddressNamespace {
        self.namespace
    }
    /// Returns the full expected genesis hash.
    #[must_use]
    pub const fn genesis_hash(&self) -> BlockHash {
        self.genesis_hash
    }
    /// Returns the explicit non-genesis fork checkpoint.
    #[must_use]
    pub const fn fork_checkpoint(&self) -> ForkCheckpoint {
        self.fork_checkpoint
    }
    /// Returns the caller display alias, which does not establish identity.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkFields {
    namespace: AddressNamespace,
    genesis_hash: BlockHash,
    fork_checkpoint: ForkCheckpoint,
    alias: String,
}
impl TryFrom<NetworkFields> for NetworkIdentity {
    type Error = Error;
    fn try_from(v: NetworkFields) -> Result<Self, Error> {
        Self::new(v.namespace, v.genesis_hash, v.fork_checkpoint, v.alias)
    }
}

/// Exactly 80 source header bytes and their maintained double-`SHA256` identity.
/// The header is not proof of valid work, chain membership or transaction inclusion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HeaderFields")]
pub struct BlockHeader {
    hex: String,
    hash: BlockHash,
}
impl BlockHeader {
    /// Retains exact header bytes and computes their hash using maintained `SHA256`.
    /// # Errors
    /// Requires precisely 160 hexadecimal characters without a prefix.
    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        if hex.len() != 160 {
            return Err(super::invalid());
        }
        let bytes: [u8; 80] = const_hex::decode_to_array(hex).map_err(|_| super::invalid())?;
        Ok(Self {
            hex: const_hex::encode(bytes),
            hash: BlockHash::from_display_bytes(sha256d_display(&bytes)),
        })
    }
    /// Returns exact lowercase source header encoding.
    #[must_use]
    pub fn hex(&self) -> &str {
        &self.hex
    }
    /// Returns the computed complete block identity.
    #[must_use]
    pub const fn hash(&self) -> BlockHash {
        self.hash
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderFields {
    hex: String,
    hash: BlockHash,
}
impl TryFrom<HeaderFields> for BlockHeader {
    type Error = Error;
    fn try_from(v: HeaderFields) -> Result<Self, Error> {
        let header = Self::from_hex(&v.hex)?;
        if header.hash != v.hash {
            return Err(super::invalid());
        }
        Ok(header)
    }
}
