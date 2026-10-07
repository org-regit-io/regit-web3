// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! TON address/network validation, balances, history, network data and transfers.
//!
//! Pure exact capabilities and optional outgoing TON Center API-v2 backend.
//! History linkage, actual source blocks, message acknowledgement and execution
//! are separate facts. Wallets own signing and approval; preparation never submits.

use crate::{
    domain::ton::{
        AccountBalance, Address, Cursor, FeeEstimate, FeeRequest, Hash, HistoryPage,
        HistoryRequest, MessageStatus, NetworkData, Observation, SignedSubmission,
        SubmissionResult, Transaction, TransactionStatus,
    },
    error::Error,
};
use std::future::Future;
#[cfg(feature = "ton-http")]
mod http;
#[cfg(feature = "ton-http")]
mod wire;
#[cfg(feature = "ton-http")]
pub use http::{TonClient, TonHttpConfig};

/// Runtime-independent TON read/estimate capability with replaceable backends.
/// Futures are `Send`; implementations do not discover runtimes or credentials.
pub trait TonReader {
    /// Retrieves full current masterchain and expected zero-state identity.
    fn get_network_data(
        &self,
    ) -> impl Future<Output = Result<Observation<NetworkData>, Error>> + Send;
    /// Reads account state against a selected full masterchain block.
    fn get_account_balance(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<Observation<AccountBalance>, Error>> + Send;
    /// Reads one bounded linked account-history page without hidden enumeration.
    fn get_account_history(
        &self,
        request: HistoryRequest,
    ) -> impl Future<Output = Result<Observation<HistoryPage>, Error>> + Send;
    /// Retrieves exact account-qualified logical-time/hash transaction identity.
    fn get_transaction(
        &self,
        address: Address,
        cursor: Cursor,
    ) -> impl Future<Output = Result<Observation<Transaction>, Error>> + Send;
    /// Retrieves actual supported execution facts without invented block inclusion.
    fn get_transaction_status(
        &self,
        address: Address,
        cursor: Cursor,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + Send;
    /// Scans one explicit bounded page for the incoming representation hash.
    fn get_message_status(
        &self,
        message_hash: Hash,
        request: HistoryRequest,
    ) -> impl Future<Output = Result<Observation<MessageStatus>, Error>> + Send;
    /// Estimates exact fee components for explicit caller-supplied wallet-body bytes.
    fn estimate_fee(
        &self,
        request: FeeRequest,
    ) -> impl Future<Output = Result<Observation<FeeEstimate>, Error>> + Send;
}
/// Controlled one-shot external-message submission, independent of signing.
/// Acknowledgement is not validated execution. After dispatch, unresolved errors
/// retain possible submission; dropping the future cannot prove no dispatch.
pub trait TonSubmitter {
    /// Dispatches one explicit ready-for-submission message; never retries writes.
    fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> impl Future<Output = Result<Observation<SubmissionResult>, Error>> + Send;
}
