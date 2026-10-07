// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure Cardano identities, exact indexed records and explicit ordinary payment review.
//!
//! Address encoding, network discrimination and asset identity are family-specific.
//! Construction records caller-supplied facts without a clock or network access.
//! Indexed observations do not claim evaluation at an independently captured tip.
//! Original transaction CBOR is bounded and hashed without body re-encoding.
//! Conway key-spend preparation retains selected inputs, all outputs, fees, absolute
//! slot validity and witness-count policy. Cryptographic signing is external.

mod identity;
mod indexed;
mod observation;
mod preparation;
mod transaction;
mod value;

pub use identity::{
    AssetId, AssetKind, AssetName, Hash, Network, NetworkId, PaymentAddress, PolicyId, ScriptHash,
    StakeAddress,
};
pub use indexed::*;
pub use observation::{Context, Observation, ObservationValue, Operation};
pub use preparation::*;
pub use transaction::*;
pub use value::{
    AddressBalance, AssetAmount, HexData, Order, OutputData, PageRequest, PageStatus, Utxo,
    UtxoPage,
};
