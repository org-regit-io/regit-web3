// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Typed Bitcoin indexed reads with an optional bounded Esplora implementation.
//!
//! Capabilities use the caller's backend and executor. Indexed address state
//! and history are not hash-pinned snapshots; source and retrieval facts remain
//! explicit. No transaction signing or submission capability is provided.

use std::future::Future;

use crate::{
    domain::bitcoin::{
        Address, AddressBalance, FeeEstimates, HistoryCursor, HistoryPage, Observation,
        TransactionStatus, Txid,
    },
    error::Error,
};

#[cfg(feature = "bitcoin-esplora")]
mod esplora;
#[cfg(feature = "bitcoin-esplora")]
mod wire;
#[cfg(feature = "bitcoin-esplora")]
pub use esplora::{EsploraClient, EsploraConfig};

/// Read-only Bitcoin index capabilities implemented by a caller-selected backend.
///
/// Implementations must validate query identity and retain exact units, source
/// and retrieval context. Pagination limits and source-reported inclusion must
/// remain explicit; no current index query establishes lasting finality or a
/// historical state pin. Returned futures are `Send`; the reader itself need
/// not be `Send` or `Sync`. This trait supports static, generic dispatch.
pub trait BitcoinReader {
    /// Reads confirmed funds and the separate signed mempool delta.
    fn get_address_balance(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + Send;
    /// Reads one bounded history chunk with its exact requested cursor.
    fn get_address_history(
        &self,
        address: Address,
        cursor: HistoryCursor,
    ) -> impl Future<Output = Result<Observation<HistoryPage>, Error>> + Send;
    /// Reads exact satoshi-per-vbyte estimates by positive block horizon.
    fn get_fee_estimates(
        &self,
    ) -> impl Future<Output = Result<Observation<FeeEstimates>, Error>> + Send;
    /// Reads source-reported inclusion for a transaction; absence is an error.
    fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + Send;
}
