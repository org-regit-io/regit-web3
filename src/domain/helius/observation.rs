// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    Asset, AssetRequest, HistoryPage, HistoryRequest, OwnerPage, OwnerRequest, ParseRequest,
    ParsedBatch,
};
use crate::{
    domain::{Source, Timestamp, solana::Network},
    error::Error,
};
use serde::{Deserialize, Serialize};

/// Immutable method-specific Helius query, without a historical snapshot claim.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    content = "request",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Request {
    /// Exact DAS asset query.
    Asset(AssetRequest),
    /// Explicit DAS owner-assets page query.
    OwnerAssets(OwnerRequest),
    /// Ordered Parsed Events signature batch.
    ParseTransactions(ParseRequest),
    /// Explicit Parsed Events address-history page.
    AddressHistory(HistoryRequest),
}
/// Attribution and actual provider index progress, with no fabricated evaluation slot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields")]
pub struct Context {
    network: Network,
    request: Request,
    last_indexed_slot: Option<u64>,
    source: Source,
    retrieved_at: Timestamp,
}
impl Context {
    /// Records caller-supplied network/query/source facts and actual DAS progress.
    /// This pure constructor does not verify the network or provider attribution.
    /// # Errors
    /// Rejects a DAS index-progress label attached to a Parsed Events request.
    pub fn new(
        network: Network,
        request: Request,
        last_indexed_slot: Option<u64>,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        if last_indexed_slot.is_some()
            && matches!(
                request,
                Request::ParseTransactions(_) | Request::AddressHistory(_)
            )
        {
            return Err(super::bounded::invalid_request());
        }
        Ok(Self {
            network,
            request,
            last_indexed_slot,
            source,
            retrieved_at,
        })
    }
    /// Returns expected full Solana genesis identity, independently of its display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns immutable exact query controls.
    #[must_use]
    pub const fn request(&self) -> &Request {
        &self.request
    }
    /// Returns actual optional provider index progress; this is not an evaluation slot.
    #[must_use]
    pub const fn last_indexed_slot(&self) -> Option<u64> {
        self.last_indexed_slot
    }
    /// Returns provider/method/version attribution without credentials.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns local response-completion time, distinct from block time/cache age.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFields {
    network: Network,
    request: Request,
    last_indexed_slot: Option<u64>,
    source: Source,
    retrieved_at: Timestamp,
}
impl TryFrom<ContextFields> for Context {
    type Error = Error;
    fn try_from(f: ContextFields) -> Result<Self, Error> {
        Self::new(
            f.network,
            f.request,
            f.last_indexed_slot,
            f.source,
            f.retrieved_at,
        )
    }
}
mod sealed {
    pub trait Sealed {}
}
/// Supported Helius observation data, with validated method/query correlation.
/// The trait is sealed; alternate readers can produce the same public records.
pub trait ObservedData: sealed::Sealed {
    /// Checks that the value corresponds to its exact recorded method/query.
    #[doc(hidden)]
    fn matches_request(&self, request: &Request) -> bool;
}
impl sealed::Sealed for Asset {}
impl ObservedData for Asset {
    fn matches_request(&self, r: &Request) -> bool {
        matches!(r,Request::Asset(q) if q.id==self.id())
    }
}
impl sealed::Sealed for OwnerPage {}
impl ObservedData for OwnerPage {
    fn matches_request(&self, r: &Request) -> bool {
        matches!(r,Request::OwnerAssets(q) if q==self.request())
    }
}
impl sealed::Sealed for ParsedBatch {}
impl ObservedData for ParsedBatch {
    fn matches_request(&self, r: &Request) -> bool {
        matches!(r,Request::ParseTransactions(q) if q==self.request())
    }
}
impl sealed::Sealed for HistoryPage {}
impl ObservedData for HistoryPage {
    fn matches_request(&self, r: &Request) -> bool {
        matches!(r,Request::AddressHistory(q) if q==self.request())
    }
}
/// Exact typed provider result and attribution, with constructor/serde correlation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    try_from = "ObservationFields<T>",
    bound(deserialize = "T: Deserialize<'de> + ObservedData")
)]
pub struct Observation<T> {
    value: T,
    context: Context,
}
impl<T: ObservedData> Observation<T> {
    /// Validates immutable request/value agreement without cryptographic/source proof.
    /// # Errors
    /// Rejects mismatched method or query identity with a fixed diagnostic.
    pub fn new(value: T, context: Context) -> Result<Self, Error> {
        if !value.matches_request(context.request()) {
            return Err(super::bounded::invalid_request());
        }
        Ok(Self { value, context })
    }
    /// Returns every supported source field.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
    /// Returns exact source attribution and query/index facts.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T:Deserialize<'de>"))]
struct ObservationFields<T> {
    value: T,
    context: Context,
}
impl<T: ObservedData> TryFrom<ObservationFields<T>> for Observation<T> {
    type Error = Error;
    fn try_from(f: ObservationFields<T>) -> Result<Self, Error> {
        Self::new(f.value, f.context)
    }
}
