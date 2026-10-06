// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Reusable Web3 primitives for Rust.
//!
//! The [`domain`] module provides exact amounts, validated EVM identities, and
//! native-balance observations with explicit block and source context.
//! Constructors and deserialization enforce the same domain invariants. The
//! [`error`] module provides typed failures with fixed diagnostics.
//! With the `evm` feature, explicit configuration and client establishment verify
//! a provider's chain identity. Native-balance reads resolve a block and request
//! exact state at its canonical hash, with bounded requests and source context.
//! Other operations and integrations remain unimplemented.
//!
//! Read a native balance through an established EVM client:
//!
//! ```no_run
//! # #[cfg(feature = "evm")]
//! # fn main() {
//! use regit_web3::{
//!     chains::evm::EvmClient,
//!     domain::{Address, Balance, BlockSelector, Observation},
//!     error::Error,
//! };
//!
//! async fn read_balance(
//!     client: &EvmClient,
//!     address: Address,
//! ) -> Result<Observation<Balance>, Error> {
//!     client.get_native_balance(address, Some(BlockSelector::Safe)).await
//! }
//! # }
//! # #[cfg(not(feature = "evm"))]
//! # fn main() {}
//! ```

pub mod chains;
#[cfg(feature = "evm")]
pub mod config;
pub mod domain;
pub mod error;
pub mod protocols;
pub mod providers;
pub mod wallets;
