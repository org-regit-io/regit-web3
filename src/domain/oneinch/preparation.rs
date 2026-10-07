// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Method, Quote, Spender, SwapRequest, invalid_payload};
use crate::{
    domain::{
        Address, NetworkId,
        evm::{Data, Quantity},
    },
    error::Error,
    wallets::Preparation,
};
use serde::{Deserialize, Serialize};

/// Source-reported ordinary EVM transaction envelope; calldata semantics are unverified.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTransactionData {
    /// Source sender; must match explicit swap `from` review policy.
    pub from: Address,
    /// Source router; must match the caller's expected router identity.
    pub to: Address,
    /// Bounded opaque source calldata; no decoded ABI intent is asserted.
    pub data: Data,
    /// Exact source native value in wei.
    pub value: Quantity,
    /// Exact suggested legacy gas price in wei per gas, not caller signing policy.
    pub gas_price: Quantity,
    /// Actual source transaction gas limit suggestion, not simulation/execution proof.
    pub gas: Quantity,
    /// Optional additional source estimate, absent/null when unreported.
    pub gas_used: Option<Quantity>,
}
/// Structurally checked source transaction data without nonce, expiry or execution proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionFields")]
pub struct SourceTransaction {
    network: NetworkId,
    data: SourceTransactionData,
}
impl SourceTransaction {
    /// Retains bounded source payload bytes and source gas suggestions for independent review.
    /// # Errors
    /// Rejects empty/non-selector/excessive calldata, zero sender/router or gas.
    pub fn new(network: NetworkId, data: SourceTransactionData) -> Result<Self, Error> {
        if data.data.bytes().len() < 4
            || network.chain_id().value().is_zero()
            || data.data.bytes().len() > 65_536
            || data.from.bytes() == [0; 20]
            || data.to.bytes() == [0; 20]
            || data.from.bytes() == [0xee; 20]
            || data.to.bytes() == [0xee; 20]
            || data.gas.value().is_zero()
        {
            return Err(invalid_payload());
        }
        Ok(Self { network, data })
    }
    /// Returns explicit expected EVM network, without source genesis attestation.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns opaque source payload and exact source suggestions.
    #[must_use]
    pub const fn data(&self) -> &SourceTransactionData {
        &self.data
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionFields {
    network: NetworkId,
    data: SourceTransactionData,
}
impl TryFrom<TransactionFields> for SourceTransaction {
    type Error = Error;
    fn try_from(v: TransactionFields) -> Result<Self, Error> {
        Self::new(v.network, v.data)
    }
}

/// Original immutable quote/intent and separate fresh source estimate plus unsigned payload.
/// Structural field checks never prove opaque calldata enforces recipient/floor/spender.
/// Callers need independent calldata/contract review before any signing or execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PreparedFields")]
pub struct PreparedSwap {
    request: SwapRequest,
    fresh_quote: Quote,
    transaction: SourceTransaction,
    spender: Spender,
}
impl PreparedSwap {
    /// Builds a new review after correlating exact source identities and ordinary envelope.
    /// No source route ID is claimed; the fresh route can differ from original selection.
    /// # Errors
    /// Rejects request/network/sender/router/native-value conflicts or output below caller floor.
    pub fn new(
        request: SwapRequest,
        fresh_quote: Quote,
        transaction: SourceTransaction,
        spender: Spender,
    ) -> Result<Self, Error> {
        let settings = request.settings();
        if fresh_quote.request() != request.selection().request()
            || fresh_quote.context().method != Method::Swap
            || transaction.network() != request.network()
            || transaction.data().from != settings.from
            || transaction.data().to != settings.router
            || transaction.data().value != settings.native_value
            || &spender.context().network != request.network()
            || spender.address() != settings.spender
            || fresh_quote.data().destination_amount.value() < settings.output_floor.value()
        {
            return Err(invalid_payload());
        }
        if fresh_quote.data().source_token.fee_on_transfer == Some(true)
            || fresh_quote.data().destination_token.fee_on_transfer == Some(true)
        {
            return Err(Error::UnsupportedCapability);
        }
        Ok(Self {
            request,
            fresh_quote,
            transaction,
            spender,
        })
    }
    /// Returns exact original selection and all caller from/origin/recipient/floor/settings.
    #[must_use]
    pub const fn request(&self) -> &SwapRequest {
        &self.request
    }
    /// Returns independent fresh source output/graph/attribution, not a locked selected route.
    #[must_use]
    pub const fn fresh_quote(&self) -> &Quote {
        &self.fresh_quote
    }
    /// Returns actual unsigned source envelope; nonce/signing fees remain caller policy.
    #[must_use]
    pub const fn transaction(&self) -> &SourceTransaction {
        &self.transaction
    }
    /// Returns the separately retrieved source spender, without atomicity or approval proof.
    #[must_use]
    pub const fn spender(&self) -> &Spender {
        &self.spender
    }
}
impl Preparation for PreparedSwap {
    type Network = NetworkId;
    type Intent = SwapRequest;
    type UnsignedPayload = SourceTransaction;
    fn network(&self) -> &NetworkId {
        self.request.network()
    }
    fn intent(&self) -> &SwapRequest {
        &self.request
    }
    fn unsigned_payload(&self) -> &SourceTransaction {
        &self.transaction
    }
    fn validate(&self) -> Result<(), Error> {
        Self::new(
            self.request.clone(),
            self.fresh_quote.clone(),
            self.transaction.clone(),
            self.spender.clone(),
        )
        .map(|_| ())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedFields {
    request: SwapRequest,
    fresh_quote: Quote,
    transaction: SourceTransaction,
    spender: Spender,
}
impl TryFrom<PreparedFields> for PreparedSwap {
    type Error = Error;
    fn try_from(v: PreparedFields) -> Result<Self, Error> {
        Self::new(v.request, v.fresh_quote, v.transaction, v.spender)
    }
}
