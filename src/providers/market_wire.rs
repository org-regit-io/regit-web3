// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Private lexical numeric decoding and duplicate-preserving bounded collections.
use crate::{
    domain::{
        ExactDecimal,
        market::{ItemLimit, NonnegativeDecimal},
    },
    error::{Error, ProviderError},
};
use serde::{
    Deserialize, Deserializer,
    de::{DeserializeOwned, MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, fmt, marker::PhantomData};

pub(super) fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
pub(super) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
pub(super) fn check_limit(count: usize, limit: ItemLimit) -> Result<(), Error> {
    if count > limit.get() as usize {
        Err(invalid())
    } else {
        Ok(())
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
impl Number {
    pub(super) fn nonnegative(self) -> Result<NonnegativeDecimal, Error> {
        NonnegativeDecimal::new(self.0).map_err(|_| invalid())
    }
}
pub(super) fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

pub(super) struct List<T>(pub(super) Vec<T>);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for List<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ListVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ListVisitor<T> {
            type Value = List<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded provider array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = a.next_element()? {
                    if values.len() == ItemLimit::MAXIMUM as usize {
                        return Err(serde::de::Error::custom("provider array exceeds bound"));
                    }
                    values.push(value);
                }
                Ok(List(values))
            }
        }
        d.deserialize_seq(ListVisitor(PhantomData))
    }
}
pub(super) struct Map<T>(pub(super) BTreeMap<String, T>);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Map<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct MapVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for MapVisitor<T> {
            type Value = Map<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded provider object with unique keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut values = BTreeMap::new();
                while let Some(key) = a.next_key::<String>()? {
                    if key.len() > 512
                        || values.len() == ItemLimit::MAXIMUM as usize
                        || values.contains_key(&key)
                    {
                        return Err(serde::de::Error::custom(
                            "provider object violates key bound or uniqueness",
                        ));
                    }
                    values.insert(key, a.next_value()?);
                }
                Ok(Map(values))
            }
        }
        d.deserialize_map(MapVisitor(PhantomData))
    }
}
