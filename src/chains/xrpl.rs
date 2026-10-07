// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! XRP Ledger capabilities with exact balances and optional outgoing HTTP operations.
//!
//! Ledger validation remains source-reported. Unsigned Payment fields are local;
//! callers own signing, fee selection and approval policy.

use std::future::Future;

use crate::{
    domain::xrpl::{
        AccountBalance, Address, FeeEstimate, Hash, HistoryPage, HistoryRequest, Observation,
        PageRequest, SignedSubmission, SubmissionResult, Transaction, TransactionStatus,
        TrustLinePage,
    },
    error::Error,
};

#[cfg(feature = "xrpl-http")]
mod http;
#[cfg(feature = "xrpl-http")]
mod wire;
#[cfg(feature = "xrpl-http")]
pub use http::{XrplClient, XrplHttpConfig};

/// Generic statically dispatched XRP Ledger read capabilities.
///
/// Futures are `Send` on native targets and host-local on JavaScript WebAssembly.
/// Readers need not themselves be `Send` or `Sync`.
/// Implementations retain exact XRP/issued units and actual ledger attribution.
pub trait XrplReader {
    /// Reads XRP drops and account sequence at an optional exact validated ledger.
    fn get_account_balance(
        &self,
        account: Address,
        ledger_hash: Option<Hash>,
    ) -> impl Future<Output = Result<Observation<AccountBalance>, Error>> + crate::future::MaybeSend;
    /// Reads one bounded issued-balance/trustline page with ledger-bound pagination.
    fn get_trust_lines(
        &self,
        account: Address,
        request: PageRequest,
    ) -> impl Future<Output = Result<Observation<TrustLinePage>, Error>> + crate::future::MaybeSend;
    /// Reads actual open-ledger fee suggestions without choosing a fee policy.
    fn get_fee_estimate(
        &self,
    ) -> impl Future<Output = Result<Observation<FeeEstimate>, Error>> + crate::future::MaybeSend;
    /// Retrieves hash-checked opaque transaction bytes and reported inclusion.
    fn get_transaction(
        &self,
        hash: Hash,
    ) -> impl Future<Output = Result<Observation<Transaction>, Error>> + crate::future::MaybeSend;
    /// Retrieves execution code separately from source-reported validation.
    fn get_transaction_status(
        &self,
        hash: Hash,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + crate::future::MaybeSend;
    /// Reads one bounded account-history page over an explicit search range.
    fn get_account_history(
        &self,
        account: Address,
        request: HistoryRequest,
    ) -> impl Future<Output = Result<Observation<HistoryPage>, Error>> + crate::future::MaybeSend;
}

/// Explicit statically dispatched submission of caller-supplied signed bytes.
///
/// Separate from preparation and reads. Implementations do not sign, fill fields
/// or automatically retry submissions. Preliminary acceptance is not validated
/// execution. Dropping or externally timing out the future after dispatch cannot
/// establish that the transaction was not submitted.
pub trait XrplSubmitter {
    /// Sends one explicit payload and retains unknown outcomes after dispatch.
    fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> impl Future<Output = Result<Observation<SubmissionResult>, Error>> + crate::future::MaybeSend;
}
