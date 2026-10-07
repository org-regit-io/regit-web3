// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Blockfrost indexed Cardano operations with explicit optional HTTP composition.
//!
//! The `blockfrost-http` backend provides balances, bounded output/address/asset/
//! reward pages, named asset metadata, epoch/parameters/staking/network data,
//! original transaction retrieval and separate indexed status. Ordinary Conway
//! payments retain explicit input/output/fee/validity policy for pure preparation
//! and fresh-parameter fee estimates. Externally signed payloads can be submitted
//! once as exact CBOR; acknowledgement does not imply execution or finality.
//! Every operation verifies expected genesis magic under one total deadline.
//! Indexed state is dynamic and is not hash-selected. The caller supplies the
//! explicit endpoint and project credential; no ambient discovery occurs.

#[cfg(feature = "blockfrost-http")]
mod http;
#[cfg(feature = "blockfrost-http")]
mod wire;
#[cfg(feature = "blockfrost-http")]
pub use http::{BlockfrostClient, BlockfrostHttpConfig};
