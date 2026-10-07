// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    AccountBalance, Block, FeeEstimate, HistoryPage, MessageStatus, Network, NetworkData,
    SubmissionResult, Transaction, TransactionStatus,
};
use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// Actual represented TON operation, without a consensus-finality inference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Masterchain/network identity.
    NetworkData,
    /// Full-block-associated account balance/state.
    AccountBalance,
    /// One bounded account-linked history page.
    AccountHistory,
    /// One account-qualified transaction lookup.
    Transaction,
    /// Actual decoded transaction execution.
    TransactionStatus,
    /// One explicitly bounded incoming-message history scan.
    MessageStatus,
    /// Source wallet-body fee suggestions.
    FeeEstimate,
    /// Preliminary one-shot external message submission.
    Submission,
}
/// Explicit TON source, retrieval and optional actual full-block attribution.
/// History lookups have no invented common evaluation or inclusion block.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    operation: Operation,
    network: Network,
    block: Option<Block>,
    source: Source,
    retrieved_at: Timestamp,
}
impl Context {
    /// Records explicit caller/source facts without network access or a clock.
    #[must_use]
    pub const fn new(
        operation: Operation,
        network: Network,
        block: Option<Block>,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Self {
        Self {
            operation,
            network,
            block,
            source,
            retrieved_at,
        }
    }
    /// Returns actual represented operation.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }
    /// Returns full expected zero-state network identity.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
    /// Returns actual source full block only when this operation supplies it.
    #[must_use]
    pub const fn block(&self) -> Option<&Block> {
        self.block.as_ref()
    }
    /// Returns explicit non-secret attribution.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns retrieval Unix seconds, separate from transaction/source sync times.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}
/// Strict typed TON observation; current schema version is one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    try_from = "Fields<T>",
    bound(deserialize = "T: Deserialize<'de> + ObservationValue")
)]
pub struct Observation<T> {
    schema_version: u16,
    context: Context,
    value: T,
}
impl<T: ObservationValue> Observation<T> {
    /// Records typed values under an explicit expected operation.
    ///
    /// # Errors
    /// Rejects operation mismatch; account/network reads require a real masterchain block.
    pub fn new(value: T, context: Context) -> Result<Self, Error> {
        let expected = T::operation();
        if context.operation != expected {
            return Err(ValidationError::ObservationOperationMismatch.into());
        }
        if matches!(expected, Operation::NetworkData | Operation::AccountBalance)
            && !context.block.as_ref().is_some_and(Block::is_masterchain)
        {
            return Err(super::invalid_record());
        }
        value.validate_context(&context)?;
        Ok(Self {
            schema_version: 1,
            context,
            value,
        })
    }
    /// Returns schema version one.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns exact source/network/block attribution.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns the complete typed value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields<T> {
    schema_version: u16,
    context: Context,
    value: T,
}
impl<T: ObservationValue> TryFrom<Fields<T>> for Observation<T> {
    type Error = Error;
    fn try_from(v: Fields<T>) -> Result<Self, Error> {
        if v.schema_version != 1 {
            return Err(super::invalid_record());
        }
        Self::new(v.value, v.context)
    }
}

/// Family-specific observation correlation contract for independently implemented backends.
/// Custom implementations are trusted to retain their actual operation/network facts.
pub trait ObservationValue {
    /// Returns the one represented operation for this value type.
    fn operation() -> Operation;
    /// Checks actual value facts against the supplied source context.
    ///
    /// # Errors
    /// Rejects contradictory operation/network/block facts.
    fn validate_context(&self, context: &Context) -> Result<(), Error>;
}
impl ObservationValue for NetworkData {
    fn operation() -> Operation {
        Operation::NetworkData
    }
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        if c.network() != self.network() || c.block() != Some(self.last()) {
            return Err(super::invalid_record());
        }
        Ok(())
    }
}
impl ObservationValue for AccountBalance {
    fn operation() -> Operation {
        Operation::AccountBalance
    }
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        self.address().check_network(c.network())
    }
}
impl ObservationValue for HistoryPage {
    fn operation() -> Operation {
        Operation::AccountHistory
    }
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        self.request().address().check_network(c.network())
    }
}
impl ObservationValue for Transaction {
    fn operation() -> Operation {
        Operation::Transaction
    }
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        self.account().check_network(c.network())
    }
}
impl ObservationValue for TransactionStatus {
    fn operation() -> Operation {
        Operation::TransactionStatus
    }
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        self.transaction().account().check_network(c.network())
    }
}
impl ObservationValue for MessageStatus {
    fn operation() -> Operation {
        Operation::MessageStatus
    }
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        self.page().request().address().check_network(c.network())
    }
}
impl ObservationValue for FeeEstimate {
    fn operation() -> Operation {
        Operation::FeeEstimate
    }
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        if self.request().network() != c.network() {
            return Err(super::invalid_record());
        }
        Ok(())
    }
}
impl ObservationValue for SubmissionResult {
    fn operation() -> Operation {
        Operation::Submission
    }
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        if self.submission().network() != c.network() {
            return Err(super::invalid_record());
        }
        Ok(())
    }
}
