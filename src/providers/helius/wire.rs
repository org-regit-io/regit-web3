// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

mod asset;
mod metadata;
mod parsed;
use crate::{
    domain::{ExactDecimal, helius::bounded_entries},
    error::{Error, ProviderError},
};
pub(super) use asset::{AssetWire, OwnerWire, asset_params, owner_params};
pub(super) use parsed::{HistoryWire, ResultWire, history_params, parse_params};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

pub(super) fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
pub(super) fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
pub(super) struct List<T>(pub(super) Vec<T>);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for List<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_entries(d).map(Self)
    }
}
pub(super) struct Number(pub(super) ExactDecimal);
impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = Box::<RawValue>::deserialize(d)?;
        ExactDecimal::parse(raw.get())
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}
pub(super) fn rpc_error(code: i64) -> Error {
    if code == -32004 {
        Error::UnavailableData
    } else {
        Error::Provider(ProviderError::Rpc)
    }
}

// Current migration documentation permits raw token units as JSON integer or
// integer string. Display decimals, exponents and noncanonical integer strings
// are not converted into base units.
pub(super) fn raw_units<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let raw = Box::<RawValue>::deserialize(d)?;
    let text = if raw.get().starts_with('"') {
        serde_json::from_str::<String>(raw.get()).map_err(serde::de::Error::custom)?
    } else {
        raw.get().to_owned()
    };
    if text.is_empty()
        || text.len() > 20
        || !text.bytes().all(|b| b.is_ascii_digit())
        || text.len() > 1 && text.starts_with('0')
    {
        return Err(serde::de::Error::custom("invalid raw token units"));
    }
    text.parse()
        .map_err(|_| serde::de::Error::custom("invalid raw token units"))
}
