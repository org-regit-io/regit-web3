// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bounded, ordered collections of independently contextualized observations.
//!
//! Keys and observations remain caller-selected types. A collection neither
//! reads data nor establishes a shared snapshot, remote completeness, or a
//! relationship between a key and its observation. Family observation types
//! retain their own validation and source, network, and evaluation contexts.

use std::{fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};

use crate::error::Error;

/// A fixed, input-free collection validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CollectionError {
    /// The item limit is zero or exceeds [`CollectionLimit::MAX_ITEMS`].
    InvalidLimit,
    /// More items were supplied than the explicitly configured limit permits.
    TooManyItems,
    /// At least two supplied keys compare equal.
    DuplicateKey,
}

impl fmt::Display for CollectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "invalid observation collection limit",
            Self::TooManyItems => "observation collection item limit exceeded",
            Self::DuplicateKey => "duplicate observation collection key",
        })
    }
}

impl std::error::Error for CollectionError {}

/// An explicit maximum item count between one and 1,024 inclusive.
///
/// Serialization uses an integer. Deserialization applies the constructor's
/// same bound; there is no implicit default or unlimited mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "usize", into = "usize")]
pub struct CollectionLimit(usize);

impl CollectionLimit {
    /// The absolute item-count ceiling for construction and deserialization.
    pub const MAX_ITEMS: usize = 1_024;

    /// Constructs an explicitly bounded item limit.
    ///
    /// # Errors
    /// Returns [`CollectionError::InvalidLimit`] for zero or a value above
    /// [`Self::MAX_ITEMS`].
    pub const fn new(maximum: usize) -> Result<Self, CollectionError> {
        if maximum == 0 || maximum > Self::MAX_ITEMS {
            return Err(CollectionError::InvalidLimit);
        }
        Ok(Self(maximum))
    }

    /// Returns the explicitly configured maximum item count.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl TryFrom<usize> for CollectionLimit {
    type Error = CollectionError;

    fn try_from(maximum: usize) -> Result<Self, Self::Error> {
        Self::new(maximum)
    }
}

impl From<CollectionLimit> for usize {
    fn from(limit: CollectionLimit) -> Self {
        limit.get()
    }
}

/// One exact caller-selected key and either a typed observation or failure.
///
/// A failed item retains its key and the existing fixed [`Error`] category.
/// Successes retain the observation unchanged. The caller establishes whether
/// a key identifies its observation; no key normalization or context is added.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationItem<K, O> {
    key: K,
    outcome: Result<O, Error>,
}

impl<K, O> ObservationItem<K, O> {
    /// Records an exact key and its explicit success or failure.
    #[must_use]
    pub const fn new(key: K, outcome: Result<O, Error>) -> Self {
        Self { key, outcome }
    }

    /// Returns the unchanged caller-selected key.
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the typed observation or fixed item failure.
    pub const fn outcome(&self) -> &Result<O, Error> {
        &self.outcome
    }

    /// Returns the successful observation, if this item succeeded.
    #[must_use]
    pub fn observation(&self) -> Option<&O> {
        self.outcome.as_ref().ok()
    }

    /// Returns the typed failure, if this item failed.
    #[must_use]
    pub fn failure(&self) -> Option<Error> {
        self.outcome.as_ref().err().copied()
    }

    /// Consumes the item and returns its exact key and outcome.
    pub fn into_parts(self) -> (K, Result<O, Error>) {
        (self.key, self.outcome)
    }
}

/// An explicitly bounded collection retaining caller order and unique keys.
///
/// Equality of keys uses only `K: Eq`, permitting existing family identities
/// without imposing hashing, sorting, or normalization. Construction checks
/// duplicates in bounded quadratic time; lookup is linear in at most 1,024
/// items. Supplied observation values and contexts remain unchanged, including
/// different networks, sources, retrieval times, and evaluation anchors.
///
/// Deserialization buffers at most [`CollectionLimit::MAX_ITEMS`] items, never
/// allocates from an untrusted sequence size hint, and rejects an extra item
/// before decoding its key or outcome. When `limit` precedes `items`, decoding
/// uses the configured lower bound immediately. Otherwise the absolute ceiling
/// applies while decoding and the configured limit is checked afterward.
/// Memory within each key or observation remains governed by its own type and
/// deserializer; this container bounds item count, not input bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ObservationCollection<K, O> {
    limit: CollectionLimit,
    items: Vec<ObservationItem<K, O>>,
}

impl<K: Eq, O> ObservationCollection<K, O> {
    /// Validates a supplied ordered collection without changing keys or values.
    ///
    /// Empty collections are permitted. Every supplied key must be unique,
    /// including keys of failed items. Construction does not fetch missing
    /// items or prove that all requested or remote records were supplied.
    ///
    /// # Errors
    /// Returns [`CollectionError::TooManyItems`] when the item count exceeds
    /// `limit`, or [`CollectionError::DuplicateKey`] for equal supplied keys.
    pub fn new(
        limit: CollectionLimit,
        items: Vec<ObservationItem<K, O>>,
    ) -> Result<Self, CollectionError> {
        if items.len() > limit.get() {
            return Err(CollectionError::TooManyItems);
        }
        for (index, item) in items.iter().enumerate() {
            if items[..index]
                .iter()
                .any(|previous| previous.key == item.key)
            {
                return Err(CollectionError::DuplicateKey);
            }
        }
        Ok(Self { limit, items })
    }

    /// Finds a supplied item by exact key equality in bounded linear time.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&ObservationItem<K, O>> {
        self.items.iter().find(|item| item.key() == key)
    }
}

impl<K, O> ObservationCollection<K, O> {
    /// Returns the explicitly supplied item-count limit.
    #[must_use]
    pub const fn limit(&self) -> CollectionLimit {
        self.limit
    }

    /// Returns all supplied items in their original order.
    #[must_use]
    pub fn items(&self) -> &[ObservationItem<K, O>] {
        &self.items
    }

    /// Returns the number of supplied successful and failed items together.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns whether no items were supplied.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Counts successful outcomes among supplied items only.
    #[must_use]
    pub fn success_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.outcome.is_ok())
            .count()
    }

    /// Counts explicit failures among supplied items only.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.len() - self.success_count()
    }

    /// Returns whether every supplied item succeeded, including an empty set.
    ///
    /// This describes supplied outcomes only. It does not prove that every
    /// requested item was included, remote coverage, or a shared snapshot.
    #[must_use]
    pub fn all_succeeded(&self) -> bool {
        self.items.iter().all(|item| item.outcome.is_ok())
    }

    /// Consumes the collection and returns its limit and ordered items.
    #[must_use]
    pub fn into_parts(self) -> (CollectionLimit, Vec<ObservationItem<K, O>>) {
        (self.limit, self.items)
    }
}

impl<'de, K, O> Deserialize<'de> for ObservationCollection<K, O>
where
    K: Deserialize<'de> + Eq,
    O: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "ObservationCollection",
            &["limit", "items"],
            CollectionVisitor(PhantomData),
        )
    }
}

enum Field {
    Limit,
    Items,
}

impl<'de> Deserialize<'de> for Field {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FieldVisitor;

        impl Visitor<'_> for FieldVisitor {
            type Value = Field;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an observation collection field")
            }

            fn visit_str<E: de::Error>(self, field: &str) -> Result<Field, E> {
                match field {
                    "limit" => Ok(Field::Limit),
                    "items" => Ok(Field::Items),
                    _ => Err(E::custom("unknown observation collection field")),
                }
            }

            fn visit_bytes<E: de::Error>(self, field: &[u8]) -> Result<Field, E> {
                match field {
                    b"limit" => Ok(Field::Limit),
                    b"items" => Ok(Field::Items),
                    _ => Err(E::custom("unknown observation collection field")),
                }
            }

            fn visit_u64<E: de::Error>(self, field: u64) -> Result<Field, E> {
                match field {
                    0 => Ok(Field::Limit),
                    1 => Ok(Field::Items),
                    _ => Err(E::custom("unknown observation collection field")),
                }
            }
        }

        deserializer.deserialize_identifier(FieldVisitor)
    }
}

struct CollectionVisitor<K, O>(PhantomData<(K, O)>);

impl<'de, K, O> Visitor<'de> for CollectionVisitor<K, O>
where
    K: Deserialize<'de> + Eq,
    O: Deserialize<'de>,
{
    type Value = ObservationCollection<K, O>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an observation collection with an explicit limit and items")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut limit: Option<CollectionLimit> = None;
        let mut items = None;
        while let Some(field) = map.next_key()? {
            match field {
                Field::Limit => {
                    if limit.is_some() {
                        return Err(de::Error::duplicate_field("limit"));
                    }
                    limit = Some(map.next_value()?);
                }
                Field::Items => {
                    if items.is_some() {
                        return Err(de::Error::duplicate_field("items"));
                    }
                    items = Some(map.next_value_seed(ItemsSeed {
                        maximum: limit.map_or(CollectionLimit::MAX_ITEMS, CollectionLimit::get),
                        marker: PhantomData,
                    })?);
                }
            }
        }
        let limit = limit.ok_or_else(|| de::Error::missing_field("limit"))?;
        let items = items.ok_or_else(|| de::Error::missing_field("items"))?;
        ObservationCollection::new(limit, items).map_err(de::Error::custom)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let limit: CollectionLimit = sequence
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let items = sequence
            .next_element_seed(ItemsSeed {
                maximum: limit.get(),
                marker: PhantomData,
            })?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        sequence.next_element_seed(RejectValue("unexpected observation collection field"))?;
        ObservationCollection::new(limit, items).map_err(de::Error::custom)
    }
}

struct ItemsSeed<K, O> {
    maximum: usize,
    marker: PhantomData<(K, O)>,
}

impl<'de, K: Deserialize<'de>, O: Deserialize<'de>> DeserializeSeed<'de> for ItemsSeed<K, O> {
    type Value = Vec<ObservationItem<K, O>>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, K: Deserialize<'de>, O: Deserialize<'de>> Visitor<'de> for ItemsSeed<K, O> {
    type Value = Vec<ObservationItem<K, O>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an ordered, bounded observation item sequence")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while items.len() < self.maximum {
            match sequence.next_element()? {
                Some(item) => items.push(item),
                None => return Ok(items),
            }
        }
        sequence.next_element_seed(RejectValue("observation collection item limit exceeded"))?;
        Ok(items)
    }
}

struct RejectValue(&'static str);

impl<'de> DeserializeSeed<'de> for RejectValue {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, _deserializer: D) -> Result<Self::Value, D::Error> {
        Err(de::Error::custom(self.0))
    }
}
