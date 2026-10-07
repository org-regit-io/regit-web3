// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::{Pubkey, TransactionVersion, UnsignedMessage};
use crate::error::Error;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{SeqAccess, Visitor},
};
use solana_transaction_error::TransactionError;
use std::{collections::HashSet, fmt, marker::PhantomData};

pub(crate) fn bounded_entries<'de, D: Deserializer<'de>, T: Deserialize<'de>, const N: usize>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    struct Bounded<T, const N: usize>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for Bounded<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a bounded source collection")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Vec<T>, A::Error> {
            let mut v = Vec::new();
            while let Some(item) = a.next_element()? {
                if v.len() == N {
                    return Err(serde::de::Error::custom("source collection exceeds limit"));
                }
                v.push(item);
            }
            Ok(v)
        }
    }
    d.deserialize_seq(Bounded::<T, N>(PhantomData))
}
fn accounts<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    bounded_entries::<D, T, 256>(d)
}
fn instructions<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    bounded_entries::<D, T, 1024>(d)
}
fn groups<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    bounded_entries::<D, T, 256>(d)
}
fn optional_groups<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<InnerInstructions>>, D::Error> {
    #[derive(Deserialize)]
    struct Values(#[serde(deserialize_with = "groups")] Vec<InnerInstructions>);
    Ok(Option::<Values>::deserialize(d)?.map(|v| v.0))
}
pub(super) fn optional_logs<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<LogMessage>>, D::Error> {
    #[derive(Deserialize)]
    struct Values(#[serde(deserialize_with = "instructions")] Vec<LogMessage>);
    Ok(Option::<Values>::deserialize(d)?.map(|v| v.0))
}
fn optional_tokens<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<TokenBalanceRecord>>, D::Error> {
    #[derive(Deserialize)]
    struct Values(#[serde(deserialize_with = "accounts")] Vec<TokenBalanceRecord>);
    Ok(Option::<Values>::deserialize(d)?.map(|v| v.0))
}
fn optional_rewards<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<Reward>>, D::Error> {
    #[derive(Deserialize)]
    struct Values(#[serde(deserialize_with = "instructions")] Vec<Reward>);
    Ok(Option::<Values>::deserialize(d)?.map(|v| v.0))
}

/// Maintained typed execution result, separate from inclusion or confirmation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutionOutcome {
    /// Source explicitly reports no transaction error.
    Succeeded,
    /// Source reports a maintained typed Solana transaction error.
    Failed {
        /// Actual source error; no arbitrary diagnostic string is retained.
        error: TransactionError,
    },
}
impl ExecutionOutcome {
    /// Maps an explicit required RPC `err` field, where null means source success.
    #[must_use]
    pub fn from_error(error: Option<TransactionError>) -> Self {
        error.map_or(Self::Succeeded, |error| Self::Failed { error })
    }
    pub(super) fn validate_message(&self, message: &UnsignedMessage) -> Result<(), Error> {
        if let Self::Failed {
            error: TransactionError::InstructionError(index, ..),
        } = self
            && usize::from(*index) >= message.decoded_message().instructions().len()
        {
            return Err(super::invalid());
        }
        Ok(())
    }
}
/// Exact bounded opaque instruction/return bytes, serialized as canonical lowercase hex.
#[derive(Clone, Eq, PartialEq)]
pub struct ExecutionBytes(Vec<u8>);
impl ExecutionBytes {
    /// Local per-field resource bound, without a consensus or semantic approval claim.
    pub const MAX_BYTES: usize = 4096;
    /// Retains exact bounded bytes, including an explicit empty byte sequence.
    /// # Errors
    /// Rejects excessive length.
    pub fn new(v: Vec<u8>) -> Result<Self, Error> {
        if v.len() > Self::MAX_BYTES {
            return Err(super::invalid());
        }
        Ok(Self(v))
    }
    /// Parses canonical base58 data from a compiled source instruction.
    /// # Errors
    /// Rejects malformed, noncanonical or excessive source encodings.
    pub fn from_base58(v: &str) -> Result<Self, Error> {
        if v.len() > Self::MAX_BYTES * 2 {
            return Err(super::invalid());
        }
        let bytes = bs58::decode(v).into_vec().map_err(|_| super::invalid())?;
        let value = Self::new(bytes)?;
        if bs58::encode(&value.0).into_string() != v {
            return Err(super::invalid());
        }
        Ok(value)
    }
    /// Returns exact bytes without interpreting instruction intent.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for ExecutionBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutionBytes")
            .field("length", &self.0.len())
            .finish()
    }
}
impl Serialize for ExecutionBytes {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&const_hex::encode(&self.0))
    }
}
impl<'de> Deserialize<'de> for ExecutionBytes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = String::deserialize(d)?;
        if v.len() > Self::MAX_BYTES * 2
            || !v.len().is_multiple_of(2)
            || v.bytes()
                .any(|b| !b.is_ascii_digit() && !(b'a'..=b'f').contains(&b))
        {
            return Err(serde::de::Error::custom("invalid Solana execution bytes"));
        }
        Self::new(
            const_hex::decode(v)
                .map_err(|_| serde::de::Error::custom("invalid Solana execution bytes"))?,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// Bounded exact source UTF-8 log, preserving empty strings and control characters.
/// Diagnostics expose only length, never source log contents.
#[derive(Clone, Eq, PartialEq)]
pub struct LogMessage(String);
impl LogMessage {
    /// Retains up to 16 KiB of source log text without semantic interpretation.
    /// # Errors
    /// Rejects excessive byte length.
    pub fn new(v: impl Into<String>) -> Result<Self, Error> {
        let v = v.into();
        if v.len() > 16384 {
            return Err(super::invalid());
        }
        Ok(Self(v))
    }
    /// Returns the exact retained UTF-8 log.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for LogMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogMessage")
            .field("length", &self.0.len())
            .finish()
    }
}
impl Serialize for LogMessage {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for LogMessage {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
pub(super) fn validate_logs(v: Option<&[LogMessage]>) -> Result<(), Error> {
    if v.is_some_and(|v| v.len() > 1024 || v.iter().map(|s| s.0.len()).sum::<usize>() > 1024 * 1024)
    {
        return Err(super::invalid());
    }
    Ok(())
}

/// Program return bytes with actual source program identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnData {
    /// Actual returning program address.
    pub program_id: Pubkey,
    /// Exact opaque bytes, without ABI or intent inference.
    pub data: ExecutionBytes,
}
/// Actual v0 lookup-table addresses reported by execution metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadedAddresses {
    /// Source writable lookup accounts in actual supplied order.
    #[serde(deserialize_with = "accounts")]
    pub writable: Vec<Pubkey>,
    /// Source read-only lookup accounts in actual supplied order.
    #[serde(deserialize_with = "accounts")]
    pub readonly: Vec<Pubkey>,
}
/// Exact source token-account units, without scaled/UI amount conversion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenBalanceRecord {
    /// Actual transaction account index.
    pub account_index: u8,
    /// Actual source mint identity.
    pub mint: Pubkey,
    /// Optional source token account owner.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub owner: Option<Pubkey>,
    /// Optional source token program identity.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub program_id: Option<Pubkey>,
    /// Exact unsigned 64-bit base units.
    pub raw_amount: u64,
    /// Actual source decimal metadata, independent of raw base units.
    pub decimals: u8,
}
/// Actual source compiled inner instruction; signatures/intent are not inferred.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledInnerInstruction {
    /// Actual program index in static plus loaded transaction accounts.
    pub program_id_index: u8,
    /// Actual account indices in supplied order.
    #[serde(deserialize_with = "accounts")]
    pub accounts: Vec<u8>,
    /// Exact decoded canonical source instruction bytes.
    pub data: ExecutionBytes,
    /// Optional source invocation stack height.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub stack_height: Option<u32>,
}
/// Source-recorded inner execution for one top-level instruction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InnerInstructions {
    /// Actual top-level instruction index.
    pub index: u8,
    /// Bounded actual inner records; empty is distinct from absent recording.
    #[serde(deserialize_with = "instructions")]
    pub instructions: Vec<CompiledInnerInstruction>,
}
/// Supported source reward classification without inferring transfer intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardKind {
    /// Transaction fee reward.
    Fee,
    /// Rent reward.
    Rent,
    /// Staking reward.
    Staking,
    /// Voting reward.
    Voting,
}
/// Exact source reward in signed lamports and post-reward native balance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reward {
    /// Actual reward account.
    pub pubkey: Pubkey,
    /// Exact signed lamports, independent of token decimals.
    pub lamports: i64,
    /// Exact source post-reward lamports.
    pub post_balance: u64,
    /// Optional source reward classification.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub reward_type: Option<RewardKind>,
    /// Optional source commission percentage.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub commission: Option<u8>,
}

/// Exact source execution metadata, with absent recording retained independently.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionMetadataData {
    /// Actual source transaction execution result.
    pub outcome: ExecutionOutcome,
    /// Exact source fee in native lamports.
    pub fee_lamports: u64,
    /// Native lamports before execution, indexed by all transaction accounts.
    #[serde(deserialize_with = "accounts")]
    pub pre_balances: Vec<u64>,
    /// Native lamports after execution, indexed by all transaction accounts.
    #[serde(deserialize_with = "accounts")]
    pub post_balances: Vec<u64>,
    /// Optional source v0 lookup-address recording.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub loaded_addresses: Option<LoadedAddresses>,
    /// Optional recording of exact pre-execution token base units.
    #[serde(deserialize_with = "optional_tokens")]
    pub pre_token_balances: Option<Vec<TokenBalanceRecord>>,
    /// Optional recording of exact post-execution token base units.
    #[serde(deserialize_with = "optional_tokens")]
    pub post_token_balances: Option<Vec<TokenBalanceRecord>>,
    /// Optional inner execution recording, distinct from an explicit empty recording.
    #[serde(deserialize_with = "optional_groups")]
    pub inner_instructions: Option<Vec<InnerInstructions>>,
    /// Optional exact bounded source log recording.
    #[serde(deserialize_with = "optional_logs")]
    pub log_messages: Option<Vec<LogMessage>>,
    /// Optional program return bytes.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub return_data: Option<ReturnData>,
    /// Optional source reward recording.
    #[serde(deserialize_with = "optional_rewards")]
    pub rewards: Option<Vec<Reward>>,
    /// Optional actual source compute consumption.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub compute_units_consumed: Option<u64>,
    /// Optional source scheduler cost, distinct from consumed compute units.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub cost_units: Option<u64>,
}
/// Validated bounded metadata; transaction construction checks byte/index agreement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionMetadataData")]
pub struct TransactionMetadata {
    #[serde(flatten)]
    data: TransactionMetadataData,
}
impl TransactionMetadata {
    /// Retains bounded exact metadata and rejects invalid resource/width facts.
    /// # Errors
    /// Rejects excessive collections, duplicate index recording and invalid commission.
    pub fn new(data: TransactionMetadataData) -> Result<Self, Error> {
        if data.pre_balances.len() > 256
            || data.post_balances.len() > 256
            || data
                .loaded_addresses
                .as_ref()
                .is_some_and(|a| a.writable.len() > 256 || a.readonly.len() > 256)
            || data.rewards.as_ref().is_some_and(|v| {
                v.len() > 1024 || v.iter().any(|r| r.commission.is_some_and(|c| c > 100))
            })
        {
            return Err(super::invalid());
        }
        for balances in [&data.pre_token_balances, &data.post_token_balances]
            .into_iter()
            .flatten()
        {
            let mut indices = HashSet::new();
            if balances.len() > 256 || balances.iter().any(|b| !indices.insert(b.account_index)) {
                return Err(super::invalid());
            }
        }
        if let Some(groups) = &data.inner_instructions {
            let mut indices = HashSet::new();
            if groups.len() > 256
                || groups.iter().any(|g| {
                    !indices.insert(g.index)
                        || g.instructions.len() > 1024
                        || g.instructions.iter().any(|i| i.accounts.len() > 256)
                })
                || groups.iter().map(|g| g.instructions.len()).sum::<usize>() > 8192
            {
                return Err(super::invalid());
            }
        }
        validate_logs(data.log_messages.as_deref())?;
        Ok(Self { data })
    }
    /// Returns immutable exact source facts, preserving unavailable recordings.
    #[must_use]
    pub const fn data(&self) -> &TransactionMetadataData {
        &self.data
    }
    pub(super) fn validate_message(&self, message: &UnsignedMessage) -> Result<(), Error> {
        let d = &self.data;
        let total = message.account_count();
        if d.pre_balances.len() != total || d.post_balances.len() != total {
            return Err(super::invalid());
        }
        if let Some(a) = &d.loaded_addresses {
            let m = message.decoded_message();
            let (w, r) = m.address_table_lookups().map_or((0, 0), |v| {
                v.iter().fold((0, 0), |(w, r), l| {
                    (w + l.writable_indexes.len(), r + l.readonly_indexes.len())
                })
            });
            if a.writable.len() != w
                || a.readonly.len() != r
                || message.version() != TransactionVersion::V0
                    && (!a.writable.is_empty() || !a.readonly.is_empty())
            {
                return Err(super::invalid());
            }
        }
        for values in [&d.pre_token_balances, &d.post_token_balances]
            .into_iter()
            .flatten()
        {
            if values.iter().any(|v| usize::from(v.account_index) >= total) {
                return Err(super::invalid());
            }
        }
        if let Some(groups) = &d.inner_instructions {
            let instruction_count = message.decoded_message().instructions().len();
            if groups.iter().any(|g| {
                usize::from(g.index) >= instruction_count
                    || g.instructions.iter().any(|i| {
                        usize::from(i.program_id_index) >= total
                            || i.accounts.iter().any(|a| usize::from(*a) >= total)
                    })
            }) {
                return Err(super::invalid());
            }
        }
        d.outcome.validate_message(message)
    }
}
impl TryFrom<TransactionMetadataData> for TransactionMetadata {
    type Error = Error;
    fn try_from(v: TransactionMetadataData) -> Result<Self, Error> {
        Self::new(v)
    }
}
