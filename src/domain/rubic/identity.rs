// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::{
        Address, ChainId,
        solana::{Network, Pubkey, Signature},
    },
    error::Error,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Bounded case-sensitive provider alias, route ID or referrer; Debug is opaque.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Identifier(String);
impl Identifier {
    /// Accepts 1–256 ASCII letters, digits, hyphens, underscores, dots and colons.
    /// # Errors
    /// Rejects invalid supplied text without echoing it.
    pub fn new(v: &str) -> Result<Self, Error> {
        if v.is_empty()
            || v.len() > 256
            || !v
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
        {
            return Err(super::invalid_identity());
        }
        Ok(Self(v.into()))
    }
    /// Returns the exact supplied identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identifier").finish_non_exhaustive()
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

/// Bounded display or provider-qualified identifier text, with opaque diagnostics.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Text(String);
impl Text {
    /// Retains 1–512 bytes without controls, whitespace-only text or truncation.
    /// # Errors
    /// Rejects malformed text without retaining it in errors.
    pub fn new(v: &str) -> Result<Self, Error> {
        if v.trim().is_empty() || v.len() > 512 || v.chars().any(char::is_control) {
            return Err(super::invalid_identity());
        }
        Ok(Self(v.into()))
    }
    /// Returns exact source or caller text for deliberate review.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Text").finish_non_exhaustive()
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

/// Explicit locally qualified family identity, distinct from Rubic catalogue IDs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum Family {
    /// Exact EVM chain ID; the Rubic API cannot prove server genesis.
    Evm {
        /// Caller-qualified EVM identity.
        chain_id: ChainId,
    },
    /// Exact Solana genesis declaration, without an API genesis proof.
    Solana {
        /// Caller-qualified Solana network.
        network: Network,
    },
    /// Other explicitly named provider family; identifiers remain source records.
    Provider {
        /// Exact source chain-type label, never implicitly EVM.
        kind: Identifier,
    },
}
impl Family {
    /// Returns the expected source chain-type label.
    #[must_use]
    pub fn source_type(&self) -> &str {
        match self {
            Self::Evm { .. } => "EVM",
            Self::Solana { .. } => "SOLANA",
            Self::Provider { kind } => kind.as_str(),
        }
    }
}
/// Caller-qualified alias, optional provider ID, family and explicit testnet flag.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ChainFields")]
pub struct Chain {
    alias: Identifier,
    provider_id: Option<u64>,
    family: Family,
    testnet: bool,
}
impl Chain {
    /// Checks positive provider IDs and family/identifier separation.
    /// # Errors
    /// Rejects contradictory EVM numbers and reserved provider-family labels.
    pub fn new(
        alias: Identifier,
        provider_id: Option<u64>,
        family: Family,
        testnet: bool,
    ) -> Result<Self, Error> {
        if provider_id == Some(0)
            || matches!(&family,Family::Provider{kind} if matches!(kind.as_str(),"EVM"|"SOLANA"))
            || matches!(&family,Family::Evm{chain_id} if provider_id.map(ChainId::from)!=Some(*chain_id))
        {
            return Err(super::invalid_identity());
        }
        Ok(Self {
            alias,
            provider_id,
            family,
            testnet,
        })
    }
    /// Returns the provider alias, without an inferred numeric identity.
    #[must_use]
    pub const fn alias(&self) -> &Identifier {
        &self.alias
    }
    /// Returns the actual declared optional provider catalogue number.
    #[must_use]
    pub const fn provider_id(&self) -> Option<u64> {
        self.provider_id
    }
    /// Returns the explicit family-specific qualification.
    #[must_use]
    pub const fn family(&self) -> &Family {
        &self.family
    }
    /// Returns the explicit provider testnet category.
    #[must_use]
    pub const fn testnet(&self) -> bool {
        self.testnet
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainFields {
    alias: Identifier,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    provider_id: Option<u64>,
    family: Family,
    testnet: bool,
}
impl TryFrom<ChainFields> for Chain {
    type Error = Error;
    fn try_from(v: ChainFields) -> Result<Self, Error> {
        Self::new(v.alias, v.provider_id, v.family, v.testnet)
    }
}

/// Complete caller lookup for aliases referenced by source routes and fees.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Chain>", into = "Vec<Chain>")]
pub struct Catalogue(Vec<Chain>);
impl Catalogue {
    /// Checks 1–512 unique aliases; null IDs remain distinct by alias.
    /// # Errors
    /// Rejects duplicates and resource excess instead of truncating.
    pub fn new(v: Vec<Chain>) -> Result<Self, Error> {
        if v.is_empty()
            || v.len() > 512
            || !crate::domain::market::unique(v.iter().map(|c| c.alias.clone()))
        {
            return Err(super::invalid_identity());
        }
        Ok(Self(v))
    }
    /// Returns every qualification in supplied order.
    #[must_use]
    pub fn chains(&self) -> &[Chain] {
        &self.0
    }
    /// Resolves an exact alias; unknown families are never guessed.
    /// # Errors
    /// Returns unsupported capability for an unqualified alias.
    pub fn resolve(&self, alias: &str) -> Result<&Chain, Error> {
        self.0
            .iter()
            .find(|c| c.alias.as_str() == alias)
            .ok_or(Error::UnsupportedCapability)
    }
}
impl TryFrom<Vec<Chain>> for Catalogue {
    type Error = Error;
    fn try_from(v: Vec<Chain>) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Catalogue> for Vec<Chain> {
    fn from(v: Catalogue) -> Self {
        v.0
    }
}

/// Exact family-specific token identity; native uses Rubic's zero-address sentinel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "identity",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AssetIdentifier {
    /// Provider-native identity, without a guessed decimal precision.
    Native,
    /// Exact nonzero EVM token address.
    EvmToken(Address),
    /// Exact Solana mint; native and wrapped SOL are separate choices.
    SolanaMint(Pubkey),
    /// Explicit other-family provider identity; content is not locally authenticated.
    Provider(Text),
}
/// Rubic core 1.9.1's native-SOL provider sentinel. This is not an SPL mint,
/// the wrapped-SOL mint, or proof of an on-chain token account.
pub const SOLANA_NATIVE_ASSET_ADDRESS: &str = "So11111111111111111111111111111111111111111";
/// Exact qualified asset and mandatory caller/source decimal precision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AssetFields")]
pub struct Asset {
    chain: Chain,
    identifier: AssetIdentifier,
    decimals: u8,
}
impl Asset {
    /// Checks family agreement and rejects EVM-zero and Solana-native sentinel
    /// collisions with actual token/mint identities.
    /// # Errors
    /// Rejects incompatible family encodings.
    pub fn new(chain: Chain, identifier: AssetIdentifier, decimals: u8) -> Result<Self, Error> {
        let valid = match (&identifier, chain.family()) {
            (AssetIdentifier::EvmToken(a), Family::Evm { .. }) => a.bytes() != [0; 20],
            (AssetIdentifier::SolanaMint(p), Family::Solana { .. }) => {
                p.to_string() != SOLANA_NATIVE_ASSET_ADDRESS
            }
            (AssetIdentifier::Native, _)
            | (AssetIdentifier::Provider(_), Family::Provider { .. }) => true,
            _ => false,
        };
        if !valid {
            return Err(super::invalid_identity());
        }
        Ok(Self {
            chain,
            identifier,
            decimals,
        })
    }
    /// Returns the complete explicit chain qualification.
    #[must_use]
    pub const fn chain(&self) -> &Chain {
        &self.chain
    }
    /// Returns the family-specific asset identity.
    #[must_use]
    pub const fn identifier(&self) -> &AssetIdentifier {
        &self.identifier
    }
    /// Returns explicit known precision; no default is introduced.
    #[must_use]
    pub const fn decimals(&self) -> u8 {
        self.decimals
    }
    /// Returns Rubic's request encoding for this qualified identity.
    #[must_use]
    pub fn source_address(&self) -> String {
        match &self.identifier {
            AssetIdentifier::Native if matches!(self.chain.family(), Family::Solana { .. }) => {
                SOLANA_NATIVE_ASSET_ADDRESS.into()
            }
            AssetIdentifier::Native => "0x0000000000000000000000000000000000000000".into(),
            AssetIdentifier::EvmToken(a) => a.to_string(),
            AssetIdentifier::SolanaMint(a) => a.to_string(),
            AssetIdentifier::Provider(a) => a.as_str().into(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetFields {
    chain: Chain,
    identifier: AssetIdentifier,
    decimals: u8,
}
impl TryFrom<AssetFields> for Asset {
    type Error = Error;
    fn try_from(v: AssetFields) -> Result<Self, Error> {
        Self::new(v.chain, v.identifier, v.decimals)
    }
}

/// Exact family-specific account, with no private key or authorization policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "family",
    content = "address",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Account {
    /// Validated EVM address.
    Evm(Address),
    /// Validated Solana public key.
    Solana(Pubkey),
    /// Explicit other-family provider account, without a local address proof.
    Provider(Text),
}
impl Account {
    /// Checks exact family agreement.
    /// # Errors
    /// Rejects an account interpreted under another family.
    pub fn validate(&self, chain: &Chain) -> Result<(), Error> {
        if matches!(
            (self, chain.family()),
            (Self::Evm(_), Family::Evm { .. })
                | (Self::Solana(_), Family::Solana { .. })
                | (Self::Provider(_), Family::Provider { .. })
        ) {
            Ok(())
        } else {
            Err(super::invalid_identity())
        }
    }
    /// Returns deliberate account text for outgoing provider requests.
    #[must_use]
    pub fn source_address(&self) -> String {
        match self {
            Self::Evm(a) => a.to_string(),
            Self::Solana(a) => a.to_string(),
            Self::Provider(a) => a.as_str().into(),
        }
    }
}
/// Exact source/destination transaction identity with an explicit encoding family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "family",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TransactionId {
    /// Exact 32-byte EVM transaction hash.
    Evm(crate::domain::evm::TransactionId),
    /// Exact Solana first-signature identity.
    Solana(Signature),
    /// Other explicitly qualified provider transaction identity.
    Provider(Text),
}
impl TransactionId {
    /// Checks the identity's family without asserting inclusion or finality.
    /// # Errors
    /// Rejects a mismatched family.
    pub fn validate(&self, chain: &Chain) -> Result<(), Error> {
        if matches!(
            (self, chain.family()),
            (Self::Evm(_), Family::Evm { .. })
                | (Self::Solana(_), Family::Solana { .. })
                | (Self::Provider(_), Family::Provider { .. })
        ) {
            Ok(())
        } else {
            Err(super::invalid_identity())
        }
    }
    /// Returns the exact wire identity.
    #[must_use]
    pub fn source_id(&self) -> String {
        match self {
            Self::Evm(v) => v.to_string(),
            Self::Solana(v) => v.to_string(),
            Self::Provider(v) => v.as_str().into(),
        }
    }
}
