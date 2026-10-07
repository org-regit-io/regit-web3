// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use solana_message::VersionedMessage;
use solana_transaction::versioned::VersionedTransaction;

use super::{Hash, Pubkey, Signature};
use crate::error::{Error, ValidationError};

fn invalid() -> Error {
    ValidationError::InvalidSolanaTransaction.into()
}

/// A structurally supported Solana message format, independent of node support.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionVersion {
    /// The original compact message format.
    Legacy,
    /// A version-zero message with optional address-table lookups.
    V0,
    /// A version-one message with inline accounts and transaction configuration.
    V1,
}
impl TransactionVersion {
    /// Returns the maintained maximum serialized transaction size for this format.
    #[must_use]
    pub const fn maximum_transaction_bytes(self) -> usize {
        match self {
            Self::Legacy | Self::V0 => 1232,
            Self::V1 => solana_message::v1::MAX_TRANSACTION_SIZE,
        }
    }
}
fn version(message: &VersionedMessage) -> TransactionVersion {
    match message {
        VersionedMessage::Legacy(_) => TransactionVersion::Legacy,
        VersionedMessage::V0(_) => TransactionVersion::V0,
        VersionedMessage::V1(_) => TransactionVersion::V1,
    }
}
fn config() -> impl wincode::config::Config {
    wincode::config::DefaultConfig::default()
        .with_preallocation_size_limit::<65536>()
        .with_deserialization_size_limit::<4096>()
}
fn encode_message(message: &VersionedMessage) -> Result<Vec<u8>, Error> {
    wincode::serialize(message).map_err(|_| invalid())
}
fn decode_message(bytes: &[u8]) -> Result<VersionedMessage, Error> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err(invalid());
    }
    let message: VersionedMessage =
        wincode::config::deserialize_exact(bytes, config()).map_err(|_| invalid())?;
    message.sanitize().map_err(|_| invalid())?;
    if message.header().num_required_signatures == 0 || encode_message(&message)? != bytes {
        return Err(invalid());
    }
    Ok(message)
}
fn encode_transaction(transaction: &VersionedTransaction) -> Result<Vec<u8>, Error> {
    wincode::serialize(transaction).map_err(|_| invalid())
}
fn decode_transaction(bytes: &[u8]) -> Result<VersionedTransaction, Error> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err(invalid());
    }
    let transaction: VersionedTransaction =
        wincode::config::deserialize_exact(bytes, config()).map_err(|_| invalid())?;
    transaction.sanitize().map_err(|_| invalid())?;
    if transaction.signatures.is_empty()
        || bytes.len() > version(&transaction.message).maximum_transaction_bytes()
        || encode_transaction(&transaction)? != bytes
    {
        return Err(invalid());
    }
    Ok(transaction)
}
fn from_hex(value: &str) -> Result<Vec<u8>, Error> {
    if value.is_empty()
        || value.len() > 8192
        || !value.len().is_multiple_of(2)
        || value
            .bytes()
            .any(|b| !b.is_ascii_digit() && !(b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    const_hex::decode(value).map_err(|_| invalid())
}

/// Exact canonical message bytes with maintained structural validation.
///
/// This type does not verify account ownership, signatures, address-table
/// contents, consensus validity, funding, instruction intent or blockhash lifetime.
#[derive(Clone, Eq, PartialEq)]
pub struct UnsignedMessage {
    bytes: Vec<u8>,
    message: VersionedMessage,
}
impl UnsignedMessage {
    /// Decodes one canonical legacy, v0 or v1 message with bounded allocation.
    /// # Errors
    /// Rejects malformed, noncanonical, trailing or structurally invalid bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let message = decode_message(&bytes)?;
        let transaction = VersionedTransaction {
            signatures: vec![
                solana_signature::Signature::default();
                usize::from(message.header().num_required_signatures)
            ],
            message: message.clone(),
        };
        if encode_transaction(&transaction)?.len() > version(&message).maximum_transaction_bytes() {
            return Err(invalid());
        }
        Ok(Self { bytes, message })
    }
    /// Canonically encodes a maintained message and validates the resulting bytes.
    /// # Errors
    /// Rejects structurally invalid or excessive messages.
    pub fn from_message(message: VersionedMessage) -> Result<Self, Error> {
        message.sanitize().map_err(|_| invalid())?;
        if message.header().num_required_signatures == 0
            || wincode::serialized_size(&message).map_err(|_| invalid())? > 4096
        {
            return Err(invalid());
        }
        let bytes = encode_message(&message)?;
        let transaction = VersionedTransaction {
            signatures: vec![
                solana_signature::Signature::default();
                usize::from(message.header().num_required_signatures)
            ],
            message: message.clone(),
        };
        if wincode::serialized_size(&transaction).map_err(|_| invalid())?
            > version(&message).maximum_transaction_bytes() as u64
        {
            return Err(invalid());
        }
        Ok(Self { bytes, message })
    }
    /// Returns the exact canonical bytes to review or provide to an external signer.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Returns the actual supported message version.
    #[must_use]
    pub fn version(&self) -> TransactionVersion {
        version(&self.message)
    }
    /// Returns the exact encoded recent blockhash or v1 lifetime specifier.
    #[must_use]
    pub fn recent_blockhash(&self) -> Hash {
        Hash::from_bytes(self.message.recent_blockhash().to_bytes())
    }
    /// Returns an owned maintained message without granting mutable access to this snapshot.
    #[must_use]
    pub fn decoded_message(&self) -> VersionedMessage {
        self.message.clone()
    }
    /// Returns the required number of signature slots, without validating their owners.
    #[must_use]
    pub fn required_signatures(&self) -> u8 {
        self.message.header().num_required_signatures
    }
    /// Returns the static account identities; lookup-table accounts remain unresolved.
    #[must_use]
    pub fn static_accounts(&self) -> Vec<Pubkey> {
        self.message
            .static_account_keys()
            .iter()
            .map(|a| Pubkey::from_bytes(a.to_bytes()))
            .collect()
    }
    /// Returns the encoded static and lookup account count used for index bounds.
    #[must_use]
    pub fn account_count(&self) -> usize {
        self.message.static_account_keys().len()
            + self.message.address_table_lookups().map_or(0, |v| {
                v.iter()
                    .map(|l| l.writable_indexes.len() + l.readonly_indexes.len())
                    .sum::<usize>()
            })
    }
}

/// Canonical transaction bytes containing only zero signature placeholders.
/// No signer, secret loading, submission or funding check is performed.
#[derive(Clone, Eq, PartialEq)]
pub struct UnsignedTransaction {
    bytes: Vec<u8>,
    message: UnsignedMessage,
}
impl UnsignedTransaction {
    /// Builds the exact zero-placeholder transaction for a validated message.
    /// # Errors
    /// Rejects a transaction exceeding its version-specific bound.
    pub fn from_message(message: UnsignedMessage) -> Result<Self, Error> {
        let transaction = VersionedTransaction {
            signatures: vec![
                solana_signature::Signature::default();
                usize::from(message.required_signatures())
            ],
            message: message.decoded_message(),
        };
        let bytes = encode_transaction(&transaction)?;
        if bytes.len() > message.version().maximum_transaction_bytes() {
            return Err(invalid());
        }
        Ok(Self { bytes, message })
    }
    /// Accepts canonical structurally valid bytes only when every signature is zero.
    /// # Errors
    /// Rejects signed, partially signed, malformed or excessive bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let transaction = decode_transaction(&bytes)?;
        if transaction
            .signatures
            .iter()
            .any(|s| *s != solana_signature::Signature::default())
        {
            return Err(invalid());
        }
        Ok(Self {
            message: UnsignedMessage::from_message(transaction.message)?,
            bytes,
        })
    }
    /// Returns the exact immutable zero-placeholder bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Returns the canonical unsigned message.
    #[must_use]
    pub const fn message(&self) -> &UnsignedMessage {
        &self.message
    }
}

/// Canonical transaction bytes with a nonzero signature in every required slot.
/// Signature encoding and message structure are checked, not cryptographic
/// validity, signer ownership, funding, approval or consensus acceptance.
#[derive(Clone, Eq, PartialEq)]
pub struct SignedTransaction {
    bytes: Vec<u8>,
    message: UnsignedMessage,
    signatures: Vec<Signature>,
}
impl SignedTransaction {
    /// Accepts bounded canonical bytes and checks all required signature slots.
    /// # Errors
    /// Rejects unsigned, partially signed, malformed or excessive bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let transaction = decode_transaction(&bytes)?;
        if transaction
            .signatures
            .iter()
            .any(|s| *s == solana_signature::Signature::default())
        {
            return Err(invalid());
        }
        let signatures = transaction
            .signatures
            .iter()
            .map(|s| Signature::from_bytes(*s.as_array()))
            .collect();
        Ok(Self {
            message: UnsignedMessage::from_message(transaction.message)?,
            bytes,
            signatures,
        })
    }
    /// Returns the exact immutable canonical bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Returns the canonical message; semantic intent is not independently inferred.
    #[must_use]
    pub const fn message(&self) -> &UnsignedMessage {
        &self.message
    }
    /// Returns all source signature slots without cryptographic verification.
    #[must_use]
    pub fn signatures(&self) -> &[Signature] {
        &self.signatures
    }
    /// Returns the first signature used by Solana as transaction lookup identity.
    #[must_use]
    pub fn signature(&self) -> Signature {
        self.signatures[0]
    }
}
macro_rules! wire {
    ($type:ident) => {
        impl fmt::Debug for $type {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($type))
                    .field("length", &self.bytes.len())
                    .finish_non_exhaustive()
            }
        }
        impl Serialize for $type {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&const_hex::encode(self.bytes()))
            }
        }
        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                from_hex(&String::deserialize(d)?)
                    .and_then(Self::from_bytes)
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}
wire!(UnsignedMessage);
wire!(UnsignedTransaction);
wire!(SignedTransaction);
