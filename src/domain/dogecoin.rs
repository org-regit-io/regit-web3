// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Dogecoin family identities, exact native quantities and checked indexed observations.
//!
//! Indexed inclusion and confirmation counts remain source facts. Opaque raw
//! payloads are not independently decoded or hashed and do not establish consensus,
//! signature, `AuxPoW` or historical state proof.

mod identity;

use super::indexed_utxo as common;
#[doc(hidden)]
pub use common::{AddressPolicy, NetworkPolicy};
pub use common::{
    BlockHash, Bytes, CoinbaseSource, HistoryRequest, OutPoint, ReferenceDirection,
    ReferenceInclusion, SourceText, TransactionStatus, Txid,
};
pub use identity::{Address, AddressKind, Network};

/// Complete standard genesis identity and caller display alias.
pub type NetworkId = common::NetworkId<Network>;
/// Exact indexed balance qualified by a Dogecoin address.
pub type AddressBalance = common::AddressBalance<Address>;
/// Exact confirmed and signed unconfirmed Dogecoin balance facts.
pub type BalanceData = common::BalanceData<Address>;
/// Exact unsigned Dogecoin native atomic units; eight digits form one coin.
pub type Koinu = common::AtomicAmount<Address>;
/// Exact signed Dogecoin native atomic-unit unconfirmed delta.
pub type MempoolDelta = common::MempoolDelta<Address>;
/// Source fee buckets in Dogecoin atomic units per 1000 bytes.
pub type FeeEstimates = common::FeeEstimates<Address>;
/// Bounded Dogecoin address input/output-reference history.
pub type HistoryPage = common::HistoryPage<Address>;
/// One exact indexed Dogecoin input or output reference.
pub type TransactionReference = common::TransactionReference<Address>;
/// Typed complete source Dogecoin input facts.
pub type TransactionInput = common::TransactionInput<Address>;
/// Typed complete source Dogecoin output facts.
pub type TransactionOutput = common::TransactionOutput<Address>;
/// Complete typed Dogecoin indexed transaction fields.
pub type TransactionData = common::TransactionData<Address>;
/// Checked complete Dogecoin indexed transaction, without raw consensus decoding.
pub type Transaction = common::Transaction<Address>;
/// Explicit Dogecoin source, network, query and retrieval context.
pub type Context = common::Context<Address>;
/// The exact Dogecoin indexed operation and caller inputs.
pub type Operation = common::Operation<Address>;
/// A checked typed Dogecoin indexed value and its matching source/query context.
pub type Observation<T> = common::Observation<T, Address>;
