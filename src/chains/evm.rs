// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! EVM capabilities over typed inputs and observations.
//!
//! [`NativeBalanceReader`] supports generic, static dispatch to a
//! caller-provided implementation. Its future is `Send` and uses the standard
//! library's [`Future`]; this capability feature requires no HTTP client or
//! runtime. The optional `evm-http` feature provides the bounded HTTP client
//! implementation.
//!
//! ```no_run
//! use std::future::Future;
//! use regit_web3::{
//!     chains::evm::NativeBalanceReader,
//!     domain::{Address, Balance, BlockSelector, Observation},
//!     error::Error,
//! };
//!
//! fn read<R: NativeBalanceReader>(
//!     reader: &R,
//!     address: Address,
//! ) -> impl Future<Output = Result<Observation<Balance>, Error>> + Send {
//!     reader.get_native_balance(address, Some(BlockSelector::Safe))
//! }
//! ```

use std::future::Future;

use crate::{
    domain::{Address, Balance, BlockSelector, Observation},
    error::Error,
};

use crate::domain::evm::{
    Erc20Allowance, Erc20Balance, Erc20Metadata, OperationObservation, ReceiptLookup,
    TransactionId, TransactionLookup, TransactionStatus,
};

#[cfg(feature = "evm-http")]
mod http;
#[cfg(feature = "evm-http")]
mod wire;

#[cfg(feature = "evm-http")]
pub use http::EvmClient;

/// A caller-implemented capability for exact EVM native-balance reads.
///
/// Generic callers select their reader implementation through this trait.
/// The implementation owns network verification, state resolution, request
/// execution, and source attribution. The returned observation must identify
/// the requested address and resolved state, retaining the requested selector,
/// exact native precision, retrieval timestamp, and honest finality metadata.
/// Domain constructors validate the record's internal consistency.
pub trait NativeBalanceReader {
    /// Reads a native balance using an explicit or implementation-configured selector.
    ///
    /// `None` selects the reader's explicitly configured default. The returned
    /// future may borrow the reader and can be sent between threads. Each
    /// implementation documents its execution and runtime requirements.
    ///
    /// # Errors
    ///
    /// Returns typed validation, configuration, timeout, provider, capability,
    /// or unavailable-data failures according to the implementation's contract.
    fn get_native_balance(
        &self,
        address: Address,
        selector: Option<BlockSelector>,
    ) -> impl Future<Output = Result<Observation<Balance>, Error>> + Send;
}

/// Independent pure capability for exact ERC-20 reads at one canonical state.
///
/// Metadata methods are optional in ERC-20. Each returned metadata field retains
/// a precise outcome; implementations never invent token precision or display
/// identity. Balance/allowance amounts are exact raw units with no assumed decimals.
pub trait Erc20Reader {
    /// Reads `balanceOf(owner)` at an explicit or configured selector.
    ///
    /// # Errors
    /// Reports unavailable state, malformed ABI, source revert and backend failures.
    fn get_erc20_balance(
        &self,
        contract: Address,
        owner: Address,
        selector: Option<BlockSelector>,
    ) -> impl Future<Output = Result<OperationObservation<Erc20Balance>, Error>> + Send;
    /// Reads `allowance(owner, spender)` at an explicit or configured selector.
    ///
    /// # Errors
    /// Reports unavailable state, malformed ABI, source revert and backend failures.
    fn get_erc20_allowance(
        &self,
        contract: Address,
        owner: Address,
        spender: Address,
        selector: Option<BlockSelector>,
    ) -> impl Future<Output = Result<OperationObservation<Erc20Allowance>, Error>> + Send;
    /// Reads independent optional name/symbol/decimals outcomes at one state.
    ///
    /// # Errors
    /// Reports network, state resolution, protocol, transport and unknown RPC failures.
    fn get_erc20_metadata(
        &self,
        contract: Address,
        selector: Option<BlockSelector>,
    ) -> impl Future<Output = Result<OperationObservation<Erc20Metadata>, Error>> + Send;
}

/// Independent pure capability for transaction, receipt and observed status reads.
///
/// Null lookup results retain absence. Inclusion and top-level execution remain
/// separate source facts; implementations do not infer lasting finality.
pub trait TransactionReader {
    /// Retrieves typed source fields for an exact transaction identifier.
    ///
    /// # Errors
    /// Reports malformed or mismatched identity/network/inclusion and backend failures.
    fn get_transaction(
        &self,
        hash: TransactionId,
    ) -> impl Future<Output = Result<OperationObservation<TransactionLookup>, Error>> + Send;
    /// Retrieves source receipt fields with actual execution uncertainty.
    ///
    /// # Errors
    /// Reports malformed or mismatched identity/inclusion/logs and backend failures.
    fn get_receipt(
        &self,
        hash: TransactionId,
    ) -> impl Future<Output = Result<OperationObservation<ReceiptLookup>, Error>> + Send;
    /// Matches sequential transaction/receipt reads into an observed lifecycle.
    ///
    /// # Errors
    /// Rejects contradictory identities/inclusion/execution records and backend failures.
    fn get_transaction_status(
        &self,
        hash: TransactionId,
    ) -> impl Future<Output = Result<OperationObservation<TransactionStatus>, Error>> + Send;
}
