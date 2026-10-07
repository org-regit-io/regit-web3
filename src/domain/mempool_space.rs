// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact Bitcoin mempool statistics, bounded collections and source observations.
//!
//! Mempool membership and fee suggestions are source-reported moving facts.
//! Separate requests are not an atomic snapshot, a confirmation promise or
//! lasting finality. Transaction payloads reuse the Bitcoin family contracts.

mod observation;
mod records;

pub use observation::{Context, Observation, Operation};
pub use records::{
    FeeHistogramBin, FeeRate, MempoolSummary, RecentTransaction, RecentTransactions,
    RecommendedFees, TransactionIds, TransactionLimit, VirtualSize,
};
