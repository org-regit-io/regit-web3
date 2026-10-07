// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{domain::ExactDecimal, error::Error};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Explicit family qualification for a LI.FI chain number.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// Ethereum-compatible address and transaction encodings.
    Evm,
    /// Solana-compatible public keys and serialized transactions.
    Solana,
    /// UTXO family; a number still distinguishes Bitcoin, BCH, LTC, DOGE and ZEC.
    Utxo,
    /// Move family, including Sui's provider-specific chain number.
    Move,
    /// TRON source identities and protobuf payloads.
    Tron,
    /// Stellar source identities and XDR envelopes.
    Stellar,
}

/// A caller-qualified LI.FI number, decoded as an exact unsigned integer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "ChainFields")]
pub struct Chain {
    id: u64,
    family: Family,
}
impl Chain {
    /// Constructs a positive number with its explicit family.
    /// Known reserved non-EVM numbers cannot be assigned to another family.
    ///
    /// # Errors
    /// Rejects zero or a contradictory known family qualification.
    pub fn new(id: u64, family: Family) -> Result<Self, Error> {
        let reserved = match id {
            1_151_111_081_099_710 | 1_021_111_031_099_710 => Some(Family::Solana),
            9_270_000_000_000_000 => Some(Family::Move),
            20_000_000_000_001..=20_000_000_000_005 => Some(Family::Utxo),
            728_126_428 => Some(Family::Tron),
            1_201_081_091_099_710 => Some(Family::Stellar),
            _ => None,
        };
        if id == 0 || reserved.is_some_and(|expected| expected != family) {
            return Err(super::invalid_identity());
        }
        Ok(Self { id, family })
    }
    /// Returns the exact provider number, not an assumed EVM chain ID.
    #[must_use]
    pub const fn id(self) -> u64 {
        self.id
    }
    /// Returns the caller's validated family qualification.
    #[must_use]
    pub const fn family(self) -> Family {
        self.family
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainFields {
    id: u64,
    family: Family,
}
impl TryFrom<ChainFields> for Chain {
    type Error = Error;
    fn try_from(v: ChainFields) -> Result<Self, Error> {
        Self::new(v.id, v.family)
    }
}

/// Complete explicit family lookup for every chain referenced by a response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Chain>", into = "Vec<Chain>")]
pub struct ChainCatalogue(Vec<Chain>);
impl ChainCatalogue {
    /// Records 1–128 uniquely numbered caller-qualified chains.
    ///
    /// # Errors
    /// Rejects duplicate numbers or an exceeded capacity, without truncation.
    pub fn new(chains: Vec<Chain>) -> Result<Self, Error> {
        let mut ids = BTreeSet::new();
        if chains.is_empty() || chains.len() > 128 || chains.iter().any(|c| !ids.insert(c.id)) {
            return Err(super::invalid_identity());
        }
        Ok(Self(chains))
    }
    /// Resolves an explicitly supplied family, never an inferred one.
    ///
    /// # Errors
    /// Returns unsupported capability when the caller did not qualify the number.
    pub fn resolve(&self, id: u64) -> Result<Chain, Error> {
        self.0
            .iter()
            .find(|c| c.id == id)
            .copied()
            .ok_or(Error::UnsupportedCapability)
    }
    /// Returns every configured chain in supplied order.
    #[must_use]
    pub fn chains(&self) -> &[Chain] {
        &self.0
    }
}
impl TryFrom<Vec<Chain>> for ChainCatalogue {
    type Error = Error;
    fn try_from(v: Vec<Chain>) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<ChainCatalogue> for Vec<Chain> {
    fn from(v: ChainCatalogue) -> Self {
        v.0
    }
}

/// A bounded, case-sensitive step, route, tool or transfer identifier.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Identifier(String);
impl Identifier {
    /// Accepts 1–256 ASCII letters, digits, `-`, `_`, `.`, `:` and `+`.
    ///
    /// # Errors
    /// Rejects malformed text with a fixed diagnostic.
    pub fn new(text: &str) -> Result<Self, Error> {
        if text.is_empty()
            || text.len() > 256
            || !text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'+'))
        {
            return Err(super::invalid_identity());
        }
        Ok(Self(text.to_owned()))
    }
    /// Returns the exact identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Identifier {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(&v)
    }
}
impl From<Identifier> for String {
    fn from(v: Identifier) -> Self {
        v.0
    }
}

/// Bounded provider display text, separate from any identity or request path.
/// UTF-8 and controls are preserved; JSON and Debug escape their contents.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Text(String);
impl Text {
    /// Preserves up to 512 UTF-8 bytes, including an explicitly empty value.
    ///
    /// # Errors
    /// Rejects an exceeded byte bound without echoing source text.
    pub fn new(text: &str) -> Result<Self, Error> {
        if text.len() > 512 {
            return Err(super::invalid_record());
        }
        Ok(Self(text.to_owned()))
    }
    /// Returns the source text without normalization.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
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

/// An exact token identifier in an explicitly qualified provider chain.
/// EVM addresses are normalized; SVM identifiers decode to exactly 32 bytes.
/// Other families retain bounded source identifiers without checksum claims.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IdentityFields")]
pub struct Asset {
    chain: Chain,
    identifier: String,
}
impl Asset {
    /// Validates the family encoding contract and binds it to the supplied chain.
    ///
    /// # Errors
    /// Rejects controls, whitespace, excessive length and malformed EVM/SVM IDs.
    pub fn new(chain: Chain, identifier: &str) -> Result<Self, Error> {
        Ok(Self {
            chain,
            identifier: qualified_text(chain, identifier, false)?,
        })
    }
    /// Returns the explicitly qualified chain.
    #[must_use]
    pub const fn chain(&self) -> Chain {
        self.chain
    }
    /// Returns a source token identifier, never a resolved symbol guess.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// An account identity bound to a provider chain.
/// EVM/SVM syntax and Bitcoin account checksum are checked. Other families are
/// bounded source strings; they are not advertised as checksum-verified addresses.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IdentityFields")]
pub struct Account {
    chain: Chain,
    identifier: String,
}
impl Account {
    /// Qualifies a bounded account identifier without inferring its network.
    ///
    /// # Errors
    /// Rejects malformed supported encodings or invalid bounded source text.
    pub fn new(chain: Chain, identifier: &str) -> Result<Self, Error> {
        Ok(Self {
            chain,
            identifier: qualified_text(chain, identifier, true)?,
        })
    }
    /// Returns its exact provider chain and family.
    #[must_use]
    pub const fn chain(&self) -> Chain {
        self.chain
    }
    /// Returns the account identifier in its family's encoding.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityFields {
    chain: Chain,
    identifier: String,
}
impl TryFrom<IdentityFields> for Asset {
    type Error = Error;
    fn try_from(v: IdentityFields) -> Result<Self, Error> {
        Self::new(v.chain, &v.identifier)
    }
}
impl TryFrom<IdentityFields> for Account {
    type Error = Error;
    fn try_from(v: IdentityFields) -> Result<Self, Error> {
        Self::new(v.chain, &v.identifier)
    }
}
fn qualified_text(chain: Chain, text: &str, account: bool) -> Result<String, Error> {
    if text.is_empty() || text.len() > 1024 || !text.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(super::invalid_identity());
    }
    match chain.family {
        Family::Evm => crate::domain::Address::parse(text)
            .map(|v| v.to_string())
            .map_err(|_| super::invalid_identity()),
        Family::Solana => {
            let mut bytes = [0_u8; 32];
            if bs58::decode(text).onto(&mut bytes).ok() != Some(32)
                || bs58::encode(bytes).into_string() != text
            {
                return Err(super::invalid_identity());
            }
            Ok(text.to_owned())
        }
        Family::Utxo if account && chain.id == 20_000_000_000_001 => {
            let address = text
                .parse::<bitcoin::Address<bitcoin::address::NetworkUnchecked>>()
                .map_err(|_| super::invalid_identity())?;
            address
                .require_network(bitcoin::Network::Bitcoin)
                .map(|v| v.to_string())
                .map_err(|_| super::invalid_identity())
        }
        _ => Ok(text.to_owned()),
    }
}

/// Exact slippage as a decimal proportion: `0.005` is one half percent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ExactDecimal", into = "ExactDecimal")]
pub struct Slippage(ExactDecimal);
impl Slippage {
    /// Requires `0 <= value < 1`, without floating-point conversion.
    ///
    /// # Errors
    /// Rejects negative or unit-and-larger proportions.
    pub fn new(value: ExactDecimal) -> Result<Self, Error> {
        if value.is_negative() || value >= ExactDecimal::parse("1")? {
            return Err(super::invalid_record());
        }
        Ok(Self(value))
    }
    /// Returns the exact proportion.
    #[must_use]
    pub const fn value(&self) -> &ExactDecimal {
        &self.0
    }
}
impl TryFrom<ExactDecimal> for Slippage {
    type Error = Error;
    fn try_from(v: ExactDecimal) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Slippage> for ExactDecimal {
    fn from(v: Slippage) -> Self {
        v.0
    }
}
