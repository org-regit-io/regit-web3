// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Wallet transaction preparation, review and external signing-handoff contracts.
//!
//! Preparations retain family-specific network, intent and unsigned payload
//! types. A handoff binds a caller-generated ID to that exact snapshot. A returned
//! signed payload remains unverified until a trusted caller-supplied verifier
//! confirms actual signed-content binding. Echoed metadata is not verification.
//!
//! Custom preparation and signed types must represent immutable snapshots.
//! The library cannot police interior mutability, incomplete equality or a
//! dishonest verifier implementation. Signing, approval, replay tracking and
//! custody are caller-owned; these operations never sign or submit transactions.

mod handoff;
mod preparation;
#[cfg(feature = "xrpl")]
mod xrpl;

pub use handoff::{
    HandoffId, HandoffRequest, HandoffResponse, SignedPayloadVerifier, VerificationDecision,
    VerifiedSignedPayload, verify_handoff,
};
pub use preparation::{Preparation, PreparedRequest, Review};
#[cfg(feature = "xrpl")]
pub use xrpl::XrplPaymentPreparation;
