// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{assets, invalid_response, required_option};
use crate::{
    domain::{
        ExactDecimal,
        cardano::{
            Hash, HexData, IndexedInput, IndexedOutput, Lovelace, Network, OutputData,
            PaymentAddress, ScriptHash, Transaction, TransactionCbor, TransactionData,
            TransactionUtxos,
        },
    },
    error::Error,
};
use serde::Deserialize;
fn checked<T>(v: Result<T, Error>) -> Result<T, Error> {
    v.map_err(|_| invalid_response())
}
#[derive(Deserialize)]
pub(in crate::providers::blockfrost) struct Summary {
    hash: String,
    block: String,
    block_height: u64,
    block_time: u64,
    slot: u64,
    index: u32,
    output_amount: Vec<super::UnitQuantity>,
    fees: Lovelace,
    deposit: String,
    size: u32,
    #[serde(deserialize_with = "required_option")]
    invalid_before: Option<String>,
    #[serde(deserialize_with = "required_option")]
    invalid_hereafter: Option<String>,
    utxo_count: u32,
    valid_contract: bool,
    treasury_donation: Lovelace,
}
impl Summary {
    pub(in crate::providers::blockfrost) fn decode(
        bytes: &[u8],
        requested: Hash,
    ) -> Result<Self, Error> {
        let v: Self = serde_json::from_slice(bytes).map_err(|_| invalid_response())?;
        if checked(Hash::parse(&v.hash))? != requested {
            return Err(invalid_response());
        }
        Ok(v)
    }
    pub(in crate::providers::blockfrost) fn into_domain(
        self,
        network: &Network,
        cbor: TransactionCbor,
    ) -> Result<Transaction, Error> {
        let data = TransactionData {
            transaction_id: checked(Hash::parse(&self.hash))?,
            block: checked(Hash::parse(&self.block))?,
            block_height: self.block_height,
            block_unix_seconds: self.block_time,
            slot: self.slot,
            index: self.index,
            output_amount: assets(self.output_amount, network.identity())?,
            fees: self.fees,
            deposit: checked(ExactDecimal::parse(&self.deposit))?,
            size_bytes: self.size,
            invalid_before: self
                .invalid_before
                .map(|s| checked(Lovelace::parse(&s)).and_then(|v| checked(v.payment_coin())))
                .transpose()?,
            invalid_hereafter: self
                .invalid_hereafter
                .map(|s| checked(Lovelace::parse(&s)).and_then(|v| checked(v.payment_coin())))
                .transpose()?,
            utxo_count: self.utxo_count,
            valid_contract: self.valid_contract,
            treasury_donation: self.treasury_donation,
        };
        checked(Transaction::new(network.clone(), data, cbor))
    }
}
#[derive(Deserialize)]
struct CborWire {
    cbor: String,
}
pub(in crate::providers::blockfrost) fn cbor(bytes: &[u8]) -> Result<TransactionCbor, Error> {
    let v: CborWire = serde_json::from_slice(bytes).map_err(|_| invalid_response())?;
    checked(TransactionCbor::parse(&v.cbor))
}
#[derive(Deserialize)]
struct Input {
    address: String,
    amount: Vec<super::UnitQuantity>,
    tx_hash: String,
    output_index: u16,
    #[serde(deserialize_with = "required_option")]
    data_hash: Option<String>,
    #[serde(deserialize_with = "required_option")]
    inline_datum: Option<String>,
    #[serde(deserialize_with = "required_option")]
    reference_script_hash: Option<String>,
    collateral: bool,
    reference: Option<bool>,
}
#[derive(Deserialize)]
struct OutputWire {
    address: String,
    amount: Vec<super::UnitQuantity>,
    output_index: u16,
    #[serde(deserialize_with = "required_option")]
    data_hash: Option<String>,
    #[serde(deserialize_with = "required_option")]
    inline_datum: Option<String>,
    #[serde(deserialize_with = "required_option")]
    reference_script_hash: Option<String>,
    collateral: bool,
    consumed_by_tx: Option<String>,
}
fn data(
    hash: Option<String>,
    inline: Option<String>,
    script: Option<String>,
) -> Result<OutputData, Error> {
    Ok(OutputData::new(
        hash.map(|s| checked(Hash::parse(&s))).transpose()?,
        inline.map(|s| checked(HexData::parse(&s))).transpose()?,
        script.map(|s| checked(ScriptHash::parse(&s))).transpose()?,
    ))
}
#[derive(Deserialize)]
struct Utxos {
    hash: String,
    inputs: Vec<Input>,
    outputs: Vec<OutputWire>,
}
pub(in crate::providers::blockfrost) fn utxos(
    bytes: &[u8],
    network: &Network,
    requested: Hash,
) -> Result<TransactionUtxos, Error> {
    let v: Utxos = serde_json::from_slice(bytes).map_err(|_| invalid_response())?;
    if checked(Hash::parse(&v.hash))? != requested
        || v.inputs.len() > 1000
        || v.outputs.len() > 1000
    {
        return Err(invalid_response());
    }
    let inputs = v
        .inputs
        .into_iter()
        .map(|v| {
            Ok(IndexedInput {
                address: checked(PaymentAddress::parse(&v.address))?,
                transaction_id: checked(Hash::parse(&v.tx_hash))?,
                output_index: v.output_index,
                assets: assets(v.amount, network.identity())?,
                data: data(v.data_hash, v.inline_datum, v.reference_script_hash)?,
                collateral: v.collateral,
                reference: v.reference,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let outputs = v
        .outputs
        .into_iter()
        .map(|v| {
            Ok(IndexedOutput {
                address: checked(PaymentAddress::parse(&v.address))?,
                output_index: v.output_index,
                assets: assets(v.amount, network.identity())?,
                data: data(v.data_hash, v.inline_datum, v.reference_script_hash)?,
                collateral: v.collateral,
                consumed_by: v
                    .consumed_by_tx
                    .map(|s| checked(Hash::parse(&s)))
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    checked(TransactionUtxos::new(
        network.clone(),
        requested,
        inputs,
        outputs,
    ))
}
pub(in crate::providers::blockfrost) fn acknowledged(bytes: &[u8]) -> Result<Hash, Error> {
    let text: String = serde_json::from_slice(bytes).map_err(|_| invalid_response())?;
    checked(Hash::parse(&text))
}
