// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Chain, Identifier, TransactionId};
use crate::error::Error;
use serde::{Deserialize, Serialize};

/// Supported source status literals; none establishes chain inclusion or finality.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderStatus {
    /// Source reports progress in flight.
    Pending,
    /// Older source reports prolonged progress, without a local timeout policy.
    LongPending,
    /// Source indexing/provider status is unavailable; definite nonexecution is unproven.
    NotFound,
    /// Older source reversal state; no locally inferred refund.
    Revert,
    /// Source reports destination incomplete and a refund issued.
    Reverted,
    /// Source says destination tokens require a separate claim.
    ReadyToClaim,
    /// Source reports destination success; consensus/finality is not independently checked.
    Success,
    /// Source reports failure; no automatic refund policy is inferred.
    Fail,
    /// Deposit-provider source status retained for honest external status retrieval.
    KycRequired,
}
/// Explicit current direct-route status ID, source transaction and expected destination.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QueryFields")]
pub struct StatusQuery {
    id: Identifier,
    source: Chain,
    source_transaction: TransactionId,
    destination: Chain,
}
impl StatusQuery {
    /// Checks family-qualified transaction identity; IDs belong to one source namespace.
    /// # Errors
    /// Rejects contradictory source encodings.
    pub fn new(
        id: Identifier,
        source: Chain,
        source_transaction: TransactionId,
        destination: Chain,
    ) -> Result<Self, Error> {
        source_transaction.validate(&source)?;
        Ok(Self {
            id,
            source,
            source_transaction,
            destination,
        })
    }
    /// Returns the selected source swap ID, required by statusExtended.
    #[must_use]
    pub const fn id(&self) -> &Identifier {
        &self.id
    }
    /// Returns the exact caller-qualified source chain.
    #[must_use]
    pub const fn source(&self) -> &Chain {
        &self.source
    }
    /// Returns the exact caller source hash/signature.
    #[must_use]
    pub const fn source_transaction(&self) -> &TransactionId {
        &self.source_transaction
    }
    /// Returns expected destination qualification, without an API genesis claim.
    #[must_use]
    pub const fn destination(&self) -> &Chain {
        &self.destination
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryFields {
    id: Identifier,
    source: Chain,
    source_transaction: TransactionId,
    destination: Chain,
}
impl TryFrom<QueryFields> for StatusQuery {
    type Error = Error;
    fn try_from(v: QueryFields) -> Result<Self, Error> {
        Self::new(v.id, v.source, v.source_transaction, v.destination)
    }
}
/// Actual source destination transaction with correlated destination catalogue facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "DestinationFields")]
pub struct DestinationTransaction {
    chain: Chain,
    id: TransactionId,
}
impl DestinationTransaction {
    /// Checks exact family agreement without independently proving the transaction.
    /// # Errors
    /// Rejects incompatible encodings.
    pub fn new(chain: Chain, id: TransactionId) -> Result<Self, Error> {
        id.validate(&chain)?;
        Ok(Self { chain, id })
    }
    /// Returns the caller's expected destination qualification. A response without
    /// network fields does not independently attest this identity.
    #[must_use]
    pub const fn chain(&self) -> &Chain {
        &self.chain
    }
    /// Returns actual source destination ID.
    #[must_use]
    pub const fn id(&self) -> &TransactionId {
        &self.id
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DestinationFields {
    chain: Chain,
    id: TransactionId,
}
impl TryFrom<DestinationFields> for DestinationTransaction {
    type Error = Error;
    fn try_from(v: DestinationFields) -> Result<Self, Error> {
        Self::new(v.chain, v.id)
    }
}
/// Source-reported status facts, separately bound to the exact queried ID and hash.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StatusFields")]
pub struct Status {
    query: StatusQuery,
    #[serde(rename = "status")]
    state: ProviderStatus,
    destination: Option<DestinationTransaction>,
    final_human_amount: Option<crate::domain::market::NonnegativeDecimal>,
    final_raw_amount: Option<crate::domain::Amount>,
    #[serde(rename = "sub_status")]
    detail: Option<super::Text>,
}
impl Status {
    /// Checks actual destination agreement without inferring missing hashes or execution.
    /// Success with an unavailable destination transaction remains source success only.
    /// # Errors
    /// Rejects a returned destination under another family/chain qualification.
    pub fn new(
        query: StatusQuery,
        status: ProviderStatus,
        destination: Option<DestinationTransaction>,
        final_human_amount: Option<crate::domain::market::NonnegativeDecimal>,
        final_raw_amount: Option<crate::domain::Amount>,
        sub_status: Option<super::Text>,
    ) -> Result<Self, Error> {
        if destination
            .as_ref()
            .is_some_and(|d| d.chain() != query.destination())
            || final_raw_amount.is_some_and(|a| a.decimals().is_some())
        {
            return Err(super::invalid_status());
        }
        Ok(Self {
            query,
            state: status,
            destination,
            final_human_amount,
            final_raw_amount,
            detail: sub_status,
        })
    }
    /// Returns exact caller query; a source response need not echo it.
    #[must_use]
    pub const fn query(&self) -> &StatusQuery {
        &self.query
    }
    /// Returns exact supported source status literal.
    #[must_use]
    pub const fn status(&self) -> ProviderStatus {
        self.state
    }
    /// Returns actual optional destination transaction without fabricating one.
    #[must_use]
    pub const fn destination(&self) -> Option<&DestinationTransaction> {
        self.destination.as_ref()
    }
    /// Returns source final human units, with no inferred token precision.
    #[must_use]
    pub const fn final_human_amount(&self) -> Option<&crate::domain::market::NonnegativeDecimal> {
        self.final_human_amount.as_ref()
    }
    /// Returns source final raw units, with unknown precision retained as unknown.
    #[must_use]
    pub const fn final_raw_amount(&self) -> Option<crate::domain::Amount> {
        self.final_raw_amount
    }
    /// Returns a bounded source sub-status, without deriving execution/finality.
    #[must_use]
    pub const fn sub_status(&self) -> Option<&super::Text> {
        self.detail.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusFields {
    query: StatusQuery,
    status: ProviderStatus,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    destination: Option<DestinationTransaction>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    final_human_amount: Option<crate::domain::market::NonnegativeDecimal>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    final_raw_amount: Option<crate::domain::Amount>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    sub_status: Option<super::Text>,
}
impl TryFrom<StatusFields> for Status {
    type Error = Error;
    fn try_from(v: StatusFields) -> Result<Self, Error> {
        Self::new(
            v.query,
            v.status,
            v.destination,
            v.final_human_amount,
            v.final_raw_amount,
            v.sub_status,
        )
    }
}
