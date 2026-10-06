// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Reusable Web3 primitives for Rust.
//!
//! The [`domain`] module provides exact amounts, validated EVM identities, and
//! native-balance observations with explicit block and source context.
//! Constructors and deserialization enforce the same domain invariants. The
//! [`error`] module provides typed failures with fixed diagnostics.
//! With the `evm` feature, explicit configuration and client establishment verify
//! a provider's chain identity through a bounded read-only request. Balance
//! operations and other integrations remain unimplemented.

pub mod chains;
#[cfg(feature = "evm")]
pub mod config;
pub mod domain;
pub mod error;
pub mod protocols;
pub mod providers;
pub mod wallets;
