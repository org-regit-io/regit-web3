// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Chain, Family, Identifier, Origin, Token};
use crate::{
    domain::{Amount, ExactDecimal, Timestamp},
    error::Error,
};
use serde::{Deserialize, Serialize};

/// A chain-qualified transaction identifier, distinct from LI.FI transfer IDs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionIdFields")]
pub struct TransactionId {
    chain: Chain,
    identifier: String,
}
impl TransactionId {
    /// Checks EVM/UTXO/TRON/Stellar hexadecimal or SVM/Move base58 hash syntax.
    /// This does not prove existence or canonical chain inclusion.
    ///
    /// # Errors
    /// Rejects incompatible bounded family encodings.
    pub fn new(chain: Chain, text: &str) -> Result<Self, Error> {
        if text.len() > 128 {
            return Err(super::invalid_identity());
        }
        let identifier = match chain.family() {
            Family::Evm => crate::domain::BlockHash::parse(text)
                .map_err(|_| super::invalid_identity())?
                .to_string(),
            Family::Solana | Family::Move => {
                let size = if chain.family() == Family::Solana {
                    64
                } else {
                    32
                };
                let mut bytes = [0_u8; 64];
                if bs58::decode(text).onto(&mut bytes[..size]).ok() != Some(size)
                    || bs58::encode(&bytes[..size]).into_string() != text
                {
                    return Err(super::invalid_identity());
                }
                text.to_owned()
            }
            Family::Utxo | Family::Tron | Family::Stellar => {
                if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(super::invalid_identity());
                }
                text.to_ascii_lowercase()
            }
        };
        Ok(Self { chain, identifier })
    }
    /// Returns the exact qualified chain.
    #[must_use]
    pub const fn chain(&self) -> Chain {
        self.chain
    }
    /// Returns the family's canonical transaction identifier.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionIdFields {
    chain: Chain,
    identifier: String,
}
impl TryFrom<TransactionIdFields> for TransactionId {
    type Error = Error;
    fn try_from(v: TransactionIdFields) -> Result<Self, Error> {
        Self::new(v.chain, &v.identifier)
    }
}

/// A provider's 32-byte hexadecimal transfer identity, not a chain transaction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TransferId(String);
impl TransferId {
    /// Checks the observed provider identity's `0x`-prefixed 32-byte syntax.
    /// This does not prove a transfer exists or identify a signed chain payload.
    ///
    /// # Errors
    /// Rejects step UUIDs, incomplete hex and incompatible identity syntax.
    pub fn new(text: &str) -> Result<Self, Error> {
        let value = crate::domain::BlockHash::parse(text).map_err(|_| super::invalid_identity())?;
        Ok(Self(value.to_string()))
    }
    /// Returns the provider's normalized hexadecimal identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for TransferId {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(&v)
    }
}
impl From<TransferId> for String {
    fn from(v: TransferId) -> Self {
        v.0
    }
}

/// Exact status lookup kind; transfer IDs are not coerced into chain hashes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StatusSelector {
    /// Source transaction hash, checked against any reported sending hash.
    SendingTransaction(TransactionId),
    /// Destination transaction hash, checked against any reported receiving hash.
    ReceivingTransaction(TransactionId),
    /// Provider transfer ID, checked against any actual reported transfer ID.
    /// Quote/route step selection UUIDs are not supported status identifiers.
    ProviderTransfer(TransferId),
}
/// Immutable explicit source/destination chain and selector for a status query.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StatusQueryFields")]
pub struct StatusQuery {
    from: Chain,
    to: Chain,
    selector: StatusSelector,
    bridge: Option<Identifier>,
}
impl StatusQuery {
    /// Checks that supplied chain hashes belong to their selected query side.
    ///
    /// # Errors
    /// Rejects contradictory hash/chain qualification.
    pub fn new(
        from: Chain,
        to: Chain,
        selector: StatusSelector,
        bridge: Option<Identifier>,
    ) -> Result<Self, Error> {
        if matches!(&selector,StatusSelector::SendingTransaction(id) if id.chain()!=from)
            || matches!(&selector,StatusSelector::ReceivingTransaction(id) if id.chain()!=to)
        {
            return Err(super::invalid_identity());
        }
        Ok(Self {
            from,
            to,
            selector,
            bridge,
        })
    }
    /// Returns the explicitly requested sending chain.
    #[must_use]
    pub const fn from(&self) -> Chain {
        self.from
    }
    /// Returns the explicitly requested receiving chain.
    #[must_use]
    pub const fn to(&self) -> Chain {
        self.to
    }
    /// Returns the exact chain-hash or provider-transfer lookup kind.
    #[must_use]
    pub const fn selector(&self) -> &StatusSelector {
        &self.selector
    }
    /// Returns a caller-selected bridge filter, if supplied.
    #[must_use]
    pub const fn bridge(&self) -> Option<&Identifier> {
        self.bridge.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusQueryFields {
    from: Chain,
    to: Chain,
    selector: StatusSelector,
    bridge: Option<Identifier>,
}
impl TryFrom<StatusQueryFields> for StatusQuery {
    type Error = Error;
    fn try_from(v: StatusQueryFields) -> Result<Self, Error> {
        Self::new(v.from, v.to, v.selector, v.bridge)
    }
}

/// Actual provider transfer progress; DONE is not a consensus finality proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Progress {
    /// Provider did not find the transaction or transfer.
    NotFound,
    /// Provider cannot determine valid progress from an upstream service.
    Invalid,
    /// Transfer remains pending according to the provider.
    Pending,
    /// Provider reports completion, partial delivery or refund via substatus.
    Done,
    /// Provider reports a failed transfer.
    Failed,
}
/// Role of the actually reported receiving record, without a success inference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceivingRole {
    /// No receiving record was reported.
    Unreported,
    /// The record belongs to the requested destination chain, possibly pending.
    Destination,
    /// The source explicitly reports a refund record on the original source chain.
    Refund,
}
/// A reported executed leg; no step or transaction identifier is invented.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedLeg {
    /// Actual tool key.
    pub tool: Identifier,
    /// Reported input token and units.
    pub from_token: Token,
    /// Exact reported input amount.
    pub from_amount: Amount,
    /// Reported output token and units.
    pub to_token: Token,
    /// Exact reported output amount.
    pub to_amount: Amount,
    /// Bridge-delivered units if explicitly supplied.
    pub bridged_amount: Option<Amount>,
}
/// Actual pending or mined transaction source fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionData {
    /// Explicitly reported chain, also present on pending receiving records.
    pub chain: Chain,
    /// Actual transaction identifier, or absent for pending receiving records.
    pub id: Option<TransactionId>,
    /// Reported token, if supplied; alternate delivery tokens remain explicit.
    pub token: Option<Token>,
    /// Reported token units; unknown token precision stays unknown.
    pub amount: Option<Amount>,
    /// Actual source Unix timestamp in whole seconds, if supplied.
    pub timestamp: Option<Timestamp>,
    /// Exact native transaction value, if reported, without assumed precision.
    pub value: Option<Amount>,
    /// Exact source suggested/reported gas price, if present.
    pub gas_price: Option<ExactDecimal>,
    /// Exact reported gas units, if present.
    pub gas_used: Option<Amount>,
    /// Actual reported gas token, if present.
    pub gas_token: Option<Token>,
    /// Actual gas token units; unknown precision stays unknown.
    pub gas_amount: Option<Amount>,
    /// Exact reported gas USD cost, if present.
    pub gas_amount_usd: Option<ExactDecimal>,
    /// Reported executed legs; missing and empty remain distinct.
    pub included_legs: Option<Vec<ObservedLeg>>,
}
/// Checked reported transaction metadata, without fabricated block/finality facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionData", into = "TransactionData")]
pub struct Transaction(TransactionData);
impl Transaction {
    /// Checks identity qualification, reported units and bounded leg collections.
    ///
    /// # Errors
    /// Rejects contradictory chain identities or malformed precision/cost values.
    pub fn new(data: TransactionData) -> Result<Self, Error> {
        if data.id.as_ref().is_some_and(|id| id.chain() != data.chain)
            || data
                .token
                .as_ref()
                .is_some_and(|t| t.data().asset.chain() != data.chain)
            || data
                .gas_token
                .as_ref()
                .is_some_and(|t| t.data().asset.chain() != data.chain)
            || data.value.is_some_and(|a| a.decimals().is_some())
            || data.gas_used.is_some_and(|a| a.decimals().is_some())
            || data
                .gas_price
                .as_ref()
                .is_some_and(ExactDecimal::is_negative)
            || data
                .gas_amount_usd
                .as_ref()
                .is_some_and(ExactDecimal::is_negative)
        {
            return Err(super::invalid_record());
        }
        for (amount, token) in [
            (data.amount, &data.token),
            (data.gas_amount, &data.gas_token),
        ] {
            if let Some(amount) = amount
                && amount.decimals() != token.as_ref().map(|t| t.data().decimals)
            {
                return Err(super::invalid_record());
            }
        }
        if let Some(legs) = &data.included_legs
            && (legs.len() > 256
                || legs.iter().any(|l| {
                    l.from_amount.decimals() != Some(l.from_token.data().decimals)
                        || l.to_amount.decimals() != Some(l.to_token.data().decimals)
                        || l.bridged_amount
                            .is_some_and(|a| a.decimals() != Some(l.to_token.data().decimals))
                }))
        {
            return Err(super::invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns immutable reported fields, with missing values preserved.
    #[must_use]
    pub const fn data(&self) -> &TransactionData {
        &self.0
    }
}
impl TryFrom<TransactionData> for Transaction {
    type Error = Error;
    fn try_from(v: TransactionData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Transaction> for TransactionData {
    fn from(v: Transaction) -> Self {
        v.0
    }
}

/// Actual transfer progress and independent sending/receiving facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusData {
    /// Source-reported progress, without consensus finality inference.
    pub progress: Progress,
    /// Actual substatus such as COMPLETED/PARTIAL/REFUNDED, if supplied.
    pub substatus: Option<Identifier>,
    /// Source tool identity, if supplied.
    pub tool: Option<Identifier>,
    /// Provider transfer ID; this is not a source/destination transaction hash.
    pub transfer_id: Option<TransferId>,
    /// Actual sending record, or absent for unknown transactions.
    pub sending: Option<Transaction>,
    /// Actual receiving record, including source-chain refunds when reported.
    pub receiving: Option<Transaction>,
    /// Actual provider fee list; missing and empty remain distinct.
    pub fees: Option<Vec<super::Fee>>,
}
/// Request-bound provider progress with retrieval/source attribution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StatusFields")]
pub struct Status {
    query: StatusQuery,
    data: StatusData,
    origin: Origin,
}
impl Status {
    /// Checks actual reported chain/hash correlation and bounded fees.
    ///
    /// # Errors
    /// Rejects contradictory query hashes/chains and exceeded collection limits.
    pub fn new(query: StatusQuery, data: StatusData, origin: Origin) -> Result<Self, Error> {
        let refund = data
            .substatus
            .as_ref()
            .is_some_and(|s| matches!(s.as_str(), "REFUNDED" | "REFUND_IN_PROGRESS"));
        if data
            .sending
            .as_ref()
            .is_some_and(|s| s.data().chain != query.from)
            || data.receiving.as_ref().is_some_and(|r| {
                r.data().chain != query.to && !(refund && r.data().chain == query.from)
            })
            || data.fees.as_ref().is_some_and(|v| v.len() > 128)
        {
            return Err(super::invalid_record());
        }
        let reported = match &query.selector {
            StatusSelector::SendingTransaction(expected) => Some((expected, data.sending.as_ref())),
            StatusSelector::ReceivingTransaction(expected) => {
                Some((expected, data.receiving.as_ref()))
            }
            StatusSelector::ProviderTransfer(expected) => {
                if data.transfer_id.as_ref().is_some_and(|id| id != expected) {
                    return Err(super::invalid_record());
                }
                None
            }
        };
        if let Some((expected, Some(actual))) = reported
            && actual.data().id.as_ref().is_some_and(|id| id != expected)
        {
            return Err(super::invalid_record());
        }
        Ok(Self {
            query,
            data,
            origin,
        })
    }
    /// Returns the exact original lookup kind and chains.
    #[must_use]
    pub const fn query(&self) -> &StatusQuery {
        &self.query
    }
    /// Returns actual reported progress, amounts, hashes and optional times.
    #[must_use]
    pub const fn data(&self) -> &StatusData {
        &self.data
    }
    /// Returns actual retrieval/source attribution.
    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }
    /// Distinguishes source-chain refunds from intended destination delivery.
    #[must_use]
    pub fn receiving_role(&self) -> ReceivingRole {
        match &self.data.receiving {
            None => ReceivingRole::Unreported,
            Some(r)
                if r.data().chain == self.query.from
                    && self.data.substatus.as_ref().is_some_and(|s| {
                        matches!(s.as_str(), "REFUNDED" | "REFUND_IN_PROGRESS")
                    }) =>
            {
                ReceivingRole::Refund
            }
            Some(_) => ReceivingRole::Destination,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusFields {
    query: StatusQuery,
    data: StatusData,
    origin: Origin,
}
impl TryFrom<StatusFields> for Status {
    type Error = Error;
    fn try_from(v: StatusFields) -> Result<Self, Error> {
        Self::new(v.query, v.data, v.origin)
    }
}
