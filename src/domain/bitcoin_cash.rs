// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bitcoin Cash identities, exact values and bounded source-read contracts.
//!
//! `CashAddr` namespace compatibility is separate from a chain's complete genesis
//! and fork-checkpoint identity. Source facts do not establish consensus,
//! signatures, chainwork or finality. No network, environment or clock is read.

mod identity;
mod observation;
mod records;
mod transaction;

#[cfg(all(
    feature = "bitcoin-cash-electrum",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
pub(crate) use records::{bounded_addresses, bounded_entries};

pub use identity::{
    Address, AddressKind, AddressNamespace, BlockHash, BlockHeader, ForkCheckpoint,
    NetworkIdentity, ScriptHash, TokenCategory, Txid,
};
pub use observation::{Context, Observation, Operation, ProtocolMetadata, ReadValue};
pub use records::{
    AddressBalance, CollectionLimit, FeeEstimate, FeeTarget, History, HistoryEntry, HistoryRange,
    HistoryState, HistoryUpperBound, Nft, NftCapability, Satoshis, SignedSatoshis, TokenAmount,
    TokenData, TokenFilter, UnspentOutput, UnspentOutputs,
};
pub use transaction::{
    Bytes, RawTransaction, SourceHeight, SourceInclusion, SourceText, Transaction, TransactionData,
    TransactionInput, TransactionOutput, TransactionStatus,
};

use crate::error::{Error, ValidationError};

fn invalid() -> Error {
    ValidationError::InvalidBitcoinCashRecord.into()
}
