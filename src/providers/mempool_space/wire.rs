// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Private bounded source decoding, preserving raw numeric lexemes and duplicates.

use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

use crate::{
    domain::{
        ExactDecimal,
        bitcoin::{Satoshis, Txid, deserialize_bounded_vec},
        mempool_space::{
            FeeHistogramBin, FeeRate, MempoolSummary, RecentTransaction, RecentTransactions,
            RecommendedFees, TransactionIds, TransactionLimit, VirtualSize,
        },
    },
    error::{Error, ProviderError},
};

pub(super) const fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
pub(super) fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
pub(super) struct List<T, const MAX: usize>(pub(super) Vec<T>);
impl<'de, T: Deserialize<'de>, const MAX: usize> Deserialize<'de> for List<T, MAX> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        deserialize_bounded_vec::<_, _, MAX>(d).map(Self)
    }
}
struct Number(ExactDecimal);
impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = Box::<RawValue>::deserialize(d)?;
        ExactDecimal::parse(raw.get())
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}
#[derive(Deserialize)]
pub(super) struct Summary {
    count: u64,
    vsize: u64,
    total_fee: u64,
    fee_histogram: List<(Number, u64), { MempoolSummary::MAX_HISTOGRAM_BINS }>,
}
impl Summary {
    pub(super) fn into_domain(self) -> Result<MempoolSummary, Error> {
        let histogram = self
            .fee_histogram
            .0
            .into_iter()
            .map(|(rate, bytes)| FeeRate::new(rate.0).map(|rate| FeeHistogramBin::new(rate, bytes)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| invalid())?;
        MempoolSummary::new(
            self.count,
            self.vsize,
            Satoshis::new(self.total_fee),
            histogram,
        )
        .map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
pub(super) struct Recent {
    txid: Txid,
    fee: u64,
    vsize: Number,
    value: u64,
}
pub(super) fn recent(
    values: List<Recent, { RecentTransactions::MAX_ENTRIES }>,
) -> Result<RecentTransactions, Error> {
    let values = values
        .0
        .into_iter()
        .map(|value| {
            RecentTransaction::new(
                value.txid,
                Satoshis::new(value.fee),
                VirtualSize::new(value.vsize.0)?,
                Satoshis::new(value.value),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    RecentTransactions::new(values).map_err(|_| invalid())
}
pub(super) fn txids(
    values: List<Txid, { TransactionLimit::MAXIMUM as usize }>,
    limit: TransactionLimit,
) -> Result<TransactionIds, Error> {
    TransactionIds::new(limit, values.0).map_err(|_| invalid())
}
#[derive(Deserialize)]
pub(super) struct Fees {
    #[serde(rename = "fastestFee")]
    fastest: Number,
    #[serde(rename = "halfHourFee")]
    half_hour: Number,
    #[serde(rename = "hourFee")]
    hour: Number,
    #[serde(rename = "economyFee")]
    economy: Number,
    #[serde(rename = "minimumFee")]
    minimum: Number,
}
impl Fees {
    pub(super) fn into_domain(self) -> Result<RecommendedFees, Error> {
        let rate = |v: Number| FeeRate::new(v.0).map_err(|_| invalid());
        Ok(RecommendedFees::new(
            rate(self.fastest)?,
            rate(self.half_hour)?,
            rate(self.hour)?,
            rate(self.economy)?,
            rate(self.minimum)?,
        ))
    }
}
