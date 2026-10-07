// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    Address, AddressBalance, CollectionLimit, FeeEstimate, FeeTarget, History, HistoryRange,
    NetworkIdentity, TokenFilter, Transaction, TransactionStatus, Txid, UnspentOutputs,
};
use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// Exact caller operation/query identity, without a fabricated shared block anchor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Confirmed native BCH balance and separate signed unconfirmed delta.
    AddressBalance {
        /// Caller-qualified address.
        address: Address,
        /// Explicit native-output token inclusion policy.
        token_filter: TokenFilter,
    },
    /// Complete history for an explicit bounded height range.
    AddressHistory {
        /// Exact caller-qualified address.
        address: Address,
        /// Caller-owned height interval and capacity.
        range: HistoryRange,
    },
    /// Exact source estimated BCH per 1,000 bytes.
    FeeEstimate {
        /// Exact requested block confirmation target.
        target: FeeTarget,
    },
    /// Source-reported status for one exact transaction identity.
    TransactionStatus {
        /// Exact requested identifier.
        txid: Txid,
    },
    /// Typed source transaction fields plus separately hashed opaque raw bytes.
    Transaction {
        /// Exact requested transaction identity.
        txid: Txid,
        /// Hard complete-input/output capacity.
        limit: CollectionLimit,
    },
    /// Complete source UTXOs, including supplied exact `CashToken` metadata.
    UnspentOutputs {
        /// Exact caller-qualified address.
        address: Address,
        /// Explicit native-output inclusion policy.
        token_filter: TokenFilter,
        /// Caller hard result capacity.
        limit: CollectionLimit,
    },
}

/// Actual source software, negotiated protocol and advertised token support.
/// These are source metadata, not authenticated software or consensus guarantees.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolMetadata {
    /// Actual source software description, with bounded opaque diagnostics.
    pub software: super::SourceText,
    /// Actual negotiated source protocol version.
    pub version: super::SourceText,
    /// Actual source-advertised `CashTokens` support.
    pub cash_tokens: bool,
}

/// Schema-versioned expected chain/query/source/retrieval facts for a BCH read.
/// Fork-checkpoint matching establishes source identity agreement, not an
/// independently authenticated chain or a snapshot anchor for other read facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields")]
pub struct Context {
    schema_version: u16,
    network: NetworkIdentity,
    operation: Operation,
    source: Source,
    retrieved_at: Timestamp,
    protocol_metadata: Option<ProtocolMetadata>,
}
impl Context {
    /// Records explicitly supplied expected network, exact query and source attribution.
    /// # Errors
    /// Rejects an address incompatible with the expected address namespace.
    pub fn new(
        network: NetworkIdentity,
        operation: Operation,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        let address = match &operation {
            Operation::AddressBalance { address, .. }
            | Operation::AddressHistory { address, .. }
            | Operation::UnspentOutputs { address, .. } => Some(address),
            _ => None,
        };
        if address.is_some_and(|v| v.namespace() != network.namespace()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(Self {
            schema_version: 1,
            network,
            operation,
            source,
            retrieved_at,
            protocol_metadata: None,
        })
    }
    /// Returns the supported observation schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns explicit expected chain facts and compatible namespace.
    #[must_use]
    pub const fn network(&self) -> &NetworkIdentity {
        &self.network
    }
    /// Returns the exact caller operation/query inputs.
    #[must_use]
    pub const fn operation(&self) -> &Operation {
        &self.operation
    }
    /// Returns non-secret source attribution and library version.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns actual supplied whole Unix retrieval seconds.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
    /// Attaches actual source negotiation facts without inventing a read snapshot.
    #[must_use]
    pub fn with_protocol_metadata(mut self, value: ProtocolMetadata) -> Self {
        self.protocol_metadata = Some(value);
        self
    }
    /// Returns source negotiation metadata when the reader actually supplied it.
    #[must_use]
    pub const fn protocol_metadata(&self) -> Option<&ProtocolMetadata> {
        self.protocol_metadata.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFields {
    schema_version: u16,
    network: NetworkIdentity,
    operation: Operation,
    source: Source,
    retrieved_at: Timestamp,
    protocol_metadata: Option<ProtocolMetadata>,
}
impl TryFrom<ContextFields> for Context {
    type Error = Error;
    fn try_from(v: ContextFields) -> Result<Self, Error> {
        if v.schema_version != 1 {
            return Err(ValidationError::UnsupportedSchemaVersion.into());
        }
        let mut result = Self::new(v.network, v.operation, v.source, v.retrieved_at)?;
        result.protocol_metadata = v.protocol_metadata;
        Ok(result)
    }
}

/// Immutable typed value correlated with exact network/query/source context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ObservationFields<T>", bound(deserialize = "T: ReadValue"))]
pub struct Observation<T: ReadValue> {
    context: Context,
    value: T,
}
impl<T: ReadValue> Observation<T> {
    /// Correlates a validated typed source value with its exact operation context.
    /// # Errors
    /// Rejects operation, identity, filter, capacity or network mismatch.
    pub fn new(context: Context, value: T) -> Result<Self, Error> {
        if !value.matches(&context) {
            return Err(ValidationError::ObservationOperationMismatch.into());
        }
        Ok(Self { context, value })
    }
    /// Returns expected chain, exact query and source attribution.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns the validated correlated typed value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: ReadValue"))]
struct ObservationFields<T: ReadValue> {
    context: Context,
    value: T,
}
impl<T: ReadValue> TryFrom<ObservationFields<T>> for Observation<T> {
    type Error = Error;
    fn try_from(v: ObservationFields<T>) -> Result<Self, Error> {
        Self::new(v.context, v.value)
    }
}

mod sealed {
    pub trait Sealed {}
}
/// Library-owned values accepted by checked BCH observations.
/// This sealed contract correlates context; custom readers can return these same primitives.
#[doc(hidden)]
pub trait ReadValue: sealed::Sealed + for<'de> Deserialize<'de> {
    /// Reports whether the complete value agrees with the supplied query/network context.
    fn matches(&self, context: &Context) -> bool;
}
impl sealed::Sealed for AddressBalance {}
impl ReadValue for AddressBalance {
    fn matches(&self, c: &Context) -> bool {
        matches!(c.operation(),Operation::AddressBalance{address,token_filter} if address==&self.address&&*token_filter==self.token_filter)
    }
}
impl sealed::Sealed for History {}
impl ReadValue for History {
    fn matches(&self, c: &Context) -> bool {
        matches!(c.operation(),Operation::AddressHistory{address,range} if address==self.address()&&*range==self.range())
    }
}
impl sealed::Sealed for FeeEstimate {}
impl ReadValue for FeeEstimate {
    fn matches(&self, c: &Context) -> bool {
        matches!(c.operation(),Operation::FeeEstimate{target} if *target==self.target())
    }
}
impl sealed::Sealed for TransactionStatus {}
impl ReadValue for TransactionStatus {
    fn matches(&self, c: &Context) -> bool {
        matches!(c.operation(),Operation::TransactionStatus{txid} if *txid==self.txid())
    }
}
impl sealed::Sealed for Transaction {}
impl ReadValue for Transaction {
    fn matches(&self, c: &Context) -> bool {
        matches!(c.operation(),Operation::Transaction{txid,limit} if *txid==self.txid()&&*limit==self.limit())
            && c.network().namespace() == self.data().namespace
    }
}
impl sealed::Sealed for UnspentOutputs {}
impl ReadValue for UnspentOutputs {
    fn matches(&self, c: &Context) -> bool {
        matches!(c.operation(),Operation::UnspentOutputs{address,token_filter,limit} if address==self.address()&&*token_filter==self.token_filter()&&*limit==self.limit())
    }
}
