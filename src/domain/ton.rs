// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! TON addresses, exact nanotons, account history and reviewed message preparation.
//!
//! Standard addresses contain a signed workchain and account bytes, not a network.
//! Network qualification uses the complete expected masterchain zero-state identity.
//! Account logical-time linkage and provider block attribution are distinct from
//! independently checked consensus inclusion. Wallets supply signing and approval.

mod boc;
mod execution;
mod identity;
mod observation;
mod preparation;
mod records;
mod transaction;

pub use boc::Boc;
pub use execution::{
    FeeEstimate, FeePart, FeeRequest, MessageStatus, SignedSubmission, SubmissionResult,
};
pub use identity::{
    Address, AddressFormat, FriendlyFlags, Hash, Network, NetworkCategory, ZeroState,
};
pub use observation::{Context, Observation, ObservationValue, Operation};
pub use preparation::{TransferIntent, TransferPreparation};
pub use records::*;
pub use transaction::{
    Action, Compute, ComputeSkip, Execution, Message, MessageInfo, ProviderFees, SourceMessageFees,
    Transaction, TransactionStatus,
};

use crate::error::{Error, ValidationError};
fn invalid_address() -> Error {
    ValidationError::InvalidTonAddress.into()
}
fn invalid_hash() -> Error {
    ValidationError::InvalidTonHash.into()
}
fn invalid_network() -> Error {
    ValidationError::InvalidTonNetwork.into()
}
fn invalid_boc() -> Error {
    ValidationError::InvalidTonBoc.into()
}
fn invalid_record() -> Error {
    ValidationError::InvalidTonRecord.into()
}
fn invalid_transfer() -> Error {
    ValidationError::InvalidTonTransfer.into()
}
