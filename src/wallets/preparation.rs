// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::Error;

/// An immutable family-specific preparation with complete value equality.
///
/// Implementations retain the actual family network, intent and unsigned payload
/// types instead of erasing their semantics into generic JSON or bytes. Cloning
/// must preserve the snapshot, and equality must include every binding-relevant
/// fact. Validation must enforce consistency and resource bounds without signing,
/// submitting, loading secrets or inventing caller approval policy.
///
/// This is a trusted extension contract. Generic Rust code cannot police custom
/// interior mutability, incomplete equality or a dishonest validation method.
pub trait Preparation: Clone + Eq {
    /// The family-specific network identity, including its actual identity rules.
    type Network: Eq;
    /// Exact reviewable intent, including relevant amounts, fees and constraints.
    type Intent: Eq;
    /// Family-specific unsigned representation, which need not be signing bytes.
    type UnsignedPayload: Eq;

    /// Returns the immutable network bound to this preparation.
    fn network(&self) -> &Self::Network;
    /// Returns the complete immutable intent to review.
    fn intent(&self) -> &Self::Intent;
    /// Returns the exact immutable family-specific unsigned payload.
    fn unsigned_payload(&self) -> &Self::UnsignedPayload;
    /// Validates consistency and bounds of the supplied snapshot.
    ///
    /// # Errors
    /// Returns a fixed typed failure when family-specific binding is invalid.
    fn validate(&self) -> Result<(), Error>;
}

/// A validated owned preparation, with no signing or approval side effect.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct PreparedRequest<P: Preparation> {
    preparation: P,
}
impl<P: Preparation> PreparedRequest<P> {
    /// Validates an immutable family snapshot before accepting it for review.
    ///
    /// # Errors
    /// Propagates the preparation's typed validation failure.
    pub fn new(preparation: P) -> Result<Self, Error> {
        preparation.validate()?;
        Ok(Self { preparation })
    }
    /// Returns the exact preparation without granting mutable access.
    #[must_use]
    pub const fn preparation(&self) -> &P {
        &self.preparation
    }
    /// Returns a read-only view of the exact network, intent and unsigned payload.
    ///
    /// Creating this view records no approval decision or policy.
    #[must_use]
    pub const fn review(&self) -> Review<'_, P> {
        Review { request: self }
    }
    pub(super) fn matches(&self, other: &Self) -> bool {
        self.preparation == other.preparation
            && self.preparation.network() == other.preparation.network()
            && self.preparation.intent() == other.preparation.intent()
            && self.preparation.unsigned_payload() == other.preparation.unsigned_payload()
    }
}
impl<P: Preparation> fmt::Debug for PreparedRequest<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedRequest").finish_non_exhaustive()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields<P> {
    preparation: P,
}
impl<'de, P: Preparation + Deserialize<'de>> Deserialize<'de> for PreparedRequest<P> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let fields = Fields::<P>::deserialize(d)?;
        Self::new(fields.preparation).map_err(serde::de::Error::custom)
    }
}

/// A read-only view for caller-owned review, without an approval marker.
pub struct Review<'a, P: Preparation> {
    request: &'a PreparedRequest<P>,
}
impl<P: Preparation> Review<'_, P> {
    /// Returns the exact family network of the prepared snapshot.
    #[must_use]
    pub fn network(&self) -> &P::Network {
        self.request.preparation.network()
    }
    /// Returns the complete intent of the prepared snapshot.
    #[must_use]
    pub fn intent(&self) -> &P::Intent {
        self.request.preparation.intent()
    }
    /// Returns the unsigned representation without inferring signing readiness.
    #[must_use]
    pub fn unsigned_payload(&self) -> &P::UnsignedPayload {
        self.request.preparation.unsigned_payload()
    }
}
impl<P: Preparation> fmt::Debug for Review<'_, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Review").finish_non_exhaustive()
    }
}
