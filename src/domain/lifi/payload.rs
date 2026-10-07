// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Account, Chain, Family, Identifier};
use crate::{
    domain::{Amount, BlockHash},
    error::Error,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Source payload encoding; encoding validation is distinct from intent proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadEncoding {
    /// EVM call bytes, hexadecimal with a `0x` prefix.
    EvmCallData,
    /// Solana serialized transaction bytes, canonical standard base64.
    SolanaBase64,
    /// Maintained Bitcoin PSBT parser accepted this BTC-qualified container.
    BitcoinPsbtHex,
    /// Other UTXO source bytes; no Bitcoin consensus/PSBT compatibility claim.
    UtxoHex,
    /// Sui SDK serialized transaction text; content remains source-reported.
    MoveSdkText,
    /// TRON raw protobuf bytes; no protobuf/intent verification claim.
    TronProtobufHex,
    /// Stellar envelope bytes; base64 is checked, XDR semantics are unverified.
    StellarXdrBase64,
}

/// Exact bounded provider payload for explicit caller review.
/// Debug hides contents. Serialization/access deliberately expose the payload
/// for review; this type establishes encoding only, not an independently checked
/// transfer intent, signatures, freshness or executable success.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EncodedFields")]
pub struct EncodedPayload {
    chain: Chain,
    encoding: PayloadEncoding,
    text: String,
    contract_type: Option<Identifier>,
}
impl EncodedPayload {
    /// Maximum encoded source text length: 256 KiB; no truncation.
    pub const MAX_TEXT_BYTES: usize = 256 * 1024;
    /// Checks family/encoding agreement and bounded encoded bytes.
    /// Only a BTC-qualified PSBT invokes Bitcoin's maintained container parser.
    ///
    /// # Errors
    /// Rejects malformed encodings, contradictory families and oversized text.
    pub fn new(
        chain: Chain,
        encoding: PayloadEncoding,
        text: &str,
        contract_type: Option<Identifier>,
    ) -> Result<Self, Error> {
        if text.is_empty() || text.len() > Self::MAX_TEXT_BYTES {
            return Err(super::invalid_payload());
        }
        let mut normalized = text.to_owned();
        match encoding {
            PayloadEncoding::EvmCallData if chain.family() == Family::Evm => {
                normalized = hex(text, true)?;
            }
            PayloadEncoding::SolanaBase64 if chain.family() == Family::Solana => {
                base64(text)?;
            }
            PayloadEncoding::StellarXdrBase64 if chain.family() == Family::Stellar => {
                base64(text)?;
            }
            PayloadEncoding::BitcoinPsbtHex
                if chain.family() == Family::Utxo && chain.id() == 20_000_000_000_001 =>
            {
                normalized = hex(text, false)?;
                let bytes = const_hex::decode(&normalized).map_err(|_| super::invalid_payload())?;
                bitcoin::psbt::Psbt::deserialize(&bytes).map_err(|_| super::invalid_payload())?;
            }
            PayloadEncoding::UtxoHex
                if chain.family() == Family::Utxo && chain.id() != 20_000_000_000_001 =>
            {
                normalized = hex(text, false)?;
            }
            PayloadEncoding::TronProtobufHex if chain.family() == Family::Tron => {
                normalized = hex(text, false)?;
            }
            PayloadEncoding::MoveSdkText if chain.family() == Family::Move => {}
            _ => return Err(super::invalid_payload()),
        }
        if encoding != PayloadEncoding::TronProtobufHex && contract_type.is_some() {
            return Err(super::invalid_payload());
        }
        Ok(Self {
            chain,
            encoding,
            text: normalized,
            contract_type,
        })
    }
    /// Returns the explicitly qualified source chain.
    #[must_use]
    pub const fn chain(&self) -> Chain {
        self.chain
    }
    /// Returns which encoding checks were applied.
    #[must_use]
    pub const fn encoding(&self) -> PayloadEncoding {
        self.encoding
    }
    /// Returns exact payload text for caller review; it may contain signatures.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Returns an explicitly reported TRON contract type, without inventing one.
    #[must_use]
    pub const fn contract_type(&self) -> Option<&Identifier> {
        self.contract_type.as_ref()
    }
}
impl fmt::Debug for EncodedPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncodedPayload")
            .field("chain", &self.chain)
            .field("encoding", &self.encoding)
            .field("text_bytes", &self.text.len())
            .finish_non_exhaustive()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedFields {
    chain: Chain,
    encoding: PayloadEncoding,
    text: String,
    contract_type: Option<Identifier>,
}
impl TryFrom<EncodedFields> for EncodedPayload {
    type Error = Error;
    fn try_from(v: EncodedFields) -> Result<Self, Error> {
        Self::new(v.chain, v.encoding, &v.text, v.contract_type)
    }
}
fn hex(text: &str, prefixed: bool) -> Result<String, Error> {
    let bytes = if prefixed {
        text.strip_prefix("0x").ok_or_else(super::invalid_payload)?
    } else {
        text.strip_prefix("0x").unwrap_or(text)
    };
    if bytes.len() % 2 != 0
        || !bytes.bytes().all(|b| b.is_ascii_hexdigit())
        || (!prefixed && bytes.is_empty())
    {
        return Err(super::invalid_payload());
    }
    Ok(if prefixed {
        format!("0x{}", bytes.to_ascii_lowercase())
    } else {
        bytes.to_ascii_lowercase()
    })
}
fn base64(text: &str) -> Result<(), Error> {
    let bytes = STANDARD
        .decode(text)
        .map_err(|_| super::invalid_payload())?;
    if bytes.is_empty() || STANDARD.encode(&bytes) != text {
        return Err(super::invalid_payload());
    }
    Ok(())
}

/// An explicitly reported EVM access-list entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessEntry {
    /// The source-reported account on the same EVM chain.
    pub account: Account,
    /// Exact 32-byte storage keys; parent payload enforces aggregate bounds.
    pub storage_keys: Vec<BlockHash>,
}
/// Source-supplied EVM transaction fields, separate from transfer intent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmPayloadData {
    /// Explicit source chain.
    pub chain: Chain,
    /// Sender if actually supplied; missing is never filled from a request.
    pub from: Option<Account>,
    /// Source-supplied transaction target, which may be a protocol contract.
    pub to: Account,
    /// Exact native base units, with no assumed precision.
    pub value: Amount,
    /// Exact hexadecimal call bytes; content/intent is unverified.
    pub data: EncodedPayload,
    /// Exact gas limit, if supplied.
    pub gas_limit: Option<Amount>,
    /// Exact legacy gas price, if supplied.
    pub gas_price: Option<Amount>,
    /// Exact maximum priority fee, if supplied.
    pub max_priority_fee_per_gas: Option<Amount>,
    /// Exact maximum fee, if supplied.
    pub max_fee_per_gas: Option<Amount>,
    /// Explicit nonce, without node lookup.
    pub nonce: Option<u64>,
    /// Source transaction type byte, if supplied.
    pub transaction_type: Option<u8>,
    /// Missing and empty source access lists remain distinct.
    pub access_list: Option<Vec<AccessEntry>>,
}
/// Checked cross-field EVM source payload. No ABI intent verification is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EvmPayloadData", into = "EvmPayloadData")]
pub struct EvmPayload(EvmPayloadData);
impl EvmPayload {
    /// Checks chain agreement, access-list bounds and exact fee relationships.
    ///
    /// # Errors
    /// Rejects mismatched families, attached precision and contradictory fee caps.
    pub fn new(data: EvmPayloadData) -> Result<Self, Error> {
        if data.chain.family() != Family::Evm
            || data.to.chain() != data.chain
            || data.from.as_ref().is_some_and(|a| a.chain() != data.chain)
            || data.data.chain() != data.chain
            || data.data.encoding() != PayloadEncoding::EvmCallData
        {
            return Err(super::invalid_payload());
        }
        if std::iter::once(&data.value)
            .chain(
                [
                    data.gas_limit.as_ref(),
                    data.gas_price.as_ref(),
                    data.max_priority_fee_per_gas.as_ref(),
                    data.max_fee_per_gas.as_ref(),
                ]
                .into_iter()
                .flatten(),
            )
            .any(|a| a.decimals().is_some())
        {
            return Err(super::invalid_payload());
        }
        if let (Some(priority), Some(max)) = (data.max_priority_fee_per_gas, data.max_fee_per_gas)
            && priority.raw() > max.raw()
        {
            return Err(super::invalid_payload());
        }
        if let Some(entries) = &data.access_list
            && (entries.len() > 1024
                || entries.iter().any(|e| e.account.chain() != data.chain)
                || entries
                    .iter()
                    .try_fold(0_usize, |n, e| n.checked_add(e.storage_keys.len()))
                    .is_none_or(|n| n > 4096))
        {
            return Err(super::invalid_payload());
        }
        Ok(Self(data))
    }
    /// Returns immutable checked source transaction fields.
    #[must_use]
    pub const fn data(&self) -> &EvmPayloadData {
        &self.0
    }
}
impl TryFrom<EvmPayloadData> for EvmPayload {
    type Error = Error;
    fn try_from(v: EvmPayloadData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<EvmPayload> for EvmPayloadData {
    fn from(v: EvmPayload) -> Self {
        v.0
    }
}

/// Source transaction suggestion; no variant proves decoded transfer intent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Payload {
    /// Checked EVM field identities and exact quantities, opaque calldata.
    Evm(Box<EvmPayload>),
    /// Explicit family/encoding with opaque transaction content.
    Encoded(EncodedPayload),
}
impl Payload {
    /// Returns the payload's actual qualified source chain.
    #[must_use]
    pub fn chain(&self) -> Chain {
        match self {
            Self::Evm(v) => v.data().chain,
            Self::Encoded(v) => v.chain(),
        }
    }
}
