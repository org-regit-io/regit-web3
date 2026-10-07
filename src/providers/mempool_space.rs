// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! mempool.space Bitcoin mempool, fee and transaction data.
//!
//! Source-attributed bounded reads with an optional explicit HTTP(S) backend.
//! Pure capabilities select no HTTP, runtime or credentials. Full transaction
//! bytes and inclusion reuse Bitcoin's distinct canonical/indexed contracts.

use std::future::Future;

use crate::{
    domain::{
        bitcoin::{Observation as BitcoinObservation, Transaction, TransactionStatus, Txid},
        mempool_space::{
            MempoolSummary, Observation, RecentTransactions, RecommendedFees, TransactionIds,
            TransactionLimit,
        },
    },
    error::Error,
};

#[cfg(feature = "mempool-space-http")]
mod http;
#[cfg(feature = "mempool-space-http")]
mod wire;
#[cfg(feature = "mempool-space-http")]
pub use http::{MempoolSpaceClient, MempoolSpaceHttpConfig};

/// Caller-selected read-only Bitcoin mempool/fees/transaction capabilities.
///
/// Implementations retain exact units, source retrieval and full expected network
/// identity. Collections fail wholly at limits; no atomic cross-request snapshot
/// or continuing membership/finality is implied. Returned futures are `Send`.
pub trait MempoolSpaceReader {
    /// Reads backlog totals and individual source fee-distribution bins.
    fn get_mempool_summary(
        &self,
    ) -> impl Future<Output = Result<Observation<MempoolSummary>, Error>> + Send;
    /// Reads at most ten recent source arrivals, never an exhaustive listing.
    fn get_recent_transactions(
        &self,
    ) -> impl Future<Output = Result<Observation<RecentTransactions>, Error>> + Send;
    /// Reads every ID from the unpaged endpoint or fails wholly at the explicit cap.
    fn get_mempool_txids(
        &self,
        limit: TransactionLimit,
    ) -> impl Future<Output = Result<Observation<TransactionIds>, Error>> + Send;
    /// Reads five independent exact sat/vB suggestions with their named classes.
    fn get_recommended_fees(
        &self,
    ) -> impl Future<Output = Result<Observation<RecommendedFees>, Error>> + Send;
    /// Reads canonical raw transaction bytes and separately supplied index facts.
    fn get_transaction(
        &self,
        txid: Txid,
    ) -> impl Future<Output = Result<BitcoinObservation<Transaction>, Error>> + Send;
    /// Reads source inclusion; unavailable does not mean unconfirmed or failed.
    fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> impl Future<Output = Result<BitcoinObservation<TransactionStatus>, Error>> + Send;
}
