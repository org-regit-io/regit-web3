// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Remote Esplora DTO decoding and exact record conversion, without sending.

use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};

use crate::{
    domain::{
        ExactDecimal, Timestamp,
        bitcoin::{
            Address, AddressBalance, BlockHash, BlockReference, Bytes, FeeEstimates, HistoryEntry,
            MAX_INPUTS, MAX_OUTPUTS, Network, OutPoint, PreviousOutput, Satoshis, Transaction,
            TransactionBody, TransactionStatus, Txid, canonical_target, deserialize_bounded_vec,
        },
    },
    error::{Error, ProviderError},
};

pub(super) fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

// Allow documented transaction/address extensions while rejecting duplicate
// known fields before a map or Value could overwrite them.
#[derive(Deserialize)]
pub(super) struct AddressStats {
    address: String,
    chain_stats: Stats,
    mempool_stats: Stats,
}
#[derive(Deserialize)]
struct Stats {
    funded_txo_sum: u64,
    spent_txo_sum: u64,
    tx_count: u64,
}
impl AddressStats {
    pub(super) fn into_balance(self, address: Address) -> Result<AddressBalance, Error> {
        if Address::parse(&self.address, address.network())? != address {
            return Err(invalid_response());
        }
        AddressBalance::from_stats(
            address,
            self.chain_stats.funded_txo_sum,
            self.chain_stats.spent_txo_sum,
            self.mempool_stats.funded_txo_sum,
            self.mempool_stats.spent_txo_sum,
            self.chain_stats.tx_count,
            self.mempool_stats.tx_count,
        )
    }
}

#[derive(Deserialize)]
pub(super) struct HistoryTransaction {
    txid: String,
    fee: u64,
    status: Status,
}
impl HistoryTransaction {
    pub(super) fn into_entry(self) -> Result<HistoryEntry, Error> {
        let txid = Txid::parse(&self.txid).map_err(|_| invalid_response())?;
        Ok(HistoryEntry::new(
            txid,
            Satoshis::new(self.fee),
            self.status.into_status()?,
        ))
    }
}
#[derive(Deserialize)]
pub(super) struct Status {
    confirmed: bool,
    block_height: Option<u64>,
    block_hash: Option<String>,
    block_time: Option<u64>,
}
impl Status {
    pub(super) fn into_status(self) -> Result<TransactionStatus, Error> {
        match (
            self.confirmed,
            self.block_height,
            self.block_hash,
            self.block_time,
        ) {
            (false, None, None, None) => Ok(TransactionStatus::Unconfirmed),
            (true, Some(height), Some(hash), Some(time)) => {
                let hash = BlockHash::parse(&hash).map_err(|_| invalid_response())?;
                Ok(TransactionStatus::Confirmed(BlockReference::new(
                    height,
                    hash,
                    Timestamp::from_unix_seconds(time),
                )))
            }
            _ => Err(invalid_response()),
        }
    }
}

#[derive(Deserialize)]
pub(super) struct IndexedTransaction {
    txid: String,
    version: i32,
    locktime: u32,
    size: u64,
    weight: u64,
    fee: Option<u64>,
    #[serde(deserialize_with = "inputs")]
    vin: Vec<IndexedInput>,
    #[serde(deserialize_with = "outputs")]
    vout: Vec<IndexedOutput>,
    status: Status,
}

#[derive(Deserialize)]
struct IndexedInput {
    txid: String,
    vout: u32,
    is_coinbase: bool,
    scriptsig: Bytes,
    sequence: u32,
    #[serde(default, deserialize_with = "witness")]
    witness: Vec<Bytes>,
    prevout: Option<IndexedOutput>,
}

#[derive(Deserialize)]
struct IndexedOutput {
    scriptpubkey: Bytes,
    scriptpubkey_address: Option<String>,
    value: u64,
}

impl IndexedOutput {
    fn into_previous_output(self, network: Network) -> Result<PreviousOutput, Error> {
        let output = PreviousOutput::new(Satoshis::new(self.value), self.scriptpubkey)
            .map_err(|_| invalid_response())?;
        if let Some(address) = self.scriptpubkey_address {
            let supplied = Address::parse(&address, network).map_err(|_| invalid_response())?;
            if output.address(network).as_ref() != Some(&supplied) {
                return Err(invalid_response());
            }
        }
        Ok(output)
    }
}

impl IndexedTransaction {
    pub(super) fn into_transaction(
        self,
        body: TransactionBody,
        network: Network,
    ) -> Result<Transaction, Error> {
        if Txid::parse(&self.txid).map_err(|_| invalid_response())? != body.txid()
            || self.version != body.version()
            || self.locktime != body.lock_time()
            || self.size != u64::try_from(body.size()).map_err(|_| invalid_response())?
            || self.weight != body.weight()
            || self.vin.len() != body.inputs().len()
            || self.vout.len() != body.outputs().len()
        {
            return Err(invalid_response());
        }
        for (indexed, canonical) in self.vout.into_iter().zip(body.outputs()) {
            let indexed = indexed.into_previous_output(network)?;
            if indexed.value() != canonical.value()
                || indexed.script_pubkey() != canonical.script_pubkey()
            {
                return Err(invalid_response());
            }
        }
        let mut previous_outputs = Vec::with_capacity(self.vin.len());
        for (indexed, canonical) in self.vin.into_iter().zip(body.inputs()) {
            let txid = Txid::parse(&indexed.txid).map_err(|_| invalid_response())?;
            let expected = canonical.previous_output();
            let previous_output = if indexed.is_coinbase {
                if indexed.vout != u32::MAX || indexed.txid != "0".repeat(64) {
                    return Err(invalid_response());
                }
                None
            } else {
                Some(OutPoint::new(txid, indexed.vout).map_err(|_| invalid_response())?)
            };
            if indexed.is_coinbase != expected.is_none()
                || previous_output != expected
                || indexed.scriptsig.as_slice() != canonical.script_sig()
                || indexed.sequence != canonical.sequence()
                || !indexed
                    .witness
                    .iter()
                    .map(Bytes::as_slice)
                    .eq(canonical.witness())
            {
                return Err(invalid_response());
            }
            previous_outputs.push(
                indexed
                    .prevout
                    .map(|output| output.into_previous_output(network))
                    .transpose()?,
            );
        }
        Transaction::new(
            network,
            body,
            previous_outputs,
            self.fee.map(Satoshis::new),
            self.status.into_status()?,
        )
        .map_err(|_| invalid_response())
    }
}

fn inputs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<IndexedInput>, D::Error> {
    deserialize_bounded_vec::<_, _, MAX_INPUTS>(deserializer)
}
fn outputs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<IndexedOutput>, D::Error> {
    deserialize_bounded_vec::<_, _, MAX_OUTPUTS>(deserializer)
}
fn witness<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Bytes>, D::Error> {
    // Every witness element consumes at least one compact-size byte in raw data.
    deserialize_bounded_vec::<_, _, { Bytes::MAX_LEN }>(deserializer)
}

pub(super) struct FeeRates(pub(super) FeeEstimates);
impl<'de> Deserialize<'de> for FeeRates {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FeeVisitor;
        impl<'de> Visitor<'de> for FeeVisitor {
            type Value = FeeRates;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("exact numeric fee estimates")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((key, rate)) =
                    map.next_entry::<String, Box<serde_json::value::RawValue>>()?
                {
                    let target = canonical_target(&key).map_err(serde::de::Error::custom)?;
                    let rate = ExactDecimal::parse(rate.get()).map_err(serde::de::Error::custom)?;
                    entries.push((target, rate));
                }
                FeeEstimates::new(entries)
                    .map(FeeRates)
                    .map_err(serde::de::Error::custom)
            }
        }
        deserializer.deserialize_map(FeeVisitor)
    }
}
