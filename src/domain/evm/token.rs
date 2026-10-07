// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Address, Amount, ChainId, OperationContext, OperationValue, ReadOperation, ReadState};
use crate::error::{Error, ValidationError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Exact token raw units at a separately recorded state; precision is unknown.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BalanceFields")]
pub struct Erc20Balance {
    chain_id: ChainId,
    contract: Address,
    owner: Address,
    amount: Amount,
}
impl Erc20Balance {
    /// Records exact raw units without inventing token decimals.
    #[must_use]
    pub const fn new(
        chain_id: ChainId,
        contract: Address,
        owner: Address,
        raw: super::U256,
    ) -> Self {
        Self {
            chain_id,
            contract,
            owner,
            amount: Amount::new(raw, None),
        }
    }
    /// Returns exact token chain identity.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }
    /// Returns the token's contract identity.
    #[must_use]
    pub const fn contract(&self) -> Address {
        self.contract
    }
    /// Returns the requested owner.
    #[must_use]
    pub const fn owner(&self) -> Address {
        self.owner
    }
    /// Returns exact raw units with unavailable precision.
    #[must_use]
    pub const fn amount(&self) -> Amount {
        self.amount
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BalanceFields {
    chain_id: ChainId,
    contract: Address,
    owner: Address,
    amount: Amount,
}
impl TryFrom<BalanceFields> for Erc20Balance {
    type Error = Error;
    fn try_from(v: BalanceFields) -> Result<Self, Error> {
        if v.amount.decimals().is_some() {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        Ok(Self::new(v.chain_id, v.contract, v.owner, v.amount.raw()))
    }
}

/// Exact raw allowance for one token, owner and spender at a recorded state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AllowanceFields")]
pub struct Erc20Allowance {
    chain_id: ChainId,
    contract: Address,
    owner: Address,
    spender: Address,
    amount: Amount,
}
impl Erc20Allowance {
    /// Records exact raw allowance without choosing an approval policy.
    #[must_use]
    pub const fn new(
        chain_id: ChainId,
        contract: Address,
        owner: Address,
        spender: Address,
        raw: super::U256,
    ) -> Self {
        Self {
            chain_id,
            contract,
            owner,
            spender,
            amount: Amount::new(raw, None),
        }
    }
    /// Returns exact token chain identity.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }
    /// Returns the token contract.
    #[must_use]
    pub const fn contract(&self) -> Address {
        self.contract
    }
    /// Returns the requested owner.
    #[must_use]
    pub const fn owner(&self) -> Address {
        self.owner
    }
    /// Returns the requested spender.
    #[must_use]
    pub const fn spender(&self) -> Address {
        self.spender
    }
    /// Returns exact raw allowance with unavailable precision.
    #[must_use]
    pub const fn amount(&self) -> Amount {
        self.amount
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowanceFields {
    chain_id: ChainId,
    contract: Address,
    owner: Address,
    spender: Address,
    amount: Amount,
}
impl TryFrom<AllowanceFields> for Erc20Allowance {
    type Error = Error;
    fn try_from(v: AllowanceFields) -> Result<Self, Error> {
        if v.amount.decimals().is_some() {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        Ok(Self::new(
            v.chain_id,
            v.contract,
            v.owner,
            v.spender,
            v.amount.raw(),
        ))
    }
}

/// Bounded UTF-8 metadata exactly as returned by a supported string ABI.
///
/// Empty strings are valid; no symbol or name policy is imposed. Debug is opaque.
#[derive(Clone, Eq, PartialEq)]
pub struct MetadataText(String);
impl MetadataText {
    /// The maximum UTF-8 encoded text length in bytes.
    pub const MAX_BYTES: usize = 4096;
    /// Records bounded text without normalization.
    ///
    /// # Errors
    /// Rejects text longer than the explicit byte bound.
    pub fn new(value: String) -> Result<Self, Error> {
        if value.len() > Self::MAX_BYTES {
            return Err(ValidationError::InvalidEvmMetadata.into());
        }
        Ok(Self(value))
    }
    /// Returns the exact decoded text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for MetadataText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MetadataText").finish_non_exhaustive()
    }
}
impl Serialize for MetadataText {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for MetadataText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// A source availability fact for an optional ERC-20 metadata method.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataUnavailable {
    /// The provider explicitly reports the method unsupported.
    Unsupported,
    /// The provider reports required state/data unavailable.
    StateUnavailable,
    /// The successful JSON-RPC envelope has a null result.
    NullResult,
}
/// An optional metadata outcome, preserving revert and malformed ABI distinctly.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum MetadataValue<T> {
    /// A supported ABI returned a valid exact value.
    Available(T),
    /// The source explicitly reported execution revert; not independent proof.
    Reverted,
    /// The source/method had unavailable data, with its precise category.
    Unavailable(MetadataUnavailable),
    /// Returned bytes did not satisfy the supported bounded canonical ABI.
    InvalidAbi,
}
/// Independently optional ERC-20 name, symbol and precision at one pinned state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Erc20Metadata {
    chain_id: ChainId,
    contract: Address,
    name: MetadataValue<MetadataText>,
    symbol: MetadataValue<MetadataText>,
    decimals: MetadataValue<u8>,
}
impl Erc20Metadata {
    /// Records each optional outcome independently, without a precision default.
    #[must_use]
    pub const fn new(
        chain_id: ChainId,
        contract: Address,
        name: MetadataValue<MetadataText>,
        symbol: MetadataValue<MetadataText>,
        decimals: MetadataValue<u8>,
    ) -> Self {
        Self {
            chain_id,
            contract,
            name,
            symbol,
            decimals,
        }
    }
    /// Returns the exact token chain identity.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }
    /// Returns the token contract.
    #[must_use]
    pub const fn contract(&self) -> Address {
        self.contract
    }
    /// Returns the exact name outcome.
    #[must_use]
    pub const fn name(&self) -> &MetadataValue<MetadataText> {
        &self.name
    }
    /// Returns the exact symbol outcome.
    #[must_use]
    pub const fn symbol(&self) -> &MetadataValue<MetadataText> {
        &self.symbol
    }
    /// Returns precision availability independently of either display field.
    #[must_use]
    pub const fn decimals(&self) -> &MetadataValue<u8> {
        &self.decimals
    }
}
fn validate_token(
    chain: ChainId,
    operation: ReadOperation,
    c: &OperationContext,
) -> Result<(), Error> {
    if c.operation() != operation {
        return Err(ValidationError::ObservationOperationMismatch.into());
    }
    if c.network().chain_id() != chain {
        return Err(ValidationError::NetworkMismatch.into());
    }
    if !matches!(c.state(), ReadState::CanonicalHash { .. }) {
        return Err(ValidationError::InvalidEvmRecord.into());
    }
    Ok(())
}
impl OperationValue for Erc20Balance {
    fn validate_context(&self, c: &OperationContext) -> Result<(), Error> {
        validate_token(self.chain_id, ReadOperation::Erc20Balance, c)
    }
}
impl OperationValue for Erc20Allowance {
    fn validate_context(&self, c: &OperationContext) -> Result<(), Error> {
        validate_token(self.chain_id, ReadOperation::Erc20Allowance, c)
    }
}
impl OperationValue for Erc20Metadata {
    fn validate_context(&self, c: &OperationContext) -> Result<(), Error> {
        validate_token(self.chain_id, ReadOperation::Erc20Metadata, c)
    }
}
