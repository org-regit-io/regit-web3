// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        ExactDecimal,
        bitcoin::{Satoshis, Txid},
    },
    error::{Error, ValidationError},
};

fn invalid() -> Error {
    ValidationError::InvalidMempoolRecord.into()
}

/// An exact nonnegative source fee rate in satoshis per virtual byte.
/// Suggestions are not a confirmation-time guarantee or a transaction fee.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "ExactDecimal", into = "ExactDecimal")]
pub struct FeeRate(ExactDecimal);
impl FeeRate {
    /// Requires a finite exact zero or positive sat/vB value.
    /// # Errors
    /// Rejects negative rates without rounding or input-bearing diagnostics.
    pub fn new(value: ExactDecimal) -> Result<Self, Error> {
        if value.is_negative() {
            return Err(invalid());
        }
        Ok(Self(value))
    }
    /// Parses exact decimal notation in sat/vB, without floating point.
    /// # Errors
    /// Rejects malformed, negative or resource-exceeding values.
    pub fn parse(value: &str) -> Result<Self, Error> {
        Self::new(ExactDecimal::parse(value).map_err(|_| invalid())?)
    }
    /// Returns the exact source fee rate; serde uses a decimal string.
    #[must_use]
    pub const fn value(&self) -> &ExactDecimal {
        &self.0
    }
}
impl TryFrom<ExactDecimal> for FeeRate {
    type Error = Error;
    fn try_from(value: ExactDecimal) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<FeeRate> for ExactDecimal {
    fn from(value: FeeRate) -> Self {
        value.0
    }
}

/// A positive source-reported transaction size in virtual bytes.
/// Some sources report fractional weight/4; quarter-byte increments are retained,
/// never rounded. An integer ceiling reported by another source also remains exact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ExactDecimal", into = "ExactDecimal")]
pub struct VirtualSize(ExactDecimal);
impl VirtualSize {
    /// Requires a positive size within the four-million-weight-unit block bound.
    /// # Errors
    /// Rejects zero, negative, non-quarter increments or more than one million virtual bytes.
    pub fn new(value: ExactDecimal) -> Result<Self, Error> {
        let quarter_increment = value
            .canonical()
            .split_once('.')
            .is_none_or(|(_, fraction)| matches!(fraction, "25" | "5" | "75"));
        if value.is_negative()
            || value.is_zero()
            || !quarter_increment
            || value > ExactDecimal::parse("1000000")?
        {
            return Err(invalid());
        }
        Ok(Self(value))
    }
    /// Returns exact virtual bytes, including a supplied fractional value.
    #[must_use]
    pub const fn value(&self) -> &ExactDecimal {
        &self.0
    }
}
impl TryFrom<ExactDecimal> for VirtualSize {
    type Error = Error;
    fn try_from(value: ExactDecimal) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<VirtualSize> for ExactDecimal {
    fn from(value: VirtualSize) -> Self {
        value.0
    }
}

/// One fee-distribution bin with an exact lower fee-rate boundary and bin vbytes.
/// Vbytes belong to this bin, not a cumulative total. The first bin has no upper
/// boundary; later bins use the previous bin's lower boundary as their upper bound.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeeHistogramBin {
    lower_rate: FeeRate,
    virtual_bytes: u64,
}
impl FeeHistogramBin {
    /// Records exact source boundaries and the individual bin's virtual-byte count.
    #[must_use]
    pub const fn new(lower_rate: FeeRate, virtual_bytes: u64) -> Self {
        Self {
            lower_rate,
            virtual_bytes,
        }
    }
    /// Returns the source-reported lower rate in sat/vB.
    #[must_use]
    pub const fn lower_rate(&self) -> &FeeRate {
        &self.lower_rate
    }
    /// Returns this bin's virtual-byte total, not a cumulative quantity.
    #[must_use]
    pub const fn virtual_bytes(&self) -> u64 {
        self.virtual_bytes
    }
}

/// Source-reported backlog totals and a possibly empty or partial fee histogram.
/// An empty histogram does not mean an empty mempool.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SummaryFields")]
pub struct MempoolSummary {
    transaction_count: u64,
    virtual_bytes: u64,
    total_fee: Satoshis,
    histogram: Vec<FeeHistogramBin>,
}
impl MempoolSummary {
    /// Hard collection bound independent of configured HTTP response bytes.
    pub const MAX_HISTOGRAM_BINS: usize = 100_000;
    /// Records source totals and strictly descending individual histogram bins.
    /// # Errors
    /// Rejects excessive bins, duplicate/ascending rates, overflowing or excess
    /// bin totals, and a zero transaction count with nonzero totals/bins.
    pub fn new(
        transaction_count: u64,
        virtual_bytes: u64,
        total_fee: Satoshis,
        histogram: Vec<FeeHistogramBin>,
    ) -> Result<Self, Error> {
        let sum = histogram
            .iter()
            .try_fold(0_u64, |sum, bin| sum.checked_add(bin.virtual_bytes()));
        if histogram.len() > Self::MAX_HISTOGRAM_BINS
            || histogram
                .windows(2)
                .any(|bins| bins[0].lower_rate() <= bins[1].lower_rate())
            || sum.is_none_or(|sum| sum > virtual_bytes)
            || (transaction_count == 0
                && (virtual_bytes != 0 || total_fee.raw() != 0 || !histogram.is_empty()))
            || (transaction_count != 0 && virtual_bytes == 0)
        {
            return Err(invalid());
        }
        Ok(Self {
            transaction_count,
            virtual_bytes,
            total_fee,
            histogram,
        })
    }
    /// Returns the source's mempool transaction count at this retrieval.
    #[must_use]
    pub const fn transaction_count(&self) -> u64 {
        self.transaction_count
    }
    /// Returns total virtual bytes reported by the source.
    #[must_use]
    pub const fn virtual_bytes(&self) -> u64 {
        self.virtual_bytes
    }
    /// Returns the exact aggregate fees in satoshis.
    #[must_use]
    pub const fn total_fee(&self) -> Satoshis {
        self.total_fee
    }
    /// Returns the source's individual bins, which may cover only part of the pool.
    #[must_use]
    pub fn histogram(&self) -> &[FeeHistogramBin] {
        &self.histogram
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SummaryFields {
    transaction_count: u64,
    virtual_bytes: u64,
    total_fee: Satoshis,
    histogram: Vec<FeeHistogramBin>,
}
impl TryFrom<SummaryFields> for MempoolSummary {
    type Error = Error;
    fn try_from(v: SummaryFields) -> Result<Self, Error> {
        Self::new(
            v.transaction_count,
            v.virtual_bytes,
            v.total_fee,
            v.histogram,
        )
    }
}

/// A simplified source-reported transaction that recently entered its mempool.
/// No raw transaction, membership proof or continuing unconfirmed state is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RecentFields")]
pub struct RecentTransaction {
    txid: Txid,
    fee: Satoshis,
    virtual_size: VirtualSize,
    output_value: Satoshis,
}
impl RecentTransaction {
    /// Records identity, fee, source virtual size and sum of outputs in satoshis.
    /// # Errors
    /// Rejects an output-plus-fee total above Bitcoin's maximum money range.
    pub fn new(
        txid: Txid,
        fee: Satoshis,
        virtual_size: VirtualSize,
        output_value: Satoshis,
    ) -> Result<Self, Error> {
        if output_value
            .raw()
            .checked_add(fee.raw())
            .is_none_or(|sum| sum > bitcoin::Amount::MAX_MONEY.to_sat())
        {
            return Err(invalid());
        }
        Ok(Self {
            txid,
            fee,
            virtual_size,
            output_value,
        })
    }
    /// Returns the source-reported transaction ID.
    #[must_use]
    pub const fn txid(&self) -> Txid {
        self.txid
    }
    /// Returns the transaction's source-reported fee in satoshis.
    #[must_use]
    pub const fn fee(&self) -> Satoshis {
        self.fee
    }
    /// Returns exact source virtual bytes, without rounding weight/4.
    #[must_use]
    pub const fn virtual_size(&self) -> &VirtualSize {
        &self.virtual_size
    }
    /// Returns the source-reported total value of outputs in satoshis.
    #[must_use]
    pub const fn output_value(&self) -> Satoshis {
        self.output_value
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecentFields {
    txid: Txid,
    fee: Satoshis,
    virtual_size: VirtualSize,
    output_value: Satoshis,
}
impl TryFrom<RecentFields> for RecentTransaction {
    type Error = Error;
    fn try_from(v: RecentFields) -> Result<Self, Error> {
        Self::new(v.txid, v.fee, v.virtual_size, v.output_value)
    }
}

/// The source's last at most ten arrivals, never an exhaustive mempool listing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<RecentTransaction>", into = "Vec<RecentTransaction>")]
pub struct RecentTransactions(Vec<RecentTransaction>);
impl RecentTransactions {
    /// Maximum number of entries supported by the documented recent endpoint.
    pub const MAX_ENTRIES: usize = 10;
    /// Retains supplied order without inventing arrival timestamps.
    /// # Errors
    /// Rejects more than ten entries or duplicate transaction IDs.
    pub fn new(values: Vec<RecentTransaction>) -> Result<Self, Error> {
        let mut ids = BTreeSet::new();
        if values.len() > Self::MAX_ENTRIES || values.iter().any(|value| !ids.insert(value.txid()))
        {
            return Err(invalid());
        }
        Ok(Self(values))
    }
    /// Returns source order; an empty list remains empty.
    #[must_use]
    pub fn transactions(&self) -> &[RecentTransaction] {
        &self.0
    }
}
impl TryFrom<Vec<RecentTransaction>> for RecentTransactions {
    type Error = Error;
    fn try_from(v: Vec<RecentTransaction>) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<RecentTransactions> for Vec<RecentTransaction> {
    fn from(v: RecentTransactions) -> Self {
        v.0
    }
}

/// An explicit local bound for a full, unpaged mempool transaction-ID response.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct TransactionLimit(u32);
impl TransactionLimit {
    /// Largest accepted transaction-ID collection; exceedance fails wholly.
    pub const MAXIMUM: u32 = 100_000;
    /// Records an explicit bound in 1..=100000, without changing the endpoint.
    /// # Errors
    /// Rejects zero or an excessive bound.
    pub fn new(value: u32) -> Result<Self, Error> {
        if value == 0 || value > Self::MAXIMUM {
            return Err(ValidationError::InvalidPageRequest.into());
        }
        Ok(Self(value))
    }
    /// Returns the maximum entries the caller permits in this response.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl TryFrom<u32> for TransactionLimit {
    type Error = Error;
    fn try_from(v: u32) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<TransactionLimit> for u32 {
    fn from(v: TransactionLimit) -> Self {
        v.0
    }
}

/// A bounded full source mempool ID list in arbitrary source order.
/// No pagination, sorted order or atomic agreement with another read is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IdsFields")]
pub struct TransactionIds {
    limit: TransactionLimit,
    txids: Vec<Txid>,
}
impl TransactionIds {
    /// Retains every returned ID or rejects the response wholly at the supplied cap.
    /// # Errors
    /// Rejects too many IDs or duplicates; never truncates or manufactures a cursor.
    pub fn new(limit: TransactionLimit, txids: Vec<Txid>) -> Result<Self, Error> {
        let mut ids = BTreeSet::new();
        if txids.len() > limit.get() as usize || txids.iter().any(|id| !ids.insert(*id)) {
            return Err(invalid());
        }
        Ok(Self { limit, txids })
    }
    /// Returns the caller's explicit local collection cap.
    #[must_use]
    pub const fn limit(&self) -> TransactionLimit {
        self.limit
    }
    /// Returns every source ID in its original arbitrary order.
    #[must_use]
    pub fn txids(&self) -> &[Txid] {
        &self.txids
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdsFields {
    limit: TransactionLimit,
    txids: Vec<Txid>,
}
impl TryFrom<IdsFields> for TransactionIds {
    type Error = Error;
    fn try_from(v: IdsFields) -> Result<Self, Error> {
        Self::new(v.limit, v.txids)
    }
}

/// Five exact sat/vB suggestions, retaining the source's named time/economy classes.
/// No recommendation ordering or confirmation-time promise is inferred.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecommendedFees {
    fastest: FeeRate,
    half_hour: FeeRate,
    hour: FeeRate,
    economy: FeeRate,
    minimum: FeeRate,
}
impl RecommendedFees {
    /// Records the source's five independent nonnegative suggestions.
    #[must_use]
    pub const fn new(
        fastest: FeeRate,
        half_hour: FeeRate,
        hour: FeeRate,
        economy: FeeRate,
        minimum: FeeRate,
    ) -> Self {
        Self {
            fastest,
            half_hour,
            hour,
            economy,
            minimum,
        }
    }
    /// Returns the source's fastest-confirmation suggestion in sat/vB.
    #[must_use]
    pub const fn fastest(&self) -> &FeeRate {
        &self.fastest
    }
    /// Returns the source's half-hour suggestion in sat/vB.
    #[must_use]
    pub const fn half_hour(&self) -> &FeeRate {
        &self.half_hour
    }
    /// Returns the source's hour suggestion in sat/vB.
    #[must_use]
    pub const fn hour(&self) -> &FeeRate {
        &self.hour
    }
    /// Returns the source's economy suggestion in sat/vB.
    #[must_use]
    pub const fn economy(&self) -> &FeeRate {
        &self.economy
    }
    /// Returns the source's minimum recommendation in sat/vB, not an acceptance guarantee.
    #[must_use]
    pub const fn minimum(&self) -> &FeeRate {
        &self.minimum
    }
}
