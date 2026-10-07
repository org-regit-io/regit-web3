// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Runtime-independent review/handoff and trusted verifier extension contracts.

#![cfg(test)]
use std::{
    cell::Cell,
    future::{Future, ready},
    rc::Rc,
    task::{Context, Poll, Waker},
};

use regit_web3::{
    error::{Error, ValidationError},
    wallets::{
        HandoffId, HandoffRequest, HandoffResponse, Preparation, PreparedRequest,
        SignedPayloadVerifier, VerificationDecision, verify_handoff,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

type TestError = Box<dyn std::error::Error>;

// A small test family with independently reviewable typed binding facts. It is
// deliberately not a chain implementation or a cryptographic verifier.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixturePreparation {
    network: u32,
    intent: u64,
    payload: String,
}
impl Preparation for FixturePreparation {
    type Network = u32;
    type Intent = u64;
    type UnsignedPayload = String;
    fn network(&self) -> &u32 {
        &self.network
    }
    fn intent(&self) -> &u64 {
        &self.intent
    }
    fn unsigned_payload(&self) -> &String {
        &self.payload
    }
    fn validate(&self) -> Result<(), Error> {
        if self.network == 0
            || self.intent == 0
            || self.payload.is_empty()
            || self.payload.len() > 64
        {
            return Err(ValidationError::WalletBindingMismatch.into());
        }
        Ok(())
    }
}
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedFixture {
    actual: FixturePreparation,
    signature_valid: bool,
    sensitive_text: String,
}
struct Verifier {
    calls: Rc<Cell<usize>>,
    failure: Option<Error>,
}
impl SignedPayloadVerifier<FixturePreparation, SignedFixture> for Verifier {
    fn verify_binding(
        &self,
        prepared: &PreparedRequest<FixturePreparation>,
        signed: &SignedFixture,
    ) -> impl Future<Output = Result<VerificationDecision, Error>> + regit_web3::future::MaybeSend
    {
        self.calls.set(self.calls.get() + 1);
        let result = if let Some(error) = self.failure {
            Err(error)
        } else if signed.actual == *prepared.preparation() && signed.signature_valid {
            Ok(VerificationDecision::Confirmed)
        } else {
            Ok(VerificationDecision::Rejected)
        };
        ready(result)
    }
}
fn run<F: Future>(future: F) -> Result<F::Output, TestError> {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => Ok(value),
        Poll::Pending => Err("fixture verifier unexpectedly pending".into()),
    }
}
fn preparation() -> FixturePreparation {
    FixturePreparation {
        network: 1,
        intent: 9_007_199_254_740_993,
        payload: "fixture-payload-secret".into(),
    }
}
fn request() -> Result<HandoffRequest<FixturePreparation>, Error> {
    Ok(HandoffRequest::new(
        HandoffId::new("fixture-id-secret")?,
        PreparedRequest::new(preparation())?,
    ))
}
fn signed() -> SignedFixture {
    SignedFixture {
        actual: preparation(),
        signature_valid: true,
        sensitive_text: "fixture-signature-secret".into(),
    }
}
fn verifier() -> Verifier {
    Verifier {
        calls: Rc::new(Cell::new(0)),
        failure: None,
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn handoff_id_is_bounded_caller_generated_and_secret_safe() -> Result<(), TestError> {
    let maximum = "a".repeat(HandoffId::MAX_BYTES);
    assert_eq!(HandoffId::new(&maximum)?.as_str(), maximum);
    let id = HandoffId::new("A0_-fixture-secret")?;
    assert_eq!(
        serde_json::from_str::<HandoffId>(&serde_json::to_string(&id)?)?,
        id
    );
    assert!(!format!("{id:?}").contains("fixture-secret"));
    for invalid in ["", "bad id", "bad/id", "bad\n", "é"] {
        assert_eq!(
            HandoffId::new(invalid),
            Err(Error::Validation(ValidationError::InvalidWalletHandoffId))
        );
        assert!(serde_json::from_value::<HandoffId>(json!(invalid)).is_err());
    }
    assert!(HandoffId::new(&"a".repeat(HandoffId::MAX_BYTES + 1)).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn review_preserves_exact_typed_snapshot_without_invoking_verifier() -> Result<(), TestError> {
    let verifier = verifier();
    let request = request()?;
    let review = request.prepared().review();
    assert_eq!(*review.network(), 1);
    assert_eq!(*review.intent(), 9_007_199_254_740_993);
    assert_eq!(review.unsigned_payload(), "fixture-payload-secret");
    let response = HandoffResponse::new(request.id().clone(), request.prepared().clone(), signed());
    let encoded = serde_json::to_string(&response)?;
    let restored: HandoffResponse<FixturePreparation, SignedFixture> =
        serde_json::from_str(&encoded)?;
    assert_eq!(restored, response);
    assert_eq!(restored.id(), request.id());
    assert_eq!(restored.prepared(), request.prepared());
    assert_eq!(verifier.calls.get(), 0);
    for debug in [
        format!("{:?}", request.prepared()),
        format!("{review:?}"),
        format!("{request:?}"),
        format!("{response:?}"),
    ] {
        assert!(!debug.contains("fixture-"));
        assert!(!debug.contains("9007199254740993"));
    }
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn id_network_intent_and_unsigned_payload_mismatches_skip_verifier() -> Result<(), TestError> {
    let request = request()?;
    let verifier = verifier();
    let wrong_id = HandoffResponse::new(
        HandoffId::new("different")?,
        request.prepared().clone(),
        signed(),
    );
    assert_eq!(
        run(verify_handoff(&request, wrong_id, &verifier))?.err(),
        Some(Error::Validation(ValidationError::WalletBindingMismatch))
    );
    for changed in [
        FixturePreparation {
            network: 2,
            ..preparation()
        },
        FixturePreparation {
            intent: 1,
            ..preparation()
        },
        FixturePreparation {
            payload: "altered-payload".into(),
            ..preparation()
        },
    ] {
        let response = HandoffResponse::new(
            request.id().clone(),
            PreparedRequest::new(changed)?,
            signed(),
        );
        assert_eq!(
            run(verify_handoff(&request, response, &verifier))?.err(),
            Some(Error::Validation(ValidationError::WalletBindingMismatch))
        );
    }
    assert_eq!(verifier.calls.get(), 0);
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn matching_echo_metadata_does_not_qualify_altered_actual_signed_content() -> Result<(), TestError>
{
    let request = request()?;
    let verifier = verifier();
    for actual in [
        FixturePreparation {
            network: 2,
            ..preparation()
        },
        FixturePreparation {
            intent: 1,
            ..preparation()
        },
        FixturePreparation {
            payload: "altered-signed-payload".into(),
            ..preparation()
        },
    ] {
        let response = HandoffResponse::new(
            request.id().clone(),
            request.prepared().clone(),
            SignedFixture { actual, ..signed() },
        );
        assert_eq!(
            run(verify_handoff(&request, response, &verifier))?.err(),
            Some(Error::Validation(ValidationError::SignedPayloadRejected))
        );
    }
    let response = HandoffResponse::new(
        request.id().clone(),
        request.prepared().clone(),
        SignedFixture {
            signature_valid: false,
            ..signed()
        },
    );
    assert_eq!(
        run(verify_handoff(&request, response, &verifier))?.err(),
        Some(Error::Validation(ValidationError::SignedPayloadRejected))
    );
    assert_eq!(verifier.calls.get(), 4);
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn verifier_confirmation_error_and_rejection_remain_distinct() -> Result<(), TestError> {
    let request = request()?;
    let verifier = verifier();
    let response = HandoffResponse::new(request.id().clone(), request.prepared().clone(), signed());
    let confirmed = run(verify_handoff(&request, response.clone(), &verifier))??;
    assert_eq!(confirmed.id(), request.id());
    assert_eq!(confirmed.prepared(), request.prepared());
    assert!(confirmed.signed().actual == preparation());
    assert!(!format!("{confirmed:?}").contains("fixture-"));
    assert_eq!(
        confirmed.into_signed().sensitive_text,
        "fixture-signature-secret"
    );
    assert_eq!(verifier.calls.get(), 1);
    let unsupported = Verifier {
        calls: Rc::clone(&verifier.calls),
        failure: Some(Error::UnsupportedCapability),
    };
    assert_eq!(
        run(verify_handoff(&request, response, &unsupported))?.err(),
        Some(Error::UnsupportedCapability)
    );
    assert_eq!(unsupported.calls.get(), 2);
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn composed_verification_future_is_send_for_ordinary_thread_safe_snapshots() -> Result<(), TestError>
{
    struct ThreadSafeVerifier;
    impl SignedPayloadVerifier<FixturePreparation, SignedFixture> for ThreadSafeVerifier {
        fn verify_binding(
            &self,
            prepared: &PreparedRequest<FixturePreparation>,
            signed: &SignedFixture,
        ) -> impl Future<Output = Result<VerificationDecision, Error>> + regit_web3::future::MaybeSend
        {
            ready(Ok(
                if signed.actual == *prepared.preparation() && signed.signature_valid {
                    VerificationDecision::Confirmed
                } else {
                    VerificationDecision::Rejected
                },
            ))
        }
    }
    fn requires_send<F: Future + regit_web3::future::MaybeSend>(future: F) -> F {
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        {
            fn assert_send<T: Send>(_: &T) {}
            assert_send(&future);
        }
        future
    }
    let request = request()?;
    let response = HandoffResponse::new(request.id().clone(), request.prepared().clone(), signed());
    let verified = run(requires_send(verify_handoff(
        &request,
        response,
        &ThreadSafeVerifier,
    )))??;
    assert!(verified.signed().actual == preparation());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn constructor_and_serde_validation_match_for_all_unverified_records() -> Result<(), TestError> {
    let request = request()?;
    let encoded = serde_json::to_string(&request)?;
    assert_eq!(
        serde_json::from_str::<HandoffRequest<FixturePreparation>>(&encoded)?,
        request
    );
    for invalid in [
        FixturePreparation {
            network: 0,
            ..preparation()
        },
        FixturePreparation {
            intent: 0,
            ..preparation()
        },
        FixturePreparation {
            payload: String::new(),
            ..preparation()
        },
        FixturePreparation {
            payload: "a".repeat(65),
            ..preparation()
        },
    ] {
        assert!(PreparedRequest::new(invalid.clone()).is_err());
        let fields = json!({"preparation":invalid});
        assert!(
            serde_json::from_value::<PreparedRequest<FixturePreparation>>(fields.clone()).is_err()
        );
        assert!(
            serde_json::from_value::<HandoffRequest<FixturePreparation>>(
                json!({"id":"id","prepared":fields})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<HandoffResponse<FixturePreparation, SignedFixture>>(
                json!({"id":"id","prepared":fields,"signed":signed()})
            )
            .is_err()
        );
    }
    for changed in [
        encoded.replace("\"id\":", "\"id\":\"other\",\"id\":"),
        encoded.replace("\"prepared\":", "\"unknown\":true,\"prepared\":"),
        encoded.replace("\"network\":1", "\"network\":1,\"network\":1"),
    ] {
        assert!(serde_json::from_str::<HandoffRequest<FixturePreparation>>(&changed).is_err());
    }
    let response = HandoffResponse::new(request.id().clone(), request.prepared().clone(), signed());
    let mut external: Value = serde_json::to_value(&response)?;
    external["verified"] = json!(true);
    assert!(
        serde_json::from_value::<HandoffResponse<FixturePreparation, SignedFixture>>(external)
            .is_err()
    );
    Ok(())
}
