// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Private XRPL API-v2 wire fields and HTTP result/status decoding.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::value::RawValue;

use crate::{
    domain::xrpl::{
        Address, Currency, Drops, EngineResultCode, Hash, HexData, HistoryMarker, IssuedValue,
        Ledger, LineFlags, Marker, ResultCode, Setting, SubmissionHandling, SubmissionLedgerState,
        SubmissionResult, Transaction, TransactionStatus, TrustLine,
    },
    error::{Error, ProviderError},
};

pub(super) const fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

#[derive(Serialize)]
struct Request<'a, P: ?Sized> {
    method: &'static str,
    params: [&'a P; 1],
}

pub(super) fn encode<P: Serialize + ?Sized>(
    method: &'static str,
    params: &P,
) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(&Request {
        method,
        params: [params],
    })
    .map_err(|_| Error::Configuration)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    result: Box<RawValue>,
    #[serde(rename = "warnings")]
    _warnings: Option<Vec<Warning>>,
    #[serde(rename = "warning")]
    _warning: Option<String>,
    #[serde(rename = "forwarded")]
    _forwarded: Option<bool>,
    api_version: Option<u8>,
    status: Option<String>,
    #[serde(rename = "type")]
    response_type: Option<String>,
}

#[derive(Deserialize)]
struct Warning {
    #[serde(rename = "id")]
    _id: u32,
    #[serde(rename = "message")]
    _message: String,
    #[serde(rename = "details")]
    _details: Option<serde::de::IgnoredAny>,
}

#[derive(Default)]
pub(super) enum Field<T> {
    #[default]
    Missing,
    Present(T),
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Field<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(Self::Present)
    }
}
#[derive(Deserialize)]
struct Control {
    status: String,
    #[serde(default)]
    error: Field<String>,
    #[serde(default)]
    error_code: Field<i32>,
    #[serde(default)]
    error_message: Field<String>,
}

pub(super) fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, Error> {
    let envelope: Envelope = serde_json::from_slice(body).map_err(|_| invalid_response())?;
    let control: Control =
        serde_json::from_str(envelope.result.get()).map_err(|_| invalid_response())?;
    if envelope.api_version.is_some_and(|version| version != 2)
        || envelope
            .status
            .as_ref()
            .is_some_and(|status| *status != control.status)
        || envelope
            .response_type
            .as_ref()
            .is_some_and(|kind| kind != "response")
    {
        return Err(invalid_response());
    }
    match control.status.as_str() {
        "success"
            if matches!(control.error, Field::Missing)
                && matches!(control.error_code, Field::Missing)
                && matches!(control.error_message, Field::Missing) =>
        {
            serde_json::from_str(envelope.result.get()).map_err(|_| invalid_response())
        }
        "error" => {
            let Field::Present(name) = control.error else {
                return Err(invalid_response());
            };
            if !matches!(control.error_code, Field::Present(_))
                || !matches!(control.error_message, Field::Present(_))
            {
                return Err(invalid_response());
            }
            Err(match name.as_str() {
                "actNotFound" | "lgrNotFound" | "txnNotFound" => Error::UnavailableData,
                "unknownCmd" | "notSupported" | "notEnabled" => Error::UnsupportedCapability,
                "slowDown" | "tooBusy" => Error::Provider(ProviderError::RateLimited),
                _ => Error::Provider(ProviderError::Rpc),
            })
        }
        _ => Err(invalid_response()),
    }
}

#[derive(Serialize)]
pub(super) struct ApiOptions {
    pub(super) api_version: u8,
}

#[derive(Serialize)]
pub(super) struct SubmitRequest<'a> {
    pub(super) api_version: u8,
    pub(super) tx_blob: &'a HexData,
    pub(super) fail_hard: bool,
}

#[derive(Deserialize)]
pub(super) struct SubmitResult {
    engine_result: EngineResultCode,
    #[serde(rename = "engine_result_code")]
    _engine_result_code: i32,
    #[serde(rename = "engine_result_message")]
    _engine_result_message: IgnoredString,
    tx_blob: HexData,
    tx_json: SubmitJson,
    accepted: Setting,
    applied: Setting,
    broadcast: Setting,
    kept: Setting,
    queued: Setting,
    #[serde(default)]
    account_sequence_available: Field<u32>,
    #[serde(default)]
    account_sequence_next: Field<u32>,
    #[serde(default)]
    open_ledger_cost: Field<Drops>,
    #[serde(default)]
    validated_ledger_index: Field<u32>,
}

// Enforce the documented diagnostic shape without retaining remote text. The
// enclosing derived deserializer still rejects a duplicate named field.
struct IgnoredString;
impl<'de> Deserialize<'de> for IgnoredString {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct StringVisitor;
        impl serde::de::Visitor<'_> for StringVisitor {
            type Value = IgnoredString;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string")
            }
            fn visit_str<E: serde::de::Error>(self, _value: &str) -> Result<IgnoredString, E> {
                Ok(IgnoredString)
            }
        }
        d.deserialize_str(StringVisitor)
    }
}
#[derive(Deserialize)]
struct SubmitJson {
    #[serde(default)]
    hash: Field<Hash>,
}
impl SubmitResult {
    pub(super) fn into_domain(
        self,
        payload: &HexData,
        hash: Hash,
    ) -> Result<SubmissionResult, Error> {
        if self.tx_blob != *payload
            || self.tx_blob.transaction_hash() != hash
            || matches!(self.tx_json.hash, Field::Present(actual) if actual != hash)
        {
            return Err(invalid_response());
        }
        let handling = SubmissionHandling::new(
            self.accepted,
            self.applied,
            self.broadcast,
            self.kept,
            self.queued,
        )
        .map_err(|_| invalid_response())?;
        let state = match (
            self.account_sequence_available,
            self.account_sequence_next,
            self.open_ledger_cost,
            self.validated_ledger_index,
        ) {
            (Field::Missing, Field::Missing, Field::Missing, Field::Missing) => None,
            (
                Field::Present(available),
                Field::Present(next),
                Field::Present(cost),
                Field::Present(index),
            ) => Some(
                SubmissionLedgerState::new(available, next, cost, index)
                    .map_err(|_| invalid_response())?,
            ),
            _ => return Err(invalid_response()),
        };
        Ok(SubmissionResult::new(
            hash,
            self.engine_result,
            handling,
            state,
        ))
    }
}
#[derive(Deserialize)]
pub(super) struct ServerResult {
    pub(super) info: ServerInfo,
}
#[derive(Deserialize)]
pub(super) struct ServerInfo {
    #[serde(default)]
    pub(super) network_id: Field<u32>,
}

#[derive(Serialize)]
pub(super) struct LedgerRequest {
    pub(super) api_version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ledger_index: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ledger_hash: Option<Hash>,
    pub(super) transactions: bool,
    pub(super) expand: bool,
}
#[derive(Deserialize)]
pub(super) struct LedgerResult {
    pub(super) ledger_hash: Hash,
    pub(super) ledger_index: u32,
    pub(super) validated: bool,
}
#[derive(Serialize)]
pub(super) struct AccountRequest {
    pub(super) api_version: u8,
    pub(super) account: Address,
    pub(super) ledger_hash: Hash,
    pub(super) signer_lists: bool,
}
#[derive(Deserialize)]
pub(super) struct AccountResult {
    pub(super) account_data: AccountRoot,
    pub(super) ledger_hash: Hash,
    pub(super) ledger_index: u32,
    pub(super) validated: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct AccountRoot {
    pub(super) account: Address,
    pub(super) balance: Drops,
    pub(super) sequence: u32,
    pub(super) owner_count: u32,
    pub(super) flags: u32,
    pub(super) ledger_entry_type: String,
}

#[derive(Serialize)]
pub(super) struct LinesRequest<'a> {
    pub(super) api_version: u8,
    pub(super) account: Address,
    pub(super) ledger_hash: Hash,
    pub(super) limit: u16,
    pub(super) ignore_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) marker: Option<&'a Marker>,
}
#[derive(Deserialize)]
pub(super) struct LinesResult {
    pub(super) account: Address,
    pub(super) ledger_hash: Hash,
    pub(super) ledger_index: u32,
    pub(super) validated: bool,
    pub(super) lines: Vec<WireLine>,
    pub(super) marker: Option<Marker>,
    pub(super) limit: Option<u16>,
}
#[derive(Deserialize)]
pub(super) struct WireLine {
    account: Address,
    currency: Currency,
    balance: IssuedValue,
    limit: IssuedValue,
    limit_peer: IssuedValue,
    quality_in: u32,
    quality_out: u32,
    #[serde(default)]
    no_ripple: Setting,
    #[serde(default)]
    no_ripple_peer: Setting,
    #[serde(default)]
    authorized: Setting,
    #[serde(default)]
    peer_authorized: Setting,
    #[serde(default)]
    freeze: Setting,
    #[serde(default)]
    freeze_peer: Setting,
}
impl WireLine {
    pub(super) fn into_domain(self) -> Result<TrustLine, Error> {
        TrustLine::new(
            self.account,
            self.currency,
            self.balance,
            self.limit,
            self.limit_peer,
            LineFlags {
                quality_in: self.quality_in,
                quality_out: self.quality_out,
                no_ripple: self.no_ripple,
                no_ripple_peer: self.no_ripple_peer,
                authorized: self.authorized,
                peer_authorized: self.peer_authorized,
                freeze: self.freeze,
                freeze_peer: self.freeze_peer,
            },
        )
        .map_err(|_| invalid_response())
    }
}

#[derive(Deserialize)]
pub(super) struct FeeResult {
    pub(super) drops: FeeDrops,
    pub(super) ledger_current_index: u32,
}
#[derive(Deserialize)]
pub(super) struct FeeDrops {
    #[serde(rename = "base_fee")]
    pub(super) base: Drops,
    #[serde(rename = "minimum_fee")]
    pub(super) minimum: Drops,
    #[serde(rename = "median_fee")]
    pub(super) median: Drops,
    #[serde(rename = "open_ledger_fee")]
    pub(super) open_ledger: Drops,
}

#[derive(Serialize)]
pub(super) struct TxRequest {
    pub(super) api_version: u8,
    pub(super) transaction: Hash,
    pub(super) binary: bool,
}

#[derive(Deserialize)]
pub(super) struct BinaryResult {
    pub(super) hash: Hash,
    pub(super) tx_blob: HexData,
    pub(super) meta_blob: Option<HexData>,
    pub(super) ledger_index: Option<u32>,
    pub(super) ledger_hash: Option<Hash>,
    #[serde(default)]
    pub(super) validated: bool,
}
impl BinaryResult {
    pub(super) fn into_domain(self) -> Result<Transaction, Error> {
        let ledger = reported_ledger(self.ledger_index, self.ledger_hash, self.validated)?;
        Transaction::new(self.hash, self.tx_blob, self.meta_blob, ledger)
            .map_err(|_| invalid_response())
    }
}

#[derive(Deserialize)]
pub(super) struct StatusResult {
    pub(super) hash: Hash,
    pub(super) ledger_index: Option<u32>,
    pub(super) ledger_hash: Option<Hash>,
    #[serde(default)]
    pub(super) validated: bool,
    pub(super) meta: Option<ExecutionMeta>,
    tx_json: Box<RawValue>,
}
#[derive(Deserialize)]
pub(super) struct ExecutionMeta {
    #[serde(rename = "TransactionResult")]
    result: ResultCode,
}
impl StatusResult {
    pub(super) fn into_domain(self) -> Result<TransactionStatus, Error> {
        if !self.tx_json.get().trim_start().starts_with('{') {
            return Err(invalid_response());
        }
        let ledger = reported_ledger(self.ledger_index, self.ledger_hash, self.validated)?;
        TransactionStatus::new(self.hash, ledger, self.meta.map(|meta| meta.result))
            .map_err(|_| invalid_response())
    }
}
fn reported_ledger(
    index: Option<u32>,
    hash: Option<Hash>,
    validated: bool,
) -> Result<Option<Ledger>, Error> {
    match index {
        Some(index) => Ledger::new(index, hash, validated)
            .map(Some)
            .map_err(|_| invalid_response()),
        None if hash.is_none() && !validated => Ok(None),
        None => Err(invalid_response()),
    }
}

#[derive(Serialize)]
pub(super) struct HistoryParams {
    pub(super) api_version: u8,
    pub(super) account: Address,
    pub(super) ledger_index_min: u32,
    pub(super) ledger_index_max: u32,
    pub(super) limit: u16,
    pub(super) forward: bool,
    pub(super) binary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) marker: Option<HistoryMarker>,
}
#[derive(Deserialize)]
pub(super) struct HistoryResult {
    pub(super) account: Address,
    pub(super) ledger_index_min: u32,
    pub(super) ledger_index_max: u32,
    pub(super) limit: u16,
    pub(super) transactions: Vec<HistoryEntry>,
    pub(super) marker: Option<HistoryMarker>,
    pub(super) validated: bool,
}
#[derive(Deserialize)]
pub(super) struct HistoryEntry {
    tx_blob: HexData,
    meta_blob: HexData,
    hash: Option<Hash>,
    ledger_index: u32,
    ledger_hash: Option<Hash>,
    validated: bool,
}
impl HistoryEntry {
    pub(super) fn into_domain(self) -> Result<Transaction, Error> {
        let computed = self.tx_blob.transaction_hash();
        if self.hash.is_some_and(|actual| actual != computed) {
            return Err(invalid_response());
        }
        let ledger = Ledger::new(self.ledger_index, self.ledger_hash, self.validated)
            .map_err(|_| invalid_response())?;
        Transaction::new(computed, self.tx_blob, Some(self.meta_blob), Some(ledger))
            .map_err(|_| invalid_response())
    }
}
