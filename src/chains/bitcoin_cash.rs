// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bitcoin Cash read capabilities with an optional certificate-verified TLS Electrum backend.
//!
//! Readers retain full genesis/fork identity, exact BCH and `CashToken` values,
//! explicit history ranges and source status semantics. These operations do not
//! establish consensus, signatures, chainwork, funding or finality.

use crate::{
    domain::bitcoin_cash::{
        Address, AddressBalance, CollectionLimit, FeeEstimate, FeeTarget, History, HistoryRange,
        Observation, TokenFilter, Transaction, TransactionStatus, Txid, UnspentOutputs,
    },
    error::Error,
};
use std::future::Future;

#[cfg(feature = "bitcoin-cash-electrum")]
mod electrum;
#[cfg(feature = "bitcoin-cash-electrum")]
mod wire;
#[cfg(feature = "bitcoin-cash-electrum")]
pub use electrum::{ElectrumClient, ElectrumConfig, ElectrumEndpoint, TlsTrustRoots};

/// Replaceable, runtime-independent capability for exact Bitcoin Cash source reads.
/// Implementations must retain the actual expected network, caller query and source facts.
pub trait BitcoinCashReader {
    /// Reads exact native confirmed balance and signed unconfirmed delta for an explicit filter.
    /// # Errors
    /// Reports incompatible namespace, unavailable/malformed source or bounded transport failure.
    fn get_address_balance(
        &self,
        address: Address,
        token_filter: TokenFilter,
    ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + Send;
    /// Reads complete history for one explicit inclusive-lower/exclusive-upper height interval.
    /// # Errors
    /// Rejects incomplete/over-capacity or contradictory source records rather than truncating.
    fn get_address_history(
        &self,
        address: Address,
        range: HistoryRange,
    ) -> impl Future<Output = Result<Observation<History>, Error>> + Send;
    /// Reads an exact source estimate in BCH per 1,000 bytes for the caller target.
    /// # Errors
    /// Reports unsupported, malformed or bounded transport failure; unavailable estimates remain typed.
    fn get_fee_estimate(
        &self,
        target: FeeTarget,
    ) -> impl Future<Output = Result<Observation<FeeEstimate>, Error>> + Send;
    /// Reads actual source transaction height/header facts without invented mempool or finality proof.
    /// # Errors
    /// Rejects contradictory source heights/headers and malformed or bounded transport failures.
    fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + Send;
    /// Retrieves complete bounded verbose source fields and separately hashed opaque raw bytes.
    /// # Errors
    /// Rejects wrong identities, incomplete/excessive fields and source transport failures.
    fn get_transaction(
        &self,
        txid: Txid,
        limit: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<Transaction>, Error>> + Send;
    /// Reads complete source UTXOs and supplied `CashToken` metadata under an explicit filter/capacity.
    /// # Errors
    /// Rejects duplicates, contradictory filters, malformed/excessive records and transport failure.
    fn get_unspent_outputs(
        &self,
        address: Address,
        token_filter: TokenFilter,
        limit: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<UnspentOutputs>, Error>> + Send;
}
