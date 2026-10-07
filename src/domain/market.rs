// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact provider data, explicit time units, bounded requests and attribution.
//!
//! Market observations record an attributed provider response. They do not
//! establish a blockchain snapshot, finality or equivalence between providers.

use serde::{Deserialize, Serialize};

use super::{ExactDecimal, Source, Timestamp};
use crate::error::{Error, ValidationError};

/// A case-sensitive provider identifier, not a blockchain asset identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Identifier(String);

impl Identifier {
    /// Validates a nonempty ASCII identifier of at most 256 bytes.
    ///
    /// Letters, digits, hyphens, underscores and periods are accepted; standalone
    /// `.` and `..` are rejected. Case remains significant.
    ///
    /// # Errors
    /// Returns a fixed validation failure for invalid text.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > 256
            || matches!(value, "." | "..")
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err(ValidationError::InvalidMarketIdentity.into());
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns the original validated identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Identifier {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::parse(&value)
    }
}
impl From<Identifier> for String {
    fn from(value: Identifier) -> Self {
        value.0
    }
}

/// A nonempty display label, retained separately from provider identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Label(String);
impl Label {
    /// Validates nonempty text of at most 512 bytes without control characters.
    ///
    /// # Errors
    /// Returns a fixed validation failure for invalid text.
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
            return Err(ValidationError::InvalidMarketIdentity.into());
        }
        Ok(Self(value.to_owned()))
    }
    /// Returns the original label without normalization.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Label {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::new(&value)
    }
}
impl From<Label> for String {
    fn from(value: Label) -> Self {
        value.0
    }
}

/// An exact nonnegative decimal value with no implied transaction base units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ExactDecimal", into = "ExactDecimal")]
pub struct NonnegativeDecimal(ExactDecimal);
impl NonnegativeDecimal {
    /// Requires a finite exact value that is zero or positive.
    ///
    /// # Errors
    /// Rejects a negative value with a fixed validation reason.
    pub fn new(value: ExactDecimal) -> Result<Self, Error> {
        if value.is_negative() {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self(value))
    }
    /// Returns the exact value; serialization uses a canonical decimal string.
    #[must_use]
    pub const fn value(&self) -> &ExactDecimal {
        &self.0
    }
}
impl TryFrom<ExactDecimal> for NonnegativeDecimal {
    type Error = Error;
    fn try_from(value: ExactDecimal) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<NonnegativeDecimal> for ExactDecimal {
    fn from(value: NonnegativeDecimal) -> Self {
        value.0
    }
}

/// A caller-selected item limit; exceeding it fails instead of truncating.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct ItemLimit(u32);
impl ItemLimit {
    /// Largest supported collection size, independent of HTTP byte limits.
    pub const MAXIMUM: u32 = 100_000;
    /// Constructs an explicit collection bound in `1..=100_000`.
    ///
    /// # Errors
    /// Rejects zero or an excessive limit.
    pub fn new(value: u32) -> Result<Self, Error> {
        if value == 0 || value > Self::MAXIMUM {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self(value))
    }
    /// Returns the explicit item bound.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl TryFrom<u32> for ItemLimit {
    type Error = Error;
    fn try_from(value: u32) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<ItemLimit> for u32 {
    fn from(value: ItemLimit) -> Self {
        value.0
    }
}

/// A provider data timestamp in Unix milliseconds, distinct from retrieval time.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnixMilliseconds(u64);
impl UnixMilliseconds {
    /// Records explicitly supplied Unix milliseconds without consulting a clock.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Returns the provider timestamp in milliseconds.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A validated RFC 3339 UTC timestamp, preserving its supplied fractional digits.
///
/// Supported form is `YYYY-MM-DDTHH:MM:SS[.fraction]Z`, with 1–9 fractional
/// digits. Real calendar dates are checked; leap seconds and offsets are not
/// accepted. This type does not round or convert the provider's data time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UtcDateTime(String);
impl UtcDateTime {
    /// Validates the documented UTC date-time representation.
    ///
    /// # Errors
    /// Rejects malformed dates and unsupported representations.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if !valid_datetime(value) {
            return Err(ValidationError::InvalidMarketRecord.into());
        }
        Ok(Self(value.to_owned()))
    }
    /// Returns the original, validated UTC representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for UtcDateTime {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::parse(&value)
    }
}
impl UtcDateTime {
    #[cfg(feature = "defillama")]
    pub(crate) fn chronological_key(&self) -> (&str, u32) {
        let fraction = self.0.get(20..self.0.len() - 1).unwrap_or("");
        let nanos = fraction
            .bytes()
            .fold(0_u32, |n, b| n * 10 + u32::from(b - b'0'))
            * 10_u32.pow(u32::try_from(9 - fraction.len()).unwrap_or(0));
        (&self.0[..19], nanos)
    }
}
impl From<UtcDateTime> for String {
    fn from(value: UtcDateTime) -> Self {
        value.0
    }
}
fn valid_datetime(value: &str) -> bool {
    let b = value.as_bytes();
    if !(20..=30).contains(&b.len()) || !b.is_ascii() {
        return false;
    }
    if b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[b.len() - 1] != b'Z'
    {
        return false;
    }
    if b.len() != 20
        && (b[19] != b'.'
            || !(1..=9).contains(&(b.len() - 21))
            || !b[20..b.len() - 1].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let part = |start, end| {
        value
            .get(start..end)
            .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<u16>().ok())
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        part(0, 4),
        part(5, 7),
        part(8, 10),
        part(11, 13),
        part(14, 16),
        part(17, 19),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    day >= 1 && day <= days && hour < 24 && minute < 60 && second < 60
}

/// A provider value with explicit source and retrieval time, without chain fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation<T> {
    source: Source,
    retrieved_at: Timestamp,
    value: T,
}
impl<T> Observation<T> {
    /// Records caller-supplied attribution, retrieval time and typed value.
    #[must_use]
    pub const fn new(value: T, source: Source, retrieved_at: Timestamp) -> Self {
        Self {
            source,
            retrieved_at,
            value,
        }
    }
    /// Returns the typed value and its provider-specific identities and units.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
    /// Returns explicit attribution labels; endpoints and credentials are absent.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns retrieval time in Unix seconds, separate from provider data time.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}

#[cfg(any(feature = "coingecko", feature = "defillama", feature = "rubic"))]
pub(crate) fn unique<T: Ord>(items: impl IntoIterator<Item = T>) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    items.into_iter().all(|item| seen.insert(item))
}
