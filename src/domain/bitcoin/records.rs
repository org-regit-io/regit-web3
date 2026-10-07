// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Address, BlockHash, Txid};
use crate::{
    domain::{Amount, ExactDecimal, Timestamp},
    error::{Error, ValidationError},
};

/// Exact unsigned satoshis, serialized as a decimal string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Satoshis(u64);
impl Satoshis {
    /// Records an exact unsigned base-unit value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Parses canonical unsigned decimal base units.
    ///
    /// # Errors
    /// Rejects invalid notation and values exceeding 64 bits.
    pub fn from_decimal(value: &str) -> Result<Self, Error> {
        let amount = Amount::from_decimal(value, Some(8))?;
        u64::try_from(amount.raw())
            .map(Self)
            .map_err(|_| ValidationError::BitcoinAmountOverflow.into())
    }
    /// Returns exact satoshis.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
    /// Returns the shared exact amount with Bitcoin's eight decimal places.
    #[must_use]
    pub fn amount(self) -> Amount {
        Amount::new(crate::domain::U256::from(self.0), Some(8))
    }
}
impl Serialize for Satoshis {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for Satoshis {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_decimal(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Signed mempool funding-minus-spending delta, retained separately from confirmed funds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MempoolDelta(i128);
impl MempoolDelta {
    /// Computes the exact difference of unsigned 64-bit funding and spending sums.
    #[must_use]
    pub fn from_sums(funded: u64, spent: u64) -> Self {
        Self(i128::from(funded) - i128::from(spent))
    }
    /// Returns the signed exact satoshi delta.
    #[must_use]
    pub const fn raw(self) -> i128 {
        self.0
    }
}
impl Serialize for MempoolDelta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for MempoolDelta {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let negative = text.starts_with('-');
        let digits = text.strip_prefix('-').unwrap_or(&text);
        let absolute = Satoshis::from_decimal(digits)
            .map_err(serde::de::Error::custom)?
            .raw();
        if negative && absolute == 0 {
            return Err(serde::de::Error::custom("invalid signed satoshi value"));
        }
        let value = i128::from(absolute);
        Ok(Self(if negative { -value } else { value }))
    }
}

/// Confirmed address funds and a separate, possibly negative, mempool delta.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddressBalance {
    address: Address,
    confirmed: Satoshis,
    mempool_delta: MempoolDelta,
    confirmed_tx_count: u64,
    mempool_tx_count: u64,
}
impl AddressBalance {
    /// Constructs a balance from the index's exact funding and spending sums.
    ///
    /// # Errors
    /// Rejects confirmed spending exceeding confirmed funding.
    pub fn from_stats(
        address: Address,
        funded: u64,
        spent: u64,
        mempool_funded: u64,
        mempool_spent: u64,
        confirmed_tx_count: u64,
        mempool_tx_count: u64,
    ) -> Result<Self, Error> {
        let confirmed = funded
            .checked_sub(spent)
            .ok_or(ValidationError::BitcoinBalanceInconsistent)?;
        Ok(Self {
            address,
            confirmed: Satoshis::new(confirmed),
            mempool_delta: MempoolDelta::from_sums(mempool_funded, mempool_spent),
            confirmed_tx_count,
            mempool_tx_count,
        })
    }
    /// Returns the requested, explicitly qualified address.
    #[must_use]
    pub const fn address(&self) -> &Address {
        &self.address
    }
    /// Returns confirmed indexed funds.
    #[must_use]
    pub const fn confirmed(&self) -> Satoshis {
        self.confirmed
    }
    /// Returns the separate exact mempool delta.
    #[must_use]
    pub const fn mempool_delta(&self) -> MempoolDelta {
        self.mempool_delta
    }
    /// Returns the source's confirmed transaction count.
    #[must_use]
    pub const fn confirmed_tx_count(&self) -> u64 {
        self.confirmed_tx_count
    }
    /// Returns the source's mempool transaction count.
    #[must_use]
    pub const fn mempool_tx_count(&self) -> u64 {
        self.mempool_tx_count
    }
}

/// Source-reported transaction inclusion without inferred confirmation count or finality.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockReference {
    height: u64,
    hash: BlockHash,
    timestamp: Timestamp,
}
impl BlockReference {
    /// Records an explicit reported inclusion block.
    #[must_use]
    pub const fn new(height: u64, hash: BlockHash, timestamp: Timestamp) -> Self {
        Self {
            height,
            hash,
            timestamp,
        }
    }
    /// Returns the reported inclusion height.
    #[must_use]
    pub const fn height(self) -> u64 {
        self.height
    }
    /// Returns the reported inclusion hash.
    #[must_use]
    pub const fn hash(self) -> BlockHash {
        self.hash
    }
    /// Returns the reported whole Unix-second block time.
    #[must_use]
    pub const fn timestamp(self) -> Timestamp {
        self.timestamp
    }
}

/// An unconfirmed transaction or complete source-reported inclusion.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "block",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TransactionStatus {
    /// The source reports the transaction without confirmed inclusion.
    Unconfirmed,
    /// The source reports complete confirmed inclusion fields.
    Confirmed(BlockReference),
}

/// A typed request for a bounded address-history page.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HistoryCursor {
    /// Up to fifty mempool entries together with the first twenty-five confirmed entries.
    Recent,
    /// Up to twenty-five confirmed entries following an optional last-seen transaction.
    Confirmed {
        /// The previous confirmed page's final transaction identifier.
        after: Option<Txid>,
    },
}

/// Observed page completeness under the source's documented result limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    /// The returned count is below the applicable source limit.
    Complete,
    /// The source limit was reached; additional entries may exist.
    MayHaveMore,
}

/// A transaction's exact fee and source-reported status within indexed history.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    txid: Txid,
    fee: Satoshis,
    status: TransactionStatus,
}
impl HistoryEntry {
    /// Records a transaction identifier, exact fee, and source-reported status.
    #[must_use]
    pub const fn new(txid: Txid, fee: Satoshis, status: TransactionStatus) -> Self {
        Self { txid, fee, status }
    }
    /// Returns the transaction identifier.
    #[must_use]
    pub const fn txid(&self) -> Txid {
        self.txid
    }
    /// Returns exact fee satoshis.
    #[must_use]
    pub const fn fee(&self) -> Satoshis {
        self.fee
    }
    /// Returns the reported transaction status.
    #[must_use]
    pub const fn status(&self) -> TransactionStatus {
        self.status
    }
}

/// A bounded history chunk with separate confirmed and mempool completeness.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryFields")]
pub struct HistoryPage {
    address: Address,
    cursor: HistoryCursor,
    entries: Vec<HistoryEntry>,
}
impl HistoryPage {
    /// Validates source limits, cursor semantics and unique transaction identities.
    ///
    /// # Errors
    /// Rejects duplicate entries, more than twenty-five confirmed or fifty
    /// mempool entries, or mempool entries in a confirmed-only request.
    pub fn new(
        address: Address,
        cursor: HistoryCursor,
        entries: Vec<HistoryEntry>,
    ) -> Result<Self, Error> {
        let mut seen = BTreeSet::new();
        let mut confirmed = 0;
        let mut mempool = 0;
        for entry in &entries {
            if !seen.insert(entry.txid) {
                return Err(ValidationError::InvalidBitcoinHistory.into());
            }
            match entry.status {
                TransactionStatus::Confirmed(_) => confirmed += 1,
                TransactionStatus::Unconfirmed => mempool += 1,
            }
        }
        if confirmed > 25
            || mempool > 50
            || (matches!(cursor, HistoryCursor::Confirmed { .. }) && mempool != 0)
        {
            return Err(ValidationError::InvalidBitcoinHistory.into());
        }
        if let HistoryCursor::Confirmed { after: Some(after) } = cursor
            && seen.contains(&after)
        {
            return Err(ValidationError::InvalidBitcoinHistory.into());
        }
        Ok(Self {
            address,
            cursor,
            entries,
        })
    }
    /// Returns the requested qualified address.
    #[must_use]
    pub const fn address(&self) -> &Address {
        &self.address
    }
    /// Returns the exact requested cursor.
    #[must_use]
    pub const fn cursor(&self) -> HistoryCursor {
        self.cursor
    }
    /// Returns the bounded source entries.
    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }
    /// Returns confirmed completeness under the twenty-five-entry limit.
    #[must_use]
    pub fn confirmed_completeness(&self) -> Completeness {
        if self
            .entries
            .iter()
            .filter(|entry| matches!(entry.status, TransactionStatus::Confirmed(_)))
            .count()
            == 25
        {
            Completeness::MayHaveMore
        } else {
            Completeness::Complete
        }
    }
    /// Returns mempool completeness for a recent request, otherwise absence.
    #[must_use]
    pub fn mempool_completeness(&self) -> Option<Completeness> {
        matches!(self.cursor, HistoryCursor::Recent).then(|| {
            if self
                .entries
                .iter()
                .filter(|entry| entry.status == TransactionStatus::Unconfirmed)
                .count()
                == 50
            {
                Completeness::MayHaveMore
            } else {
                Completeness::Complete
            }
        })
    }
    /// Returns the next confirmed-only cursor when the confirmed limit was reached.
    #[must_use]
    pub fn next_cursor(&self) -> Option<HistoryCursor> {
        if self.confirmed_completeness() == Completeness::Complete {
            return None;
        }
        self.entries
            .iter()
            .rev()
            .find(|entry| matches!(entry.status, TransactionStatus::Confirmed(_)))
            .map(|entry| HistoryCursor::Confirmed {
                after: Some(entry.txid),
            })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryFields {
    address: Address,
    cursor: HistoryCursor,
    entries: Vec<HistoryEntry>,
}
impl TryFrom<HistoryFields> for HistoryPage {
    type Error = Error;
    fn try_from(value: HistoryFields) -> Result<Self, Error> {
        Self::new(value.address, value.cursor, value.entries)
    }
}

/// Exact nonnegative satoshi-per-vbyte estimates keyed by positive block horizons.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FeeEstimates(BTreeMap<u64, ExactDecimal>);
impl FeeEstimates {
    /// Validates unique positive horizons and exact nonnegative rates.
    ///
    /// # Errors
    /// Rejects zero or duplicate horizons and negative rates.
    pub fn new(entries: impl IntoIterator<Item = (u64, ExactDecimal)>) -> Result<Self, Error> {
        let mut values = BTreeMap::new();
        for (target, rate) in entries {
            if target == 0 || rate.is_negative() || values.insert(target, rate).is_some() {
                return Err(ValidationError::InvalidBitcoinFeeEstimate.into());
            }
        }
        Ok(Self(values))
    }
    /// Returns exact satoshi-per-vbyte rates by positive confirmation target.
    #[must_use]
    pub const fn rates(&self) -> &BTreeMap<u64, ExactDecimal> {
        &self.0
    }
}
impl<'de> Deserialize<'de> for FeeEstimates {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FeesVisitor;
        impl<'de> serde::de::Visitor<'de> for FeesVisitor {
            type Value = FeeEstimates;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("exact Bitcoin fee estimates")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((key, rate)) = map.next_entry::<String, ExactDecimal>()? {
                    let target = canonical_target(&key).map_err(serde::de::Error::custom)?;
                    entries.push((target, rate));
                }
                FeeEstimates::new(entries).map_err(serde::de::Error::custom)
            }
        }
        deserializer.deserialize_map(FeesVisitor)
    }
}

pub(crate) fn canonical_target(value: &str) -> Result<u64, Error> {
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ValidationError::InvalidBitcoinFeeEstimate.into());
    }
    value
        .parse()
        .map_err(|_| ValidationError::InvalidBitcoinFeeEstimate.into())
}
