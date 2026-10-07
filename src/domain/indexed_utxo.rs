// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Shared indexed UTXO facts; family identities and units remain explicit.

mod identity;
mod observation;
mod records;

pub use identity::{AddressPolicy, BlockHash, Bytes, NetworkPolicy, SourceText, Txid};
pub(crate) use identity::{AtomicAmount, MempoolDelta, NetworkId};
pub(crate) use observation::{Context, Observation, Operation};
pub(crate) use records::{
    AddressBalance, BalanceData, FeeEstimates, HistoryPage, Transaction, TransactionData,
    TransactionInput, TransactionOutput, TransactionReference,
};
pub use records::{
    CoinbaseSource, HistoryRequest, OutPoint, ReferenceDirection, ReferenceInclusion,
    TransactionStatus,
};

use crate::error::{Error, ValidationError};

pub(crate) mod sealed {
    pub trait Network {}
    pub trait Address {}
}

pub(crate) fn invalid() -> Error {
    ValidationError::InvalidUtxoRecord.into()
}

pub(crate) const MAXIMUM_ITEMS: usize = 100_000;

pub(crate) fn bounded_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    bounded_vec_limit::<D, T, MAXIMUM_ITEMS>(deserializer)
}
fn bounded_vec_limit<'de, D, T, const LIMIT: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    struct Visitor<T, const LIMIT: usize>(std::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>, const LIMIT: usize> serde::de::Visitor<'de>
        for Visitor<T, LIMIT>
    {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bounded indexed record array")
        }
        fn visit_seq<S: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: S,
        ) -> Result<Self::Value, S::Error> {
            let mut items = Vec::new();
            while let Some(item) = sequence.next_element()? {
                if items.len() == LIMIT {
                    return Err(serde::de::Error::custom("indexed record limit exceeded"));
                }
                items.push(item);
            }
            Ok(items)
        }
    }
    deserializer.deserialize_seq(Visitor::<T, LIMIT>(std::marker::PhantomData))
}
fn bounded_optional_limit<'de, D, T, const LIMIT: usize>(
    deserializer: D,
) -> Result<Option<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    struct Bounded<T, const LIMIT: usize>(Vec<T>);
    impl<'de, T: serde::Deserialize<'de>, const LIMIT: usize> serde::Deserialize<'de>
        for Bounded<T, LIMIT>
    {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            bounded_vec_limit::<D, T, LIMIT>(d).map(Self)
        }
    }
    <Option<Bounded<T, LIMIT>> as serde::Deserialize>::deserialize(deserializer)
        .map(|v| v.map(|v| v.0))
}
#[cfg(any(feature = "litecoin-http", feature = "dogecoin-http"))]
pub(crate) fn bounded_optional_vec<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    bounded_optional_limit::<D, T, MAXIMUM_ITEMS>(deserializer)
}
pub(crate) fn bounded_optional_addresses<'de, D, T>(
    deserializer: D,
) -> Result<Option<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    bounded_optional_limit::<D, T, 100>(deserializer)
}
pub(crate) fn bounded_optional_witness<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<Bytes>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bounded_optional_limit::<D, Bytes, 10_000>(deserializer)
}
