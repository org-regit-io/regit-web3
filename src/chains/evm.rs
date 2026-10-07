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

#[cfg(feature = "evm-http")]
mod http;
#[cfg(feature = "evm-http")]
mod rpc;

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
