// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, SeqAccess, Visitor},
};
use std::{fmt, marker::PhantomData};

pub(crate) fn list<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    struct Entries<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Entries<T> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a bounded Helius collection")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Vec<T>, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = a.next_element()? {
                if values.len() == 4096 {
                    return Err(serde::de::Error::custom("Helius collection exceeds bound"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Entries(PhantomData))
}

pub(super) fn optional_list<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Vec<T>>, D::Error> {
    struct Bounded<T>(Vec<T>);
    impl<'de, T: Deserialize<'de>> Deserialize<'de> for Bounded<T> {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            list(d).map(Self)
        }
    }
    Option::<Bounded<T>>::deserialize(d).map(|v| v.map(|v| v.0))
}

pub(crate) fn map<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<std::collections::BTreeMap<String, T>, D::Error> {
    struct Fields<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Fields<T> {
        type Value = std::collections::BTreeMap<String, T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("unique bounded Helius metadata fields")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
            let mut fields = std::collections::BTreeMap::new();
            while let Some(key) = a.next_key::<String>()? {
                if key.len() > 256 || fields.len() == 4096 || fields.contains_key(&key) {
                    return Err(serde::de::Error::custom("invalid Helius metadata fields"));
                }
                fields.insert(key, a.next_value()?);
            }
            Ok(fields)
        }
    }
    d.deserialize_map(Fields(PhantomData))
}

pub(super) fn invalid_request() -> crate::error::Error {
    crate::error::ValidationError::InvalidHeliusRequest.into()
}
pub(super) fn invalid_asset() -> crate::error::Error {
    crate::error::ValidationError::InvalidHeliusAsset.into()
}
pub(super) fn invalid_transaction() -> crate::error::Error {
    crate::error::ValidationError::InvalidHeliusTransaction.into()
}
