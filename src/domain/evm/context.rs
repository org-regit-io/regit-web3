// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{BlockContext, BlockSelector, Finality, Inclusion, NetworkId, Source, Timestamp};
use crate::error::{Error, ValidationError};
use serde::{Deserialize, Deserializer, Serialize};

/// The typed EVM operation described by a wider read observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadOperation {
    /// Exact ERC-20 `balanceOf` at a captured canonical hash.
    Erc20Balance,
    /// Exact ERC-20 `allowance` at a captured canonical hash.
    Erc20Allowance,
    /// Separate optional ERC-20 metadata outcomes at one captured canonical hash.
    Erc20Metadata,
    /// Source transaction fields for an explicit transaction identifier.
    Transaction,
    /// Source receipt and top-level execution outcome.
    Receipt,
    /// Joint transaction/receipt status with matched observed identities.
    TransactionStatus,
}

/// Actual state attribution for a read, without invented block timestamps.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReadState {
    /// State was read at this exact hash with source-asserted canonicality.
    CanonicalHash {
        /// The original caller/configuration selector, retained independently.
        requested_selector: BlockSelector,
        /// The resolved number, hash and separately source-reported timestamp.
        block: BlockContext,
    },
    /// Source-reported inclusion; no block timestamp or canonicality proof.
    Included {
        /// The actual block hash/number and transaction position.
        inclusion: Inclusion,
    },
    /// The source returned an actual pending transaction with no inclusion.
    Pending,
    /// No single observed state anchor is available.
    Unanchored,
}
impl ReadState {
    fn validate(self) -> Result<(), Error> {
        if let Self::CanonicalHash {
            requested_selector,
            block,
        } = self
        {
            let mismatch = match requested_selector {
                BlockSelector::Number(n) => n != block.number(),
                BlockSelector::Hash(h) => h != *block.hash(),
                BlockSelector::Latest | BlockSelector::Safe | BlockSelector::Finalized => false,
            };
            if mismatch {
                return Err(ValidationError::BlockMismatch.into());
            }
        }
        Ok(())
    }
}

/// Provenance for wider EVM reads, independent of the original native API.
///
/// Finality is unknown and confirmations absent. Tags, successful execution
/// and returned inclusion never independently prove either fact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields")]
pub struct OperationContext {
    schema_version: u16,
    operation: ReadOperation,
    network: NetworkId,
    state: ReadState,
    source: Source,
    retrieved_at: Timestamp,
    finality: Finality,
    confirmations: Option<u64>,
}
impl OperationContext {
    /// Records explicit operation, network, state, provenance and retrieval time.
    ///
    /// # Errors
    /// Rejects selector/resolved-block mismatches.
    pub fn new(
        operation: ReadOperation,
        network: NetworkId,
        state: ReadState,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        state.validate()?;
        Ok(Self {
            schema_version: 1,
            operation,
            network,
            state,
            source,
            retrieved_at,
            finality: Finality::Unknown,
            confirmations: None,
        })
    }
    /// Returns the serialized schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns the declared operation.
    #[must_use]
    pub const fn operation(&self) -> ReadOperation {
        self.operation
    }
    /// Returns the explicit expected network.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns the actual attribution state.
    #[must_use]
    pub const fn state(&self) -> ReadState {
        self.state
    }
    /// Returns source attribution.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns actual retrieval time independently of any block timestamp.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
    /// Returns unknown finality; no independent proof is supplied.
    #[must_use]
    pub const fn finality(&self) -> Finality {
        self.finality
    }
    /// Returns absent confirmations; no head-depth calculation is supplied.
    #[must_use]
    pub const fn confirmations(&self) -> Option<u64> {
        self.confirmations
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFields {
    schema_version: u16,
    operation: ReadOperation,
    network: NetworkId,
    state: ReadState,
    source: Source,
    retrieved_at: Timestamp,
    finality: Finality,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    confirmations: Option<u64>,
}
impl TryFrom<ContextFields> for OperationContext {
    type Error = Error;
    fn try_from(v: ContextFields) -> Result<Self, Error> {
        if v.schema_version != 1 {
            return Err(ValidationError::UnsupportedSchemaVersion.into());
        }
        if v.finality != Finality::Unknown || v.confirmations.is_some() {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        Self::new(v.operation, v.network, v.state, v.source, v.retrieved_at)
    }
}

/// A typed value's consistency contract for an EVM operation observation.
///
/// Implementors validate operation, network and actual state attribution. This
/// is a structural source-record contract, not independent chain verification.
pub trait OperationValue {
    /// Validates the complete value against its supplied provenance.
    ///
    /// # Errors
    /// Rejects operation, network, anchor or internal record inconsistencies.
    fn validate_context(&self, context: &OperationContext) -> Result<(), Error>;
}

/// A validated typed wider EVM read result with explicit provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OperationObservation<T: OperationValue> {
    value: T,
    context: OperationContext,
}
impl<T: OperationValue> OperationObservation<T> {
    /// Validates a typed record against its operation, network and state.
    ///
    /// # Errors
    /// Propagates structural consistency failures from the typed record.
    pub fn new(value: T, context: OperationContext) -> Result<Self, Error> {
        value.validate_context(&context)?;
        Ok(Self { value, context })
    }
    /// Returns the typed operation value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
    /// Returns its separately retained provenance.
    #[must_use]
    pub const fn context(&self) -> &OperationContext {
        &self.context
    }
}
impl<'de, T: OperationValue + Deserialize<'de>> Deserialize<'de> for OperationObservation<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields<T> {
            value: T,
            context: OperationContext,
        }
        let v = Fields::<T>::deserialize(d)?;
        Self::new(v.value, v.context).map_err(serde::de::Error::custom)
    }
}
