// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! `THORChain` identities, exact protocol values and attributed cross-chain records.
//!
//! Cosmos chain IDs and account prefixes are separate caller-qualified facts.
//! Protocol amounts use 1e8 units; external dust/gas quantities retain their
//! actual units. Source observations do not independently establish finality.

mod identity;
mod observation;
mod records;

pub use identity::{
    AccountPrefix, Address, Asset, AssetKind, Chain, ChainAddress, CollectionLimit, Network,
    ProtocolAmount, RawQuantity, Text, TransactionMemo, Txid, TxidKind,
};
pub use observation::{Context, Observation, Operation};
pub use records::*;
#[cfg(feature = "thorchain-http")]
pub(crate) use records::{deserialize_optional_vec, deserialize_vec};

use crate::error::{Error, ValidationError};

fn invalid_record() -> Error {
    ValidationError::InvalidThorchainRecord.into()
}
