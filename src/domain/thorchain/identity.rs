// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use bech32::{Bech32, primitives::decode::CheckedHrpstring};
use serde::{Deserialize, Serialize};

use crate::{
    domain::{Amount, U256},
    error::{Error, ValidationError},
};

/// A supported THOR account prefix, independent of the exact Cosmos chain ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AccountPrefix {
    /// Main-network account prefix; no hard-fork chain ID is inferred.
    #[serde(rename = "thor")]
    Thor,
    /// Mock-network account prefix recognized by `THORNode`.
    #[serde(rename = "tthor")]
    Tthor,
    /// Stage-network account prefix recognized by `THORNode`.
    #[serde(rename = "sthor")]
    Sthor,
    /// Chain-network account prefix recognized by `THORNode`.
    #[serde(rename = "cthor")]
    Cthor,
}
impl AccountPrefix {
    /// Returns the canonical human-readable prefix.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Thor => "thor",
            Self::Tthor => "tthor",
            Self::Sthor => "sthor",
            Self::Cthor => "cthor",
        }
    }
}

/// Explicit Cosmos chain ID, supported THOR account prefix and display alias.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct Network {
    chain_id: String,
    account_prefix: AccountPrefix,
    alias: String,
}
impl Network {
    /// Validates an explicit chain ID (1..=50 bytes) and bounded display alias.
    ///
    /// # Errors
    /// Rejects invalid ASCII IDs/labels. No chain-ID/prefix pairing is inferred.
    pub fn new(chain_id: &str, account_prefix: AccountPrefix, alias: &str) -> Result<Self, Error> {
        if !valid_id(chain_id, 50) || !super::super::valid_label(alias, 64) {
            return Err(ValidationError::InvalidThorchainNetwork.into());
        }
        Ok(Self {
            chain_id: chain_id.to_owned(),
            account_prefix,
            alias: alias.to_owned(),
        })
    }
    /// Returns the exact expected Cosmos chain ID; case is significant.
    #[must_use]
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }
    /// Returns the separately supplied expected account prefix.
    #[must_use]
    pub const fn account_prefix(&self) -> AccountPrefix {
        self.account_prefix
    }
    /// Returns display metadata, excluded from network identity comparisons.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkFields {
    chain_id: String,
    account_prefix: AccountPrefix,
    alias: String,
}
impl TryFrom<NetworkFields> for Network {
    type Error = Error;
    fn try_from(v: NetworkFields) -> Result<Self, Error> {
        Self::new(&v.chain_id, v.account_prefix, &v.alias)
    }
}
fn valid_id(value: &str, cap: usize) -> bool {
    !value.is_empty()
        && value.len() <= cap
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// A canonical original-`Bech32` THOR account address with a 20-byte payload.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Address(String);
impl Address {
    /// Checks the maintained original `Bech32` checksum, payload and canonical encoding.
    ///
    /// Uppercase `Bech32` input is normalized; mixed case, Bech32m, wrong padding,
    /// unsupported prefixes and non-account payload lengths are rejected.
    ///
    /// # Errors
    /// Returns a fixed address failure without retaining the input.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let invalid = || Error::from(ValidationError::InvalidThorchainAddress);
        if value.len() > 90 {
            return Err(invalid());
        }
        let checked = CheckedHrpstring::new::<Bech32>(value).map_err(|_| invalid())?;
        let prefix = checked.hrp().to_string().to_ascii_lowercase();
        if !matches!(prefix.as_str(), "thor" | "tthor" | "sthor" | "cthor") {
            return Err(invalid());
        }
        let bytes: Vec<u8> = checked.byte_iter().collect();
        if bytes.len() != 20 {
            return Err(invalid());
        }
        let canonical = bech32::encode::<Bech32>(checked.hrp(), &bytes)
            .map_err(|_| invalid())?
            .to_ascii_lowercase();
        if !canonical.eq_ignore_ascii_case(value) {
            return Err(invalid());
        }
        Ok(Self(canonical))
    }
    /// Returns the validated original-`Bech32` account prefix.
    #[must_use]
    pub fn prefix(&self) -> AccountPrefix {
        if self.0.starts_with("tthor1") {
            AccountPrefix::Tthor
        } else if self.0.starts_with("sthor1") {
            AccountPrefix::Sthor
        } else if self.0.starts_with("cthor1") {
            AccountPrefix::Cthor
        } else {
            AccountPrefix::Thor
        }
    }
    /// Returns the canonical lowercase address.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Address {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<Address> for String {
    fn from(v: Address) -> Self {
        v.0
    }
}

/// A canonical `THORNode` chain code: 3..=10 uppercase ASCII letters.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Chain(String);
impl Chain {
    /// Normalizes case and validates the maintained `THORNode` chain-code grammar.
    ///
    /// # Errors
    /// Rejects invalid length or characters without retaining supplied text.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if !(3..=10).contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err(ValidationError::InvalidThorchainAsset.into());
        }
        Ok(Self(value.to_ascii_uppercase()))
    }
    /// Returns the canonical chain code; this does not identify a particular network.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Chain {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<Chain> for String {
    fn from(v: Chain) -> Self {
        v.0
    }
}

/// The distinct THOR asset representation; native holding does not imply layer one.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// Dot-separated layer-one/native asset identity.
    LayerOne,
    /// Slash-separated synthetic asset held on `THORChain`.
    Synthetic,
    /// Tilde-separated trade asset held on `THORChain`.
    Trade,
    /// Hyphen-separated secured asset held on `THORChain`.
    Secured,
}

/// Canonical caller-supplied THOR asset syntax with an explicit chain and symbol.
///
/// Parsing checks lexical identity, not catalogue existence or provider resolution.
/// This parser never resolves a dynamically supplied short code.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Asset {
    chain: Chain,
    symbol: String,
    kind: AssetKind,
}
impl Asset {
    /// Parses caller-supplied layer-one, synthetic, trade or secured asset syntax.
    ///
    /// # Errors
    /// Rejects missing full chain/symbol, invalid characters or wrapped THOR assets.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let invalid = || Error::from(ValidationError::InvalidThorchainAsset);
        if value.len() > 256 {
            return Err(invalid());
        }
        let (index, delimiter) = value
            .char_indices()
            .find(|(_, c)| matches!(c, '.' | '/' | '~' | '-'))
            .ok_or_else(invalid)?;
        let chain = Chain::parse(&value[..index])?;
        let symbol = &value[index + 1..];
        if symbol.is_empty()
            || !symbol
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
        {
            return Err(invalid());
        }
        let kind = match delimiter {
            '/' => AssetKind::Synthetic,
            '~' => AssetKind::Trade,
            '-' => AssetKind::Secured,
            _ => AssetKind::LayerOne,
        };
        if kind != AssetKind::LayerOne && chain.as_str() == "THOR" {
            return Err(invalid());
        }
        Ok(Self {
            chain,
            symbol: symbol.to_ascii_uppercase(),
            kind,
        })
    }
    /// Returns the underlying layer-one chain code.
    #[must_use]
    pub const fn chain(&self) -> &Chain {
        &self.chain
    }
    /// Returns the canonical full symbol, including a contract suffix when supplied.
    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }
    /// Returns the exact representation; synthetic/trade/secured are distinct.
    #[must_use]
    pub const fn kind(&self) -> AssetKind {
        self.kind
    }
    /// Returns whether this identity is a THOR-held representation.
    #[must_use]
    pub fn is_thor_held(&self) -> bool {
        self.chain.as_str() == "THOR" || self.kind != AssetKind::LayerOne
    }
}
impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let separator = match self.kind {
            AssetKind::LayerOne => '.',
            AssetKind::Synthetic => '/',
            AssetKind::Trade => '~',
            AssetKind::Secured => '-',
        };
        write!(f, "{}{separator}{}", self.chain.as_str(), self.symbol)
    }
}
impl TryFrom<String> for Asset {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<Asset> for String {
    fn from(v: Asset) -> Self {
        v.to_string()
    }
}

/// A bounded foreign-chain address record, without an implied checksum/network proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ChainAddressFields")]
pub struct ChainAddress {
    chain: Chain,
    address: Text,
}
impl ChainAddress {
    /// Qualifies bounded address text by its THOR chain code.
    ///
    /// THOR account addresses additionally use maintained checksum validation.
    /// Other families retain supplied text without claiming foreign validation.
    ///
    /// # Errors
    /// Rejects controls/whitespace, excessive length or an invalid THOR account.
    pub fn new(chain: Chain, address: &str) -> Result<Self, Error> {
        if address.is_empty()
            || address.len() > 256
            || !address.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(super::invalid_record());
        }
        let text = if chain.as_str() == "THOR" {
            Address::parse(address)?.as_str().to_owned()
        } else {
            address.to_owned()
        };
        Ok(Self {
            chain,
            address: Text::new(&text)?,
        })
    }
    /// Returns the supplied chain identity.
    #[must_use]
    pub const fn chain(&self) -> &Chain {
        &self.chain
    }
    /// Returns bounded address text; foreign address checksums are not established.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.address.as_str()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainAddressFields {
    chain: Chain,
    address: Text,
}
impl TryFrom<ChainAddressFields> for ChainAddress {
    type Error = Error;
    fn try_from(v: ChainAddressFields) -> Result<Self, Error> {
        Self::new(v.chain, v.address.as_str())
    }
}

/// Bounded source text, including an explicitly empty source field; `Debug` prints its length.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Text(String);
impl Text {
    /// Retains text up to 4096 bytes without control characters.
    ///
    /// # Errors
    /// Rejects invalid text with a fixed record failure.
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.len() > 4096 || value.chars().any(char::is_control) {
            return Err(super::invalid_record());
        }
        Ok(Self(value.to_owned()))
    }
    /// Returns the explicitly retained source text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Text")
            .field("bytes", &self.0.len())
            .finish()
    }
}
impl TryFrom<String> for Text {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(&v)
    }
}
impl From<Text> for String {
    fn from(v: Text) -> Self {
        v.0
    }
}

/// Raw UTF-8 source transaction memo with opaque diagnostics and a byte ceiling.
///
/// Empty strings and control characters remain exact source data. The local
/// 250-byte resource ceiling follows the current `THORNode` transaction memo
/// bound. It does not validate memo semantics, signatures or execution approval.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TransactionMemo(String);
impl TransactionMemo {
    /// Largest retained UTF-8 memo, measured in bytes rather than characters.
    pub const MAXIMUM_BYTES: usize = 250;
    /// Retains exact source UTF-8, including empty strings and control characters.
    ///
    /// # Errors
    /// Rejects a memo exceeding the local byte ceiling without retaining its text.
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.len() > Self::MAXIMUM_BYTES {
            return Err(super::invalid_record());
        }
        Ok(Self(value.to_owned()))
    }
    /// Returns the exact explicitly retained memo; its contents are not approved.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for TransactionMemo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionMemo")
            .field("bytes", &self.0.len())
            .finish()
    }
}
impl TryFrom<String> for TransactionMemo {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::new(&value)
    }
}
impl From<TransactionMemo> for String {
    fn from(value: TransactionMemo) -> Self {
        value.0
    }
}

macro_rules! quantity {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(U256);
        impl $name {
            /// Records an exact unsigned 256-bit value without floating point.
            #[must_use]
            pub const fn new(value: U256) -> Self {
                Self(value)
            }
            /// Parses canonical unsigned decimal text within the 256-bit range.
            ///
            /// # Errors
            /// Rejects signs, leading zeros, nondecimal notation and overflow.
            pub fn from_decimal(value: &str) -> Result<Self, Error> {
                Ok(Self(super::super::amount::parse_uint(value)?))
            }
            /// Returns the exact unsigned value; units are defined by its typed field.
            #[must_use]
            pub const fn raw(self) -> U256 {
                self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = Error;
            fn try_from(v: String) -> Result<Self, Error> {
                Self::from_decimal(&v)
            }
        }
        impl From<$name> for String {
            fn from(v: $name) -> Self {
                v.0.to_string()
            }
        }
    };
}
quantity!(
    RawQuantity,
    "An exact raw quantity; its field defines units, with no inferred decimal precision."
);
quantity!(
    ProtocolAmount,
    "An exact `THORChain` protocol amount normalized to 1e8 units, not foreign native units."
);
impl ProtocolAmount {
    /// Returns a shared exact amount with the protocol's explicitly defined eight decimals.
    #[must_use]
    pub const fn amount(self) -> Amount {
        Amount::new(self.0, Some(8))
    }
}

/// An explicit collection limit; source collections are never silently truncated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct CollectionLimit(u32);
impl CollectionLimit {
    /// Largest supported collection, independently of configured HTTP byte bounds.
    pub const MAXIMUM: u32 = 100_000;
    /// Validates a limit in 1..=100000.
    ///
    /// # Errors
    /// Rejects zero or an excessive collection limit.
    pub fn new(value: u32) -> Result<Self, Error> {
        if value == 0 || value > Self::MAXIMUM {
            return Err(super::invalid_record());
        }
        Ok(Self(value))
    }
    /// Returns the selected item ceiling.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl TryFrom<u32> for CollectionLimit {
    type Error = Error;
    fn try_from(v: u32) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<CollectionLimit> for u32 {
    fn from(v: CollectionLimit) -> Self {
        v.0
    }
}

/// The actual transaction identifier representation supported by `THORNode`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxidKind {
    /// A 32-byte hexadecimal hash.
    Hash,
    /// A 32-byte hash retaining its source 0x prefix.
    PrefixedHash,
    /// A Cosmos hash retaining its appended decimal transaction index.
    CosmosIndexed,
    /// A case-sensitive `Base58`-encoded 64-byte Solana signature; not a signature verification.
    SolanaSignature,
}

/// Bounded cross-chain transaction identity retaining the actual source representation.
///
/// Hexadecimal digits are canonical uppercase. Solana signature case and indexed
/// Cosmos suffix digits are retained; no signature or consensus validation is implied.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Txid {
    value: String,
    kind: TxidKind,
}
impl Txid {
    /// Parses a supported hash, prefixed hash, indexed Cosmos hash or Solana signature.
    ///
    /// Indexed suffixes are bounded to twenty decimal digits representable by u64;
    /// the exact supplied digits remain part of identity. Solana encodings must be
    /// canonical `Base58`, 87/88 characters and decode to exactly 64 bytes.
    ///
    /// # Errors
    /// Rejects invalid widths, encodings, index overflow or unsupported source forms.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let invalid = super::invalid_record;
        if !value.is_ascii() {
            return Err(invalid());
        }
        let (canonical, kind) = match value.len() {
            64 if value.bytes().all(|b| b.is_ascii_hexdigit()) => {
                (value.to_ascii_uppercase(), TxidKind::Hash)
            }
            66 if value[..2].eq_ignore_ascii_case("0x")
                && value[2..].bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                (value.to_ascii_uppercase(), TxidKind::PrefixedHash)
            }
            66..=85
                if value.as_bytes()[64] == b'-'
                    && value[..64].bytes().all(|b| b.is_ascii_hexdigit())
                    && value[65..].bytes().all(|b| b.is_ascii_digit())
                    && value[65..].parse::<u64>().is_ok() =>
            {
                (value.to_ascii_uppercase(), TxidKind::CosmosIndexed)
            }
            87 | 88 => {
                let mut bytes = [0u8; 64];
                if bs58::decode(value)
                    .onto(&mut bytes)
                    .map_err(|_| invalid())?
                    != 64
                    || bs58::encode(bytes).into_string() != value
                {
                    return Err(invalid());
                }
                (value.to_owned(), TxidKind::SolanaSignature)
            }
            _ => return Err(invalid()),
        };
        Ok(Self {
            value: canonical,
            kind,
        })
    }
    /// Returns the actual supported identifier representation.
    #[must_use]
    pub const fn kind(&self) -> TxidKind {
        self.kind
    }
    /// Returns canonical source-shaped text without discarding a prefix or index.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
    /// Returns whether the source supplied `THORNode`'s all-zero blank-ID sentinel.
    ///
    /// Such a record does not identify a signed external transaction.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        matches!(self.kind, TxidKind::Hash | TxidKind::PrefixedHash)
            && self.identity_key().bytes().all(|b| b == b'0')
    }
    /// Compares the actual transaction identity while allowing a hexadecimal 0x alias.
    ///
    /// Cosmos message indices remain significant, with decimal zero padding normalized
    /// for comparison. Solana signature case remains significant. Serialization and
    /// request paths retain each validated source-shaped representation.
    #[must_use]
    pub fn same_transaction(&self, other: &Self) -> bool {
        self.identity_key() == other.identity_key()
    }
    pub(crate) fn identity_key(&self) -> String {
        match self.kind {
            TxidKind::Hash | TxidKind::SolanaSignature => self.value.clone(),
            TxidKind::PrefixedHash => self.value[2..].to_owned(),
            TxidKind::CosmosIndexed => {
                let index = self.value[65..].trim_start_matches('0');
                format!(
                    "{}-{}",
                    &self.value[..64],
                    if index.is_empty() { "0" } else { index }
                )
            }
        }
    }
}
impl fmt::Display for Txid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}
impl TryFrom<String> for Txid {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<Txid> for String {
    fn from(v: Txid) -> Self {
        v.value
    }
}
