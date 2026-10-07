// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{List, invalid};
use crate::{
    domain::{
        ExactDecimal,
        helius::{MetadataNode, MetadataValue, SourceText, bounded_fields},
    },
    error::Error,
};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

pub(super) struct Metadata(pub(super) MetadataValue);
impl<'de> Deserialize<'de> for Metadata {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = Box::<RawValue>::deserialize(d)?;
        parse(raw.get(), 0, &mut 0)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}
#[derive(Deserialize)]
struct Fields(#[serde(deserialize_with = "bounded_fields")] BTreeMap<String, Box<RawValue>>);
fn parse(raw: &str, depth: usize, nodes: &mut usize) -> Result<MetadataValue, Error> {
    *nodes += 1;
    if depth > MetadataValue::MAX_DEPTH || *nodes > MetadataValue::MAX_NODES {
        return Err(invalid());
    }
    let node = match raw.as_bytes().first() {
        Some(b'n') => {
            if raw != "null" {
                return Err(invalid());
            }
            MetadataNode::Null
        }
        Some(b't' | b'f') => {
            MetadataNode::Boolean(serde_json::from_str(raw).map_err(|_| invalid())?)
        }
        Some(b'"') => MetadataNode::Text(
            SourceText::new(serde_json::from_str::<String>(raw).map_err(|_| invalid())?)
                .map_err(|_| invalid())?,
        ),
        Some(b'[') => MetadataNode::Array(
            serde_json::from_str::<List<Box<RawValue>>>(raw)
                .map_err(|_| invalid())?
                .0
                .into_iter()
                .map(|v| parse(v.get(), depth + 1, nodes))
                .collect::<Result<_, _>>()?,
        ),
        Some(b'{') => MetadataNode::Object(
            serde_json::from_str::<Fields>(raw)
                .map_err(|_| invalid())?
                .0
                .into_iter()
                .map(|(k, v)| Ok((k, parse(v.get(), depth + 1, nodes)?)))
                .collect::<Result<_, Error>>()?,
        ),
        Some(b'-' | b'0'..=b'9') => {
            MetadataNode::Number(ExactDecimal::parse(raw).map_err(|_| invalid())?)
        }
        _ => return Err(invalid()),
    };
    MetadataValue::new(node).map_err(|_| invalid())
}
