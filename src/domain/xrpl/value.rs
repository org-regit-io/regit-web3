// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{collections::BTreeSet, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    domain::ExactDecimal,
    error::{Error, ValidationError},
};

use super::{Address, Currency, Hash};

fn invalid_record() -> Error {
    ValidationError::InvalidXrplRecord.into()
}

/// An exact nonnegative XRP amount in drops, bounded by the original supply.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Drops(u64);
impl Drops {
    /// Maximum XRP amount, expressed in drops.
    pub const MAX: u64 = 100_000_000_000_000_000;
    /// Records drops without converting through floating point.
    ///
    /// # Errors
    /// Rejects values exceeding the protocol's maximum XRP amount.
    pub const fn new(value: u64) -> Result<Self, Error> {
        if value > Self::MAX {
            return Err(Error::Validation(ValidationError::InvalidXrplAmount));
        }
        Ok(Self(value))
    }
    /// Parses canonical unsigned decimal drops.
    ///
    /// # Errors
    /// Rejects signs, fractional values, leading zeros and overflow.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.is_empty()
            || (text.len() > 1 && text.starts_with('0'))
            || !text.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(ValidationError::InvalidXrplAmount.into());
        }
        Self::new(
            text.parse()
                .map_err(|_| Error::from(ValidationError::InvalidXrplAmount))?,
        )
    }
    /// Returns exact integer drops; one XRP is one million drops.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}
impl fmt::Display for Drops {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for Drops {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        Self::parse(text)
    }
}
impl Serialize for Drops {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for Drops {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// An exact signed issued-currency value representable by XRPL's `STAmount`.
///
/// Values use a normalized 16-digit integer mantissa and exponent -96 through
/// 80, or exact zero. No rounding, float conversion or token decimals are added.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct IssuedValue(ExactDecimal);
impl IssuedValue {
    /// Parses exact finite decimal notation and rejects nonrepresentable values.
    ///
    /// # Errors
    /// Rejects excess significant digits, exponent bounds and malformed decimals.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let value = ExactDecimal::parse(text)
            .map_err(|_| Error::from(ValidationError::InvalidXrplAmount))?;
        if !value.is_zero() {
            let canonical = value.canonical();
            let absolute = canonical.strip_prefix('-').unwrap_or(&canonical);
            let decimal_position = absolute.find('.').unwrap_or(absolute.len());
            let digits: String = absolute.chars().filter(|c| *c != '.').collect();
            let leading = digits.bytes().take_while(|b| *b == b'0').count();
            let significant = digits.trim_start_matches('0').trim_end_matches('0').len();
            let order = i64::try_from(decimal_position).map_err(|_| invalid_record())?
                - i64::try_from(leading).map_err(|_| invalid_record())?
                - 1;
            if significant > 16 || !(-96..=80).contains(&(order - 15)) {
                return Err(ValidationError::InvalidXrplAmount.into());
            }
        }
        Ok(Self(value))
    }
    /// Returns the exact signed decimal value, without attaching a token scale.
    #[must_use]
    pub const fn value(&self) -> &ExactDecimal {
        &self.0
    }
}
impl FromStr for IssuedValue {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        Self::parse(text)
    }
}
impl<'de> Deserialize<'de> for IssuedValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Exact account XRP balance and sequencing facts at an attributed ledger.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountBalance {
    account: Address,
    balance: Drops,
    sequence: u32,
    owner_count: u32,
    flags: u32,
}
impl AccountBalance {
    /// Records actual account-root facts; the balance is not a spendable estimate.
    #[must_use]
    pub const fn new(
        account: Address,
        balance: Drops,
        sequence: u32,
        owner_count: u32,
        flags: u32,
    ) -> Self {
        Self {
            account,
            balance,
            sequence,
            owner_count,
            flags,
        }
    }
    /// Returns the classic account.
    #[must_use]
    pub const fn account(&self) -> Address {
        self.account
    }
    /// Returns exact XRP drops without subtracting fees or reserves.
    #[must_use]
    pub const fn balance(&self) -> Drops {
        self.balance
    }
    /// Returns the account's actual next ordinary sequence number.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }
    /// Returns source-reported owner count, not an independently computed reserve.
    #[must_use]
    pub const fn owner_count(&self) -> u32 {
        self.owner_count
    }
    /// Returns the raw account-root flags.
    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }
}

/// A source-reported two-state trustline setting, serialized as a boolean.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(from = "bool", into = "bool")]
pub enum Setting {
    /// The source reports the setting disabled or the protocol default false.
    #[default]
    Disabled,
    /// The source reports the setting enabled.
    Enabled,
}
impl From<bool> for Setting {
    fn from(value: bool) -> Self {
        if value { Self::Enabled } else { Self::Disabled }
    }
}
impl From<Setting> for bool {
    fn from(value: Setting) -> Self {
        matches!(value, Setting::Enabled)
    }
}

/// Source-reported trustline settings from the perspective account's side.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineFlags {
    /// Whether the perspective account disables rippling on this line.
    pub no_ripple: Setting,
    /// Whether the counterparty disables rippling on this line.
    pub no_ripple_peer: Setting,
    /// Whether the perspective account authorized the line.
    pub authorized: Setting,
    /// Whether the counterparty authorized the line.
    pub peer_authorized: Setting,
    /// Whether the perspective account froze the line.
    pub freeze: Setting,
    /// Whether the counterparty froze the line.
    pub freeze_peer: Setting,
    /// Incoming quality ratio per one billion units; zero has protocol meaning.
    pub quality_in: u32,
    /// Outgoing quality ratio per one billion units; zero has protocol meaning.
    pub quality_out: u32,
}

/// A signed issued balance against one counterparty, preserving credit settings.
///
/// Positive means the perspective account holds value; negative means it owes
/// value. The peer is a trustline counterparty, not a guaranteed token issuer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LineFields")]
pub struct TrustLine {
    peer: Address,
    currency: Currency,
    balance: IssuedValue,
    limit: IssuedValue,
    limit_peer: IssuedValue,
    settings: LineFlags,
}
impl TrustLine {
    /// Records exact signed balance and nonnegative credit limits.
    ///
    /// # Errors
    /// Rejects negative limits without changing the sign of the balance.
    pub fn new(
        peer: Address,
        currency: Currency,
        balance: IssuedValue,
        limit: IssuedValue,
        limit_peer: IssuedValue,
        settings: LineFlags,
    ) -> Result<Self, Error> {
        if limit.value().is_negative() || limit_peer.value().is_negative() {
            return Err(invalid_record());
        }
        Ok(Self {
            peer,
            currency,
            balance,
            limit,
            limit_peer,
            settings,
        })
    }
    /// Returns the counterparty account.
    #[must_use]
    pub const fn peer(&self) -> Address {
        self.peer
    }
    /// Returns the exact issued currency identity.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
    /// Returns the exact signed balance from the requested perspective.
    #[must_use]
    pub const fn balance(&self) -> &IssuedValue {
        &self.balance
    }
    /// Returns the perspective-side credit limit.
    #[must_use]
    pub const fn limit(&self) -> &IssuedValue {
        &self.limit
    }
    /// Returns the peer-side credit limit.
    #[must_use]
    pub const fn limit_peer(&self) -> &IssuedValue {
        &self.limit_peer
    }
    /// Returns the reported settings.
    #[must_use]
    pub const fn settings(&self) -> LineFlags {
        self.settings
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineFields {
    peer: Address,
    currency: Currency,
    balance: IssuedValue,
    limit: IssuedValue,
    limit_peer: IssuedValue,
    settings: LineFlags,
}
impl TryFrom<LineFields> for TrustLine {
    type Error = Error;
    fn try_from(v: LineFields) -> Result<Self, Error> {
        Self::new(
            v.peer,
            v.currency,
            v.balance,
            v.limit,
            v.limit_peer,
            v.settings,
        )
    }
}

/// A bounded opaque string marker from an XRPL pagination response.
///
/// Markers are scoped to one method/account/ledger; the page request records
/// the ledger hash explicitly. Diagnostic formatting omits marker contents.
#[derive(Clone, Eq, PartialEq)]
pub struct Marker(String);
impl Marker {
    /// Records a bounded printable string without interpreting provider contents.
    ///
    /// # Errors
    /// Rejects empty values, controls, non-ASCII data and values over 1024 bytes.
    pub fn new(text: impl Into<String>) -> Result<Self, Error> {
        let text = text.into();
        if text.is_empty() || text.len() > 1024 || !text.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(ValidationError::InvalidPageRequest.into());
        }
        Ok(Self(text))
    }
    /// Returns the marker for resuming the same account and ledger query.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Marker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Marker([REDACTED])")
    }
}
impl Serialize for Marker {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for Marker {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Explicit bounded trustline pagination, pinned when resuming a page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PageFields")]
pub struct PageRequest {
    limit: u16,
    ledger_hash: Option<Hash>,
    marker: Option<Marker>,
}
impl PageRequest {
    /// Requests a page with optional exact ledger hash and continuation marker.
    ///
    /// # Errors
    /// Rejects limits outside 10..=400 and a marker lacking its ledger hash.
    pub fn new(
        limit: u16,
        ledger_hash: Option<Hash>,
        marker: Option<Marker>,
    ) -> Result<Self, Error> {
        if !(10..=400).contains(&limit) || (marker.is_some() && ledger_hash.is_none()) {
            return Err(ValidationError::InvalidPageRequest.into());
        }
        Ok(Self {
            limit,
            ledger_hash,
            marker,
        })
    }
    /// Returns the explicit page bound.
    #[must_use]
    pub const fn limit(&self) -> u16 {
        self.limit
    }
    /// Returns the exact ledger, or none to resolve the latest validated ledger.
    #[must_use]
    pub const fn ledger_hash(&self) -> Option<Hash> {
        self.ledger_hash
    }
    /// Returns the optional same-ledger continuation marker.
    #[must_use]
    pub const fn marker(&self) -> Option<&Marker> {
        self.marker.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageFields {
    limit: u16,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    ledger_hash: Option<Hash>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    marker: Option<Marker>,
}
impl TryFrom<PageFields> for PageRequest {
    type Error = Error;
    fn try_from(v: PageFields) -> Result<Self, Error> {
        Self::new(v.limit, v.ledger_hash, v.marker)
    }
}

/// One bounded trustline page, retaining issued balances and continuation facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PageValueFields")]
pub struct TrustLinePage {
    account: Address,
    request: PageRequest,
    lines: Vec<TrustLine>,
    next: Option<Marker>,
}
impl TrustLinePage {
    /// Records a bounded page; no marker means this queried ledger page is complete.
    ///
    /// # Errors
    /// Rejects excess entries, self-peers, duplicate peer/currency identities and
    /// a nonadvancing marker. Empty pages with a marker remain valid partial pages.
    pub fn new(
        account: Address,
        request: PageRequest,
        lines: Vec<TrustLine>,
        next: Option<Marker>,
    ) -> Result<Self, Error> {
        let mut identities = BTreeSet::new();
        if lines.len() > usize::from(request.limit())
            || lines.iter().any(|line| {
                line.peer() == account || !identities.insert((line.peer(), line.currency()))
            })
            || next
                .as_ref()
                .is_some_and(|marker| Some(marker) == request.marker())
        {
            return Err(invalid_record());
        }
        Ok(Self {
            account,
            request,
            lines,
            next,
        })
    }
    /// Returns the requested perspective account.
    #[must_use]
    pub const fn account(&self) -> Address {
        self.account
    }
    /// Returns the exact page request.
    #[must_use]
    pub const fn request(&self) -> &PageRequest {
        &self.request
    }
    /// Returns exact signed balances and trustline settings.
    #[must_use]
    pub fn lines(&self) -> &[TrustLine] {
        &self.lines
    }
    /// Returns continuation within the observation's actual ledger hash.
    #[must_use]
    pub const fn next_marker(&self) -> Option<&Marker> {
        self.next.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageValueFields {
    account: Address,
    request: PageRequest,
    lines: Vec<TrustLine>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    next: Option<Marker>,
}
impl TryFrom<PageValueFields> for TrustLinePage {
    type Error = Error;
    fn try_from(v: PageValueFields) -> Result<Self, Error> {
        Self::new(v.account, v.request, v.lines, v.next)
    }
}

/// Actual source fee suggestions in drops for an ordinary single-signed transaction.
///
/// Open-ledger values can change; these are neither a guarantee nor a fee ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeeEstimate {
    #[serde(rename = "base_fee")]
    base: Drops,
    #[serde(rename = "minimum_fee")]
    minimum: Drops,
    #[serde(rename = "median_fee")]
    median: Drops,
    #[serde(rename = "open_ledger_fee")]
    open_ledger: Drops,
}
impl FeeEstimate {
    /// Records the exact source-reported fee components without choosing a policy.
    #[must_use]
    pub const fn new(
        base_fee: Drops,
        minimum_fee: Drops,
        median_fee: Drops,
        open_ledger_fee: Drops,
    ) -> Self {
        Self {
            base: base_fee,
            minimum: minimum_fee,
            median: median_fee,
            open_ledger: open_ledger_fee,
        }
    }
    /// Returns the reference base fee.
    #[must_use]
    pub const fn base_fee(self) -> Drops {
        self.base
    }
    /// Returns the queue minimum fee.
    #[must_use]
    pub const fn minimum_fee(self) -> Drops {
        self.minimum
    }
    /// Returns the queue median fee.
    #[must_use]
    pub const fn median_fee(self) -> Drops {
        self.median
    }
    /// Returns the open-ledger fee suggestion.
    #[must_use]
    pub const fn open_ledger_fee(self) -> Drops {
        self.open_ledger
    }
}
