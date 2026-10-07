// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bitcoin identities, exact satoshi values, canonical transactions and index observations.
//!
//! Network genesis identity is separate from display aliases. Address encoding
//! can establish compatibility with a network without distinguishing networks
//! that share the same prefixes. Indexed reads carry retrieval context rather
//! than an invented historical state anchor.
//! Canonical transaction bytes remain network-independent; indexed previous
//! outputs, fees and inclusion are separately supplied facts.

mod identity;
mod observation;
mod records;
mod transaction;

#[cfg(feature = "bitcoin-esplora")]
pub(crate) use records::canonical_target;
#[cfg(feature = "bitcoin-esplora")]
pub(crate) use transaction::{MAX_INPUTS, MAX_OUTPUTS, deserialize_bounded_vec};

pub use identity::{Address, BlockHash, Network, NetworkId, Txid, Wtxid};
pub use observation::{Context, Observation, Operation};
pub use records::{
    AddressBalance, BlockReference, Completeness, FeeEstimates, HistoryCursor, HistoryEntry,
    HistoryPage, MempoolDelta, Satoshis, TransactionStatus,
};
pub use transaction::{
    Bytes, OutPoint, PreviousOutput, Transaction, TransactionBody, TransactionInput,
    TransactionOutput,
};
