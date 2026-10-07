// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit bindings between family-qualified asset keys and market listings.
//!
//! A binding records a caller-supplied relationship, not verified identity,
//! economic equivalence, decimal precision or price policy. Provider namespaces
//! and listing IDs remain exact and case-sensitive. No symbol matching, network
//! lookup or automatic resolution occurs.
//!
//! The generic asset key must retain the family's full technical identity and
//! have stable equality. Existing identity types, or tuples of existing network
//! and asset identities, can be used without flattening different networks.
//! Custom keys own their validation, immutable snapshot and deserialization
//! contracts. Display symbols, aliases and decimal metadata should not be keys.
//! For example, EVM native assets use [`super::Asset::identity`], ERC-20 keys
//! can use `(ChainId, Address)`, and Solana tokens use their genesis/mint/program
//! `TokenIdentity`. A metadata-bearing network wrapper may include a display
//! alias in equality; this table does not reinterpret that equality.

use std::{fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, DeserializeSeed, SeqAccess, Visitor},
};

use super::{Source, Timestamp, market::Identifier};

/// Fixed diagnostics for bounded explicit mapping construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum BindingError {
    /// The requested capacity is zero or exceeds the supported ceiling.
    InvalidCapacity,
    /// The mapping count exceeds its declared or absolute capacity.
    CapacityExceeded,
    /// An asset/provider pair repeats the same listing, even with new attribution.
    DuplicateMapping,
    /// An asset/provider pair selects two different listing IDs.
    ConflictingMapping,
}

impl fmt::Display for BindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCapacity => "invalid asset binding capacity",
            Self::CapacityExceeded => "asset binding capacity exceeded",
            Self::DuplicateMapping => "duplicate asset market mapping",
            Self::ConflictingMapping => "conflicting asset market mapping",
        })
    }
}

impl std::error::Error for BindingError {}

/// An exact listing ID scoped to an explicit provider namespace.
///
/// The provider identifier declares a namespace; it does not authenticate the
/// provider. Two equal listing IDs in different namespaces remain distinct.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketListing {
    provider: Identifier,
    listing_id: Identifier,
}

impl MarketListing {
    /// Records validated identifiers without network access or identity inference.
    #[must_use]
    pub const fn new(provider: Identifier, listing_id: Identifier) -> Self {
        Self {
            provider,
            listing_id,
        }
    }

    /// Returns the exact provider namespace, independently of source attribution.
    #[must_use]
    pub const fn provider(&self) -> &Identifier {
        &self.provider
    }

    /// Returns the provider-scoped listing identifier, never a display symbol.
    #[must_use]
    pub const fn listing_id(&self) -> &Identifier {
        &self.listing_id
    }
}

impl fmt::Debug for MarketListing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MarketListing")
            .finish_non_exhaustive()
    }
}

/// One caller-declared asset-to-listing relationship with explicit attribution.
///
/// `A` must be an immutable, fully qualified technical asset key. The recorded
/// source and timestamp describe the mapping assertion, not a balance, market
/// price, independently verified relationship or observation freshness.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetMarketBinding<A> {
    asset_key: A,
    listing: MarketListing,
    source: Source,
    recorded_at: Timestamp,
}

impl<A> AssetMarketBinding<A> {
    /// Records a relationship using caller-validated key, listing and attribution.
    #[must_use]
    pub const fn new(
        asset_key: A,
        listing: MarketListing,
        source: Source,
        recorded_at: Timestamp,
    ) -> Self {
        Self {
            asset_key,
            listing,
            source,
            recorded_at,
        }
    }

    /// Returns the full caller-supplied technical identity without mutable access.
    #[must_use]
    pub const fn asset_key(&self) -> &A {
        &self.asset_key
    }

    /// Returns the exact provider-scoped listing selected by the caller.
    #[must_use]
    pub const fn listing(&self) -> &MarketListing {
        &self.listing
    }

    /// Returns attribution for the mapping assertion, not a price observation.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Returns the explicitly supplied time of the mapping assertion.
    #[must_use]
    pub const fn recorded_at(&self) -> Timestamp {
        self.recorded_at
    }
}

impl<A> fmt::Debug for AssetMarketBinding<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AssetMarketBinding")
            .finish_non_exhaustive()
    }
}

/// A bounded immutable explicit mapping table with exact asset/provider lookup.
///
/// Each asset key has at most one listing per provider. Different asset keys may
/// explicitly select the same listing; that does not attest asset equivalence.
/// Lookup never consults another provider, falls back to a symbol, or fetches
/// prices. An empty table is valid and resolves no mappings.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct AssetMarketRegistry<A> {
    capacity: u16,
    bindings: Vec<AssetMarketBinding<A>>,
}

impl<A> AssetMarketRegistry<A> {
    /// Absolute entry ceiling, also enforced during sequence deserialization.
    pub const MAX_BINDINGS: u16 = 1024;

    /// Returns the explicitly configured maximum number of mappings.
    #[must_use]
    pub const fn capacity(&self) -> u16 {
        self.capacity
    }

    /// Returns the validated records in caller-supplied order.
    #[must_use]
    pub fn bindings(&self) -> &[AssetMarketBinding<A>] {
        &self.bindings
    }
}

impl<A: Eq> AssetMarketRegistry<A> {
    /// Validates capacity and one unambiguous mapping per asset/provider pair.
    ///
    /// # Errors
    /// Rejects capacity outside 1–1024, exceeded entry counts, duplicate mappings
    /// and conflicting listing IDs. Diagnostics contain no keys or identifiers.
    pub fn new(capacity: u16, bindings: Vec<AssetMarketBinding<A>>) -> Result<Self, BindingError> {
        if capacity == 0 || capacity > Self::MAX_BINDINGS {
            return Err(BindingError::InvalidCapacity);
        }
        if bindings.len() > usize::from(capacity) {
            return Err(BindingError::CapacityExceeded);
        }
        for (position, binding) in bindings.iter().enumerate() {
            if let Some(previous) = bindings[..position].iter().find(|previous| {
                previous.asset_key == binding.asset_key
                    && previous.listing.provider == binding.listing.provider
            }) {
                return Err(
                    if previous.listing.listing_id == binding.listing.listing_id {
                        BindingError::DuplicateMapping
                    } else {
                        BindingError::ConflictingMapping
                    },
                );
            }
        }
        Ok(Self { capacity, bindings })
    }

    /// Resolves only an exact asset key and provider namespace.
    ///
    /// Missing mappings return `None`, without fallback, synthesis or fetching.
    #[must_use]
    pub fn lookup(&self, asset_key: &A, provider: &Identifier) -> Option<&AssetMarketBinding<A>> {
        self.bindings.iter().find(|binding| {
            &binding.asset_key == asset_key && binding.listing.provider() == provider
        })
    }
}

impl<A> fmt::Debug for AssetMarketRegistry<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AssetMarketRegistry")
            .field("capacity", &self.capacity)
            .field("binding_count", &self.bindings.len())
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFields<A> {
    capacity: u16,
    #[serde(
        deserialize_with = "deserialize_bindings",
        bound(deserialize = "A: Deserialize<'de>")
    )]
    bindings: Vec<AssetMarketBinding<A>>,
}

impl<'de, A: Deserialize<'de> + Eq> Deserialize<'de> for AssetMarketRegistry<A> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = RegistryFields::<A>::deserialize(deserializer)?;
        Self::new(fields.capacity, fields.bindings).map_err(de::Error::custom)
    }
}

fn deserialize_bindings<'de, D, A>(deserializer: D) -> Result<Vec<AssetMarketBinding<A>>, D::Error>
where
    D: Deserializer<'de>,
    A: Deserialize<'de>,
{
    struct BindingsVisitor<A>(PhantomData<fn() -> A>);

    impl<'de, A: Deserialize<'de>> Visitor<'de> for BindingsVisitor<A> {
        type Value = Vec<AssetMarketBinding<A>>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded asset binding sequence")
        }

        fn visit_seq<S: SeqAccess<'de>>(self, mut sequence: S) -> Result<Self::Value, S::Error> {
            let maximum = usize::from(AssetMarketRegistry::<A>::MAX_BINDINGS);
            if sequence.size_hint().is_some_and(|size| size > maximum) {
                return Err(de::Error::custom(BindingError::CapacityExceeded));
            }
            let mut bindings = Vec::new();
            for _ in 0..maximum {
                match sequence.next_element()? {
                    Some(binding) => bindings.push(binding),
                    None => return Ok(bindings),
                }
            }
            // The seed errors before decoding or allocating an overflow item.
            if sequence
                .next_element_seed(RejectAdditionalBinding)?
                .is_some()
            {
                return Err(de::Error::custom(BindingError::CapacityExceeded));
            }
            Ok(bindings)
        }
    }

    deserializer.deserialize_seq(BindingsVisitor(PhantomData))
}

struct RejectAdditionalBinding;

impl<'de> DeserializeSeed<'de> for RejectAdditionalBinding {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, _deserializer: D) -> Result<(), D::Error> {
        Err(de::Error::custom(BindingError::CapacityExceeded))
    }
}
