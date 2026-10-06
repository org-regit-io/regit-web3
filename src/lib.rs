// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Reusable Web3 primitives for Rust.
//!
//! The [`domain`] module provides exact amounts, validated EVM identities, and
//! native-balance observations with explicit block and source context.
//! Constructors and deserialization enforce the same domain invariants. The
//! [`error`] module provides typed failures with fixed diagnostics.
//! Integrations are module shells; no network operation or wallet backend is
//! implemented yet.

pub mod chains;
pub mod config;
pub mod domain;
pub mod error;
pub mod protocols;
pub mod providers;
pub mod wallets;
