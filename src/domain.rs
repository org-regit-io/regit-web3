// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact EVM values and attributable native-balance observations.
//!
//! All metadata is supplied explicitly. These types do not read configuration,
//! consult a clock, or contact a network. Addresses and hashes serialize as
//! lowercase, `0x`-prefixed hexadecimal strings; amounts and chain identifiers
//! serialize as decimal strings.

mod amount;
mod identity;
mod observation;

pub use amount::Amount;
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
