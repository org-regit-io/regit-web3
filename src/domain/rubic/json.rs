// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::invalid_quote as invalid;
use crate::error::Error;
#[cfg(feature = "rubic-http")]
use crate::error::ProviderError;
use serde::de::{MapAccess, SeqAccess, Visitor};
#[cfg(feature = "rubic-http")]
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
#[cfg(feature = "rubic-http")]
use std::marker::PhantomData;
use std::{collections::BTreeSet, fmt};
#[cfg(feature = "rubic-http")]
pub(crate) const MAX_DOCUMENT: usize = 2 * 1024 * 1024;
#[cfg(feature = "rubic-http")]
pub(crate) fn document(bytes: &[u8]) -> Result<Box<RawValue>, Error> {
    if bytes.len() > MAX_DOCUMENT {
        return Err(Error::Provider(ProviderError::ResponseTooLarge));
    }
    let raw: Box<RawValue> = serde_json::from_slice(bytes)
        .map_err(|_| Error::Provider(ProviderError::InvalidResponse))?;
    unique(&raw, 0, &mut 0, 100_000)
        .map_err(|_| Error::Provider(ProviderError::InvalidResponse))?;
    Ok(raw)
}
pub(super) fn additional_object(value: &str) -> Result<(), Error> {
    if value.len() > 65_536 {
        return Err(invalid());
    }
    let raw: Box<RawValue> = serde_json::from_str(value).map_err(|_| invalid())?;
    if !raw.get().trim_start().starts_with('{') {
        return Err(invalid());
    }
    unique(&raw, 0, &mut 0, 8192)
}
fn unique(raw: &RawValue, depth: u8, nodes: &mut usize, limit: usize) -> Result<(), Error> {
    *nodes += 1;
    if depth > 32 || *nodes > limit {
        return Err(invalid());
    }
    let input = raw.get().trim_start();
    let mut d = serde_json::Deserializer::from_str(input);
    if input.starts_with('{') {
        serde::Deserializer::deserialize_map(
            &mut d,
            Object {
                depth,
                nodes,
                limit,
            },
        )
        .map_err(|_| invalid())?;
    } else if input.starts_with('[') {
        serde::Deserializer::deserialize_seq(
            &mut d,
            Array {
                depth,
                nodes,
                limit,
            },
        )
        .map_err(|_| invalid())?;
    }
    Ok(())
}
struct Object<'a> {
    depth: u8,
    nodes: &'a mut usize,
    limit: usize,
}
impl<'de> Visitor<'de> for Object<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded unique source object")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<(), M::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = m.next_key::<String>()? {
            if keys.len() >= 4096 || key.len() > 4096 || !keys.insert(key) {
                return Err(serde::de::Error::custom("invalid source object"));
            }
            let raw = m.next_value::<Box<RawValue>>()?;
            unique(&raw, self.depth + 1, self.nodes, self.limit)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}
struct Array<'a> {
    depth: u8,
    nodes: &'a mut usize,
    limit: usize,
}
impl<'de> Visitor<'de> for Array<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded source array")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut s: S) -> Result<(), S::Error> {
        let mut count = 0;
        while let Some(raw) = s.next_element::<Box<RawValue>>()? {
            count += 1;
            if count > 4096 {
                return Err(serde::de::Error::custom("invalid source array"));
            }
            unique(&raw, self.depth + 1, self.nodes, self.limit)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}
#[cfg(feature = "rubic-http")]
pub(crate) struct Bounded<T, const N: usize>(pub(crate) Vec<T>);
#[cfg(feature = "rubic-http")]
impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for Bounded<T, N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Items<T, const N: usize>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for Items<T, N> {
            type Value = Bounded<T, N>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded source collection")
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut s: S) -> Result<Self::Value, S::Error> {
                let mut items = Vec::new();
                while let Some(item) = s.next_element::<T>()? {
                    if items.len() >= N {
                        return Err(serde::de::Error::custom("excess source items"));
                    }
                    items.push(item);
                }
                Ok(Bounded(items))
            }
        }
        d.deserialize_seq(Items::<T, N>(PhantomData))
    }
}
