// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure Cardano identities, indexed balances and unspent-output observations.
//!
//! Address encoding, network discrimination and asset identity are family-specific.
//! Construction records caller-supplied facts without a clock or network access.
//! Indexed observations do not claim evaluation at an independently captured tip.

mod identity;
mod observation;
mod value;

pub use identity::{
    AssetId, AssetKind, AssetName, Hash, Network, NetworkId, PaymentAddress, PolicyId, ScriptHash,
    StakeAddress,
};
pub use observation::{Context, Observation, Operation};
pub use value::{
    AddressBalance, AssetAmount, HexData, Order, OutputData, PageRequest, PageStatus, Utxo,
    UtxoPage,
};
