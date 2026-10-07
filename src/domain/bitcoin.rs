// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bitcoin identities, exact satoshi values and attributed index observations.
//!
//! Network genesis identity is separate from display aliases. Address encoding
//! can establish compatibility with a network without distinguishing networks
//! that share the same prefixes. Indexed reads carry retrieval context rather
//! than an invented historical state anchor.

mod identity;
mod observation;
mod records;

#[cfg(feature = "bitcoin-esplora")]
pub(crate) use records::canonical_target;

pub use identity::{Address, BlockHash, Network, NetworkId, Txid};
pub use observation::{Context, Observation, Operation};
pub use records::{
    AddressBalance, BlockReference, Completeness, FeeEstimates, HistoryCursor, HistoryEntry,
    HistoryPage, MempoolDelta, Satoshis, TransactionStatus,
};
