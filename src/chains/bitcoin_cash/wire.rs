// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::{
        ExactDecimal,
        bitcoin_cash::{
            Address, AddressBalance, AddressNamespace, BlockHash, Bytes, CollectionLimit,
            FeeEstimate, FeeTarget, History, HistoryEntry, HistoryRange, HistoryState, Nft,
            NftCapability, RawTransaction, Satoshis, SignedSatoshis, SourceText, TokenAmount,
            TokenCategory, TokenData, TokenFilter, Transaction, TransactionData, TransactionInput,
            TransactionOutput, TransactionStatus, Txid, UnspentOutput, UnspentOutputs,
            bounded_addresses, bounded_entries,
        },
    },
    error::{Error, ProviderError},
};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::{Number, value::RawValue};

fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

fn present_addresses<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<String>>, D::Error> {
    bounded_addresses(d).map(Some)
}

#[derive(Deserialize)]
struct Envelope {
    jsonrpc: String,
    id: u64,
    #[serde(default, deserialize_with = "present")]
    result: Option<Box<RawValue>>,
    #[serde(default, deserialize_with = "present")]
    error: Option<RpcError>,
}
#[derive(Deserialize)]
struct RpcError {
    #[serde(rename = "code")]
    _code: i64,
    #[serde(rename = "message")]
    _message: String,
}
pub(super) fn response<T: DeserializeOwned>(bytes: &[u8], id: u64) -> Result<T, Error> {
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if envelope.jsonrpc != "2.0" || envelope.id != id {
        return Err(invalid());
    }
    match (envelope.result, envelope.error) {
        (Some(result), None) => serde_json::from_str(result.get()).map_err(|_| invalid()),
        (None, Some(_error)) => Err(Error::Provider(ProviderError::Rpc)),
        _ => Err(invalid()),
    }
}

#[derive(Deserialize)]
pub(super) struct Features {
    pub genesis_hash: BlockHash,
    pub hash_function: String,
    pub cashtokens: Option<bool>,
    pub server_version: String,
    pub protocol_min: String,
    pub protocol_max: String,
}

#[derive(Deserialize)]
pub(super) struct Balance {
    confirmed: u64,
    unconfirmed: Number,
}
pub(super) fn balance(
    address: Address,
    token_filter: TokenFilter,
    v: &Balance,
) -> Result<AddressBalance, Error> {
    let unconfirmed = SignedSatoshis::parse(v.unconfirmed.as_str()).map_err(|_| invalid())?;
    Ok(AddressBalance {
        address,
        token_filter,
        confirmed: Satoshis::from_raw(v.confirmed),
        unconfirmed,
    })
}

#[derive(Deserialize)]
struct HistoryWire {
    tx_hash: Txid,
    height: i64,
    #[serde(default, deserialize_with = "present")]
    fee: Option<u64>,
}
#[derive(Deserialize)]
pub(super) struct HistoryList(#[serde(deserialize_with = "bounded_entries")] Vec<HistoryWire>);
pub(super) fn history(
    address: Address,
    range: HistoryRange,
    v: HistoryList,
) -> Result<History, Error> {
    let entries =
        v.0.into_iter()
            .map(|v| {
                let state = match v.height {
                    -1 => HistoryState::UnconfirmedParents,
                    0 => HistoryState::ZeroHeight,
                    n if n > 0 => HistoryState::Confirmed {
                        height: u32::try_from(n).map_err(|_| invalid())?,
                    },
                    _ => return Err(invalid()),
                };
                HistoryEntry::new(v.tx_hash, state, v.fee.map(Satoshis::from_raw))
                    .map_err(|_| invalid())
            })
            .collect::<Result<_, _>>()?;
    History::new(address, range, entries).map_err(|_| invalid())
}

pub(super) fn fee(target: FeeTarget, number: &Number) -> Result<FeeEstimate, Error> {
    let value = ExactDecimal::parse(number.as_str()).map_err(|_| invalid())?;
    let value = if value == ExactDecimal::parse("-1").map_err(|_| invalid())? {
        None
    } else {
        Some(value)
    };
    FeeEstimate::new(target, value).map_err(|_| invalid())
}

#[derive(Deserialize)]
struct NftWire {
    capability: NftCapability,
    commitment: Bytes,
}
#[derive(Deserialize)]
struct TokenWire {
    category: TokenCategory,
    amount: TokenAmount,
    #[serde(default, deserialize_with = "present")]
    nft: Option<NftWire>,
}
impl TokenWire {
    fn convert(self) -> Result<TokenData, Error> {
        let nft = self
            .nft
            .map(|v| Nft::new(v.capability, v.commitment))
            .transpose()
            .map_err(|_| invalid())?;
        TokenData::new(self.category, self.amount, nft).map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
struct UnspentWire {
    height: u32,
    tx_hash: Txid,
    tx_pos: u32,
    value: u64,
    #[serde(default, deserialize_with = "present")]
    token_data: Option<TokenWire>,
}
#[derive(Deserialize)]
pub(super) struct UnspentList(#[serde(deserialize_with = "bounded_entries")] Vec<UnspentWire>);
pub(super) fn unspent(
    address: Address,
    filter: TokenFilter,
    limit: CollectionLimit,
    v: UnspentList,
) -> Result<UnspentOutputs, Error> {
    let outputs =
        v.0.into_iter()
            .map(|v| {
                Ok(UnspentOutput {
                    txid: v.tx_hash,
                    output_index: v.tx_pos,
                    height: v.height,
                    value: Satoshis::from_raw(v.value),
                    token_data: v.token_data.map(TokenWire::convert).transpose()?,
                })
            })
            .collect::<Result<_, Error>>()?;
    UnspentOutputs::new(address, filter, limit, outputs).map_err(|_| invalid())
}

#[derive(Deserialize)]
pub(super) struct Confirmed {
    #[serde(rename = "block_height")]
    pub height: u32,
    #[serde(rename = "block_hash")]
    pub hash: BlockHash,
    #[serde(rename = "block_header")]
    pub header: String,
}

#[derive(Deserialize)]
struct ScriptSig {
    hex: Bytes,
}
#[derive(Deserialize)]
struct InputWire {
    sequence: u32,
    #[serde(default, deserialize_with = "present")]
    coinbase: Option<Bytes>,
    #[serde(default, deserialize_with = "present")]
    txid: Option<Txid>,
    #[serde(default, deserialize_with = "present")]
    vout: Option<u32>,
    #[serde(rename = "scriptSig", default, deserialize_with = "present")]
    script_sig: Option<ScriptSig>,
}
impl InputWire {
    fn convert(self) -> Result<TransactionInput, Error> {
        match (self.coinbase, self.txid, self.vout, self.script_sig) {
            (Some(unlocking_bytes), None, None, None) => Ok(TransactionInput::Coinbase {
                unlocking_bytes,
                sequence: self.sequence,
            }),
            (None, Some(previous_txid), Some(output_index), Some(script)) => {
                Ok(TransactionInput::Ordinary {
                    previous_txid,
                    output_index,
                    unlocking_bytes: script.hex,
                    sequence: self.sequence,
                })
            }
            _ => Err(invalid()),
        }
    }
}
#[derive(Deserialize)]
struct ScriptWire {
    hex: Bytes,
    #[serde(rename = "type")]
    script_type: SourceText,
    #[serde(default, deserialize_with = "present_addresses")]
    addresses: Option<Vec<String>>,
    #[serde(rename = "reqSigs", default, deserialize_with = "present")]
    required_signatures: Option<u32>,
}
#[derive(Deserialize)]
struct OutputWire {
    n: u32,
    value: Number,
    #[serde(rename = "scriptPubKey")]
    script: ScriptWire,
    #[serde(rename = "tokenData", default, deserialize_with = "present")]
    token_data: Option<TokenWire>,
}
impl OutputWire {
    fn convert(self, namespace: AddressNamespace) -> Result<TransactionOutput, Error> {
        let decimal = ExactDecimal::parse(self.value.as_str()).map_err(|_| invalid())?;
        let addresses = self
            .script
            .addresses
            .map(|values| {
                if values.len() > 100 {
                    return Err(invalid());
                }
                values
                    .into_iter()
                    .map(|v| Address::parse(&v, namespace).map_err(|_| invalid()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Ok(TransactionOutput {
            index: self.n,
            value: Satoshis::from_bch(&decimal).map_err(|_| invalid())?,
            locking_bytes: self.script.hex,
            script_type: self.script.script_type,
            addresses,
            required_signatures: self.script.required_signatures,
            token_data: self.token_data.map(TokenWire::convert).transpose()?,
        })
    }
}
#[derive(Deserialize)]
pub(super) struct TransactionWire {
    txid: Txid,
    hex: Bytes,
    size: u32,
    version: i32,
    locktime: u32,
    #[serde(deserialize_with = "bounded_entries")]
    vin: Vec<InputWire>,
    #[serde(deserialize_with = "bounded_entries")]
    vout: Vec<OutputWire>,
    #[serde(default, deserialize_with = "present")]
    hash: Option<Txid>,
    #[serde(default, deserialize_with = "present")]
    confirmations: Option<u32>,
    #[serde(default, deserialize_with = "present")]
    blocktime: Option<u64>,
    #[serde(default, deserialize_with = "present")]
    blockhash: Option<BlockHash>,
}
pub(super) fn transaction(
    namespace: AddressNamespace,
    txid: Txid,
    limit: CollectionLimit,
    v: TransactionWire,
    raw: &str,
    status: TransactionStatus,
) -> Result<Transaction, Error> {
    let raw =
        RawTransaction::new(Bytes::from_hex(raw).map_err(|_| invalid())?).map_err(|_| invalid())?;
    if raw.bytes() != &v.hex {
        return Err(invalid());
    }
    let inputs = v
        .vin
        .into_iter()
        .map(InputWire::convert)
        .collect::<Result<_, _>>()?;
    let outputs = v
        .vout
        .into_iter()
        .map(|v| v.convert(namespace))
        .collect::<Result<_, _>>()?;
    let data = TransactionData {
        namespace,
        raw,
        reported_txid: v.txid,
        reported_hash: v.hash,
        version: v.version,
        lock_time: v.locktime,
        size: v.size,
        inputs,
        outputs,
        reported_confirmations: v.confirmations,
        reported_block_time: v.blocktime,
        reported_block_hash: v.blockhash,
    };
    Transaction::new(txid, limit, data, status).map_err(|_| invalid())
}
