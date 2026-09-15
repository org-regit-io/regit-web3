// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Reusable Web3 primitives for Rust consumers.
//!
//! This foundation reserves typed chain operations, exact amounts, and
//! observations with block and source context. Integrations are module shells;
//! no network operation or wallet backend is implemented yet.

pub mod chains;
pub mod config;
pub mod domain;
pub mod error;
pub mod protocols;
pub mod providers;
pub mod wallets;
