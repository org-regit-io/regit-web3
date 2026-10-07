// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::solana::Pubkey,
    error::{Error, ValidationError},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
/// Bounded opaque instruction bytes, serialized as canonical standard base64.
/// Debug never reveals content. Encoding validity does not prove program semantics.
#[derive(Clone, Eq, PartialEq)]
pub struct InstructionData(Vec<u8>);
impl InstructionData {
    /// Maximum bytes retained per source instruction.
    pub const MAX_BYTES: usize = 4096;
    /// Records exact bounded bytes without interpreting their semantics.
    /// # Errors
    /// Rejects excess data.
    pub fn new(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(invalid());
        }
        Ok(Self(bytes))
    }
    /// Parses bounded canonical standard base64 without echoing content.
    /// # Errors
    /// Rejects malformed/noncanonical/oversized input.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() > Self::MAX_BYTES.div_ceil(3) * 4 {
            return Err(invalid());
        }
        let bytes = STANDARD.decode(value).map_err(|_| invalid())?;
        if STANDARD.encode(&bytes) != value {
            return Err(invalid());
        }
        Self::new(bytes)
    }
    /// Returns exact bytes for caller semantic verification or maintained compilation.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for InstructionData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstructionData")
            .field("byte_length", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl Serialize for InstructionData {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(&self.0))
    }
}
impl<'de> Deserialize<'de> for InstructionData {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
/// Exact source account meta; order/repeated accounts are not normalized.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountMeta {
    /// Exact source account identity.
    pub pubkey: Pubkey,
    /// Actual source-required signing flag, not a signature proof.
    pub is_signer: bool,
    /// Actual source write flag, not authorization.
    pub is_writable: bool,
}
/// Complete source instruction fields, without decoded program-intent proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstructionFields {
    /// Actual source program identity.
    pub program_id: Pubkey,
    /// Ordered source account metas, retaining legitimate duplicates.
    pub accounts: Vec<AccountMeta>,
    /// Exact opaque bounded instruction bytes.
    pub data: InstructionData,
}
/// Immutable bounded source instruction, not a verified trade or wallet approval.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "InstructionFields", into = "InstructionFields")]
pub struct Instruction(InstructionFields);
impl Instruction {
    /// Checks the source account-index capacity without inferring program semantics.
    /// # Errors
    /// Rejects more than 256 account metas.
    pub fn new(v: InstructionFields) -> Result<Self, Error> {
        if v.accounts.len() > 256 {
            return Err(invalid());
        }
        Ok(Self(v))
    }
    /// Returns every exact immutable source instruction field.
    #[must_use]
    pub const fn fields(&self) -> &InstructionFields {
        &self.0
    }
}
impl TryFrom<InstructionFields> for Instruction {
    type Error = Error;
    fn try_from(v: InstructionFields) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Instruction> for InstructionFields {
    fn from(v: Instruction) -> Self {
        v.0
    }
}
/// Full source-reported lookup-table address ordering, without account-state authentication.
/// Duplicate address entries are legal and retained; the table identity is unique per build.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LookupFields", into = "LookupFields")]
pub struct LookupTable {
    address: Pubkey,
    entries: Vec<Pubkey>,
}
impl LookupTable {
    /// Checks a source table's actual maximum of 256 ordered entries.
    /// # Errors
    /// Rejects excess entries without normalizing duplicates.
    pub fn new(address: Pubkey, entries: Vec<Pubkey>) -> Result<Self, Error> {
        if entries.len() > 256 {
            return Err(invalid());
        }
        Ok(Self { address, entries })
    }
    /// Returns the exact source lookup-table account identity.
    #[must_use]
    pub const fn address(&self) -> Pubkey {
        self.address
    }
    /// Returns the exact ordered source entries.
    #[must_use]
    pub fn entries(&self) -> &[Pubkey] {
        &self.entries
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupFields {
    address: Pubkey,
    entries: Vec<Pubkey>,
}
impl TryFrom<LookupFields> for LookupTable {
    type Error = Error;
    fn try_from(v: LookupFields) -> Result<Self, Error> {
        Self::new(v.address, v.entries)
    }
}
impl From<LookupTable> for LookupFields {
    fn from(v: LookupTable) -> Self {
        Self {
            address: v.address,
            entries: v.entries,
        }
    }
}
fn invalid() -> Error {
    ValidationError::InvalidJupiterBuild.into()
}
