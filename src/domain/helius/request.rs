// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::solana::{Commitment, Pubkey, Signature},
    error::Error,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Opaque source pagination text; it is not assumed to be an asset or signature.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Cursor(String);
impl Cursor {
    /// Records a nonempty cursor of at most 2048 bytes, without control characters.
    /// # Errors
    /// Rejects malformed cursors without retaining their text in diagnostics.
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty() || value.len() > 2048 || value.chars().any(char::is_control) {
            return Err(super::bounded::invalid_request());
        }
        Ok(Self(value))
    }
    /// Returns exact source text for an explicit continuation request.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cursor { .. }")
    }
}
impl TryFrom<String> for Cursor {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Cursor> for String {
    fn from(v: Cursor) -> Self {
        v.0
    }
}

/// Explicit source ordering; equal-slot histories need not have a canonical index.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Source ascending order.
    Asc,
    /// Source descending order.
    Desc,
}
/// Explicit DAS sorting field, without a stable-snapshot claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetSort {
    /// Binary asset identity order; required for keyset pagination.
    Id,
    /// Creation order.
    Created,
    /// Most recent indexed action.
    RecentAction,
    /// Last indexed update.
    Updated,
    /// Unordered source response; page stability is not promised.
    None,
}
/// One explicit DAS pagination mode. Modes are never mixed implicitly.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssetPosition {
    /// One-based page selection.
    Page {
        /// Positive page number.
        page: u32,
    },
    /// Keyset cursor; `None` explicitly starts cursor pagination.
    Cursor {
        /// Caller-supplied source cursor; provenance is a caller assertion.
        cursor: Option<Cursor>,
    },
    /// Explicit binary asset-ID range, with exclusive source bounds.
    Range {
        /// Upper asset-ID bound.
        before: Option<Pubkey>,
        /// Lower asset-ID bound.
        after: Option<Pubkey>,
    },
}
/// Explicit common DAS display controls; no URI is fetched by the library.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetOptions {
    /// Request grouping for unverified collections.
    pub show_unverified_collections: bool,
    /// Request collection metadata.
    pub show_collection_metadata: bool,
    /// Request fungible token facts.
    pub show_fungible: bool,
}
/// Exact single-asset lookup; an ID need not be a mint for compressed assets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRequest {
    /// Exact canonical Solana DAS asset identity.
    pub id: Pubkey,
    /// Explicit source display controls.
    pub options: AssetOptions,
}
/// Explicit owner-page controls, separate from a source-reported grand total.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OwnerRequestFields")]
pub struct OwnerRequest {
    owner: Pubkey,
    limit: u16,
    position: AssetPosition,
    sort: AssetSort,
    direction: SortDirection,
    options: AssetOptions,
    show_grand_total: bool,
    show_native_balance: bool,
    show_zero_balance: bool,
}
impl OwnerRequest {
    /// Validates an explicit request without selecting or crawling another page.
    /// # Errors
    /// Rejects a zero/excessive limit, invalid page/range or non-ID keyset sorting.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner: Pubkey,
        limit: u16,
        position: AssetPosition,
        sort: AssetSort,
        direction: SortDirection,
        options: AssetOptions,
        show_grand_total: bool,
        show_native_balance: bool,
        show_zero_balance: bool,
    ) -> Result<Self, Error> {
        if !(1..=1000).contains(&limit) {
            return Err(super::bounded::invalid_request());
        }
        match &position {
            AssetPosition::Page { page: 0 } => return Err(super::bounded::invalid_request()),
            AssetPosition::Range { before, after }
                if before.is_none() && after.is_none()
                    || before
                        .zip(*after)
                        .is_some_and(|(b, a)| a.bytes() >= b.bytes()) =>
            {
                return Err(super::bounded::invalid_request());
            }
            _ => {}
        }
        if !matches!(position, AssetPosition::Page { .. }) && sort != AssetSort::Id {
            return Err(super::bounded::invalid_request());
        }
        Ok(Self {
            owner,
            limit,
            position,
            sort,
            direction,
            options,
            show_grand_total,
            show_native_balance,
            show_zero_balance,
        })
    }
    /// Returns the requested owner.
    #[must_use]
    pub const fn owner(&self) -> Pubkey {
        self.owner
    }
    /// Returns the explicit maximum items in this response.
    #[must_use]
    pub const fn limit(&self) -> u16 {
        self.limit
    }
    /// Returns the original pagination mode.
    #[must_use]
    pub const fn position(&self) -> &AssetPosition {
        &self.position
    }
    /// Returns the source sort field.
    #[must_use]
    pub const fn sort(&self) -> AssetSort {
        self.sort
    }
    /// Returns the source ordering.
    #[must_use]
    pub const fn direction(&self) -> SortDirection {
        self.direction
    }
    /// Returns explicit common display controls.
    #[must_use]
    pub const fn options(&self) -> AssetOptions {
        self.options
    }
    /// Returns whether a grand total was requested.
    #[must_use]
    pub const fn show_grand_total(&self) -> bool {
        self.show_grand_total
    }
    /// Returns whether native SOL balance was requested.
    #[must_use]
    pub const fn show_native_balance(&self) -> bool {
        self.show_native_balance
    }
    /// Returns whether zero token balances were requested.
    #[must_use]
    pub const fn show_zero_balance(&self) -> bool {
        self.show_zero_balance
    }
    /// Reuses every immutable query control with an explicitly supplied cursor.
    /// Cursor provenance is a caller assertion; concrete client handles additionally
    /// bind continuations to the original configured client.
    /// # Errors
    /// Rejects use outside ID-sorted cursor pagination.
    pub fn with_cursor(&self, cursor: Cursor) -> Result<Self, Error> {
        if !matches!(self.position, AssetPosition::Cursor { .. }) {
            return Err(super::bounded::invalid_request());
        }
        let mut next = self.clone();
        next.position = AssetPosition::Cursor {
            cursor: Some(cursor),
        };
        Ok(next)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerRequestFields {
    owner: Pubkey,
    limit: u16,
    position: AssetPosition,
    sort: AssetSort,
    direction: SortDirection,
    options: AssetOptions,
    show_grand_total: bool,
    show_native_balance: bool,
    show_zero_balance: bool,
}
impl TryFrom<OwnerRequestFields> for OwnerRequest {
    type Error = Error;
    fn try_from(f: OwnerRequestFields) -> Result<Self, Error> {
        Self::new(
            f.owner,
            f.limit,
            f.position,
            f.sort,
            f.direction,
            f.options,
            f.show_grand_total,
            f.show_native_balance,
            f.show_zero_balance,
        )
    }
}

/// Immutable ordered parse batch. Duplicate signatures intentionally retain positions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ParseRequestFields")]
pub struct ParseRequest {
    signatures: Vec<Signature>,
    commitment: Commitment,
}
impl ParseRequest {
    /// Records 1..=100 exact signatures at confirmed/finalized commitment.
    /// Original raw RPC transaction objects are not requested or synthesized.
    /// # Errors
    /// Rejects unsupported commitment or an empty/excessive batch.
    pub fn new(signatures: Vec<Signature>, commitment: Commitment) -> Result<Self, Error> {
        if signatures.is_empty() || signatures.len() > 100 || commitment == Commitment::Processed {
            return Err(super::bounded::invalid_request());
        }
        Ok(Self {
            signatures,
            commitment,
        })
    }
    /// Returns requested signatures in exact input order.
    #[must_use]
    pub fn signatures(&self) -> &[Signature] {
        &self.signatures
    }
    /// Returns actual confirmed/finalized lookup commitment.
    #[must_use]
    pub const fn commitment(&self) -> Commitment {
        self.commitment
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParseRequestFields {
    #[serde(deserialize_with = "super::bounded::list")]
    signatures: Vec<Signature>,
    commitment: Commitment,
}
impl TryFrom<ParseRequestFields> for ParseRequest {
    type Error = Error;
    fn try_from(f: ParseRequestFields) -> Result<Self, Error> {
        Self::new(f.signatures, f.commitment)
    }
}

/// Optional explicit inclusive/exclusive source comparisons in one unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BoundsFields")]
pub struct Bounds {
    gt: Option<u64>,
    gte: Option<u64>,
    lt: Option<u64>,
    lte: Option<u64>,
}
impl Bounds {
    /// Validates nonempty, unambiguous and nonempty integer comparison bounds.
    /// # Errors
    /// Rejects simultaneous gt/gte or lt/lte, overflow or an empty integer interval.
    pub fn new(
        gt: Option<u64>,
        gte: Option<u64>,
        lt: Option<u64>,
        lte: Option<u64>,
    ) -> Result<Self, Error> {
        if gt.is_none() && gte.is_none() && lt.is_none() && lte.is_none()
            || gt.is_some() && gte.is_some()
            || lt.is_some() && lte.is_some()
        {
            return Err(super::bounded::invalid_request());
        }
        let min = match gt {
            Some(v) => v
                .checked_add(1)
                .ok_or_else(super::bounded::invalid_request)?,
            None => gte.unwrap_or(0),
        };
        let max = match lt {
            Some(v) => v
                .checked_sub(1)
                .ok_or_else(super::bounded::invalid_request)?,
            None => lte.unwrap_or(u64::MAX),
        };
        if min > max {
            return Err(super::bounded::invalid_request());
        }
        Ok(Self { gt, gte, lt, lte })
    }
    /// Returns exclusive lower, inclusive lower, exclusive upper and inclusive upper.
    #[must_use]
    pub const fn comparisons(self) -> [Option<u64>; 4] {
        [self.gt, self.gte, self.lt, self.lte]
    }
    pub(super) fn contains(self, value: u64) -> bool {
        self.gt.is_none_or(|v| value > v)
            && self.gte.is_none_or(|v| value >= v)
            && self.lt.is_none_or(|v| value < v)
            && self.lte.is_none_or(|v| value <= v)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundsFields {
    gt: Option<u64>,
    gte: Option<u64>,
    lt: Option<u64>,
    lte: Option<u64>,
}
impl TryFrom<BoundsFields> for Bounds {
    type Error = Error;
    fn try_from(f: BoundsFields) -> Result<Self, Error> {
        Self::new(f.gt, f.gte, f.lt, f.lte)
    }
}
/// Explicit history request. Pagination never fetches additional pages automatically.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryRequestFields")]
pub struct HistoryRequest {
    address: Pubkey,
    limit: u8,
    commitment: Commitment,
    direction: SortDirection,
    before_signature: Option<Signature>,
    after_signature: Option<Signature>,
    slot: Option<Bounds>,
    time: Option<Bounds>,
    pagination_token: Option<Cursor>,
}
impl HistoryRequest {
    /// Records independent source controls, retaining bounds during token continuation.
    /// # Errors
    /// Rejects invalid limits/commitment or identical exclusive signature bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: Pubkey,
        limit: u8,
        commitment: Commitment,
        direction: SortDirection,
        before_signature: Option<Signature>,
        after_signature: Option<Signature>,
        slot: Option<Bounds>,
        time: Option<Bounds>,
        pagination_token: Option<Cursor>,
    ) -> Result<Self, Error> {
        if !(1..=100).contains(&limit)
            || commitment == Commitment::Processed
            || before_signature
                .zip(after_signature)
                .is_some_and(|(a, b)| a == b)
        {
            return Err(super::bounded::invalid_request());
        }
        Ok(Self {
            address,
            limit,
            commitment,
            direction,
            before_signature,
            after_signature,
            slot,
            time,
            pagination_token,
        })
    }
    /// Returns exact history address.
    #[must_use]
    pub const fn address(&self) -> Pubkey {
        self.address
    }
    /// Returns explicit page capacity.
    #[must_use]
    pub const fn limit(&self) -> u8 {
        self.limit
    }
    /// Returns actual source commitment.
    #[must_use]
    pub const fn commitment(&self) -> Commitment {
        self.commitment
    }
    /// Returns source ordering.
    #[must_use]
    pub const fn direction(&self) -> SortDirection {
        self.direction
    }
    /// Returns explicit exclusive signature bounds.
    #[must_use]
    pub const fn signatures(&self) -> [Option<Signature>; 2] {
        [self.before_signature, self.after_signature]
    }
    /// Returns slot bounds, distinct from Unix-second bounds.
    #[must_use]
    pub const fn slot(&self) -> Option<Bounds> {
        self.slot
    }
    /// Returns whole Unix-second source block-time bounds.
    #[must_use]
    pub const fn time(&self) -> Option<Bounds> {
        self.time
    }
    /// Returns an exact caller-supplied pagination token.
    #[must_use]
    pub const fn pagination_token(&self) -> Option<&Cursor> {
        self.pagination_token.as_ref()
    }
    /// Continues the original query controls with an explicit source token.
    /// Signature/slot/time bounds, ordering, commitment, address and limit remain
    /// unchanged. The token supplies position without broadening the interval.
    #[must_use]
    pub fn with_token(&self, token: Cursor) -> Self {
        let mut next = self.clone();
        next.pagination_token = Some(token);
        next
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryRequestFields {
    address: Pubkey,
    limit: u8,
    commitment: Commitment,
    direction: SortDirection,
    before_signature: Option<Signature>,
    after_signature: Option<Signature>,
    slot: Option<Bounds>,
    time: Option<Bounds>,
    pagination_token: Option<Cursor>,
}
impl TryFrom<HistoryRequestFields> for HistoryRequest {
    type Error = Error;
    fn try_from(f: HistoryRequestFields) -> Result<Self, Error> {
        Self::new(
            f.address,
            f.limit,
            f.commitment,
            f.direction,
            f.before_signature,
            f.after_signature,
            f.slot,
            f.time,
            f.pagination_token,
        )
    }
}
