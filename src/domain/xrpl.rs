// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact XRP Ledger identities, ledger observations and unsigned payments.
//!
//! Classic addresses identify accounts without identifying a network. X-addresses
//! carry a main/test category and optional destination tag, not a test-network ID.
//! Issued balances are signed decimal values; they have no universal token scale.

mod identity;
mod observation;
mod payment;
mod transaction;
mod value;

pub use identity::{Address, Currency, Hash, Network, NetworkId, XAddress, XAddressCategory};
pub use observation::{Context, Ledger, Observation, Operation};
pub use payment::{Destination, PaymentAmount, PaymentRequest, PreparedPayment};
pub use transaction::{
    HexData, HistoryMarker, HistoryPage, HistoryRequest, LedgerRange, ResultCode, Transaction,
    TransactionStatus,
};
pub use value::{
    AccountBalance, Drops, FeeEstimate, IssuedValue, LineFlags, Marker, PageRequest, Setting,
    TrustLine, TrustLinePage,
};
