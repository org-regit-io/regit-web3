// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact values, source attribution, and family-specific Web3 identities.
//!
//! All metadata is supplied explicitly. These types do not read configuration,
//! consult a clock, or contact a network. [`Amount`] retains nonnegative integer
//! base units; [`ExactDecimal`] retains signed decimal values without floating
//! point. Both serialize their exact numeric values as strings.
//!
//! Existing address, chain identity, asset, and native-balance types are EVM
//! specific. Their original exports remain available, together with explicitly
//! qualified exports in [`evm`]. EVM addresses and hashes serialize as lowercase,
//! `0x`-prefixed hexadecimal strings; EVM chain identifiers serialize as decimal
//! strings.

mod amount;
#[cfg(feature = "bitcoin")]
pub mod bitcoin;
#[cfg(feature = "cardano")]
pub mod cardano;
mod decimal;
pub mod evm;
mod identity;
mod observation;
#[cfg(feature = "solana")]
pub mod solana;

pub use amount::Amount;
pub use decimal::ExactDecimal;
pub use identity::{Address, Asset, AssetId, AssetKind, ChainId, MetadataOrigin, NetworkId};
pub use observation::{
    Balance, BlockContext, BlockHash, BlockSelector, Finality, Observation, ObservationContext,
    Operation, Source, Timestamp,
};
/// The maintained 256-bit unsigned integer used for exact EVM values.
pub use ruint::aliases::U256;

use serde::{Deserialize, Deserializer};

fn deserialize_optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

fn valid_label(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+'))
}
