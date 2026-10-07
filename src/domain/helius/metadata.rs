// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{domain::ExactDecimal, error::Error};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

/// Bounded opaque provider text, preserving UTF-8, controls and explicit empty text.
/// Text is not rendered as a diagnostic, fetched as a URL, or trusted as an instruction.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceText(String);
impl SourceText {
    /// Maximum retained bytes per source text value.
    pub const MAX_BYTES: usize = 16_384;
    /// Preserves an explicitly supplied provider string within the resource bound.
    /// # Errors
    /// Rejects excessive text without echoing it.
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.len() > Self::MAX_BYTES {
            return Err(super::bounded::invalid_transaction());
        }
        Ok(Self(value))
    }
    /// Returns source text without sanitizing or interpreting it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SourceText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceText")
            .field("bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl TryFrom<String> for SourceText {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<SourceText> for String {
    fn from(v: SourceText) -> Self {
        v.0
    }
}

/// Structured IDL/source metadata, without an implied program-specific interpretation.
/// Numeric values are exact decimals; strings, including integer strings, stay strings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum MetadataNode {
    /// Explicit source null.
    Null,
    /// Source boolean.
    Boolean(bool),
    /// Exact lexical numeric value, without float conversion or implied base units.
    Number(ExactDecimal),
    /// Uninterpreted source string.
    Text(SourceText),
    /// Ordered bounded metadata values.
    Array(#[serde(deserialize_with = "super::bounded::list")] Vec<MetadataValue>),
    /// Unique named fields; names retain their source spelling.
    Object(#[serde(deserialize_with = "super::bounded::map")] BTreeMap<String, MetadataValue>),
}
/// Validated bounded metadata tree; this is unverified parser interpretation.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MetadataNode", into = "MetadataNode")]
pub struct MetadataValue(MetadataNode);
impl MetadataValue {
    /// Maximum nested collection depth.
    pub const MAX_DEPTH: usize = 16;
    /// Maximum nodes in one retained metadata tree.
    pub const MAX_NODES: usize = 4096;
    /// Validates depth, nodes, keys and cumulative text resource bounds.
    /// # Errors
    /// Rejects excessive metadata with a fixed diagnostic.
    pub fn new(node: MetadataNode) -> Result<Self, Error> {
        fn check(
            n: &MetadataNode,
            depth: usize,
            nodes: &mut usize,
            bytes: &mut usize,
        ) -> Result<(), Error> {
            *nodes += 1;
            if depth > MetadataValue::MAX_DEPTH || *nodes > MetadataValue::MAX_NODES {
                return Err(super::bounded::invalid_transaction());
            }
            match n {
                MetadataNode::Text(t) => *bytes += t.as_str().len(),
                MetadataNode::Number(n) => *bytes += n.canonical().len(),
                MetadataNode::Array(a) => {
                    for v in a {
                        check(&v.0, depth + 1, nodes, bytes)?;
                    }
                }
                MetadataNode::Object(m) => {
                    for (k, v) in m {
                        if k.len() > 256 {
                            return Err(super::bounded::invalid_transaction());
                        }
                        *bytes += k.len();
                        check(&v.0, depth + 1, nodes, bytes)?;
                    }
                }
                MetadataNode::Null | MetadataNode::Boolean(_) => {}
            }
            if *bytes > 1_048_576 {
                return Err(super::bounded::invalid_transaction());
            }
            Ok(())
        }
        check(&node, 0, &mut 0, &mut 0)?;
        Ok(Self(node))
    }
    /// Returns structured source values without asserting their IDL semantics.
    #[must_use]
    pub const fn node(&self) -> &MetadataNode {
        &self.0
    }
}
impl fmt::Debug for MetadataValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MetadataValue { .. }")
    }
}
impl TryFrom<MetadataNode> for MetadataValue {
    type Error = Error;
    fn try_from(n: MetadataNode) -> Result<Self, Error> {
        Self::new(n)
    }
}
impl From<MetadataValue> for MetadataNode {
    fn from(v: MetadataValue) -> Self {
        v.0
    }
}
