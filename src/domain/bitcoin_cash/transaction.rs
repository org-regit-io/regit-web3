// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    Address, AddressNamespace, BlockHash, BlockHeader, CollectionLimit, Satoshis, TokenData, Txid,
    identity::sha256d_display,
    records::{MAX_ENTRIES, MAX_MONEY, bounded_entries},
};
use crate::error::{Error, ValidationError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashSet, fmt};

fn optional_addresses<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<Address>>, D::Error> {
    #[derive(Deserialize)]
    struct Addresses(#[serde(deserialize_with = "super::records::bounded_addresses")] Vec<Address>);
    let values = Option::<Addresses>::deserialize(d)?.map(|v| v.0);
    if values.as_ref().is_some_and(|v| v.len() > 100) {
        return Err(serde::de::Error::custom("address collection exceeds limit"));
    }
    Ok(values)
}

/// Exact opaque bytes bounded to one MiB; diagnostics retain only their length.
/// This local resource bound does not declare Bitcoin Cash consensus limits.
#[derive(Clone, Eq, PartialEq)]
pub struct Bytes(Vec<u8>);
impl Bytes {
    /// Maximum retained bytes per opaque field.
    pub const MAX_BYTES: usize = 1024 * 1024;
    /// Retains exact bytes without interpretation or signature validation.
    /// # Errors
    /// Rejects excessive length.
    pub fn new(value: Vec<u8>) -> Result<Self, Error> {
        if value.len() > Self::MAX_BYTES {
            return Err(ValidationError::InvalidBitcoinCashBytes.into());
        }
        Ok(Self(value))
    }
    /// Decodes even-width hexadecimal, accepting empty scripts and commitments.
    /// # Errors
    /// Rejects prefixed, malformed or excessive encodings.
    pub fn from_hex(value: &str) -> Result<Self, Error> {
        if value.len() > 2 * Self::MAX_BYTES
            || !value.len().is_multiple_of(2)
            || value.starts_with("0x")
        {
            return Err(ValidationError::InvalidBitcoinCashBytes.into());
        }
        Self::new(
            const_hex::decode(value)
                .map_err(|_| Error::Validation(ValidationError::InvalidBitcoinCashBytes))?,
        )
    }
    /// Returns the exact retained bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
    /// Returns lowercase hexadecimal with no prefix.
    #[must_use]
    pub fn hex(&self) -> String {
        const_hex::encode(&self.0)
    }
}
impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bytes")
            .field("length", &self.0.len())
            .finish()
    }
}
impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}
impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::from_hex(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Short opaque source classification text; diagnostics exclude its contents.
#[derive(Clone, Eq, PartialEq)]
pub struct SourceText(String);
impl SourceText {
    /// Retains up to 128 printable source bytes without interpreting the classification.
    /// # Errors
    /// Rejects empty, excessive or control-character text.
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(super::invalid());
        }
        Ok(Self(value))
    }
    /// Returns the retained source text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SourceText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceText")
            .field("length", &self.0.len())
            .finish()
    }
}
impl Serialize for SourceText {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for SourceText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Opaque source transaction bytes and their maintained double-`SHA256` identity.
/// Bytes are not parsed as Bitcoin consensus transactions. Script, `CashToken`,
/// structure, signature and consensus validity remain unverified by this type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawFields")]
pub struct RawTransaction {
    bytes: Bytes,
    txid: Txid,
}
impl RawTransaction {
    /// Computes exact byte identity without interpreting a BCH body.
    /// # Errors
    /// Rejects empty bytes; the opaque byte resource bound also applies.
    pub fn new(bytes: Bytes) -> Result<Self, Error> {
        if bytes.as_slice().is_empty() {
            return Err(super::invalid());
        }
        let txid = Txid::from_display_bytes(sha256d_display(bytes.as_slice()));
        Ok(Self { bytes, txid })
    }
    /// Returns exact opaque bytes.
    #[must_use]
    pub const fn bytes(&self) -> &Bytes {
        &self.bytes
    }
    /// Returns the computed full transaction identity.
    #[must_use]
    pub const fn txid(&self) -> Txid {
        self.txid
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFields {
    bytes: Bytes,
    txid: Txid,
}
impl TryFrom<RawFields> for RawTransaction {
    type Error = Error;
    fn try_from(v: RawFields) -> Result<Self, Error> {
        let result = Self::new(v.bytes)?;
        if result.txid != v.txid {
            return Err(super::invalid());
        }
        Ok(result)
    }
}

/// Source-reported height for an exact transaction query.
/// Zero is preserved as source metadata: genesis is not indexed by some servers,
/// so a zero result is not independent proof that the transaction is in a mempool.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceHeight {
    /// The source returned explicit null, meaning unknown under the protocol.
    Unknown,
    /// The source returned numeric zero, documented as unconfirmed.
    Zero,
    /// The source reported a positive confirmed height.
    Positive {
        /// The exact source block height.
        height: u32,
    },
}

/// Source asserted transaction inclusion, with checked header-byte/hash agreement.
/// No transaction Merkle proof, `PoW`, chainwork or finality proof is claimed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "InclusionFields")]
pub struct SourceInclusion {
    height: u32,
    block_hash: BlockHash,
    header: BlockHeader,
}
impl SourceInclusion {
    /// Correlates source height/hash with the exact supplied header's computed identity.
    /// # Errors
    /// Rejects zero height or hash/header disagreement.
    pub fn new(height: u32, block_hash: BlockHash, header: BlockHeader) -> Result<Self, Error> {
        if height == 0 || header.hash() != block_hash {
            return Err(super::invalid());
        }
        Ok(Self {
            height,
            block_hash,
            header,
        })
    }
    /// Returns exact source inclusion height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    /// Returns complete source block identity, checked against its header bytes.
    #[must_use]
    pub const fn block_hash(&self) -> BlockHash {
        self.block_hash
    }
    /// Returns exact header bytes, without consensus or chainwork validation.
    #[must_use]
    pub const fn header(&self) -> &BlockHeader {
        &self.header
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InclusionFields {
    height: u32,
    block_hash: BlockHash,
    header: BlockHeader,
}
impl TryFrom<InclusionFields> for SourceInclusion {
    type Error = Error;
    fn try_from(v: InclusionFields) -> Result<Self, Error> {
        Self::new(v.height, v.block_hash, v.header)
    }
}

/// Correlated source transaction status for one exact identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StatusFields")]
pub struct TransactionStatus {
    txid: Txid,
    source_height: SourceHeight,
    inclusion: Option<SourceInclusion>,
}
impl TransactionStatus {
    /// Retains source status, requiring exact positive-height/header correlation.
    /// # Errors
    /// Rejects zero positive heights, missing inclusion or contradictory inclusion facts.
    pub fn new(
        txid: Txid,
        source_height: SourceHeight,
        inclusion: Option<SourceInclusion>,
    ) -> Result<Self, Error> {
        match source_height {
            SourceHeight::Positive { height }
                if height == 0 || inclusion.as_ref().is_none_or(|v| v.height() != height) =>
            {
                return Err(super::invalid());
            }
            SourceHeight::Unknown | SourceHeight::Zero if inclusion.is_some() => {
                return Err(super::invalid());
            }
            _ => {}
        }
        Ok(Self {
            txid,
            source_height,
            inclusion,
        })
    }
    /// Returns the exact query identity.
    #[must_use]
    pub const fn txid(&self) -> Txid {
        self.txid
    }
    /// Returns the source height classification, without invented pending or finality facts.
    #[must_use]
    pub const fn source_height(&self) -> SourceHeight {
        self.source_height
    }
    /// Returns supplied positive-height source inclusion facts.
    #[must_use]
    pub const fn inclusion(&self) -> Option<&SourceInclusion> {
        self.inclusion.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusFields {
    txid: Txid,
    source_height: SourceHeight,
    inclusion: Option<SourceInclusion>,
}
impl TryFrom<StatusFields> for TransactionStatus {
    type Error = Error;
    fn try_from(v: StatusFields) -> Result<Self, Error> {
        Self::new(v.txid, v.source_height, v.inclusion)
    }
}

/// BCH daemon source input fields, without canonical body or script verification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransactionInput {
    /// A source-classified coinbase input; classification is not independently proven.
    Coinbase {
        /// Exact source coinbase unlocking bytes.
        unlocking_bytes: Bytes,
        /// Exact 32-bit source sequence.
        sequence: u32,
    },
    /// Source ordinary input with concrete previous output identity.
    Ordinary {
        /// Complete previous transaction identifier.
        previous_txid: Txid,
        /// Previous transaction output index.
        output_index: u32,
        /// Exact source unlocking bytecode.
        unlocking_bytes: Bytes,
        /// Exact source sequence.
        sequence: u32,
    },
}

/// One complete source BCH transaction output and optional supplied `CashToken` fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionOutput {
    /// Source zero-based transaction output index.
    pub index: u32,
    /// Exact native BCH atomic quantity converted without rounding.
    pub value: Satoshis,
    /// Exact source locking bytecode; source token prefix fields stay separate.
    pub locking_bytes: Bytes,
    /// Source daemon classification, retained without inferring unsupported script semantics.
    pub script_type: SourceText,
    /// Actual supplied addresses, compatible with the explicit namespace.
    #[serde(default, deserialize_with = "optional_addresses")]
    pub addresses: Option<Vec<Address>>,
    /// Source required-signature count when supplied, without fabricated default.
    pub required_signatures: Option<u32>,
    /// Actual source token metadata; absence does not independently prove a token-free raw body.
    pub token_data: Option<TokenData>,
}

/// Source verbose transaction fields and exact raw bytes, kept as separate evidence.
/// The raw body is not consensus-decoded or compared field-by-field against source JSON.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionData {
    /// Explicit compatible address namespace for supplied output addresses.
    pub namespace: AddressNamespace,
    /// Exact opaque body and computed byte identity.
    pub raw: RawTransaction,
    /// Source-reported transaction identifier, checked against query and raw-byte identity.
    pub reported_txid: Txid,
    /// Optional actual source hash field, with no synthesized identity.
    pub reported_hash: Option<Txid>,
    /// Exact signed source version.
    pub version: i32,
    /// Exact source lock time, not substituted when missing.
    pub lock_time: u32,
    /// Exact source serialized byte size, checked against retained raw length.
    pub size: u32,
    /// Exact source input facts; previous output values and fees remain unavailable.
    #[serde(deserialize_with = "bounded_entries")]
    pub inputs: Vec<TransactionInput>,
    /// Exact source output/script/token facts.
    #[serde(deserialize_with = "bounded_entries")]
    pub outputs: Vec<TransactionOutput>,
    /// Source confirmation count when actually supplied.
    pub reported_confirmations: Option<u32>,
    /// Source block timestamp when actually supplied.
    pub reported_block_time: Option<u64>,
    /// Source block identity when actually supplied.
    pub reported_block_hash: Option<BlockHash>,
}

/// Checked complete source transaction fields and separately correlated status.
/// Validation checks source structure and exact identities; it does not establish
/// raw/JSON field agreement, consensus, signatures, `CashToken` authorization or funding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionFields")]
pub struct Transaction {
    query_txid: Txid,
    limit: CollectionLimit,
    data: TransactionData,
    status: TransactionStatus,
}
impl Transaction {
    /// Checks source collection/value/identity consistency and complete explicit capacity.
    /// # Errors
    /// Rejects incomplete/empty collections, duplicate/null outpoints, mixed coinbase,
    /// excessive or overflowing outputs, incompatible addresses and contradictory identities.
    pub fn new(
        query_txid: Txid,
        limit: CollectionLimit,
        data: TransactionData,
        status: TransactionStatus,
    ) -> Result<Self, Error> {
        if data.raw.txid() != query_txid
            || data.reported_txid != query_txid
            || data.reported_hash.is_some_and(|v| v != query_txid)
            || status.txid() != query_txid
            || data.size as usize != data.raw.bytes().as_slice().len()
            || data.inputs.is_empty()
            || data.outputs.is_empty()
            || data.inputs.len() > limit.get() as usize
            || data.outputs.len() > limit.get() as usize
            || data.inputs.len() > MAX_ENTRIES
            || data.outputs.len() > MAX_ENTRIES
        {
            return Err(super::invalid());
        }
        let mut outpoints = HashSet::new();
        for input in &data.inputs {
            match input {
                TransactionInput::Coinbase { .. } if data.inputs.len() != 1 => {
                    return Err(super::invalid());
                }
                TransactionInput::Ordinary {
                    previous_txid,
                    output_index,
                    ..
                } if !outpoints.insert((*previous_txid, *output_index))
                    || previous_txid.display_bytes() == [0; 32] && *output_index == u32::MAX =>
                {
                    return Err(super::invalid());
                }
                TransactionInput::Coinbase { .. } | TransactionInput::Ordinary { .. } => {}
            }
        }
        let mut total = 0_u64;
        for (position, output) in data.outputs.iter().enumerate() {
            total = total
                .checked_add(output.value.raw())
                .ok_or_else(super::invalid)?;
            if output.index as usize != position
                || total > MAX_MONEY
                || output.addresses.as_ref().is_some_and(|values| {
                    values.len() > 100 || values.iter().any(|v| v.namespace() != data.namespace)
                })
            {
                return Err(super::invalid());
            }
        }
        if let Some(inclusion) = status.inclusion() {
            if data
                .reported_block_hash
                .is_some_and(|v| v != inclusion.block_hash())
                || data.reported_confirmations == Some(0)
            {
                return Err(super::invalid());
            }
        } else if data.reported_block_hash.is_some()
            || data.reported_block_time.is_some()
            || data.reported_confirmations.is_some_and(|v| v > 0)
        {
            return Err(super::invalid());
        }
        Ok(Self {
            query_txid,
            limit,
            data,
            status,
        })
    }
    /// Returns the exact query identity, checked against source and raw byte hash.
    #[must_use]
    pub const fn txid(&self) -> Txid {
        self.query_txid
    }
    /// Returns the complete exact source fields and opaque body evidence.
    #[must_use]
    pub const fn data(&self) -> &TransactionData {
        &self.data
    }
    /// Returns separately correlated source status.
    #[must_use]
    pub const fn status(&self) -> &TransactionStatus {
        &self.status
    }
    /// Returns the caller hard capacity applied to each full input/output collection.
    #[must_use]
    pub const fn limit(&self) -> CollectionLimit {
        self.limit
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionFields {
    query_txid: Txid,
    limit: CollectionLimit,
    data: TransactionData,
    status: TransactionStatus,
}
impl TryFrom<TransactionFields> for Transaction {
    type Error = Error;
    fn try_from(v: TransactionFields) -> Result<Self, Error> {
        Self::new(v.query_txid, v.limit, v.data, v.status)
    }
}
