// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{Error, ValidationError};

use super::{Drops, Hash, HexData, NetworkId, Setting};

fn invalid_record() -> Error {
    ValidationError::InvalidXrplRecord.into()
}

/// An explicit caller assertion that opaque bytes are ready for signed submission.
///
/// Only bounded hexadecimal and exact byte identity are verified. This does not
/// verify canonical binary encoding, signatures, embedded `NetworkID`, account
/// ownership, approval or agreement with reviewed unsigned intent. The network
/// selects the expected server, independently of any fields inside the bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SignedFields")]
pub struct SignedSubmission {
    network: NetworkId,
    payload: HexData,
    hash: Hash,
    fail_hard: bool,
}
impl SignedSubmission {
    /// Records explicit network, caller-supplied signed bytes and relay policy.
    ///
    /// `fail_hard` asks the server not to retry or relay a locally failed
    /// transaction. The library always dispatches once, for either value.
    #[must_use]
    pub fn new(network: NetworkId, payload: HexData, fail_hard: bool) -> Self {
        let hash = payload.transaction_hash();
        Self {
            network,
            payload,
            hash,
            fail_hard,
        }
    }
    /// Returns the explicit expected server network, not an embedded-byte proof.
    #[must_use]
    pub const fn network(&self) -> NetworkId {
        self.network
    }
    /// Returns exact caller-supplied bytes without signature validation.
    #[must_use]
    pub const fn payload(&self) -> &HexData {
        &self.payload
    }
    /// Returns the maintained SHA512-half transaction ID of the exact bytes.
    #[must_use]
    pub const fn hash(&self) -> Hash {
        self.hash
    }
    /// Returns the explicit server relay policy for locally failed transactions.
    #[must_use]
    pub const fn fail_hard(&self) -> bool {
        self.fail_hard
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedFields {
    network: NetworkId,
    payload: HexData,
    hash: Hash,
    fail_hard: bool,
}
impl TryFrom<SignedFields> for SignedSubmission {
    type Error = Error;
    fn try_from(v: SignedFields) -> Result<Self, Error> {
        let submission = Self::new(v.network, v.payload, v.fail_hard);
        if submission.hash() != v.hash {
            return Err(invalid_record());
        }
        Ok(submission)
    }
}

/// The family of a preliminary, source-reported transaction engine result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineResultClass {
    /// Provisional success; validated execution requires a separate lookup.
    Success,
    /// Provisional claimed-cost failure; its final code may still differ.
    ClaimedCost,
    /// A server-reported failure that can also describe an already applied transaction.
    Failure,
    /// A server-local processing failure.
    LocalError,
    /// A source-reported malformed transaction under the server's protocol rules.
    Malformed,
    /// A transaction that could be applied in a future ledger.
    Retry,
}

/// A bounded engine code whose preliminary class does not establish finality.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct EngineResultCode(String);
impl EngineResultCode {
    /// Records `tesSUCCESS` or an ordinary `tec`/`tef`/`tel`/`tem`/`ter` code.
    ///
    /// # Errors
    /// Rejects malformed, oversized or unsupported code classes. Numeric engine
    /// codes are deliberately not mapped: their values can change with servers.
    pub fn parse(code: &str) -> Result<Self, Error> {
        let valid = code == "tesSUCCESS"
            || ["tec", "tef", "tel", "tem", "ter"].iter().any(|prefix| {
                code.strip_prefix(prefix).is_some_and(|suffix| {
                    !suffix.is_empty()
                        && suffix
                            .bytes()
                            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                })
            });
        if !valid || code.len() > 64 {
            return Err(invalid_record());
        }
        Ok(Self(code.into()))
    }
    /// Returns the exact preliminary engine code without a remote diagnostic.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Returns the code's preliminary protocol class, not validated execution.
    #[must_use]
    pub fn class(&self) -> EngineResultClass {
        match self.0.get(..3) {
            Some("tec") => EngineResultClass::ClaimedCost,
            Some("tef") => EngineResultClass::Failure,
            Some("tel") => EngineResultClass::LocalError,
            Some("tem") => EngineResultClass::Malformed,
            Some("ter") => EngineResultClass::Retry,
            _ => EngineResultClass::Success,
        }
    }
}
impl<'de> Deserialize<'de> for EngineResultCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Source-reported handling of this submission, independently of final execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HandlingFields")]
pub struct SubmissionHandling {
    accepted: Setting,
    applied: Setting,
    broadcast: Setting,
    kept: Setting,
    queued: Setting,
}
impl SubmissionHandling {
    /// Records the server's preliminary flags without inferring transaction finality.
    ///
    /// # Errors
    /// Rejects `accepted` differing from the OR of applied/broadcast/kept/queued,
    /// matching the supported submit-only backend's actual handling definition.
    pub fn new(
        accepted: Setting,
        applied: Setting,
        broadcast: Setting,
        kept: Setting,
        queued: Setting,
    ) -> Result<Self, Error> {
        if (accepted == Setting::Enabled)
            != [applied, broadcast, kept, queued].contains(&Setting::Enabled)
        {
            return Err(invalid_record());
        }
        Ok(Self {
            accepted,
            applied,
            broadcast,
            kept,
            queued,
        })
    }
    /// Returns acceptance by this exchange's server, not global success or failure.
    #[must_use]
    pub const fn accepted(self) -> Setting {
        self.accepted
    }
    /// Returns provisional application to an open ledger, not validated inclusion.
    #[must_use]
    pub const fn applied(self) -> Setting {
        self.applied
    }
    /// Returns the server's broadcast assessment, including stand-alone semantics.
    #[must_use]
    pub const fn broadcast(self) -> Setting {
        self.broadcast
    }
    /// Returns whether the server retained the transaction for later processing.
    #[must_use]
    pub const fn kept(self) -> Setting {
        self.kept
    }
    /// Returns whether the server put the transaction into its queue.
    #[must_use]
    pub const fn queued(self) -> Setting {
        self.queued
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandlingFields {
    accepted: Setting,
    applied: Setting,
    broadcast: Setting,
    kept: Setting,
    queued: Setting,
}
impl TryFrom<HandlingFields> for SubmissionHandling {
    type Error = Error;
    fn try_from(v: HandlingFields) -> Result<Self, Error> {
        Self::new(v.accepted, v.applied, v.broadcast, v.kept, v.queued)
    }
}

/// Optional current-server ledger facts evaluated during submission.
///
/// The latest validated index is a source reference, not inclusion of the
/// submitted transaction. A source may omit this whole state for a rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StateFields")]
pub struct SubmissionLedgerState {
    account_sequence_available: u32,
    account_sequence_next: u32,
    open_ledger_cost: Drops,
    latest_validated_ledger_index: u32,
}
impl SubmissionLedgerState {
    /// Records exact source facts without claiming inclusion or choosing a fee.
    ///
    /// # Errors
    /// Rejects a zero latest-validated ledger index.
    pub const fn new(
        account_sequence_available: u32,
        account_sequence_next: u32,
        open_ledger_cost: Drops,
        latest_validated_ledger_index: u32,
    ) -> Result<Self, Error> {
        if latest_validated_ledger_index == 0 {
            return Err(Error::Validation(ValidationError::InvalidXrplRecord));
        }
        Ok(Self {
            account_sequence_available,
            account_sequence_next,
            open_ledger_cost,
            latest_validated_ledger_index,
        })
    }
    /// Returns the source's next sequence after pending and queued transactions.
    #[must_use]
    pub const fn account_sequence_available(self) -> u32 {
        self.account_sequence_available
    }
    /// Returns the source's next sequence after provisionally applied transactions.
    #[must_use]
    pub const fn account_sequence_next(self) -> u32 {
        self.account_sequence_next
    }
    /// Returns exact open-ledger fee cost before processing this transaction.
    #[must_use]
    pub const fn open_ledger_cost(self) -> Drops {
        self.open_ledger_cost
    }
    /// Returns the latest known validated index, independently of target inclusion.
    #[must_use]
    pub const fn latest_validated_ledger_index(self) -> u32 {
        self.latest_validated_ledger_index
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFields {
    account_sequence_available: u32,
    account_sequence_next: u32,
    open_ledger_cost: Drops,
    latest_validated_ledger_index: u32,
}
impl TryFrom<StateFields> for SubmissionLedgerState {
    type Error = Error;
    fn try_from(v: StateFields) -> Result<Self, Error> {
        Self::new(
            v.account_sequence_available,
            v.account_sequence_next,
            v.open_ledger_cost,
            v.latest_validated_ledger_index,
        )
    }
}

/// Preliminary handling of an identified submission, never validated execution.
///
/// Rejection by this server does not rule out previous submission or acceptance
/// elsewhere. Use a separate transaction lookup to establish actual inclusion
/// and execution; this result never invents those facts from preliminary flags.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionResult {
    hash: Hash,
    engine_result: EngineResultCode,
    handling: SubmissionHandling,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    ledger_state: Option<SubmissionLedgerState>,
}
impl SubmissionResult {
    /// Records a supplied identity and typed preliminary source facts.
    #[must_use]
    pub const fn new(
        hash: Hash,
        engine_result: EngineResultCode,
        handling: SubmissionHandling,
        ledger_state: Option<SubmissionLedgerState>,
    ) -> Self {
        Self {
            hash,
            engine_result,
            handling,
            ledger_state,
        }
    }
    /// Returns the ID of the exact submitted bytes.
    #[must_use]
    pub const fn hash(&self) -> Hash {
        self.hash
    }
    /// Returns the provisional engine result, independently of final execution.
    #[must_use]
    pub const fn engine_result(&self) -> &EngineResultCode {
        &self.engine_result
    }
    /// Returns this source's preliminary acceptance and processing flags.
    #[must_use]
    pub const fn handling(&self) -> SubmissionHandling {
        self.handling
    }
    /// Returns actual source ledger state, if supplied; it is not target inclusion.
    #[must_use]
    pub const fn ledger_state(&self) -> Option<SubmissionLedgerState> {
        self.ledger_state
    }
}
