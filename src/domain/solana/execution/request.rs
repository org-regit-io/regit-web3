// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::{
    Commitment, Hash, ReadOptions, Signature, UnsignedMessage, UnsignedTransaction,
};
use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Explicit transaction lookup commitment and supported numeric message version.
/// This RPC accepts confirmed/finalized commitment and no minimum context slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionOptionsFields")]
pub struct TransactionReadOptions {
    commitment: Commitment,
    maximum_supported_version: u8,
}
impl TransactionReadOptions {
    /// Records supported lookup options without changing the node's capabilities.
    /// # Errors
    /// Rejects processed commitment and numeric versions greater than one.
    pub fn new(commitment: Commitment, maximum_supported_version: u8) -> Result<Self, Error> {
        if commitment == Commitment::Processed || maximum_supported_version > 1 {
            return Err(super::invalid());
        }
        Ok(Self {
            commitment,
            maximum_supported_version,
        })
    }
    /// Returns explicit confirmed/finalized request commitment.
    #[must_use]
    pub const fn commitment(self) -> Commitment {
        self.commitment
    }
    /// Returns the explicit maximum numeric transaction version (zero or one).
    #[must_use]
    pub const fn maximum_supported_version(self) -> u8 {
        self.maximum_supported_version
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionOptionsFields {
    commitment: Commitment,
    maximum_supported_version: u8,
}
impl TryFrom<TransactionOptionsFields> for TransactionReadOptions {
    type Error = Error;
    fn try_from(f: TransactionOptionsFields) -> Result<Self, Error> {
        Self::new(f.commitment, f.maximum_supported_version)
    }
}
/// Explicit status-cache/history selection; this RPC has no commitment or min slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusOptions {
    /// Request history search beyond the recent status cache.
    pub search_transaction_history: bool,
}
/// Explicit one-shot submission controls, with node `maxRetries` always zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitOptions {
    /// Caller-selected preflight commitment, without independently proving finality.
    pub preflight_commitment: Commitment,
    /// Optional source evaluation lower bound, not an exact historical selector.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub minimum_context_slot: Option<u64>,
    /// Explicit decision whether the source should skip its preflight simulation.
    pub skip_preflight: bool,
}

/// Immutable method-specific request retained in execution observations.
/// Unsupported RPC controls are not silently added to other method variants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionRequest {
    /// Lookup of exact transaction identity at supported commitment.
    Transaction {
        /// Exact first signature query.
        signature: Signature,
        /// Actual supported lookup controls.
        options: TransactionReadOptions,
    },
    /// Explicit recent-cache or historical status lookup.
    Status {
        /// Exact first signature query.
        signature: Signature,
        /// Actual supported status controls.
        options: StatusOptions,
    },
    /// Latest blockhash lookup with reported context slot.
    LatestBlockhash {
        /// Commitment and optional context lower bound.
        options: ReadOptions,
    },
    /// Validity check of an immutable blockhash.
    BlockhashValidity {
        /// Exact frozen query identity.
        blockhash: Hash,
        /// Commitment and optional context lower bound.
        options: ReadOptions,
    },
    /// Actual block-height read, without a fabricated context slot.
    BlockHeight {
        /// Actual supported commitment selector.
        commitment: Commitment,
    },
    /// Lamport fee estimation for the exact message bytes.
    MessageFee {
        /// Exact validated immutable message bytes.
        message: UnsignedMessage,
        /// Commitment and optional context lower bound.
        options: ReadOptions,
    },
    /// Zero-signature simulation with signature verification and replacement off.
    Simulation {
        /// Exact validated zero-placeholder transaction bytes.
        transaction: UnsignedTransaction,
        /// Commitment and optional context lower bound.
        options: ReadOptions,
    },
    /// One source write of exact caller-signed bytes, identified by the first signature.
    Submission {
        /// Expected byte-derived first signature.
        signature: Signature,
        /// Explicit preflight controls; no leader retry is requested.
        options: SubmitOptions,
    },
}
impl ExecutionRequest {
    pub(super) const fn evaluation_options(&self) -> Option<ReadOptions> {
        match self {
            Self::LatestBlockhash { options }
            | Self::BlockhashValidity { options, .. }
            | Self::MessageFee { options, .. }
            | Self::Simulation { options, .. } => Some(*options),
            _ => None,
        }
    }
    pub(super) const fn has_evaluation_slot(&self) -> bool {
        matches!(
            self,
            Self::Status { .. }
                | Self::LatestBlockhash { .. }
                | Self::BlockhashValidity { .. }
                | Self::MessageFee { .. }
                | Self::Simulation { .. }
        )
    }
}
