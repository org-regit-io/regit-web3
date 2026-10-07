// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Serialize};

use crate::error::{Error, ValidationError};

use super::{
    Address, Currency, Drops, IssuedValue, Network, NetworkId, XAddress, XAddressCategory,
};

/// A resolved classic destination and optional tag bound to an explicit network.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    network: NetworkId,
    address: Address,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    tag: Option<u32>,
}
impl Destination {
    /// Records explicit classic destination facts; a classic address has no network prefix.
    #[must_use]
    pub const fn new(network: NetworkId, address: Address, tag: Option<u32>) -> Self {
        Self {
            network,
            address,
            tag,
        }
    }
    /// Resolves an X-address against an explicit expected encoding category.
    ///
    /// # Errors
    /// Rejects category mismatch. The caller selects the category separately
    /// from network ID; custom production chains can use a nonzero network ID.
    pub const fn from_xaddress(
        network: NetworkId,
        value: XAddress,
        category: XAddressCategory,
    ) -> Result<Self, Error> {
        if value.is_test() != matches!(category, XAddressCategory::Test) {
            return Err(Error::Validation(ValidationError::InvalidXrplNetwork));
        }
        Ok(Self::new(network, value.address(), value.tag()))
    }
    /// Returns the explicit technical network.
    #[must_use]
    pub const fn network(self) -> NetworkId {
        self.network
    }
    /// Returns the classic destination.
    #[must_use]
    pub const fn address(self) -> Address {
        self.address
    }
    /// Returns the tag, preserving zero separately from no tag.
    #[must_use]
    pub const fn tag(self) -> Option<u32> {
        self.tag
    }
}

/// An unsigned Payment amount using exact protocol units and issuer identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum PaymentAmount {
    /// XRP drops encoded as a decimal string.
    Xrp(Drops),
    /// Exact issued currency, issuer and decimal amount.
    Issued {
        /// Exact case-sensitive currency identity.
        currency: Currency,
        /// Explicit issuing account, independent of display metadata.
        issuer: Address,
        /// Exact amount; payment preparation requires a positive value.
        value: IssuedValue,
    },
}
impl PaymentAmount {
    fn positive(&self) -> bool {
        match self {
            Self::Xrp(drops) => drops.raw() != 0,
            Self::Issued { value, .. } => !value.value().is_zero() && !value.value().is_negative(),
        }
    }
}

/// Caller-owned ordinary Payment fields, with no signing or submission policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RequestFields")]
pub struct PaymentRequest {
    network: Network,
    account: Address,
    destination: Destination,
    amount: PaymentAmount,
    fee: Drops,
    sequence: u32,
    last_ledger_sequence: u32,
}
impl PaymentRequest {
    /// Validates exact transfer fields and a caller-selected fee and expiry.
    ///
    /// This is ordinary sequence-based preparation; ticket transactions, paths,
    /// partial payments and multisign fee calculation require separate contracts.
    /// No balance, reserve, destination existence or execution success is implied.
    ///
    /// # Errors
    /// Rejects mismatched networks, self-payments, zero/negative amounts, zero
    /// fee, zero ordinary sequence or zero expiry ledger.
    pub fn new(
        network: Network,
        account: Address,
        destination: Destination,
        amount: PaymentAmount,
        fee: Drops,
        sequence: u32,
        last_ledger_sequence: u32,
    ) -> Result<Self, Error> {
        if network.identity() != destination.network() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        if account == destination.address()
            || !amount.positive()
            || fee.raw() == 0
            || sequence == 0
            || last_ledger_sequence == 0
        {
            return Err(ValidationError::InvalidXrplRecord.into());
        }
        Ok(Self {
            network,
            account,
            destination,
            amount,
            fee,
            sequence,
            last_ledger_sequence,
        })
    }
    /// Returns the explicit intended network.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns the sender.
    #[must_use]
    pub const fn account(&self) -> Address {
        self.account
    }
    /// Returns the resolved destination and tag.
    #[must_use]
    pub const fn destination(&self) -> Destination {
        self.destination
    }
    /// Returns the exact intended amount.
    #[must_use]
    pub const fn amount(&self) -> &PaymentAmount {
        &self.amount
    }
    /// Returns the caller-selected exact fee.
    #[must_use]
    pub const fn fee(&self) -> Drops {
        self.fee
    }
    /// Returns the ordinary account sequence.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }
    /// Returns the caller-selected last permitted ledger.
    #[must_use]
    pub const fn last_ledger_sequence(&self) -> u32 {
        self.last_ledger_sequence
    }
    /// Creates reviewable unsigned fields without a codec, key, clock or network.
    #[must_use]
    pub fn prepare(self) -> PreparedPayment {
        PreparedPayment(self)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestFields {
    network: Network,
    account: Address,
    destination: Destination,
    amount: PaymentAmount,
    fee: Drops,
    sequence: u32,
    last_ledger_sequence: u32,
}
impl TryFrom<RequestFields> for PaymentRequest {
    type Error = Error;
    fn try_from(v: RequestFields) -> Result<Self, Error> {
        Self::new(
            v.network,
            v.account,
            v.destination,
            v.amount,
            v.fee,
            v.sequence,
            v.last_ledger_sequence,
        )
    }
}

/// Reviewable unsigned Payment JSON fields for caller-selected external signing.
///
/// Serialization contains no signature, private key or `SigningPubKey`.
/// `NetworkID` is omitted for IDs up to 1024 and included for higher IDs, as
/// required by XRPL replay rules. This is not a canonical binary signing blob.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedPayment(PaymentRequest);
impl PreparedPayment {
    /// Returns all exact intent and fee fields for review, including network.
    #[must_use]
    pub const fn request(&self) -> &PaymentRequest {
        &self.0
    }
}
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct UnsignedFields<'a> {
    transaction_type: &'static str,
    account: Address,
    destination: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    destination_tag: Option<u32>,
    amount: &'a PaymentAmount,
    fee: Drops,
    sequence: u32,
    last_ledger_sequence: u32,
    #[serde(rename = "NetworkID", skip_serializing_if = "Option::is_none")]
    network_id: Option<u32>,
}
impl Serialize for PreparedPayment {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let request = &self.0;
        let number = request.network().identity().number();
        UnsignedFields {
            transaction_type: "Payment",
            account: request.account(),
            destination: request.destination().address(),
            destination_tag: request.destination().tag(),
            amount: request.amount(),
            fee: request.fee(),
            sequence: request.sequence(),
            last_ledger_sequence: request.last_ledger_sequence(),
            network_id: (number > 1024).then_some(number),
        }
        .serialize(serializer)
    }
}
