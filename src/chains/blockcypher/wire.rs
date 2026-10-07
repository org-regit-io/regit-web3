// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::indexed_utxo::{
        self as domain, AddressBalance, AddressPolicy, AtomicAmount, BalanceData, BlockHash, Bytes,
        CoinbaseSource, FeeEstimates, HistoryPage, HistoryRequest, MempoolDelta, OutPoint,
        ReferenceDirection, ReferenceInclusion, SourceText, Transaction, TransactionData,
        TransactionInput, TransactionOutput, TransactionReference, TransactionStatus, Txid,
        bounded_optional_addresses, bounded_optional_vec, bounded_optional_witness, bounded_vec,
    },
    error::{Error, ProviderError},
};
use serde::{Deserialize, de::DeserializeOwned};

pub(super) fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
pub(super) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| invalid_response())
}

#[derive(Deserialize)]
pub(super) struct Genesis {
    pub hash: BlockHash,
    pub height: u64,
    pub chain: String,
}

#[derive(Deserialize)]
pub(super) struct AddressFacts {
    address: String,
    total_received: u64,
    total_sent: u64,
    balance: u64,
    unconfirmed_balance: i64,
    final_balance: u64,
    n_tx: u64,
    unconfirmed_n_tx: u64,
    final_n_tx: u64,
}
impl AddressFacts {
    fn into_data<A: AddressPolicy>(self, address: &A) -> Result<BalanceData<A>, Error> {
        if A::parse(&self.address, address.network())? != *address {
            return Err(domain::invalid());
        }
        Ok(BalanceData {
            total_received: AtomicAmount::new(self.total_received),
            total_sent: AtomicAmount::new(self.total_sent),
            confirmed: AtomicAmount::new(self.balance),
            unconfirmed: MempoolDelta::new(i128::from(self.unconfirmed_balance))?,
            final_balance: AtomicAmount::new(self.final_balance),
            confirmed_transactions: self.n_tx,
            unconfirmed_transactions: self.unconfirmed_n_tx,
            final_transactions: self.final_n_tx,
        })
    }
    pub(super) fn into_balance<A: AddressPolicy>(
        self,
        address: A,
    ) -> Result<AddressBalance<A>, Error> {
        let data = self.into_data(&address)?;
        AddressBalance::new(address, data)
    }
}

#[derive(Deserialize)]
pub(super) struct Chain {
    pub name: String,
    height: u64,
    hash: BlockHash,
    time: Option<SourceText>,
    high_fee_per_kb: u64,
    medium_fee_per_kb: u64,
    low_fee_per_kb: u64,
}
impl Chain {
    pub(super) fn into_fees<A: AddressPolicy>(self) -> FeeEstimates<A> {
        FeeEstimates {
            high_per_kilobyte: AtomicAmount::new(self.high_fee_per_kb),
            medium_per_kilobyte: AtomicAmount::new(self.medium_fee_per_kb),
            low_per_kilobyte: AtomicAmount::new(self.low_fee_per_kb),
            tip_height: self.height,
            tip_hash: self.hash,
            updated_at: self.time,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct History {
    address: String,
    total_received: u64,
    total_sent: u64,
    balance: u64,
    unconfirmed_balance: i64,
    final_balance: u64,
    n_tx: u64,
    unconfirmed_n_tx: u64,
    final_n_tx: u64,
    #[serde(default, deserialize_with = "bounded_optional_vec")]
    txrefs: Option<Vec<TxRef>>,
    #[serde(default, deserialize_with = "bounded_optional_vec")]
    unconfirmed_txrefs: Option<Vec<TxRef>>,
    #[serde(rename = "hasMore")]
    has_more: Option<bool>,
}
impl History {
    pub(super) fn into_history<A: AddressPolicy>(
        self,
        address: A,
        request: HistoryRequest,
    ) -> Result<HistoryPage<A>, Error> {
        let balance = AddressFacts {
            address: self.address,
            total_received: self.total_received,
            total_sent: self.total_sent,
            balance: self.balance,
            unconfirmed_balance: self.unconfirmed_balance,
            final_balance: self.final_balance,
            n_tx: self.n_tx,
            unconfirmed_n_tx: self.unconfirmed_n_tx,
            final_n_tx: self.final_n_tx,
        }
        .into_data(&address)?;
        // Missing arrays with a known positive count are not silently an empty history.
        if self.txrefs.is_none()
            && balance.confirmed_transactions > 0
            && request.before_height().is_none()
            || self.unconfirmed_txrefs.is_none() && balance.unconfirmed_transactions > 0
        {
            return Err(Error::UnavailableData);
        }
        let confirmed = self
            .txrefs
            .unwrap_or_default()
            .into_iter()
            .map(TxRef::into_reference)
            .collect::<Result<Vec<_>, _>>()?;
        let unconfirmed = self
            .unconfirmed_txrefs
            .unwrap_or_default()
            .into_iter()
            .map(TxRef::into_reference)
            .collect::<Result<Vec<_>, _>>()?;
        HistoryPage::new(
            address,
            request,
            balance,
            confirmed,
            unconfirmed,
            self.has_more,
        )
    }
}

#[derive(Deserialize)]
struct TxRef {
    tx_hash: Txid,
    block_height: i64,
    tx_input_n: i64,
    tx_output_n: i64,
    value: u64,
    confirmations: u64,
    confirmed: Option<SourceText>,
    ref_balance: Option<u64>,
    spent: Option<bool>,
    spent_by: Option<Txid>,
    double_spend: bool,
    #[serde(alias = "double_of", alias = "double_spend_tx")]
    double_spend_transaction: Option<Txid>,
    script: Option<Bytes>,
}
impl TxRef {
    fn into_reference<A: AddressPolicy>(self) -> Result<TransactionReference<A>, Error> {
        let direction = match (self.tx_input_n, self.tx_output_n) {
            (input, -1) if input >= 0 => ReferenceDirection::Input {
                index: u32::try_from(input).map_err(|_| domain::invalid())?,
            },
            (-1, output) if output >= 0 => ReferenceDirection::Output {
                index: u32::try_from(output).map_err(|_| domain::invalid())?,
            },
            _ => return Err(domain::invalid()),
        };
        let inclusion = inclusion(
            self.block_height,
            None,
            None,
            self.confirmed,
            self.confirmations,
        )?;
        Ok(TransactionReference {
            txid: self.tx_hash,
            direction,
            value: AtomicAmount::new(self.value),
            inclusion,
            confirmations: self.confirmations,
            balance_at_reference: self.ref_balance.map(AtomicAmount::new),
            spent: self.spent,
            spent_by: self.spent_by,
            double_spend: self.double_spend,
            double_spend_transaction: self.double_spend_transaction,
            script: self.script,
        })
    }
}

#[derive(Deserialize)]
pub(super) struct Status {
    hash: Txid,
    block_height: i64,
    block_hash: Option<BlockHash>,
    block_index: Option<u32>,
    confirmed: Option<SourceText>,
    confirmations: u64,
    double_spend: bool,
    #[serde(alias = "double_of", alias = "double_spend_tx")]
    double_spend_transaction: Option<Txid>,
}
impl Status {
    pub(super) fn into_status(self) -> Result<TransactionStatus, Error> {
        let inclusion = inclusion(
            self.block_height,
            self.block_hash,
            self.block_index,
            self.confirmed,
            self.confirmations,
        )?;
        TransactionStatus::new(
            self.hash,
            inclusion,
            self.confirmations,
            self.double_spend,
            self.double_spend_transaction,
        )
    }
}
fn inclusion(
    height: i64,
    hash: Option<BlockHash>,
    index: Option<u32>,
    time: Option<SourceText>,
    confirmations: u64,
) -> Result<Option<ReferenceInclusion>, Error> {
    if height == -1 {
        if confirmations != 0 || hash.is_some() || index.is_some() || time.is_some() {
            return Err(domain::invalid());
        }
        return Ok(None);
    }
    if height < 0 || confirmations == 0 {
        return Err(domain::invalid());
    }
    Ok(Some(ReferenceInclusion {
        height: height.cast_unsigned(),
        block_hash: hash,
        transaction_index: index,
        confirmed_at: time,
    }))
}

#[derive(Deserialize)]
pub(super) struct TransactionWire {
    hash: Txid,
    block_height: i64,
    block_hash: Option<BlockHash>,
    block_index: Option<u32>,
    confirmed: Option<SourceText>,
    confirmations: u64,
    double_spend: bool,
    #[serde(alias = "double_of", alias = "double_spend_tx")]
    double_spend_transaction: Option<Txid>,
    ver: i32,
    lock_time: Option<u32>,
    size: u32,
    vsize: Option<u32>,
    total: u64,
    fees: Option<u64>,
    vin_sz: u32,
    vout_sz: u32,
    #[serde(deserialize_with = "bounded_vec")]
    inputs: Vec<Input>,
    #[serde(deserialize_with = "bounded_vec")]
    outputs: Vec<Output>,
    hex: Option<Bytes>,
    next_inputs: Option<String>,
    next_outputs: Option<String>,
}
impl TransactionWire {
    pub(super) fn into_transaction<A: AddressPolicy>(
        self,
        network: A::Network,
        maximum_entries: u32,
    ) -> Result<Transaction<A>, Error> {
        if self.inputs.len() > maximum_entries as usize
            || self.outputs.len() > maximum_entries as usize
        {
            return Err(invalid_response());
        }
        if self.inputs.len() != self.vin_sz as usize
            || self.outputs.len() != self.vout_sz as usize
            || self.next_inputs.is_some()
            || self.next_outputs.is_some()
        {
            return Err(Error::UnavailableData);
        }
        let inputs = self
            .inputs
            .into_iter()
            .map(|input| input.into_input(network))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| invalid_response())?;
        let outputs = self
            .outputs
            .into_iter()
            .map(|output| output.into_output(network))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| invalid_response())?;
        Transaction::new(
            network,
            TransactionData {
                status: Status {
                    hash: self.hash,
                    block_height: self.block_height,
                    block_hash: self.block_hash,
                    block_index: self.block_index,
                    confirmed: self.confirmed,
                    confirmations: self.confirmations,
                    double_spend: self.double_spend,
                    double_spend_transaction: self.double_spend_transaction,
                }
                .into_status()
                .map_err(|_| invalid_response())?,
                version: self.ver,
                lock_time: self.lock_time,
                size: self.size,
                virtual_size: self.vsize,
                total_output: AtomicAmount::new(self.total),
                reported_fee: self.fees.map(AtomicAmount::new),
                inputs,
                outputs,
                raw: self.hex,
            },
        )
        .map_err(|_| invalid_response())
    }
}
#[derive(Default)]
enum Presence<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Presence<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(|value| value.map_or(Self::Null, Self::Value))
    }
}

#[derive(Deserialize)]
struct Input {
    #[serde(default)]
    prev_hash: Presence<Txid>,
    #[serde(default)]
    output_index: Presence<i64>,
    #[serde(default)]
    output_value: Presence<u64>,
    sequence: Option<u32>,
    script: Option<Bytes>,
    script_type: Option<SourceText>,
    #[serde(default, deserialize_with = "bounded_optional_addresses")]
    addresses: Option<Vec<String>>,
    #[serde(default, deserialize_with = "bounded_optional_witness")]
    witness: Option<Vec<Bytes>>,
}
impl Input {
    fn into_input<A: AddressPolicy>(
        self,
        network: A::Network,
    ) -> Result<TransactionInput<A>, Error> {
        // Classification follows the documented index schema, not raw transaction decoding.
        let (previous_output, coinbase_source, previous_output_value) =
            match (self.prev_hash, self.output_index, self.output_value) {
                (Presence::Missing, Presence::Missing, Presence::Missing) => {
                    (None, Some(CoinbaseSource::OmittedPrevoutFields), None)
                }
                (Presence::Missing, Presence::Value(-1), Presence::Missing) => {
                    (None, Some(CoinbaseSource::ExplicitSentinel), None)
                }
                (Presence::Value(hash), Presence::Value(-1), Presence::Missing)
                    if *hash.display_bytes() == [0; 32] =>
                {
                    (None, Some(CoinbaseSource::ExplicitSentinel), None)
                }
                (Presence::Value(hash), Presence::Value(index), value) if index >= 0 => {
                    let value = match value {
                        Presence::Missing => None,
                        Presence::Value(value) => Some(AtomicAmount::new(value)),
                        Presence::Null => return Err(domain::invalid()),
                    };
                    (
                        Some(OutPoint {
                            txid: hash,
                            output_index: u32::try_from(index).map_err(|_| domain::invalid())?,
                        }),
                        None,
                        value,
                    )
                }
                _ => return Err(domain::invalid()),
            };
        Ok(TransactionInput {
            previous_output,
            coinbase_source,
            previous_output_value,
            sequence: self.sequence,
            script: self.script,
            witness: self.witness,
            addresses: parse_addresses(self.addresses, network)?,
            script_type: self.script_type,
        })
    }
}
#[derive(Deserialize)]
struct Output {
    value: u64,
    script: Bytes,
    spent_by: Option<Txid>,
    script_type: Option<SourceText>,
    #[serde(default, deserialize_with = "bounded_optional_addresses")]
    addresses: Option<Vec<String>>,
}
impl Output {
    fn into_output<A: AddressPolicy>(
        self,
        network: A::Network,
    ) -> Result<TransactionOutput<A>, Error> {
        Ok(TransactionOutput {
            value: AtomicAmount::new(self.value),
            script: self.script,
            addresses: parse_addresses(self.addresses, network)?,
            spent_by: self.spent_by,
            script_type: self.script_type,
        })
    }
}
fn parse_addresses<A: AddressPolicy>(
    value: Option<Vec<String>>,
    network: A::Network,
) -> Result<Option<Vec<A>>, Error> {
    value
        .map(|addresses| {
            if addresses.len() > 100 {
                return Err(domain::invalid());
            }
            addresses
                .into_iter()
                .map(|a| A::parse(&a, network))
                .collect()
        })
        .transpose()
}
