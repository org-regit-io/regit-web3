// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    domain::xrpl::{Network, PaymentRequest, PreparedPayment},
    error::Error,
};

use super::Preparation;

/// Typed ordinary XRPL Payment preparation for external field-based signing.
///
/// The unsigned payload is reviewable JSON fields derived from validated intent,
/// not a canonical binary signing payload. Signing verification needs a separate
/// caller-supplied implementation that decodes and checks actual signed content.
#[derive(Clone, Eq, PartialEq)]
pub struct XrplPaymentPreparation {
    intent: PaymentRequest,
    unsigned: PreparedPayment,
}
impl XrplPaymentPreparation {
    /// Derives unsigned fields from the exact validated intent without signing.
    #[must_use]
    pub fn new(intent: PaymentRequest) -> Self {
        let unsigned = intent.clone().prepare();
        Self { intent, unsigned }
    }
}
impl Preparation for XrplPaymentPreparation {
    type Network = Network;
    type Intent = PaymentRequest;
    type UnsignedPayload = PreparedPayment;
    fn network(&self) -> &Network {
        self.intent.network()
    }
    fn intent(&self) -> &PaymentRequest {
        &self.intent
    }
    fn unsigned_payload(&self) -> &PreparedPayment {
        &self.unsigned
    }
    fn validate(&self) -> Result<(), Error> {
        // Both immutable values originate from the same validated intent.
        Ok(())
    }
}
impl fmt::Debug for XrplPaymentPreparation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XrplPaymentPreparation")
            .finish_non_exhaustive()
    }
}
impl Serialize for XrplPaymentPreparation {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.intent.serialize(s)
    }
}
impl<'de> Deserialize<'de> for XrplPaymentPreparation {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        PaymentRequest::deserialize(d).map(Self::new)
    }
}
