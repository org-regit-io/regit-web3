// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicitly qualified EVM identities, reads and source observations.
//!
//! These are the same types as the original [`super`] exports. Qualifying them
//! here changes neither their constructors nor their serialized representation.
//! Amount, source, and timestamp primitives are shared and re-exported for
//! convenience; addresses, network identities, assets, and block semantics here
//! follow their documented EVM contracts.

pub use super::{
    Address, Amount, Asset, AssetId, AssetKind, Balance, BlockContext, BlockHash, BlockSelector,
    ChainId, Finality, MetadataOrigin, NetworkId, Observation, ObservationContext, Operation,
    Source, Timestamp, U256,
};

mod bytes;
mod context;
mod token;
mod transaction;

pub use bytes::{Data, Quantity, TransactionId, Word};
pub use context::{
    OperationContext, OperationObservation, OperationValue, ReadOperation, ReadState,
};
pub use token::{
    Erc20Allowance, Erc20Balance, Erc20Metadata, MetadataText, MetadataUnavailable, MetadataValue,
};
pub use transaction::{
    AccessListEntry, Authorization, ExecutionOutcome, Inclusion, Log, Receipt, ReceiptData,
    ReceiptLookup, SignatureFields, Transaction, TransactionData, TransactionKind,
    TransactionLookup, TransactionState, TransactionStatus,
};
