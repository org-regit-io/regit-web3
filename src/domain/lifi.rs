// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact cross-family LI.FI request, estimate, preparation and progress records.
//!
//! Chain numbers belong to LI.FI's catalogue. They are not genesis proofs.
//! The caller qualifies every chain family explicitly; unknown numbers are never
//! interpreted as EVM. Provider payloads are source suggestions for review, not
//! independently verified transfer intent or signed execution.

mod identity;
mod payload;
mod records;
mod status;

pub use identity::{Account, Asset, Chain, ChainCatalogue, Family, Identifier, Slippage, Text};
pub use payload::{
    AccessEntry, EncodedPayload, EvmPayload, EvmPayloadData, Payload, PayloadEncoding,
};
pub use records::{
    Action, ActionData, Estimate, EstimateData, Expiry, Fee, FeeData, GasCost, GasCostData, Limits,
    Origin, PreparedStep, Request, RequestData, Route, RouteData, Routes, Step, StepData, StepKind,
    StepView, Token, TokenData, ToolFailure, UnavailableRoutes,
};
pub use status::{
    ObservedLeg, Progress, ReceivingRole, Status, StatusData, StatusQuery, StatusSelector,
    Transaction, TransactionData, TransactionId, TransferId,
};

use crate::error::{Error, ValidationError};
fn invalid_identity() -> Error {
    ValidationError::InvalidLifiIdentity.into()
}
fn invalid_record() -> Error {
    ValidationError::InvalidLifiRecord.into()
}
fn invalid_payload() -> Error {
    ValidationError::InvalidLifiPayload.into()
}
