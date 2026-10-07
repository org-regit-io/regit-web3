// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::PreparedSwap;
use crate::{
    domain::solana::{ExecutionObservation, ExecutionRequest, MessageFee, ReadOptions, Simulation},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};
/// Exact prepared swap with separate source fee and unsigned simulation observations.
/// Actual evaluation slots may differ. Neither simulation nor a reported fee proves
/// successful execution, current lifetime validity or semantic instruction review.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EstimateFields", into = "EstimateFields")]
pub struct SwapEstimate {
    prepared: PreparedSwap,
    options: ReadOptions,
    fee: ExecutionObservation<MessageFee>,
    simulation: ExecutionObservation<Simulation>,
}
impl SwapEstimate {
    /// Correlates both source requests with the exact reviewed unsigned bytes.
    /// # Errors
    /// Rejects network, request options, message or transaction mismatches.
    pub fn new(
        prepared: PreparedSwap,
        options: ReadOptions,
        fee: ExecutionObservation<MessageFee>,
        simulation: ExecutionObservation<Simulation>,
    ) -> Result<Self, Error> {
        let network = prepared
            .intent()
            .data()
            .build
            .value()
            .data()
            .request
            .data()
            .network
            .genesis_hash();
        if fee.context().network().genesis_hash() != network
            || simulation.context().network().genesis_hash() != network
            || !matches!(fee.context().request(),ExecutionRequest::MessageFee{message,options:o} if message==prepared.unsigned().transaction().message() && *o==options)
            || !matches!(simulation.context().request(),ExecutionRequest::Simulation{transaction,options:o} if transaction==prepared.unsigned().transaction() && *o==options)
        {
            return Err(ValidationError::InvalidJupiterEstimate.into());
        }
        Ok(Self {
            prepared,
            options,
            fee,
            simulation,
        })
    }
    /// Returns the exact immutable preparation whose bytes were evaluated.
    #[must_use]
    pub const fn prepared(&self) -> &PreparedSwap {
        &self.prepared
    }
    /// Returns explicit commitment and lower bound, without an exact-state claim.
    #[must_use]
    pub const fn options(&self) -> ReadOptions {
        self.options
    }
    /// Returns source lamport fee availability with its actual evaluation slot.
    #[must_use]
    pub const fn fee(&self) -> &ExecutionObservation<MessageFee> {
        &self.fee
    }
    /// Returns actual source simulation facts, including execution failure.
    #[must_use]
    pub const fn simulation(&self) -> &ExecutionObservation<Simulation> {
        &self.simulation
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EstimateFields {
    prepared: PreparedSwap,
    options: ReadOptions,
    fee: ExecutionObservation<MessageFee>,
    simulation: ExecutionObservation<Simulation>,
}
impl TryFrom<EstimateFields> for SwapEstimate {
    type Error = Error;
    fn try_from(f: EstimateFields) -> Result<Self, Error> {
        Self::new(f.prepared, f.options, f.fee, f.simulation)
    }
}
impl From<SwapEstimate> for EstimateFields {
    fn from(s: SwapEstimate) -> Self {
        Self {
            prepared: s.prepared,
            options: s.options,
            fee: s.fee,
            simulation: s.simulation,
        }
    }
}
