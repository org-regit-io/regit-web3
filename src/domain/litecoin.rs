// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Litecoin family identities, exact native quantities and checked indexed observations.
//!
//! Indexed inclusion and confirmation counts remain source facts. Opaque raw
//! payloads are not independently decoded or hashed and do not establish consensus,
//! signature, `MWEB` or historical state proof.

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
/// Exact indexed balance qualified by a Litecoin address.
pub type AddressBalance = common::AddressBalance<Address>;
/// Exact confirmed and signed unconfirmed Litecoin balance facts.
pub type BalanceData = common::BalanceData<Address>;
/// Exact unsigned Litecoin native atomic units; eight digits form one coin.
pub type Litoshis = common::AtomicAmount<Address>;
/// Exact signed Litecoin native atomic-unit unconfirmed delta.
pub type MempoolDelta = common::MempoolDelta<Address>;
/// Source fee buckets in Litecoin atomic units per 1000 bytes.
pub type FeeEstimates = common::FeeEstimates<Address>;
/// Bounded Litecoin address input/output-reference history.
pub type HistoryPage = common::HistoryPage<Address>;
/// One exact indexed Litecoin input or output reference.
pub type TransactionReference = common::TransactionReference<Address>;
/// Typed complete source Litecoin input facts.
pub type TransactionInput = common::TransactionInput<Address>;
/// Typed complete source Litecoin output facts.
pub type TransactionOutput = common::TransactionOutput<Address>;
/// Complete typed Litecoin indexed transaction fields.
pub type TransactionData = common::TransactionData<Address>;
/// Checked complete Litecoin indexed transaction, without raw consensus decoding.
pub type Transaction = common::Transaction<Address>;
/// Explicit Litecoin source, network, query and retrieval context.
pub type Context = common::Context<Address>;
/// The exact Litecoin indexed operation and caller inputs.
pub type Operation = common::Operation<Address>;
/// A checked typed Litecoin indexed value and its matching source/query context.
pub type Observation<T> = common::Observation<T, Address>;
