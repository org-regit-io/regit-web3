// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure signed-payload assertions and preliminary XRPL submission facts.

#![cfg(feature = "xrpl")]

use std::{
    future::ready,
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

use regit_web3::{
    chains::xrpl::XrplSubmitter,
    domain::{
        Source, Timestamp,
        xrpl::{
            Context, Drops, EngineResultClass, EngineResultCode, Hash, HexData, Ledger, Network,
            NetworkId, Observation, Operation, Setting, SignedSubmission, SubmissionHandling,
            SubmissionLedgerState, SubmissionResult,
        },
    },
    error::{Error, SubmissionFailure, ValidationError},
};
use serde_json::{Value, json};

fn handling() -> Result<SubmissionHandling, Error> {
    SubmissionHandling::new(
        Setting::Enabled,
        Setting::Disabled,
        Setting::Disabled,
        Setting::Enabled,
        Setting::Enabled,
    )
}
fn result(hash: Hash) -> Result<SubmissionResult, Error> {
    Ok(SubmissionResult::new(
        hash,
        EngineResultCode::parse("terQUEUED")?,
        handling()?,
        Some(SubmissionLedgerState::new(9, 7, Drops::new(10)?, 100)?),
    ))
}
fn context() -> Result<Context, Error> {
    Ok(Context::new(
        Operation::Submission,
        Network::new(NetworkId::new(0), "fixture")?,
        None,
        Source::new("fixture", "submit", env!("CARGO_PKG_VERSION"))?,
        Timestamp::from_unix_seconds(1),
    ))
}

#[test]
fn signed_submission_verifies_exact_byte_identity_without_signing_claims()
-> Result<(), Box<dyn std::error::Error>> {
    // Arbitrary bounded bytes are accepted: the caller asserts signing readiness.
    let payload = HexData::parse("abcd")?;
    let submission = SignedSubmission::new(NetworkId::new(1025), payload.clone(), true);
    assert_eq!(submission.hash(), payload.transaction_hash());
    assert_eq!(submission.payload().bytes(), &[0xab, 0xcd]);
    assert_eq!(submission.network(), NetworkId::new(1025));
    assert!(submission.fail_hard());
    let encoded = serde_json::to_string(&submission)?;
    assert_eq!(
        serde_json::from_str::<SignedSubmission>(&encoded)?,
        submission
    );
    let mut wrong: Value = serde_json::from_str(&encoded)?;
    wrong["hash"] = json!(Hash::from_bytes([1; 32]));
    assert!(serde_json::from_value::<SignedSubmission>(wrong).is_err());
    let mut extra: Value = serde_json::from_str(&encoded)?;
    extra["seed"] = json!("fixture-secret");
    assert!(serde_json::from_value::<SignedSubmission>(extra).is_err());
    assert!(!format!("{submission:?}").contains("ABCD"));
    Ok(())
}

#[test]
fn engine_results_are_preliminary_and_support_all_actual_protocol_classes()
-> Result<(), Box<dyn std::error::Error>> {
    for (code, class) in [
        ("tesSUCCESS", EngineResultClass::Success),
        ("tecPATH_DRY", EngineResultClass::ClaimedCost),
        ("tefALREADY", EngineResultClass::Failure),
        ("telCAN_NOT_QUEUE", EngineResultClass::LocalError),
        ("temBAD_SIGNATURE", EngineResultClass::Malformed),
        ("terQUEUED", EngineResultClass::Retry),
    ] {
        let engine = EngineResultCode::parse(code)?;
        assert_eq!(engine.class(), class);
        assert_eq!(engine.as_str(), code);
        assert_eq!(
            serde_json::from_str::<EngineResultCode>(&serde_json::to_string(&engine)?)?,
            engine
        );
    }
    for code in [
        "tesFAILURE",
        "ter",
        "terQueue",
        "tel BAD",
        "<secret>",
        "unknown",
    ] {
        assert!(EngineResultCode::parse(code).is_err());
    }
    assert!(EngineResultCode::parse(&format!("ter{}", "A".repeat(62))).is_err());
    Ok(())
}

#[test]
fn preliminary_handling_and_latest_ledger_do_not_fabricate_inclusion()
-> Result<(), Box<dyn std::error::Error>> {
    let hash = HexData::parse("ABCD")?.transaction_hash();
    let queued = result(hash)?;
    assert_eq!(queued.handling().queued(), Setting::Enabled);
    assert_eq!(queued.handling().applied(), Setting::Disabled);
    assert_eq!(
        queued
            .ledger_state()
            .map(SubmissionLedgerState::latest_validated_ledger_index),
        Some(100)
    );
    let observed = Observation::submission(queued, context()?)?;
    assert_eq!(observed.context().ledger(), None);
    assert_eq!(
        serde_json::from_str::<Observation<SubmissionResult>>(&serde_json::to_string(&observed)?)?,
        observed
    );
    let state = observed.value().ledger_state().ok_or("missing state")?;
    assert_eq!(state.account_sequence_available(), 9);
    assert_eq!(state.account_sequence_next(), 7);
    assert_eq!(state.open_ledger_cost(), Drops::new(10)?);
    let supplied_inclusion = Context::new(
        Operation::Submission,
        context()?.network().clone(),
        Some(Ledger::new(100, None, true)?),
        Source::new("fixture", "submit", "1")?,
        Timestamp::from_unix_seconds(1),
    );
    assert_eq!(
        Observation::submission(observed.value().clone(), supplied_inclusion),
        Err(Error::Validation(ValidationError::InvalidXrplRecord))
    );
    assert!(
        SubmissionHandling::new(
            Setting::Disabled,
            Setting::Disabled,
            Setting::Disabled,
            Setting::Disabled,
            Setting::Enabled
        )
        .is_err()
    );
    assert!(
        SubmissionHandling::new(
            Setting::Enabled,
            Setting::Disabled,
            Setting::Disabled,
            Setting::Disabled,
            Setting::Disabled
        )
        .is_err()
    );
    assert!(SubmissionLedgerState::new(0, 0, Drops::new(0)?, 0).is_err());
    let rejected = SubmissionHandling::new(
        Setting::Disabled,
        Setting::Disabled,
        Setting::Disabled,
        Setting::Disabled,
        Setting::Disabled,
    )?;
    assert_eq!(rejected.accepted(), Setting::Disabled);
    assert_eq!(rejected.broadcast(), Setting::Disabled);
    assert_eq!(rejected.kept(), Setting::Disabled);
    Ok(())
}

#[test]
fn submitter_trait_has_no_runtime_or_send_sync_supertrait_requirement()
-> Result<(), Box<dyn std::error::Error>> {
    struct Pure(Rc<()>);
    impl XrplSubmitter for Pure {
        fn submit_signed(
            &self,
            _submission: SignedSubmission,
        ) -> impl std::future::Future<Output = Result<Observation<SubmissionResult>, Error>> + Send
        {
            let _count = Rc::strong_count(&self.0);
            ready(Err(Error::UnsupportedCapability))
        }
    }
    let pure = Pure(Rc::new(()));
    let submission = SignedSubmission::new(NetworkId::new(0), HexData::parse("ABCD")?, false);
    let mut future = std::pin::pin!(pure.submit_signed(submission));
    let mut task = TaskContext::from_waker(Waker::noop());
    assert_eq!(
        std::future::Future::poll(future.as_mut(), &mut task),
        Poll::Ready(Err(Error::UnsupportedCapability))
    );
    Ok(())
}

#[test]
fn ambiguous_submission_errors_serialize_fixed_safe_categories()
-> Result<(), Box<dyn std::error::Error>> {
    for reason in [
        SubmissionFailure::Timeout,
        SubmissionFailure::Transport,
        SubmissionFailure::RateLimited,
        SubmissionFailure::HttpStatus,
        SubmissionFailure::ResponseTooLarge,
        SubmissionFailure::InvalidResponse,
        SubmissionFailure::Rpc,
    ] {
        let error = Error::SubmissionOutcomeUnknown(reason);
        assert_eq!(error.code(), "submission_outcome_unknown");
        assert!(
            error
                .to_string()
                .starts_with("submission outcome unknown: ")
        );
        assert_eq!(
            serde_json::from_str::<Error>(&serde_json::to_string(&error)?)?,
            error
        );
        assert!(!error.to_string().contains("fixture-secret"));
    }
    Ok(())
}
