// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Canonical transaction bytes and separately attributed indexed transaction facts.

use std::{collections::BTreeSet, fmt};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{SeqAccess, Visitor},
};

use super::{Address, Network, Satoshis, TransactionStatus, Txid, Wtxid};
use crate::error::{Error, ValidationError};

const MAX_WEIGHT: u64 = bitcoin::Weight::MAX_BLOCK.to_wu();
// Minimum base input/output sizes consume 41*4 and 9*4 weight units each.
pub(crate) const MAX_INPUTS: usize = 4_000_000 / (41 * 4);
#[cfg(feature = "bitcoin-esplora")]
pub(crate) const MAX_OUTPUTS: usize = 4_000_000 / (9 * 4);

fn invalid_transaction() -> Error {
    ValidationError::InvalidBitcoinTransaction.into()
}

/// Bounded exact transaction/script/witness bytes with canonical hexadecimal serde.
///
/// The bound comes from the maintained Bitcoin decoder's four-million-byte
/// limit. Raw transaction structure and aggregate weight are checked separately.
/// Empty bytes are valid for scripts and witness elements.
#[derive(Clone, Eq, PartialEq)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    /// Maximum decoded byte length accepted by the maintained Bitcoin codec.
    pub const MAX_LEN: usize = bitcoin::consensus::encode::MAX_VEC_SIZE;

    /// Records exact bytes after enforcing the protocol decoder's input bound.
    ///
    /// # Errors
    /// Rejects more than [`Self::MAX_LEN`] bytes without echoing supplied data.
    pub fn new(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > Self::MAX_LEN {
            return Err(ValidationError::InvalidBitcoinBytes.into());
        }
        Ok(Self(bytes))
    }

    /// Parses bounded canonical lowercase hexadecimal with no prefix or whitespace.
    ///
    /// # Errors
    /// Rejects odd width, noncanonical encoding and decoded values exceeding the bound.
    pub fn from_hex(value: &str) -> Result<Self, Error> {
        if value.len() > 2 * Self::MAX_LEN
            || !value.len().is_multiple_of(2)
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ValidationError::InvalidBitcoinBytes.into());
        }
        const_hex::decode(value)
            .map(Self)
            .map_err(|_| ValidationError::InvalidBitcoinBytes.into())
    }

    /// Returns exact bytes, including legitimate empty script/witness data.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Bytes")
            .field("length", &self.0.len())
            .finish()
    }
}
impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&const_hex::encode(&self.0))
    }
}
impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BytesVisitor;
        impl Visitor<'_> for BytesVisitor {
            type Value = Bytes;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("bounded canonical Bitcoin hexadecimal bytes")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Bytes, E> {
                Bytes::from_hex(value).map_err(E::custom)
            }
        }
        deserializer.deserialize_str(BytesVisitor)
    }
}

/// The exact identity of an input's previously created output.
///
/// Coinbase null outpoints are represented by `None` in [`TransactionInput`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OutPointFields")]
pub struct OutPoint {
    txid: Txid,
    vout: u32,
}
impl OutPoint {
    /// Records a non-null output identity.
    ///
    /// # Errors
    /// Rejects the all-zero transaction ID plus `u32::MAX` coinbase null sentinel.
    pub fn new(txid: Txid, vout: u32) -> Result<Self, Error> {
        if txid.to_string() == "0".repeat(64) && vout == u32::MAX {
            return Err(invalid_transaction());
        }
        Ok(Self { txid, vout })
    }
    /// Returns the previous transaction's ID.
    #[must_use]
    pub const fn txid(self) -> Txid {
        self.txid
    }
    /// Returns the exact output index.
    #[must_use]
    pub const fn vout(self) -> u32 {
        self.vout
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutPointFields {
    txid: Txid,
    vout: u32,
}
impl TryFrom<OutPointFields> for OutPoint {
    type Error = Error;
    fn try_from(value: OutPointFields) -> Result<Self, Error> {
        Self::new(value.txid, value.vout)
    }
}

/// A borrowed typed view of one canonically decoded input.
#[derive(Clone, Copy, Debug)]
pub struct TransactionInput<'a>(&'a bitcoin::TxIn);
impl<'a> TransactionInput<'a> {
    /// Returns a previous output, or `None` for the actual coinbase null outpoint.
    #[must_use]
    pub fn previous_output(self) -> Option<OutPoint> {
        (!self.0.previous_output.is_null()).then_some(OutPoint {
            txid: Txid::from_native(self.0.previous_output.txid),
            vout: self.0.previous_output.vout,
        })
    }
    /// Returns the exact encoded input script, without interpreting signatures.
    #[must_use]
    pub fn script_sig(self) -> &'a [u8] {
        self.0.script_sig.as_bytes()
    }
    /// Returns the exact sequence number.
    #[must_use]
    pub const fn sequence(self) -> u32 {
        self.0.sequence.0
    }
    /// Returns the ordered exact witness stack, including empty elements.
    #[must_use]
    pub fn witness(self) -> impl ExactSizeIterator<Item = &'a [u8]> {
        self.0.witness.iter()
    }
}

/// A borrowed typed view of one canonically decoded output.
#[derive(Clone, Copy, Debug)]
pub struct TransactionOutput<'a>(&'a bitcoin::TxOut);
impl<'a> TransactionOutput<'a> {
    /// Returns exact satoshis, independently of script interpretation.
    #[must_use]
    pub fn value(self) -> Satoshis {
        Satoshis::new(self.0.value.to_sat())
    }
    /// Returns exact output script bytes.
    #[must_use]
    pub fn script_pubkey(self) -> &'a [u8] {
        self.0.script_pubkey.as_bytes()
    }
    /// Derives a standard address for the explicitly selected network, if recognized.
    ///
    /// Unknown, bare-public-key and data-carrier scripts retain their bytes and
    /// return `None`; no synthetic address is supplied.
    #[must_use]
    pub fn address(self, network: Network) -> Option<Address> {
        Address::from_script(&self.0.script_pubkey, network)
    }
}

/// Canonically decoded network-independent Bitcoin transaction bytes.
///
/// Uses maintained Bitcoin consensus decoding and exact re-encoding. This
/// wrapper explicitly checks transaction structure, bounded block
/// weight, output-money bounds and identifiers. It does not execute scripts,
/// verify signatures, authenticate previous outputs or prove chain inclusion.
/// Serialized as canonical raw transaction hex; getters derive typed fields.
#[derive(Clone, Eq, PartialEq)]
pub struct TransactionBody(bitcoin::Transaction);
impl TransactionBody {
    /// Decodes one complete bounded canonical transaction, retaining its exact fields.
    ///
    /// # Errors
    /// Rejects malformed/trailing/noncanonical bytes, empty inputs/outputs,
    /// invalid null/duplicate outpoints or coinbase scripts, output values/sums
    /// above maintained `MAX_MONEY`, and weight above the Bitcoin block limit.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > Bytes::MAX_LEN {
            return Err(invalid_transaction());
        }
        let transaction: bitcoin::Transaction =
            bitcoin::consensus::deserialize(bytes).map_err(|_| invalid_transaction())?;
        if transaction.input.is_empty()
            || transaction.output.is_empty()
            || transaction.weight().to_wu() > MAX_WEIGHT
            || bitcoin::consensus::serialize(&transaction) != bytes
        {
            return Err(invalid_transaction());
        }
        let mut seen = BTreeSet::new();
        for input in &transaction.input {
            if !seen.insert(input.previous_output)
                || (!transaction.is_coinbase() && input.previous_output.is_null())
                || (transaction.is_coinbase() && !(2..=100).contains(&input.script_sig.len()))
            {
                return Err(invalid_transaction());
            }
        }
        let mut output_sum = 0_u64;
        for output in &transaction.output {
            output_sum = output_sum
                .checked_add(output.value.to_sat())
                .ok_or_else(invalid_transaction)?;
            if output.value.to_sat() > bitcoin::Amount::MAX_MONEY.to_sat()
                || output_sum > bitcoin::Amount::MAX_MONEY.to_sat()
            {
                return Err(invalid_transaction());
            }
        }
        Ok(Self(transaction))
    }
    /// Decodes bounded canonical lowercase transaction hexadecimal.
    ///
    /// # Errors
    /// Returns fixed byte/transaction validation errors, never supplied hex text.
    pub fn from_hex(value: &str) -> Result<Self, Error> {
        Self::from_bytes(Bytes::from_hex(value)?.as_slice())
    }
    /// Returns canonical consensus bytes, including any witness data.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        bitcoin::consensus::serialize(&self.0)
    }
    /// Returns the computed identifier excluding witness data.
    #[must_use]
    pub fn txid(&self) -> Txid {
        Txid::from_native(self.0.compute_txid())
    }
    /// Returns the computed identifier including witness data.
    #[must_use]
    pub fn wtxid(&self) -> Wtxid {
        Wtxid::from_native(self.0.compute_wtxid())
    }
    /// Returns the exact signed transaction version.
    #[must_use]
    pub const fn version(&self) -> i32 {
        self.0.version.0
    }
    /// Returns the exact consensus lock-time value, without policy inference.
    #[must_use]
    pub fn lock_time(&self) -> u32 {
        self.0.lock_time.to_consensus_u32()
    }
    /// Returns actual coinbase structure, independently of indexed field absence.
    #[must_use]
    pub fn is_coinbase(&self) -> bool {
        self.0.is_coinbase()
    }
    /// Returns the canonical total encoded byte size.
    #[must_use]
    pub fn size(&self) -> usize {
        self.0.total_size()
    }
    /// Returns exact Bitcoin weight units.
    #[must_use]
    pub fn weight(&self) -> u64 {
        self.0.weight().to_wu()
    }
    /// Returns the ceiling of weight divided by four, in virtual bytes.
    #[must_use]
    pub fn vsize(&self) -> usize {
        self.0.vsize()
    }
    /// Returns ordered borrowed typed input views.
    pub fn inputs(&self) -> impl ExactSizeIterator<Item = TransactionInput<'_>> {
        self.0.input.iter().map(TransactionInput)
    }
    /// Returns ordered borrowed typed output views.
    pub fn outputs(&self) -> impl ExactSizeIterator<Item = TransactionOutput<'_>> {
        self.0.output.iter().map(TransactionOutput)
    }
}
impl fmt::Debug for TransactionBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransactionBody")
            .field("txid", &self.txid())
            .field("wtxid", &self.wtxid())
            .field("size", &self.size())
            .field("inputs", &self.0.input.len())
            .field("outputs", &self.0.output.len())
            .finish()
    }
}
impl Serialize for TransactionBody {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&const_hex::encode(self.to_bytes()))
    }
}
impl<'de> Deserialize<'de> for TransactionBody {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = Bytes::deserialize(deserializer)?;
        Self::from_bytes(bytes.as_slice()).map_err(serde::de::Error::custom)
    }
}

/// Source-reported previous output data, separate from the raw transaction body.
///
/// Arithmetic consistency does not authenticate this record against the
/// previous transaction's commitment. Exact scripts remain available even when
/// no standard address can be derived.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PreviousOutputFields")]
pub struct PreviousOutput {
    value: Satoshis,
    script_pubkey: Bytes,
}
impl PreviousOutput {
    /// Records a bounded script and a source-reported exact Bitcoin output value.
    ///
    /// # Errors
    /// Rejects values exceeding the maintained Bitcoin maximum money range.
    pub fn new(value: Satoshis, script_pubkey: Bytes) -> Result<Self, Error> {
        if value.raw() > bitcoin::Amount::MAX_MONEY.to_sat() {
            return Err(invalid_transaction());
        }
        Ok(Self {
            value,
            script_pubkey,
        })
    }
    /// Returns source-reported exact satoshis.
    #[must_use]
    pub const fn value(&self) -> Satoshis {
        self.value
    }
    /// Returns the source-reported exact output script.
    #[must_use]
    pub fn script_pubkey(&self) -> &[u8] {
        self.script_pubkey.as_slice()
    }
    /// Derives a standard address for the explicitly selected network, if recognized.
    #[must_use]
    pub fn address(&self, network: Network) -> Option<Address> {
        Address::from_script(bitcoin::Script::from_bytes(self.script_pubkey()), network)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviousOutputFields {
    value: Satoshis,
    script_pubkey: Bytes,
}
impl TryFrom<PreviousOutputFields> for PreviousOutput {
    type Error = Error;
    fn try_from(value: PreviousOutputFields) -> Result<Self, Error> {
        Self::new(value.value, value.script_pubkey)
    }
}

/// Canonical transaction bytes with distinct nullable indexed fee/previous-output facts.
///
/// `previous_outputs` follows input order. `None` is legitimate for coinbase;
/// on ordinary inputs it means indexed data unavailable, not a zero-value input.
/// Missing fee remains `None`. Reported inclusion is not lasting finality.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionFields")]
pub struct Transaction {
    network: Network,
    body: TransactionBody,
    previous_outputs: Vec<Option<PreviousOutput>>,
    reported_fee: Option<Satoshis>,
    status: TransactionStatus,
}
impl Transaction {
    /// Combines canonical bytes with separately supplied index facts and network.
    ///
    /// # Errors
    /// Rejects differing input counts, non-null coinbase previous data, coinbase
    /// reported fees other than zero, impossible known funding sums, or a
    /// supplied fee differing from complete previous-output arithmetic.
    pub fn new(
        network: Network,
        body: TransactionBody,
        previous_outputs: Vec<Option<PreviousOutput>>,
        reported_fee: Option<Satoshis>,
        status: TransactionStatus,
    ) -> Result<Self, Error> {
        if previous_outputs.len() != body.inputs().len()
            || (body.is_coinbase()
                && (previous_outputs.iter().any(Option::is_some)
                    || reported_fee.is_some_and(|fee| fee.raw() != 0)))
        {
            return Err(invalid_transaction());
        }
        let value = Self {
            network,
            body,
            previous_outputs,
            reported_fee,
            status,
        };
        let output_sum = value
            .body
            .outputs()
            .try_fold(0_u64, |sum, output| sum.checked_add(output.value().raw()))
            .ok_or_else(invalid_transaction)?;
        let reported_funding = value
            .reported_fee
            .map(|fee| {
                output_sum
                    .checked_add(fee.raw())
                    .filter(|funding| *funding <= bitcoin::Amount::MAX_MONEY.to_sat())
                    .ok_or_else(invalid_transaction)
            })
            .transpose()?;
        if !value.body.is_coinbase() {
            let mut complete = true;
            let mut input_sum = 0_u64;
            for output in &value.previous_outputs {
                if let Some(output) = output {
                    input_sum = input_sum
                        .checked_add(output.value.raw())
                        .ok_or_else(invalid_transaction)?;
                } else {
                    complete = false;
                }
            }
            if input_sum > bitcoin::Amount::MAX_MONEY.to_sat()
                || reported_funding.is_some_and(|funding| input_sum > funding)
                || (complete
                    && (input_sum < output_sum
                        || value
                            .reported_fee
                            .is_some_and(|reported| reported.raw() != input_sum - output_sum)))
            {
                return Err(invalid_transaction());
            }
        }
        Ok(value)
    }
    /// Returns the caller-qualified network, independently of raw transaction bytes.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
    /// Returns canonical decoded network-independent transaction bytes and fields.
    #[must_use]
    pub const fn body(&self) -> &TransactionBody {
        &self.body
    }
    /// Returns nullable source-reported previous outputs, in exact input order.
    #[must_use]
    pub fn previous_outputs(&self) -> &[Option<PreviousOutput>] {
        &self.previous_outputs
    }
    /// Returns nullable source-reported fee; coinbase zero is the index's convention.
    #[must_use]
    pub const fn reported_fee(&self) -> Option<Satoshis> {
        self.reported_fee
    }
    /// Computes a fee only when all ordinary previous outputs are supplied.
    ///
    /// This is arithmetic over source-reported previous outputs, not independent
    /// authentication. Coinbase has no fee-paying input and returns `None`.
    #[must_use]
    pub fn fee_from_previous_outputs(&self) -> Option<Satoshis> {
        if self.body.is_coinbase() {
            return None;
        }
        let total = self
            .previous_outputs
            .iter()
            .try_fold(0_u64, |sum, output| {
                sum.checked_add(output.as_ref()?.value.raw())
            })?;
        let outputs = self
            .body
            .outputs()
            .try_fold(0_u64, |sum, output| sum.checked_add(output.value().raw()))?;
        total.checked_sub(outputs).map(Satoshis::new)
    }
    /// Returns distinct unconfirmed or complete source-reported inclusion.
    #[must_use]
    pub const fn status(&self) -> TransactionStatus {
        self.status
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionFields {
    network: Network,
    body: TransactionBody,
    #[serde(deserialize_with = "deserialize_previous_outputs")]
    previous_outputs: Vec<Option<PreviousOutput>>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    reported_fee: Option<Satoshis>,
    status: TransactionStatus,
}
impl TryFrom<TransactionFields> for Transaction {
    type Error = Error;
    fn try_from(value: TransactionFields) -> Result<Self, Error> {
        Self::new(
            value.network,
            value.body,
            value.previous_outputs,
            value.reported_fee,
            value.status,
        )
    }
}

fn deserialize_previous_outputs<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Option<PreviousOutput>>, D::Error> {
    deserialize_bounded_vec::<_, _, MAX_INPUTS>(deserializer)
}

pub(crate) fn deserialize_bounded_vec<
    'de,
    D: Deserializer<'de>,
    T: Deserialize<'de>,
    const MAX: usize,
>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    struct BoundedVisitor<T, const MAX: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const MAX: usize> Visitor<'de> for BoundedVisitor<T, MAX> {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded Bitcoin transaction collection")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Vec<T>, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = sequence.next_element()? {
                if values.len() == MAX {
                    return Err(serde::de::Error::custom(
                        ValidationError::InvalidBitcoinTransaction,
                    ));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(BoundedVisitor::<T, MAX>(std::marker::PhantomData))
}
