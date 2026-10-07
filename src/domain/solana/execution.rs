// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Method-specific execution requests, values and attributed source observations.

mod context;
mod metadata;
mod request;

pub use context::{ExecutionContext, ExecutionObservation};
#[cfg(feature = "solana-http")]
pub(crate) use metadata::bounded_entries;
pub use metadata::{
    CompiledInnerInstruction, ExecutionBytes, ExecutionOutcome, InnerInstructions, LoadedAddresses,
    LogMessage, ReturnData, Reward, RewardKind, TokenBalanceRecord, TransactionMetadata,
    TransactionMetadataData,
};
pub use request::{ExecutionRequest, StatusOptions, SubmitOptions, TransactionReadOptions};

use super::{BlockhashLifetime, Commitment, Hash, Signature, SignedTransaction};
use crate::error::{Error, ValidationError};
use serde::{Deserialize, Serialize};

pub(super) fn invalid() -> Error {
    ValidationError::InvalidSolanaExecution.into()
}

/// A source-reported lookup at the requested commitment; absence is not pending proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionLookup {
    /// Exact requested first signature identity.
    pub signature: Signature,
    /// Present source transaction or explicit lookup absence at the selected state.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub transaction: Option<Transaction>,
}

/// Canonical source transaction bytes with separately reported inclusion and execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionFields")]
pub struct Transaction {
    body: SignedTransaction,
    inclusion_slot: u64,
    block_time_unix_seconds: Option<i64>,
    #[serde(rename = "transaction_index")]
    index_in_block: Option<u32>,
    metadata: Option<TransactionMetadata>,
}
impl Transaction {
    /// Records source facts and checks byte/metadata structural agreement.
    /// # Errors
    /// Rejects account, instruction, lookup or execution-index inconsistencies.
    pub fn new(
        body: SignedTransaction,
        inclusion_slot: u64,
        block_time_unix_seconds: Option<i64>,
        transaction_index: Option<u32>,
        metadata: Option<TransactionMetadata>,
    ) -> Result<Self, Error> {
        if let Some(value) = &metadata {
            value.validate_message(body.message())?;
        }
        Ok(Self {
            body,
            inclusion_slot,
            block_time_unix_seconds,
            index_in_block: transaction_index,
            metadata,
        })
    }
    /// Returns canonical bytes with structurally present, unverified signatures.
    #[must_use]
    pub const fn body(&self) -> &SignedTransaction {
        &self.body
    }
    /// Returns actual source inclusion slot, without independently proving finality.
    #[must_use]
    pub const fn inclusion_slot(&self) -> u64 {
        self.inclusion_slot
    }
    /// Returns optional source block time, distinct from local retrieval time.
    #[must_use]
    pub const fn block_time_unix_seconds(&self) -> Option<i64> {
        self.block_time_unix_seconds
    }
    /// Returns optional source transaction index without inventing one for older nodes.
    #[must_use]
    pub const fn transaction_index(&self) -> Option<u32> {
        self.index_in_block
    }
    /// Returns optional source execution metadata, preserving unavailable metadata.
    #[must_use]
    pub const fn metadata(&self) -> Option<&TransactionMetadata> {
        self.metadata.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionFields {
    body: SignedTransaction,
    inclusion_slot: u64,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    block_time_unix_seconds: Option<i64>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    transaction_index: Option<u32>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    metadata: Option<TransactionMetadata>,
}
impl TryFrom<TransactionFields> for Transaction {
    type Error = Error;
    fn try_from(f: TransactionFields) -> Result<Self, Error> {
        Self::new(
            f.body,
            f.inclusion_slot,
            f.block_time_unix_seconds,
            f.transaction_index,
            f.metadata,
        )
    }
}

/// Actual status-cache/history response, independent of the lookup's evaluation slot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureStatus {
    /// Actual source inclusion slot.
    pub inclusion_slot: u64,
    /// Source confirmations; null is retained without independently inferring finality.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub confirmations: Option<u64>,
    /// Optional source confirmation classification, absent on some older responses.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub confirmation_status: Option<Commitment>,
    /// Actual source execution result, distinct from confirmation classification.
    pub outcome: ExecutionOutcome,
}
/// A status lookup in the explicitly selected recent-cache or history search.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusLookup {
    /// Exact requested first signature identity.
    pub signature: Signature,
    /// Explicit source absence remains unknown in the selected search.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub status: Option<SignatureStatus>,
}
/// Latest recent blockhash and source-reported block-height lifetime.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatestBlockhash {
    /// Recent blockhash and last-valid BLOCK HEIGHT, not an evaluation slot.
    pub lifetime: BlockhashLifetime,
}
/// Source validity check for the exact supplied recent blockhash.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockhashValidity {
    /// Exact frozen query hash.
    pub blockhash: Hash,
    /// Actual validity response at the separately reported evaluation slot.
    pub valid: bool,
}
/// Exact source block height with no invented context slot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockHeight {
    /// Source block height, distinct from slots and wall-clock time.
    pub height: u64,
}
/// Exact fee estimation for immutable canonical message bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageFee {
    /// Exact lamports or explicit fee unavailability at the requested state.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub lamports: Option<u64>,
}
/// Unsigned simulation result with no signature verification or blockhash replacement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SimulationFields")]
pub struct Simulation {
    outcome: ExecutionOutcome,
    units_consumed: Option<u64>,
    fee_lamports: Option<u64>,
    logs: Option<Vec<LogMessage>>,
    return_data: Option<ReturnData>,
}
impl Simulation {
    /// Retains bounded actual simulation facts, including execution failure.
    /// # Errors
    /// Rejects excessive source log collections.
    pub fn new(
        outcome: ExecutionOutcome,
        units_consumed: Option<u64>,
        fee_lamports: Option<u64>,
        logs: Option<Vec<LogMessage>>,
        return_data: Option<ReturnData>,
    ) -> Result<Self, Error> {
        metadata::validate_logs(logs.as_deref())?;
        Ok(Self {
            outcome,
            units_consumed,
            fee_lamports,
            logs,
            return_data,
        })
    }
    /// Returns actual execution success/failure without approval or finality claims.
    #[must_use]
    pub const fn outcome(&self) -> &ExecutionOutcome {
        &self.outcome
    }
    /// Returns optional source consumed compute units.
    #[must_use]
    pub const fn units_consumed(&self) -> Option<u64> {
        self.units_consumed
    }
    /// Returns optional actual source fee in lamports.
    #[must_use]
    pub const fn fee_lamports(&self) -> Option<u64> {
        self.fee_lamports
    }
    /// Returns bounded source logs, preserving explicit empty logs and absent logs.
    #[must_use]
    pub fn logs(&self) -> Option<&[LogMessage]> {
        self.logs.as_deref()
    }
    /// Returns optional program return bytes.
    #[must_use]
    pub const fn return_data(&self) -> Option<&ReturnData> {
        self.return_data.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SimulationFields {
    outcome: ExecutionOutcome,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    units_consumed: Option<u64>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    fee_lamports: Option<u64>,
    #[serde(deserialize_with = "metadata::optional_logs")]
    logs: Option<Vec<LogMessage>>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    return_data: Option<ReturnData>,
}
impl TryFrom<SimulationFields> for Simulation {
    type Error = Error;
    fn try_from(f: SimulationFields) -> Result<Self, Error> {
        Self::new(
            f.outcome,
            f.units_consumed,
            f.fee_lamports,
            f.logs,
            f.return_data,
        )
    }
}
/// Matching source submission acknowledgement, not inclusion or execution proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// Exact first signature returned for the caller-supplied signed bytes.
    pub signature: Signature,
}
