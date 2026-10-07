// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{AssetAmount, Hash, HexData, Lovelace, Network, OutputData, PaymentAddress};
use crate::{
    domain::ExactDecimal,
    error::{Error, ValidationError},
};
use pallas_codec::minicbor::{Decoder, data::Type};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashSet, fmt};

pub(super) fn invalid_transaction() -> Error {
    ValidationError::InvalidCardanoTransaction.into()
}

/// Bounded original transaction CBOR with a computed original-body hash.
/// This is framing/hash evidence, not a ledger-validity or signing verification.
#[derive(Clone, Eq, PartialEq)]
pub struct TransactionCbor {
    bytes: Vec<u8>,
    body: std::ops::Range<usize>,
    hash: Hash,
    fee: Option<u64>,
    total_collateral: Option<u64>,
    script_valid: Option<bool>,
    ordinary_witnesses: bool,
}
impl TransactionCbor {
    /// Maximum retained original transaction size, independently of network parameters.
    pub const MAX_BYTES: usize = 65_536;
    /// Checks supported historical/current envelope framing and hashes original body bytes.
    ///
    /// # Errors
    /// Rejects trailing data, excessive size/depth/items, malformed envelope or
    /// duplicate body/witness map keys. It does not reinterpret arbitrary scripts.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > Self::MAX_BYTES {
            return Err(invalid_transaction());
        }
        let mut checked = Decoder::new(&bytes);
        let mut items = 0;
        bounded(&mut checked, 0, &mut items)?;
        if checked.position() != bytes.len() {
            return Err(invalid_transaction());
        }
        let mut d = Decoder::new(&bytes);
        let length = d.array().map_err(|_| invalid_transaction())?;
        if !matches!(length, Some(2..=4)) {
            return Err(invalid_transaction());
        }
        let start = d.position();
        let body_type = d.datatype().map_err(|_| invalid_transaction())?;
        let decoded = if matches!(body_type, Type::Map | Type::MapIndef) && length != Some(2) {
            protocol_map(&mut d, true)?
        } else if matches!(body_type, Type::Array | Type::ArrayIndef) && length == Some(2) {
            if d.array().map_err(|_| invalid_transaction())? != Some(3) {
                return Err(invalid_transaction());
            }
            for _ in 0..3 {
                d.skip().map_err(|_| invalid_transaction())?;
            }
            DecodedMap {
                fee: None,
                total_collateral: None,
                keys: HashSet::new(),
            }
        } else {
            return Err(invalid_transaction());
        };
        let body = start..d.position();
        let ordinary_witnesses = if length == Some(2) {
            if !matches!(
                d.datatype().map_err(|_| invalid_transaction())?,
                Type::Array | Type::ArrayIndef
            ) {
                return Err(invalid_transaction());
            }
            d.skip().map_err(|_| invalid_transaction())?;
            false
        } else {
            protocol_map(&mut d, false)?
                .keys
                .into_iter()
                .all(|key| key == 0)
        };
        let script_valid = if length == Some(4) {
            Some(d.bool().map_err(|_| invalid_transaction())?)
        } else {
            None
        };
        if length != Some(2) {
            auxiliary(&mut d)?;
        }
        if d.position() != bytes.len() {
            return Err(invalid_transaction());
        }
        let digest = pallas_crypto::hash::Hasher::<256>::hash(&bytes[body.clone()]);
        let hash = Hash::from_bytes(*digest);
        Ok(Self {
            bytes,
            body,
            hash,
            fee: decoded.fee,
            total_collateral: decoded.total_collateral,
            script_valid,
            ordinary_witnesses,
        })
    }
    /// Parses bounded unprefixed hexadecimal original CBOR bytes.
    ///
    /// # Errors
    /// Rejects malformed/excessive hexadecimal and invalid CBOR framing.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.len() > 2 * Self::MAX_BYTES
            || !text.len().is_multiple_of(2)
            || text.starts_with("0x")
        {
            return Err(invalid_transaction());
        }
        Self::from_bytes(const_hex::decode(text).map_err(|_| invalid_transaction())?)
    }
    /// Returns exact original CBOR; bytes are never canonicalized before hashing.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Returns the exact encoded body slice used for transaction identity.
    #[must_use]
    pub fn body_bytes(&self) -> &[u8] {
        &self.bytes[self.body.clone()]
    }
    /// Returns maintained Blake2b-256 transaction-body identity.
    #[must_use]
    pub const fn transaction_id(&self) -> Hash {
        self.hash
    }
    /// Returns a decoded Shelley-family body fee, absent for opaque Byron body evidence.
    #[must_use]
    pub const fn body_fee(&self) -> Option<u64> {
        self.fee
    }
    /// Returns encoded Babbage/Conway total collateral, when actually present.
    /// Failed-script indexed fees can correspond to this field rather than `body_fee`.
    #[must_use]
    pub const fn total_collateral(&self) -> Option<u64> {
        self.total_collateral
    }
    pub(super) const fn ordinary_witnesses(&self) -> bool {
        self.ordinary_witnesses
    }
    /// Returns actual encoded Alonzo-family script-validity bit when present.
    /// This is separate from validating scripts or source block inclusion.
    #[must_use]
    pub const fn encoded_script_validity(&self) -> Option<bool> {
        self.script_valid
    }
}
impl fmt::Debug for TransactionCbor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionCbor")
            .field("bytes", &self.bytes.len())
            .field("transaction_id", &self.hash)
            .finish_non_exhaustive()
    }
}
impl Serialize for TransactionCbor {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&const_hex::encode(&self.bytes))
    }
}
impl<'de> Deserialize<'de> for TransactionCbor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

fn auxiliary(d: &mut Decoder<'_>) -> Result<(), Error> {
    match d.datatype().map_err(|_| invalid_transaction())? {
        Type::Map
        | Type::MapIndef
        | Type::Array
        | Type::ArrayIndef
        | Type::Null
        | Type::Undefined => d.skip().map_err(|_| invalid_transaction()),
        Type::Tag => {
            if d.tag().map_err(|_| invalid_transaction())?.as_u64() != 259
                || !matches!(
                    d.datatype().map_err(|_| invalid_transaction())?,
                    Type::Map | Type::MapIndef
                )
            {
                return Err(invalid_transaction());
            }
            d.skip().map_err(|_| invalid_transaction())
        }
        _ => Err(invalid_transaction()),
    }
}

struct DecodedMap {
    fee: Option<u64>,
    total_collateral: Option<u64>,
    keys: HashSet<u64>,
}
fn protocol_map(d: &mut Decoder<'_>, body: bool) -> Result<DecodedMap, Error> {
    let length = d.map().map_err(|_| invalid_transaction())?;
    let mut keys = HashSet::new();
    let mut fee = None;
    let mut total_collateral = None;
    loop {
        if length.is_some_and(|n| u64::try_from(keys.len()).ok() == Some(n)) {
            break;
        }
        if length.is_none() && d.datatype().map_err(|_| invalid_transaction())? == Type::Break {
            d.skip().map_err(|_| invalid_transaction())?;
            break;
        }
        let key = d.u64().map_err(|_| invalid_transaction())?;
        if !keys.insert(key) || keys.len() > 64 {
            return Err(invalid_transaction());
        }
        if body && key == 2 {
            fee = Some(d.u64().map_err(|_| invalid_transaction())?);
        } else if body && key == 17 {
            total_collateral = Some(d.u64().map_err(|_| invalid_transaction())?);
        } else {
            d.skip().map_err(|_| invalid_transaction())?;
        }
    }
    if body && (fee.is_none() || !keys.contains(&0) || !keys.contains(&1)) {
        return Err(invalid_transaction());
    }
    Ok(DecodedMap {
        fee,
        total_collateral,
        keys,
    })
}
fn bounded(d: &mut Decoder<'_>, depth: u8, items: &mut usize) -> Result<(), Error> {
    if depth > 64 || *items >= 8192 {
        return Err(invalid_transaction());
    }
    *items += 1;
    match d.datatype().map_err(|_| invalid_transaction())? {
        Type::Array | Type::ArrayIndef => {
            let n = d.array().map_err(|_| invalid_transaction())?;
            container(d, n, depth, items, false)?;
        }
        Type::Map | Type::MapIndef => {
            let n = d.map().map_err(|_| invalid_transaction())?;
            let n = n
                .map(|n| n.checked_mul(2).ok_or_else(invalid_transaction))
                .transpose()?;
            container(d, n, depth, items, true)?;
        }
        Type::Tag => {
            d.tag().map_err(|_| invalid_transaction())?;
            bounded(d, depth + 1, items)?;
        }
        Type::BytesIndef => {
            for chunk in d.bytes_iter().map_err(|_| invalid_transaction())? {
                chunk.map_err(|_| invalid_transaction())?;
                *items += 1;
                if *items > 8192 {
                    return Err(invalid_transaction());
                }
            }
        }
        Type::StringIndef => {
            for chunk in d.str_iter().map_err(|_| invalid_transaction())? {
                chunk.map_err(|_| invalid_transaction())?;
                *items += 1;
                if *items > 8192 {
                    return Err(invalid_transaction());
                }
            }
        }
        Type::Break => return Err(invalid_transaction()),
        _ => d.skip().map_err(|_| invalid_transaction())?,
    }
    Ok(())
}
fn container(
    d: &mut Decoder<'_>,
    n: Option<u64>,
    depth: u8,
    items: &mut usize,
    map: bool,
) -> Result<(), Error> {
    if let Some(n) = n {
        if n > 8192 {
            return Err(invalid_transaction());
        }
        for _ in 0..n {
            bounded(d, depth + 1, items)?;
        }
    } else {
        let mut count = 0_usize;
        while d.datatype().map_err(|_| invalid_transaction())? != Type::Break {
            bounded(d, depth + 1, items)?;
            count += 1;
        }
        if map && !count.is_multiple_of(2) {
            return Err(invalid_transaction());
        }
        d.skip().map_err(|_| invalid_transaction())?;
    }
    Ok(())
}

fn indexed_assets<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<AssetAmount>, D::Error> {
    super::value::bounded_values::<D, AssetAmount, 10_000>(d)
}

/// Source indexed transaction inclusion, quantities and execution classification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionData {
    /// Exact requested transaction identity.
    pub transaction_id: Hash,
    /// Source inclusion block hash, not an independently proved finality anchor.
    pub block: Hash,
    /// Source block height.
    pub block_height: u64,
    /// Source creation time in Unix seconds.
    pub block_unix_seconds: u64,
    /// Source absolute slot.
    pub slot: u64,
    /// Source transaction index within the block.
    pub index: u32,
    /// Source exact output totals by asset.
    #[serde(deserialize_with = "indexed_assets")]
    pub output_amount: Vec<AssetAmount>,
    /// Source paid transaction fee in lovelaces.
    pub fees: Lovelace,
    /// Source signed deposit/refund in lovelace units; fraction/exponent is not a unit.
    pub deposit: ExactDecimal,
    /// Source full transaction serialized size in bytes.
    pub size_bytes: u32,
    /// Source optional inclusive validity-start slot.
    pub invalid_before: Option<u64>,
    /// Source optional exclusive validity-end slot.
    pub invalid_hereafter: Option<u64>,
    /// Source combined `UTxO` count, not just ordinary spend input count.
    pub utxo_count: u32,
    /// Actual source contract-validity flag, separate from finality.
    pub valid_contract: bool,
    /// Source exact treasury donation in lovelaces.
    pub treasury_donation: Lovelace,
}
/// A full original transaction payload correlated with typed indexed source facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionFields")]
pub struct Transaction {
    network: Network,
    data: TransactionData,
    cbor: TransactionCbor,
}
impl Transaction {
    /// Correlates source identity/fee/validity with original CBOR evidence.
    /// For failed scripts, encoded total collateral determines known paid fees;
    /// a missing collateral total leaves indexed fees separately source-reported.
    ///
    /// # Errors
    /// Rejects mismatched hashes, known paid-fee/validity, invalid interval,
    /// noninteger deposits, wrong-network/duplicate output assets or body bounds.
    pub fn new(
        network: Network,
        data: TransactionData,
        cbor: TransactionCbor,
    ) -> Result<Self, Error> {
        super::value::validate_assets(&data.output_amount, network.identity(), false)?;
        let paid_fee = if cbor.encoded_script_validity() == Some(false) {
            cbor.total_collateral()
        } else {
            cbor.body_fee()
        };
        if data.transaction_id != cbor.transaction_id()
            || paid_fee.is_some_and(|fee| data.fees.raw() != crate::domain::U256::from(fee))
            || cbor
                .encoded_script_validity()
                .is_some_and(|v| v != data.valid_contract)
            || !data.deposit.is_integer()
            || data.size_bytes == 0
            || data
                .invalid_before
                .zip(data.invalid_hereafter)
                .is_some_and(|(start, end)| start >= end)
        {
            return Err(invalid_transaction());
        }
        Ok(Self {
            network,
            data,
            cbor,
        })
    }
    /// Returns explicit expected network attribution.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns immutable indexed source facts.
    #[must_use]
    pub const fn data(&self) -> &TransactionData {
        &self.data
    }
    /// Returns original bounded CBOR/hash evidence without a script/signature verdict.
    #[must_use]
    pub const fn cbor(&self) -> &TransactionCbor {
        &self.cbor
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionFields {
    network: Network,
    data: TransactionData,
    cbor: TransactionCbor,
}
impl TryFrom<TransactionFields> for Transaction {
    type Error = Error;
    fn try_from(v: TransactionFields) -> Result<Self, Error> {
        Self::new(v.network, v.data, v.cbor)
    }
}

/// Actual transaction status returned by a current changing index.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransactionStatus {
    /// The source indexed this exact payload and inclusion/execution facts.
    Indexed {
        /// Actual correlated full transaction evidence.
        transaction: Box<Transaction>,
    },
    /// Source lookup returned unavailable, not proof of rejection or permanent absence.
    NotIndexed {
        /// Expected caller-selected network.
        network: Network,
        /// Exact transaction that was looked up.
        transaction_id: Hash,
    },
}
impl TransactionStatus {
    /// Returns actual source network attribution for either lookup outcome.
    #[must_use]
    pub fn network(&self) -> &Network {
        match self {
            Self::Indexed { transaction } => transaction.network(),
            Self::NotIndexed { network, .. } => network,
        }
    }
}

/// Typed source transaction input facts; references and collateral are separate roles.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedInput {
    /// Exact source address.
    pub address: PaymentAddress,
    /// Exact referenced transaction hash.
    pub transaction_id: Hash,
    /// Exact referenced output index.
    pub output_index: u16,
    /// Exact source quantities.
    #[serde(deserialize_with = "indexed_assets")]
    pub assets: Vec<AssetAmount>,
    /// Source datum/reference-script evidence, not a script execution verdict.
    pub data: OutputData,
    /// Source collateral role.
    pub collateral: bool,
    /// Source reference-input role; absence remains unreported.
    pub reference: Option<bool>,
}
/// Typed source transaction output facts with honest current consumption metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedOutput {
    /// Exact source destination.
    pub address: PaymentAddress,
    /// Exact output index within the requested transaction.
    pub output_index: u16,
    /// Exact source quantities.
    #[serde(deserialize_with = "indexed_assets")]
    pub assets: Vec<AssetAmount>,
    /// Source datum/reference-script evidence.
    pub data: OutputData,
    /// Source collateral-return classification.
    pub collateral: bool,
    /// Actual nullable source consuming transaction; it is not an evaluation snapshot.
    pub consumed_by: Option<Hash>,
}
/// Complete bounded indexed transaction input/output response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionUtxosFields")]
pub struct TransactionUtxos {
    network: Network,
    transaction_id: Hash,
    inputs: Vec<IndexedInput>,
    outputs: Vec<IndexedOutput>,
}
impl TransactionUtxos {
    /// Checks exact source transaction binding and bounded unique input/output facts.
    ///
    /// # Errors
    /// Rejects more than 1000 inputs/outputs, duplicates, role conflicts or mismatched
    /// network/asset identities; spending/finality is not independently verified.
    pub fn new(
        network: Network,
        transaction_id: Hash,
        inputs: Vec<IndexedInput>,
        outputs: Vec<IndexedOutput>,
    ) -> Result<Self, Error> {
        if inputs.len() > 1000 || outputs.len() > 1000 {
            return Err(invalid_transaction());
        }
        let mut keys = HashSet::new();
        for v in &inputs {
            if !v.address.is_compatible_with(network.identity())
                || !keys.insert((v.transaction_id, v.output_index))
                || v.collateral && v.reference == Some(true)
            {
                return Err(invalid_transaction());
            }
            super::value::validate_assets(&v.assets, network.identity(), true)?;
        }
        let mut keys = HashSet::new();
        for v in &outputs {
            if !v.address.is_compatible_with(network.identity()) || !keys.insert(v.output_index) {
                return Err(invalid_transaction());
            }
            super::value::validate_assets(&v.assets, network.identity(), true)?;
        }
        Ok(Self {
            network,
            transaction_id,
            inputs,
            outputs,
        })
    }
    /// Returns expected network attribution.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns the exact source/request-correlated transaction.
    #[must_use]
    pub const fn transaction_id(&self) -> Hash {
        self.transaction_id
    }
    /// Returns all retained source input facts.
    #[must_use]
    pub fn inputs(&self) -> &[IndexedInput] {
        &self.inputs
    }
    /// Returns all retained source output facts.
    #[must_use]
    pub fn outputs(&self) -> &[IndexedOutput] {
        &self.outputs
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionUtxosFields {
    network: Network,
    transaction_id: Hash,
    #[serde(deserialize_with = "bounded_inputs")]
    inputs: Vec<IndexedInput>,
    #[serde(deserialize_with = "bounded_outputs")]
    outputs: Vec<IndexedOutput>,
}
fn bounded_inputs<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<IndexedInput>, D::Error> {
    super::value::bounded_values::<D, IndexedInput, 1000>(d)
}
fn bounded_outputs<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<IndexedOutput>, D::Error> {
    super::value::bounded_values::<D, IndexedOutput, 1000>(d)
}
impl TryFrom<TransactionUtxosFields> for TransactionUtxos {
    type Error = Error;
    fn try_from(v: TransactionUtxosFields) -> Result<Self, Error> {
        Self::new(v.network, v.transaction_id, v.inputs, v.outputs)
    }
}

/// Retains separately supplied optional datum and reference-script fields.
#[must_use]
pub const fn output_data(
    datum: Option<Hash>,
    inline: Option<HexData>,
    script: Option<super::ScriptHash>,
) -> OutputData {
    OutputData::new(datum, inline, script)
}
