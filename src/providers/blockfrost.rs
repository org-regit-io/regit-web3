// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Blockfrost indexed Cardano operations with explicit optional HTTP composition.
//!
//! The `blockfrost-http` feature provides verified-network current balances and
//! bounded unspent-output pages. Indexed state is not hash-selected. Wider asset,
//! transaction, epoch/staking and preparation operations remain required.

#[cfg(feature = "blockfrost-http")]
mod http;
#[cfg(feature = "blockfrost-http")]
mod wire;
#[cfg(feature = "blockfrost-http")]
pub use http::{BlockfrostClient, BlockfrostHttpConfig};
