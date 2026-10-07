// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, future::Future};

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{Error, ValidationError};

use super::{Preparation, PreparedRequest};

/// A bounded caller-generated correlation ID, never an approval or replay proof.
///
/// The library does not generate IDs, promise uniqueness or track prior use.
/// Callers own generation, storage, freshness and replay policy.
#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct HandoffId(String);
impl HandoffId {
    /// Maximum ASCII bytes in a correlation ID.
    pub const MAX_BYTES: usize = 128;
    /// Validates nonempty ASCII alphanumeric, hyphen and underscore IDs.
    ///
    /// # Errors
    /// Rejects an invalid lexical shape or excessive length without echoing input.
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > Self::MAX_BYTES
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(ValidationError::InvalidWalletHandoffId.into());
        }
        Ok(Self(value.into()))
    }
    /// Returns the ID for explicit correlation or serialization.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for HandoffId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandoffId").finish_non_exhaustive()
    }
}
impl<'de> Deserialize<'de> for HandoffId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// An explicit external handoff of the exact prepared snapshot and correlation ID.
///
/// Constructing or serializing it does not sign, submit, approve or verify.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct HandoffRequest<P: Preparation> {
    id: HandoffId,
    prepared: PreparedRequest<P>,
}
impl<P: Preparation> HandoffRequest<P> {
    /// Binds a caller-generated ID to an already validated preparation.
    #[must_use]
    pub const fn new(id: HandoffId, prepared: PreparedRequest<P>) -> Self {
        Self { id, prepared }
    }
    /// Returns the exact caller-generated correlation ID.
    #[must_use]
    pub const fn id(&self) -> &HandoffId {
        &self.id
    }
    /// Returns the exact reviewed preparation without mutable access.
    #[must_use]
    pub const fn prepared(&self) -> &PreparedRequest<P> {
        &self.prepared
    }
}
impl<P: Preparation> fmt::Debug for HandoffRequest<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandoffRequest").finish_non_exhaustive()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestFields<P> {
    id: HandoffId,
    prepared: P,
}
impl<'de, P: Preparation + Deserialize<'de>> Deserialize<'de> for HandoffRequest<P> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let fields = RequestFields::<PreparedRequest<P>>::deserialize(d)?;
        Ok(Self::new(fields.id, fields.prepared))
    }
}

/// An external response whose signed payload is still unverified.
///
/// Echoed ID and preparation metadata are untrusted correlation data. `S` must
/// retain immutable family-specific signed content and its own resource bounds.
/// Equality of echoed metadata alone cannot verify signed-content binding.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct HandoffResponse<P: Preparation, S> {
    id: HandoffId,
    prepared: PreparedRequest<P>,
    signed: S,
}
impl<P: Preparation, S> HandoffResponse<P, S> {
    /// Records a response without claiming signing correctness or reviewed binding.
    #[must_use]
    pub const fn new(id: HandoffId, prepared: PreparedRequest<P>, signed: S) -> Self {
        Self {
            id,
            prepared,
            signed,
        }
    }
    /// Returns the response's untrusted echoed correlation ID.
    #[must_use]
    pub const fn id(&self) -> &HandoffId {
        &self.id
    }
    /// Returns the response's untrusted echoed preparation.
    #[must_use]
    pub const fn prepared(&self) -> &PreparedRequest<P> {
        &self.prepared
    }
    /// Returns unverified signed content for explicit caller verification.
    #[must_use]
    pub const fn signed(&self) -> &S {
        &self.signed
    }
}
impl<P: Preparation, S> fmt::Debug for HandoffResponse<P, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandoffResponse").finish_non_exhaustive()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseFields<P, S> {
    id: HandoffId,
    prepared: P,
    signed: S,
}
impl<'de, P: Preparation + Deserialize<'de>, S: Deserialize<'de>> Deserialize<'de>
    for HandoffResponse<P, S>
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let fields = ResponseFields::<PreparedRequest<P>, S>::deserialize(d)?;
        Ok(Self::new(fields.id, fields.prepared, fields.signed))
    }
}

/// A trusted verifier's decision about actual signed-content binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationDecision {
    /// Actual signed content matches the supplied network, intent and unsigned payload.
    Confirmed,
    /// Actual signed content does not qualify as the reviewed preparation.
    Rejected,
}

/// Caller-supplied family-specific semantic verification of returned signed content.
///
/// `Confirmed` must mean actual signed content matches the exact reviewed
/// network, intent and unsigned payload under the family's rules, including any
/// required signature checks. Merely comparing echoed metadata is insufficient.
/// The library trusts this extension and provides no concrete cryptographic
/// verifier, signer or connector. Implementations must not sign or submit here.
/// Futures are `Send` on native targets and host-local on JavaScript WebAssembly.
/// Verifiers need not themselves be `Send` or `Sync`.
pub trait SignedPayloadVerifier<P: Preparation, S> {
    /// Checks immutable signed content against the exact reviewed preparation.
    fn verify_binding(
        &self,
        prepared: &PreparedRequest<P>,
        signed: &S,
    ) -> impl Future<Output = Result<VerificationDecision, Error>> + crate::future::MaybeSend;
}

/// Signed content confirmed by a trusted verifier against the correlated review.
///
/// This value has no public unchecked constructor or deserialization path. It
/// does not prove that a concrete cryptographic backend exists, caller approval
/// was granted, or submission/validation occurred. Its guarantees rely on the
/// custom snapshot and verifier contracts.
///
/// ```compile_fail
/// use regit_web3::wallets::VerifiedSignedPayload;
/// fn restore<P, S>()
/// where
///     P: regit_web3::wallets::Preparation + serde::de::DeserializeOwned,
///     S: serde::de::DeserializeOwned,
/// {
///     fn requires_deserialize<T: serde::de::DeserializeOwned>() {}
///     requires_deserialize::<VerifiedSignedPayload<P, S>>();
/// }
/// ```
#[derive(Clone)]
pub struct VerifiedSignedPayload<P: Preparation, S> {
    id: HandoffId,
    prepared: PreparedRequest<P>,
    signed: S,
}
impl<P: Preparation, S> VerifiedSignedPayload<P, S> {
    /// Returns the correlated request ID, without a freshness or replay claim.
    #[must_use]
    pub const fn id(&self) -> &HandoffId {
        &self.id
    }
    /// Returns the exact original reviewed preparation.
    #[must_use]
    pub const fn prepared(&self) -> &PreparedRequest<P> {
        &self.prepared
    }
    /// Returns signed content confirmed by the supplied verifier.
    #[must_use]
    pub const fn signed(&self) -> &S {
        &self.signed
    }
    /// Releases signed content for a separate explicit family submission operation.
    ///
    /// This method does not submit or retry; callers retain approval/replay policy.
    #[must_use]
    pub fn into_signed(self) -> S {
        self.signed
    }
}
impl<P: Preparation, S> fmt::Debug for VerifiedSignedPayload<P, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedSignedPayload")
            .finish_non_exhaustive()
    }
}

/// Correlates an external response before invoking trusted signed-content verification.
///
/// ID and every typed preparation binding must match the original request.
/// Confirmed output retains the original reviewed snapshot. No signing,
/// approval, submission, clock or runtime is created by this function.
/// The composed future is `Send` when its snapshot and verifier types permit it;
/// local use does not require every custom type to be `Send` or `Sync`.
///
/// # Errors
/// Returns `WalletBindingMismatch` without invoking the verifier for altered
/// correlation data; `SignedPayloadRejected` for a verifier rejection; otherwise
/// propagates the verifier's fixed typed error.
pub async fn verify_handoff<P: Preparation, S, V: SignedPayloadVerifier<P, S>>(
    request: &HandoffRequest<P>,
    response: HandoffResponse<P, S>,
    verifier: &V,
) -> Result<VerifiedSignedPayload<P, S>, Error> {
    if request.id != response.id || !request.prepared.matches(&response.prepared) {
        return Err(ValidationError::WalletBindingMismatch.into());
    }
    match verifier
        .verify_binding(&request.prepared, &response.signed)
        .await?
    {
        VerificationDecision::Rejected => Err(ValidationError::SignedPayloadRejected.into()),
        VerificationDecision::Confirmed => Ok(VerifiedSignedPayload {
            id: request.id.clone(),
            prepared: request.prepared.clone(),
            signed: response.signed,
        }),
    }
}
