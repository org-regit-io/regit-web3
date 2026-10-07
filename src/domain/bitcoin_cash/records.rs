// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Address, Bytes, TokenCategory, Txid};
use crate::{domain::ExactDecimal, error::Error};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{SeqAccess, Visitor},
};
use std::{collections::HashSet, fmt};

pub(super) const MAX_ENTRIES: usize = 100_000;
pub(super) const MAX_MONEY: u64 = 2_100_000_000_000_000;

/// Exact BCH satoshis; one BCH is 100,000,000 satoshis.
/// This value type alone is not consensus or funding validation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Satoshis(u64);
impl Satoshis {
    /// Retains an exact nonnegative atomic quantity.
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }
    /// Returns exact BCH atomic units.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
    /// Parses a canonical unsigned decimal atomic quantity.
    /// # Errors
    /// Rejects signs, leading zeros, noninteger notation and overflow.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > 20
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(super::invalid());
        }
        value.parse().map(Self).map_err(|_| super::invalid())
    }
    /// Converts an exact BCH-denominated decimal into integral satoshis.
    /// # Errors
    /// Rejects negative, fractional-satoshi or overflowing quantities.
    pub fn from_bch(value: &ExactDecimal) -> Result<Self, Error> {
        if value.is_negative() {
            return Err(super::invalid());
        }
        let text = value.canonical();
        let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
        if fraction.len() > 8 {
            return Err(super::invalid());
        }
        let whole = whole.parse::<u64>().map_err(|_| super::invalid())?;
        let scale = whole.checked_mul(100_000_000).ok_or_else(super::invalid)?;
        let fraction = if fraction.is_empty() {
            0
        } else {
            format!("{fraction:0<8}")
                .parse::<u64>()
                .map_err(|_| super::invalid())?
        };
        scale
            .checked_add(fraction)
            .map(Self)
            .ok_or_else(super::invalid)
    }
}
impl fmt::Display for Satoshis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl Serialize for Satoshis {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for Satoshis {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Exact signed BCH unconfirmed delta, separate from confirmed atomic balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedSatoshis(i128);
impl SignedSatoshis {
    /// Retains a signed atomic delta whose magnitude fits an unsigned 64-bit quantity.
    /// # Errors
    /// Rejects excessive magnitude.
    pub fn new(value: i128) -> Result<Self, Error> {
        if value.unsigned_abs() > u128::from(u64::MAX) {
            return Err(super::invalid());
        }
        Ok(Self(value))
    }
    /// Returns exact signed BCH atomic units.
    #[must_use]
    pub const fn raw(self) -> i128 {
        self.0
    }
    /// Parses a canonical signed integer, excluding negative zero.
    /// # Errors
    /// Rejects malformed, noncanonical and excessive quantities.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let magnitude = value.strip_prefix('-').unwrap_or(value);
        let amount = Satoshis::parse(magnitude)?;
        if value.starts_with('-') && amount.raw() == 0 {
            return Err(super::invalid());
        }
        Self::new(if value.starts_with('-') {
            -i128::from(amount.raw())
        } else {
            i128::from(amount.raw())
        })
    }
}
impl fmt::Display for SignedSatoshis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl Serialize for SignedSatoshis {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for SignedSatoshis {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Explicit inclusion policy for BCH satoshis carried by token-containing UTXOs.
/// This filters native satoshis, not fungible token quantities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenFilter {
    /// Include both ordinary and token-containing BCH outputs.
    IncludeTokens,
    /// Exclude all token-containing outputs.
    ExcludeTokens,
    /// Include only BCH satoshis on token-containing outputs.
    TokensOnly,
}
impl TokenFilter {
    /// Returns the precise Electrum-Cash wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IncludeTokens => "include_tokens",
            Self::ExcludeTokens => "exclude_tokens",
            Self::TokensOnly => "tokens_only",
        }
    }
}

/// A caller-owned hard collection capacity; results are rejected rather than truncated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct CollectionLimit(u32);
impl CollectionLimit {
    /// Validates a positive capacity no larger than 100,000 entries.
    /// # Errors
    /// Rejects zero or excessive capacities.
    pub fn new(value: u32) -> Result<Self, Error> {
        if value == 0 || value > 100_000 {
            return Err(super::invalid());
        }
        Ok(Self(value))
    }
    /// Returns the hard maximum accepted entries.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl TryFrom<u32> for CollectionLimit {
    type Error = Error;
    fn try_from(v: u32) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<CollectionLimit> for u32 {
    fn from(v: CollectionLimit) -> Self {
        v.0
    }
}

/// Native BCH balance facts from one explicit token-filter query.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddressBalance {
    /// The exact requested compatible address.
    pub address: Address,
    /// Explicit BCH-output inclusion policy.
    pub token_filter: TokenFilter,
    /// Source confirmed native balance in satoshis.
    pub confirmed: Satoshis,
    /// Source signed unconfirmed delta, without synthesized final balance.
    pub unconfirmed: SignedSatoshis,
}

/// The explicit exclusive upper bound for a history query.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HistoryUpperBound {
    /// A finite exclusive height; mempool entries are excluded.
    Height {
        /// Exact exclusive upper height.
        height: u32,
    },
    /// Query through the changing source tip and append source mempool records.
    OpenTip,
}

/// An explicit inclusive-lower/exclusive-upper height interval and hard capacity.
/// Height ranges do not establish a common source snapshot or implicit pagination.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RangeFields")]
pub struct HistoryRange {
    from_height: u32,
    to: HistoryUpperBound,
    limit: CollectionLimit,
}
impl HistoryRange {
    /// Records exact height bounds and maximum entries, accepting an empty finite interval.
    /// # Errors
    /// Rejects a lower height greater than a finite exclusive upper bound.
    pub fn new(
        from_height: u32,
        to: HistoryUpperBound,
        limit: CollectionLimit,
    ) -> Result<Self, Error> {
        if matches!(to,HistoryUpperBound::Height{height} if from_height>height) {
            return Err(super::invalid());
        }
        Ok(Self {
            from_height,
            to,
            limit,
        })
    }
    /// Returns the inclusive lower height.
    #[must_use]
    pub const fn from_height(self) -> u32 {
        self.from_height
    }
    /// Returns the exact upper policy.
    #[must_use]
    pub const fn upper(self) -> HistoryUpperBound {
        self.to
    }
    /// Returns the explicit hard result capacity.
    #[must_use]
    pub const fn limit(self) -> CollectionLimit {
        self.limit
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RangeFields {
    from_height: u32,
    to: HistoryUpperBound,
    limit: CollectionLimit,
}
impl TryFrom<RangeFields> for HistoryRange {
    type Error = Error;
    fn try_from(v: RangeFields) -> Result<Self, Error> {
        Self::new(v.from_height, v.to, v.limit)
    }
}

/// Source history classification under Electrum-Cash protocol 1.6.
/// These are indexed source claims, not independent inclusion or mempool proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HistoryState {
    /// Source reports a positive confirmed block height.
    Confirmed {
        /// Exact source-reported height, validated positive by the enclosing entry.
        height: u32,
    },
    /// Source reports zero, documented as mempool with confirmed parents.
    ZeroHeight,
    /// Source reports minus one, documented as having unconfirmed parents.
    UnconfirmedParents,
}

/// One exact source history identity, height classification and optional native fee.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EntryFields")]
pub struct HistoryEntry {
    txid: Txid,
    state: HistoryState,
    fee: Option<Satoshis>,
}
impl HistoryEntry {
    /// Validates the protocol's height/fee shape without inventing source status.
    /// # Errors
    /// Requires positive confirmed heights and a fee for each mempool entry.
    pub fn new(txid: Txid, state: HistoryState, fee: Option<Satoshis>) -> Result<Self, Error> {
        match state {
            HistoryState::Confirmed { height } if height == 0 || fee.is_some() => {
                return Err(super::invalid());
            }
            HistoryState::ZeroHeight | HistoryState::UnconfirmedParents if fee.is_none() => {
                return Err(super::invalid());
            }
            _ => {}
        }
        Ok(Self { txid, state, fee })
    }
    /// Returns the exact transaction identity.
    #[must_use]
    pub const fn txid(&self) -> Txid {
        self.txid
    }
    /// Returns the source height classification.
    #[must_use]
    pub const fn state(&self) -> HistoryState {
        self.state
    }
    /// Returns source fee when the method actually supplied it.
    #[must_use]
    pub const fn fee(&self) -> Option<Satoshis> {
        self.fee
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryFields {
    txid: Txid,
    state: HistoryState,
    fee: Option<Satoshis>,
}
impl TryFrom<EntryFields> for HistoryEntry {
    type Error = Error;
    fn try_from(v: EntryFields) -> Result<Self, Error> {
        Self::new(v.txid, v.state, v.fee)
    }
}

/// Complete source records for exactly one explicit bounded history interval.
/// No additional pages, exhaustion claim or fallback interval are synthesized.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryFields")]
pub struct History {
    address: Address,
    range: HistoryRange,
    entries: Vec<HistoryEntry>,
}
impl History {
    /// Validates exact identities, interval membership, order and caller capacity.
    /// # Errors
    /// Rejects duplicates, out-of-range records, mixed confirmed/mempool ordering and overflow.
    pub fn new(
        address: Address,
        range: HistoryRange,
        entries: Vec<HistoryEntry>,
    ) -> Result<Self, Error> {
        if entries.len() > range.limit.get() as usize {
            return Err(super::invalid());
        }
        let mut ids = HashSet::new();
        let mut last_height = 0;
        let mut mempool = false;
        for entry in &entries {
            if !ids.insert(entry.txid) {
                return Err(super::invalid());
            }
            match entry.state {
                HistoryState::Confirmed { height } => {
                    if mempool
                        || height < last_height
                        || height < range.from_height
                        || matches!(range.to,HistoryUpperBound::Height{height:upper} if height>=upper)
                    {
                        return Err(super::invalid());
                    }
                    last_height = height;
                }
                HistoryState::ZeroHeight | HistoryState::UnconfirmedParents => {
                    if !matches!(range.to, HistoryUpperBound::OpenTip) {
                        return Err(super::invalid());
                    }
                    mempool = true;
                }
            }
        }
        Ok(Self {
            address,
            range,
            entries,
        })
    }
    /// Returns the exact compatible query address.
    #[must_use]
    pub const fn address(&self) -> &Address {
        &self.address
    }
    /// Returns the exact caller interval and capacity.
    #[must_use]
    pub const fn range(&self) -> HistoryRange {
        self.range
    }
    /// Returns all source entries in retained source order.
    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryFields {
    address: Address,
    range: HistoryRange,
    #[serde(deserialize_with = "bounded_entries")]
    entries: Vec<HistoryEntry>,
}
impl TryFrom<HistoryFields> for History {
    type Error = Error;
    fn try_from(v: HistoryFields) -> Result<Self, Error> {
        Self::new(v.address, v.range, v.entries)
    }
}

/// Positive caller-owned confirmation target for a source estimate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct FeeTarget(u32);
impl FeeTarget {
    /// Validates a positive target count, with no guarantee of source policy or confirmation.
    /// # Errors
    /// Rejects zero.
    pub fn new(value: u32) -> Result<Self, Error> {
        if value == 0 {
            return Err(super::invalid());
        }
        Ok(Self(value))
    }
    /// Returns the exact requested number of blocks.
    #[must_use]
    pub const fn blocks(self) -> u32 {
        self.0
    }
}
impl TryFrom<u32> for FeeTarget {
    type Error = Error;
    fn try_from(v: u32) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<FeeTarget> for u32 {
    fn from(v: FeeTarget) -> Self {
        v.0
    }
}

/// Exact source estimated BCH per 1,000 serialized bytes, or documented unavailable sentinel.
/// A requested confirmation target does not establish a confirmation guarantee.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FeeFields")]
pub struct FeeEstimate {
    target: FeeTarget,
    bch_per_kilobyte: Option<ExactDecimal>,
}
impl FeeEstimate {
    /// Records an exact nonnegative BCH/kB estimate or source-unavailable result.
    /// # Errors
    /// Rejects negative decimal estimates rather than treating them as zero.
    pub fn new(target: FeeTarget, bch_per_kilobyte: Option<ExactDecimal>) -> Result<Self, Error> {
        if bch_per_kilobyte
            .as_ref()
            .is_some_and(ExactDecimal::is_negative)
        {
            return Err(super::invalid());
        }
        Ok(Self {
            target,
            bch_per_kilobyte,
        })
    }
    /// Returns the exact requested confirmation target.
    #[must_use]
    pub const fn target(&self) -> FeeTarget {
        self.target
    }
    /// Returns an exact BCH/1000-byte decimal when the source supplied an estimate.
    #[must_use]
    pub const fn bch_per_kilobyte(&self) -> Option<&ExactDecimal> {
        self.bch_per_kilobyte.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeeFields {
    target: FeeTarget,
    bch_per_kilobyte: Option<ExactDecimal>,
}
impl TryFrom<FeeFields> for FeeEstimate {
    type Error = Error;
    fn try_from(v: FeeFields) -> Result<Self, Error> {
        Self::new(v.target, v.bch_per_kilobyte)
    }
}

/// Source NFT capability under the `CashTokens` specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NftCapability {
    /// Immutable NFT, with no mutation or minting capability.
    None,
    /// Source NFT permits commitment mutation.
    Mutable,
    /// Source NFT permits minting.
    Minting,
}

/// Retained NFT capability and exact commitment bytes, without ownership proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NftFields")]
pub struct Nft {
    capability: NftCapability,
    commitment: Bytes,
}
impl Nft {
    /// Records a source NFT with an exact zero-to-40-byte commitment.
    /// # Errors
    /// Rejects commitments beyond the `CashTokens` 40-byte bound.
    pub fn new(capability: NftCapability, commitment: Bytes) -> Result<Self, Error> {
        if commitment.as_slice().len() > 40 {
            return Err(super::invalid());
        }
        Ok(Self {
            capability,
            commitment,
        })
    }
    /// Returns the source NFT capability.
    #[must_use]
    pub const fn capability(&self) -> NftCapability {
        self.capability
    }
    /// Returns the exact commitment in source/on-chain order.
    #[must_use]
    pub const fn commitment(&self) -> &Bytes {
        &self.commitment
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NftFields {
    capability: NftCapability,
    commitment: Bytes,
}
impl TryFrom<NftFields> for Nft {
    type Error = Error;
    fn try_from(v: NftFields) -> Result<Self, Error> {
        Self::new(v.capability, v.commitment)
    }
}

/// Exact fungible `CashToken` quantity, distinct from native BCH satoshis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAmount(u64);
impl TokenAmount {
    /// Retains an exact quantity bounded by the `CashTokens` signed-63-bit maximum.
    /// # Errors
    /// Rejects values greater than 9,223,372,036,854,775,807.
    pub fn new(value: u64) -> Result<Self, Error> {
        if value > i64::MAX as u64 {
            return Err(super::invalid());
        }
        Ok(Self(value))
    }
    /// Returns the exact fungible token quantity, without inferred decimal metadata.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
    /// Parses canonical unsigned decimal tokens, without floating-point conversion.
    /// # Errors
    /// Rejects malformed, noncanonical or excessive quantities.
    pub fn parse(value: &str) -> Result<Self, Error> {
        Self::new(Satoshis::parse(value)?.raw())
    }
}
impl Serialize for TokenAmount {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for TokenAmount {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Exact source `CashToken` category, fungible quantity and optional NFT facts.
/// This does not establish token genesis validity, authority or ownership.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TokenFields")]
pub struct TokenData {
    category: TokenCategory,
    amount: TokenAmount,
    nft: Option<Nft>,
}
impl TokenData {
    /// Records exact token facts with the specification's signed-63-bit quantity bound.
    /// # Errors
    /// Rejects excessive quantities and an empty token entry lacking both tokens and an NFT.
    pub fn new(
        category: TokenCategory,
        amount: TokenAmount,
        nft: Option<Nft>,
    ) -> Result<Self, Error> {
        if amount.raw() > i64::MAX as u64 || (amount.raw() == 0 && nft.is_none()) {
            return Err(super::invalid());
        }
        Ok(Self {
            category,
            amount,
            nft,
        })
    }
    /// Returns the complete category identifier.
    #[must_use]
    pub const fn category(&self) -> TokenCategory {
        self.category
    }
    /// Returns the exact fungible token quantity; its units are tokens, not BCH satoshis.
    #[must_use]
    pub const fn amount(&self) -> u64 {
        self.amount.raw()
    }
    /// Returns source NFT facts, if supplied.
    #[must_use]
    pub const fn nft(&self) -> Option<&Nft> {
        self.nft.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenFields {
    category: TokenCategory,
    amount: TokenAmount,
    nft: Option<Nft>,
}
impl TryFrom<TokenFields> for TokenData {
    type Error = Error;
    fn try_from(v: TokenFields) -> Result<Self, Error> {
        Self::new(v.category, v.amount, v.nft)
    }
}

/// One complete indexed BCH UTXO, including source `CashToken` data when supplied.
/// The source's zero height is retained separately from confirmed positive heights.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnspentOutput {
    /// Exact source transaction identity.
    pub txid: Txid,
    /// Actual transaction output index.
    pub output_index: u32,
    /// Positive source confirmed height, or zero source mempool classification.
    pub height: u32,
    /// Exact native BCH satoshis on this output.
    pub value: Satoshis,
    /// `CashToken` facts supplied by a server explicitly verified as token-aware.
    pub token_data: Option<TokenData>,
}

/// Complete bounded source UTXOs for one explicit address/filter query.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "UnspentFields")]
pub struct UnspentOutputs {
    address: Address,
    token_filter: TokenFilter,
    limit: CollectionLimit,
    outputs: Vec<UnspentOutput>,
}
impl UnspentOutputs {
    /// Validates source output identity, value, ordering, filter and capacity.
    /// # Errors
    /// Rejects duplicates, incompatible token-filter facts, excessive values or collection size.
    pub fn new(
        address: Address,
        token_filter: TokenFilter,
        limit: CollectionLimit,
        outputs: Vec<UnspentOutput>,
    ) -> Result<Self, Error> {
        if outputs.len() > limit.get() as usize {
            return Err(super::invalid());
        }
        let mut ids = HashSet::new();
        let mut last = 0;
        let mut mempool = false;
        for output in &outputs {
            if !ids.insert((output.txid, output.output_index)) || output.value.raw() > MAX_MONEY {
                return Err(super::invalid());
            }
            if matches!(token_filter, TokenFilter::ExcludeTokens) && output.token_data.is_some()
                || matches!(token_filter, TokenFilter::TokensOnly) && output.token_data.is_none()
            {
                return Err(super::invalid());
            }
            if output.height == 0 {
                mempool = true;
            } else {
                if mempool || output.height < last {
                    return Err(super::invalid());
                }
                last = output.height;
            }
        }
        Ok(Self {
            address,
            token_filter,
            limit,
            outputs,
        })
    }
    /// Returns the exact query address.
    #[must_use]
    pub const fn address(&self) -> &Address {
        &self.address
    }
    /// Returns the explicit BCH-output inclusion policy.
    #[must_use]
    pub const fn token_filter(&self) -> TokenFilter {
        self.token_filter
    }
    /// Returns the caller hard capacity.
    #[must_use]
    pub const fn limit(&self) -> CollectionLimit {
        self.limit
    }
    /// Returns all retained source outputs without truncation.
    #[must_use]
    pub fn outputs(&self) -> &[UnspentOutput] {
        &self.outputs
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnspentFields {
    address: Address,
    token_filter: TokenFilter,
    limit: CollectionLimit,
    #[serde(deserialize_with = "bounded_entries")]
    outputs: Vec<UnspentOutput>,
}
impl TryFrom<UnspentFields> for UnspentOutputs {
    type Error = Error;
    fn try_from(v: UnspentFields) -> Result<Self, Error> {
        Self::new(v.address, v.token_filter, v.limit, v.outputs)
    }
}

pub(crate) fn bounded_entries<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    bounded_collection::<D, T, MAX_ENTRIES>(d)
}

pub(crate) fn bounded_addresses<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    bounded_collection::<D, T, 100>(d)
}

fn bounded_collection<'de, D, T, const MAX: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Items<T, const MAX: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const MAX: usize> Visitor<'de> for Items<T, MAX> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a bounded collection")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut values = Vec::new();
            while let Some(item) = seq.next_element()? {
                if values.len() == MAX {
                    return Err(serde::de::Error::custom("collection exceeds limit"));
                }
                values.push(item);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Items::<T, MAX>(std::marker::PhantomData))
}
