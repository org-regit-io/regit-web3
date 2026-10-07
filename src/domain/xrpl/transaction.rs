// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha512};

use crate::error::{Error, ValidationError};

use super::{Address, Hash, Ledger};

fn invalid_record() -> Error {
    ValidationError::InvalidXrplRecord.into()
}

/// Bounded opaque protocol bytes encoded as canonical uppercase hexadecimal.
///
/// Encoding validation does not establish canonical XRPL binary structure or
/// validate transaction signatures. Metadata is retained without lossy parsing.
#[derive(Clone, Eq, PartialEq)]
pub struct HexData(Vec<u8>);
impl HexData {
    /// Maximum bytes retained for an individual transaction or metadata blob.
    pub const MAX_BYTES: usize = 1_048_576;
    /// Parses nonempty even-width hexadecimal without a prefix.
    ///
    /// # Errors
    /// Rejects invalid encoding, empty data and values above the byte bound.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.is_empty()
            || text.len() > Self::MAX_BYTES * 2
            || !text.len().is_multiple_of(2)
            || !text.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid_record());
        }
        const_hex::decode(text)
            .map(Self)
            .map_err(|_| invalid_record())
    }
    /// Returns exact opaque bytes, without binary or signature validation.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
    /// Returns canonical uppercase hexadecimal.
    #[must_use]
    pub fn to_hex(&self) -> String {
        const_hex::encode_upper(&self.0)
    }
    /// Computes XRPL's transaction ID as SHA-512 first half of `TXN\0 || bytes`.
    ///
    /// This is not SHA-512/256, a signing digest or proof of signature validity.
    #[must_use]
    pub fn transaction_hash(&self) -> Hash {
        let mut digest = Sha512::new();
        digest.update(b"TXN\0");
        digest.update(&self.0);
        let full = digest.finalize();
        let mut bytes = [0; 32];
        bytes.copy_from_slice(&full[..32]);
        Hash::from_bytes(bytes)
    }
}
impl fmt::Debug for HexData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HexData")
            .field("byte_length", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl Serialize for HexData {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}
impl<'de> Deserialize<'de> for HexData {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// A bounded source-reported ledger execution result code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ResultCode(String);
impl ResultCode {
    /// Records `tesSUCCESS` or a claimed-cost `tec` result from applied metadata.
    ///
    /// # Errors
    /// Rejects malformed codes and non-ledger engine result classes.
    pub fn parse(code: &str) -> Result<Self, Error> {
        let valid = code == "tesSUCCESS"
            || code.strip_prefix("tec").is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            });
        if !valid || code.len() > 64 {
            return Err(invalid_record());
        }
        Ok(Self(code.into()))
    }
    /// Returns the exact protocol result code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Reports source-reported success independently of ledger validation.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.0 == "tesSUCCESS"
    }
}
impl<'de> Deserialize<'de> for ResultCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Source-reported transaction inclusion and execution facts, never inferred from absence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StatusFields")]
pub struct TransactionStatus {
    hash: Hash,
    ledger: Option<Ledger>,
    execution: Option<ResultCode>,
}
impl TransactionStatus {
    /// Records actual inclusion and optional applied execution metadata.
    ///
    /// # Errors
    /// Rejects validated inclusion without execution metadata and execution
    /// metadata without any reported inclusion. Validation does not imply success.
    pub fn new(
        hash: Hash,
        ledger: Option<Ledger>,
        execution: Option<ResultCode>,
    ) -> Result<Self, Error> {
        if ledger.is_some_and(Ledger::validated) && execution.is_none()
            || execution.is_some() && ledger.is_none()
        {
            return Err(invalid_record());
        }
        Ok(Self {
            hash,
            ledger,
            execution,
        })
    }
    /// Returns the exact queried transaction ID.
    #[must_use]
    pub const fn hash(&self) -> Hash {
        self.hash
    }
    /// Returns actual reported inclusion, if known.
    #[must_use]
    pub const fn ledger(&self) -> Option<Ledger> {
        self.ledger
    }
    /// Returns applied execution code, separately from validation.
    #[must_use]
    pub const fn execution(&self) -> Option<&ResultCode> {
        self.execution.as_ref()
    }
    /// Reports only the source's validation assessment.
    #[must_use]
    pub fn validated(&self) -> bool {
        self.ledger.is_some_and(Ledger::validated)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusFields {
    hash: Hash,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    ledger: Option<Ledger>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    execution: Option<ResultCode>,
}
impl TryFrom<StatusFields> for TransactionStatus {
    type Error = Error;
    fn try_from(v: StatusFields) -> Result<Self, Error> {
        Self::new(v.hash, v.ledger, v.execution)
    }
}

/// An arbitrary transaction's exact opaque binary payload and reported inclusion.
///
/// The payload hash is recomputed, without claiming binary/signature validation.
/// Execution semantics remain in separately retrieved typed status or metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionFields")]
pub struct Transaction {
    hash: Hash,
    payload: HexData,
    metadata: Option<HexData>,
    ledger: Option<Ledger>,
}
impl Transaction {
    /// Records exact payload bytes after recomputing their transaction ID.
    ///
    /// # Errors
    /// Rejects hash mismatch, missing metadata for validated inclusion and
    /// metadata without any reported ledger. No signatures are accepted as proven.
    pub fn new(
        hash: Hash,
        payload: HexData,
        metadata: Option<HexData>,
        ledger: Option<Ledger>,
    ) -> Result<Self, Error> {
        if payload.transaction_hash() != hash
            || ledger.is_some_and(Ledger::validated) && metadata.is_none()
            || metadata.is_some() && ledger.is_none()
        {
            return Err(invalid_record());
        }
        Ok(Self {
            hash,
            payload,
            metadata,
            ledger,
        })
    }
    /// Returns the recomputed transaction ID.
    #[must_use]
    pub const fn hash(&self) -> Hash {
        self.hash
    }
    /// Returns exact opaque transaction bytes.
    #[must_use]
    pub const fn payload(&self) -> &HexData {
        &self.payload
    }
    /// Returns actual opaque applied metadata, when reported.
    #[must_use]
    pub const fn metadata(&self) -> Option<&HexData> {
        self.metadata.as_ref()
    }
    /// Returns reported inclusion, independently of payload identity.
    #[must_use]
    pub const fn ledger(&self) -> Option<Ledger> {
        self.ledger
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionFields {
    hash: Hash,
    payload: HexData,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    metadata: Option<HexData>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    ledger: Option<Ledger>,
}
impl TryFrom<TransactionFields> for Transaction {
    type Error = Error;
    fn try_from(v: TransactionFields) -> Result<Self, Error> {
        Self::new(v.hash, v.payload, v.metadata, v.ledger)
    }
}

/// An explicit positive inclusive ledger range used by account-history queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RangeFields")]
pub struct LedgerRange {
    minimum: u32,
    maximum: u32,
}
impl LedgerRange {
    /// Records a bounded inclusive range without selecting a moving latest ledger.
    ///
    /// # Errors
    /// Rejects zero or descending endpoints.
    pub const fn new(minimum: u32, maximum: u32) -> Result<Self, Error> {
        if minimum == 0 || maximum < minimum {
            return Err(Error::Validation(ValidationError::InvalidPageRequest));
        }
        Ok(Self { minimum, maximum })
    }
    /// Returns the earliest requested or actually searched ledger.
    #[must_use]
    pub const fn minimum(self) -> u32 {
        self.minimum
    }
    /// Returns the latest requested or actually searched ledger.
    #[must_use]
    pub const fn maximum(self) -> u32 {
        self.maximum
    }
    fn contains(self, index: u32) -> bool {
        (self.minimum..=self.maximum).contains(&index)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RangeFields {
    minimum: u32,
    maximum: u32,
}
impl TryFrom<RangeFields> for LedgerRange {
    type Error = Error;
    fn try_from(v: RangeFields) -> Result<Self, Error> {
        Self::new(v.minimum, v.maximum)
    }
}

/// The ledger/sequence continuation shape used by supported `account_tx` backends.
///
/// Markers are server-defined. This type does not imply universal marker support
/// or transaction ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CursorFields")]
pub struct HistoryMarker {
    ledger: u32,
    seq: u32,
}
impl HistoryMarker {
    /// Records a positive ledger and transaction sequence/index marker.
    ///
    /// # Errors
    /// Rejects a zero ledger. Sequence zero is valid.
    pub const fn new(ledger: u32, seq: u32) -> Result<Self, Error> {
        if ledger == 0 {
            return Err(Error::Validation(ValidationError::InvalidPageRequest));
        }
        Ok(Self { ledger, seq })
    }
    /// Returns the marker ledger.
    #[must_use]
    pub const fn ledger(self) -> u32 {
        self.ledger
    }
    /// Returns the marker's sequence/index without interpreting provider ordering.
    #[must_use]
    pub const fn sequence(self) -> u32 {
        self.seq
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CursorFields {
    ledger: u32,
    seq: u32,
}
impl TryFrom<CursorFields> for HistoryMarker {
    type Error = Error;
    fn try_from(v: CursorFields) -> Result<Self, Error> {
        Self::new(v.ledger, v.seq)
    }
}

/// A bounded history query retaining exact range, direction and continuation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryRequestFields")]
pub struct HistoryRequest {
    range: LedgerRange,
    limit: u16,
    forward: bool,
    marker: Option<HistoryMarker>,
}
impl HistoryRequest {
    /// Records explicit pagination; servers may search a smaller available range.
    ///
    /// # Errors
    /// Rejects limits outside 1..=400 or a marker outside the requested range.
    pub fn new(
        range: LedgerRange,
        limit: u16,
        forward: bool,
        marker: Option<HistoryMarker>,
    ) -> Result<Self, Error> {
        if !(1..=400).contains(&limit)
            || marker.is_some_and(|cursor| !range.contains(cursor.ledger()))
        {
            return Err(ValidationError::InvalidPageRequest.into());
        }
        Ok(Self {
            range,
            limit,
            forward,
            marker,
        })
    }
    /// Returns the exact requested search range.
    #[must_use]
    pub const fn range(&self) -> LedgerRange {
        self.range
    }
    /// Returns the hard local entry bound.
    #[must_use]
    pub const fn limit(&self) -> u16 {
        self.limit
    }
    /// Reports whether pages proceed oldest first; intra-page order is unspecified.
    #[must_use]
    pub const fn forward(&self) -> bool {
        self.forward
    }
    /// Returns the optional requested continuation.
    #[must_use]
    pub const fn marker(&self) -> Option<HistoryMarker> {
        self.marker
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryRequestFields {
    range: LedgerRange,
    limit: u16,
    forward: bool,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    marker: Option<HistoryMarker>,
}
impl TryFrom<HistoryRequestFields> for HistoryRequest {
    type Error = Error;
    fn try_from(v: HistoryRequestFields) -> Result<Self, Error> {
        Self::new(v.range, v.limit, v.forward, v.marker)
    }
}

/// A bounded account-history page of hash-checked opaque transactions.
///
/// Searched range remains explicit and may be smaller than requested. No marker
/// means the source completed that searched range, never an exhaustive lifetime
/// history guarantee. Account involvement remains the indexer's reported fact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryFields")]
pub struct HistoryPage {
    account: Address,
    request: HistoryRequest,
    searched: LedgerRange,
    transactions: Vec<Transaction>,
    next: Option<HistoryMarker>,
}
impl HistoryPage {
    /// Validates page identity uniqueness, actual inclusion and search-range bounds.
    ///
    /// # Errors
    /// Rejects excess entries, duplicate hashes, nonvalidated or out-of-range
    /// inclusion, a searched range extending beyond requested, and a nonadvancing
    /// or out-of-range continuation. Intra-page ordering is not invented.
    pub fn new(
        account: Address,
        request: HistoryRequest,
        searched: LedgerRange,
        transactions: Vec<Transaction>,
        next: Option<HistoryMarker>,
    ) -> Result<Self, Error> {
        let requested = request.range();
        let mut hashes = BTreeSet::new();
        if transactions.len() > usize::from(request.limit())
            || searched.minimum() < requested.minimum()
            || searched.maximum() > requested.maximum()
            || transactions.iter().any(|tx| {
                !hashes.insert(tx.hash())
                    || !tx.ledger().is_some_and(|ledger| {
                        ledger.validated() && searched.contains(ledger.index())
                    })
            })
            || next.is_some_and(|marker| {
                Some(marker) == request.marker() || !searched.contains(marker.ledger())
            })
        {
            return Err(invalid_record());
        }
        Ok(Self {
            account,
            request,
            searched,
            transactions,
            next,
        })
    }
    /// Returns the perspective account.
    #[must_use]
    pub const fn account(&self) -> Address {
        self.account
    }
    /// Returns the full requested range and pagination.
    #[must_use]
    pub const fn request(&self) -> &HistoryRequest {
        &self.request
    }
    /// Returns the provider's actually searched ledger range.
    #[must_use]
    pub const fn searched(&self) -> LedgerRange {
        self.searched
    }
    /// Returns actual hash-checked transactions with source-reported inclusion.
    #[must_use]
    pub fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }
    /// Returns continuation within the searched range.
    #[must_use]
    pub const fn next_marker(&self) -> Option<HistoryMarker> {
        self.next
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryFields {
    account: Address,
    request: HistoryRequest,
    searched: LedgerRange,
    transactions: Vec<Transaction>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    next: Option<HistoryMarker>,
}
impl TryFrom<HistoryFields> for HistoryPage {
    type Error = Error;
    fn try_from(v: HistoryFields) -> Result<Self, Error> {
        Self::new(v.account, v.request, v.searched, v.transactions, v.next)
    }
}
