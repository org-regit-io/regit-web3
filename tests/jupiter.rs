// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact Jupiter V2 requests, fresh-build preparation and wallet boundaries.
#![cfg(feature = "jupiter")]
#[path = "jupiter_support/mod.rs"]
mod support;
use regit_web3::{
    domain::{
        ExactDecimal, Source, Timestamp,
        jupiter::*,
        solana::{
            Commitment, ExecutionContext, ExecutionObservation, ExecutionOutcome, ExecutionRequest,
            Hash, MessageFee, Network, Pubkey, ReadOptions, Simulation, TransactionVersion,
        },
    },
    wallets::{HandoffId, HandoffRequest, Preparation, PreparedRequest},
};
use serde_json::json;
use support::{
    build_data, build_request, input, instruction, observation, output, prepared, quote_request,
    route, settings, taker,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn requests_validate_network_and_all_explicit_bounds_with_serde_parity() -> TestResult {
    let q = quote_request()?;
    assert_eq!(
        serde_json::from_value::<QuoteRequest>(serde_json::to_value(&q)?)?,
        q
    );
    let mut bad = q.data().clone();
    bad.amount = 0;
    assert!(QuoteRequest::new(bad.clone()).is_err());
    assert!(serde_json::from_value::<QuoteRequest>(serde_json::to_value(bad)?).is_err());
    let mut bad = q.data().clone();
    bad.network = Network::new(Hash::from_bytes([3; 32]), "wrong")?;
    assert!(QuoteRequest::new(bad).is_err());
    let mut bad = q.data().clone();
    bad.excluded_routers = vec![Router::Metis; 2];
    assert!(QuoteRequest::new(bad).is_err());
    for (accounts, expiry, percentile) in [
        (0, 150, 5000),
        (65, 150, 5000),
        (64, 0, 5000),
        (64, 301, 5000),
        (64, 150, 10001),
    ] {
        let mut bad = build_request()?.data().clone();
        bad.max_accounts = accounts;
        bad.blockhash_slots_to_expiry = expiry;
        bad.compute_unit_price_percentile = percentile;
        assert!(BuildRequest::new(bad.clone()).is_err());
        assert!(serde_json::from_value::<BuildRequest>(serde_json::to_value(bad)?).is_err());
    }
    let mut bad = build_request()?.data().clone();
    bad.destination = Destination::Native { address: taker()? };
    assert!(BuildRequest::new(bad).is_err());
    Ok(())
}
#[test]
fn exact_threshold_rounding_and_maximum_raw_units_never_default_precision() -> TestResult {
    let build = SwapBuild::new(build_data()?)?;
    assert_eq!(build.data().other_amount_threshold, 9_901);
    let mut d = build.data().clone();
    d.other_amount_threshold = 9_900;
    assert!(SwapBuild::new(d).is_ok());
    let mut d = build.data().clone();
    d.other_amount_threshold = 9_899;
    assert!(SwapBuild::new(d.clone()).is_err());
    assert!(serde_json::from_value::<SwapBuild>(serde_json::to_value(d)?).is_err());
    let mut d = build.data().clone();
    d.out_amount = u64::MAX;
    d.other_amount_threshold = 18_262_276_632_972_456_098;
    assert!(SwapBuild::new(d).is_ok());
    Ok(())
}
#[test]
fn connected_serial_routes_preserve_percentages_and_leg_amounts_without_summing() -> TestResult {
    let middle = Pubkey::from_bytes([8; 32]);
    let mut d = build_data()?;
    d.routes = vec![route(input()?, middle)?, route(middle, output()?)?];
    assert!(SwapBuild::new(d).is_ok());
    let mut d = build_data()?;
    d.routes.push(route(
        Pubkey::from_bytes([2; 32]),
        Pubkey::from_bytes([4; 32]),
    )?);
    assert!(SwapBuild::new(d).is_err());
    let mut r = route(input()?, output()?)?.data().clone();
    r.percent = ExactDecimal::parse("100.00000001")?;
    assert!(RouteStep::new(r.clone()).is_err());
    assert!(serde_json::from_value::<RouteStep>(serde_json::to_value(r)?).is_err());
    Ok(())
}
#[test]
fn lookup_table_order_and_legitimate_duplicate_entries_are_preserved() -> TestResult {
    let p = Pubkey::from_bytes([2; 32]);
    let t = LookupTable::new(Pubkey::from_bytes([3; 32]), vec![p, p, taker()?])?;
    assert_eq!(t.entries(), &[p, p, taker()?]);
    assert!(LookupTable::new(t.address(), vec![p; 257]).is_err());
    let mut d = build_data()?;
    d.lookup_tables = vec![t.clone(), t];
    assert!(SwapBuild::new(d).is_err());
    Ok(())
}
#[test]
fn instruction_bytes_require_canonical_base64_and_opaque_debug() -> TestResult {
    let bytes = InstructionData::parse("AQID")?;
    assert_eq!(bytes.bytes(), &[1, 2, 3]);
    for bad in ["AQID\n", "AQID=", "AB==", "not-a-key"] {
        assert!(InstructionData::parse(bad).is_err());
    }
    assert!(InstructionData::new(vec![0; 4097]).is_err());
    assert!(!format!("{bytes:?}").contains("AQID"));
    assert!(!format!("{:?}", Label::new("private-source-label")?).contains("private-source-label"));
    Ok(())
}
#[test]
fn source_compute_price_profile_and_explicit_ceiling_are_enforced() -> TestResult {
    let mut d = build_data()?;
    d.compute_budget[0] = instruction(
        Pubkey::parse("ComputeBudget111111111111111111111111111111")?,
        vec![2, 1, 0, 0, 0],
    )?;
    assert!(SwapBuild::new(d).is_err());
    let b = observation(SwapBuild::new(build_data()?)?)?;
    assert_eq!(b.value().compute_unit_price()?, 1484);
    let mut s = settings();
    s.maximum_compute_unit_price = 1483;
    assert!(
        SwapIntent::new(SwapIntentData {
            build: b.clone(),
            settings: s
        })
        .is_err()
    );
    let mut s = settings();
    s.compute_unit_limit = 1_400_001;
    assert!(
        SwapIntent::new(SwapIntentData {
            build: b,
            settings: s
        })
        .is_err()
    );
    Ok(())
}
#[test]
fn compilation_uses_maintained_v0_and_exact_source_hash_height_budget_fields() -> TestResult {
    let p = prepared()?;
    p.validate()?;
    assert_eq!(
        p.unsigned().transaction().message().version(),
        TransactionVersion::V0
    );
    assert_eq!(
        p.unsigned().transaction().message().recent_blockhash(),
        Hash::from_bytes([9; 32])
    );
    assert_eq!(p.unsigned().lifetime().last_valid_block_height, 400);
    let solana_message::VersionedMessage::V0(m) =
        p.unsigned().transaction().message().decoded_message()
    else {
        return Err("wrong version".into());
    };
    assert_eq!(m.instructions.len(), 3);
    assert_eq!(m.instructions[0].data, vec![2, 0x80, 0x1a, 0x06, 0]);
    assert_eq!(m.instructions[1].data, vec![3, 0xcc, 5, 0, 0, 0, 0, 0, 0]);
    assert_eq!(m.instructions[2].data, vec![1, 2, 3]);
    assert_eq!(
        p.unsigned().transaction().message().required_signatures(),
        1
    );
    assert_eq!(
        serde_json::from_value::<PreparedSwap>(serde_json::to_value(&p)?)?,
        p
    );
    Ok(())
}
#[test]
fn hidden_budget_overrides_and_unexpected_structural_signers_are_rejected() -> TestResult {
    let mut d = build_data()?;
    d.other = vec![instruction(
        Pubkey::parse("ComputeBudget111111111111111111111111111111")?,
        vec![2, 1, 0, 0, 0],
    )?];
    let intent = SwapIntent::new(SwapIntentData {
        build: observation(SwapBuild::new(d)?)?,
        settings: settings(),
    })?;
    assert!(PreparedSwap::new(intent).is_err());
    let mut d = build_data()?;
    let mut f = d.swap.fields().clone();
    f.accounts.push(AccountMeta {
        pubkey: Pubkey::from_bytes([4; 32]),
        is_signer: true,
        is_writable: false,
    });
    d.swap = Instruction::new(f)?;
    let b = observation(SwapBuild::new(d)?)?;
    assert!(
        PreparedSwap::new(SwapIntent::new(SwapIntentData {
            build: b.clone(),
            settings: settings()
        })?)
        .is_err()
    );
    let mut s = settings();
    s.additional_signers = vec![Pubkey::from_bytes([4; 32])];
    assert_eq!(
        PreparedSwap::new(SwapIntent::new(SwapIntentData {
            build: b,
            settings: s
        })?)?
        .unsigned()
        .transaction()
        .message()
        .required_signatures(),
        2
    );
    Ok(())
}
#[test]
fn wallet_handoff_retains_fresh_intent_and_never_signs_or_submits() -> TestResult {
    let p = prepared()?;
    let handoff = HandoffRequest::new(
        HandoffId::new("caller-jupiter-id")?,
        PreparedRequest::new(p.clone())?,
    );
    assert_eq!(handoff.prepared().preparation(), &p);
    assert!(!format!("{handoff:?}").contains("caller-jupiter-id"));
    let mut data = p.intent().data().clone();
    data.build = observation({
        let mut b = data.build.value().data().clone();
        b.blockhash = Hash::from_bytes([8; 32]);
        SwapBuild::new(b)?
    })?;
    assert_ne!(PreparedSwap::new(SwapIntent::new(data)?)?, p);
    Ok(())
}
#[test]
fn observations_reject_wrong_operation_network_and_unknown_serialized_fields() -> TestResult {
    let b = SwapBuild::new(build_data()?)?;
    let c = Context::new(
        Operation::OrderQuote,
        mainnet("mainnet")?,
        Source::new("fixture", "order", "1")?,
        Timestamp::from_unix_seconds(1),
    )?;
    assert!(Observation::new(b, c).is_err());
    let mut wire = serde_json::to_value(prepared()?)?;
    wire["unchecked_payload"] = json!("private");
    assert!(serde_json::from_value::<PreparedSwap>(wire).is_err());
    assert!(FetchedAt::new(0, 1_000_000_000).is_err());
    Ok(())
}
fn execution_context(
    request: ExecutionRequest,
    slot: u64,
) -> Result<ExecutionContext, regit_web3::error::Error> {
    ExecutionContext::new(
        mainnet("rpc-alias")?,
        request,
        Some(slot),
        Source::new("rpc", "fixture", env!("CARGO_PKG_VERSION"))?,
        Timestamp::from_unix_seconds(311),
    )
}
#[test]
fn estimate_preserves_independent_slots_fee_absence_and_actual_simulation_failure() -> TestResult {
    let p = prepared()?;
    let options = ReadOptions::new(Commitment::Confirmed, Some(100));
    let fee = ExecutionObservation::message_fee(
        MessageFee { lamports: None },
        execution_context(
            ExecutionRequest::MessageFee {
                message: p.unsigned().transaction().message().clone(),
                options,
            },
            101,
        )?,
    )?;
    let outcome = ExecutionOutcome::Failed {
        error: solana_transaction_error::TransactionError::AccountNotFound,
    };
    let sim = ExecutionObservation::simulation(
        Simulation::new(outcome, None, None, None, None)?,
        execution_context(
            ExecutionRequest::Simulation {
                transaction: p.unsigned().transaction().clone(),
                options,
            },
            103,
        )?,
    )?;
    let estimate = SwapEstimate::new(p, options, fee, sim)?;
    assert_eq!(estimate.fee().context().evaluation_slot(), Some(101));
    assert_eq!(estimate.simulation().context().evaluation_slot(), Some(103));
    assert_eq!(estimate.fee().value().lamports, None);
    assert_eq!(
        serde_json::from_value::<SwapEstimate>(serde_json::to_value(&estimate)?)?,
        estimate
    );
    Ok(())
}
#[test]
fn estimate_rejects_changed_message_network_and_controls() -> TestResult {
    let p = prepared()?;
    let options = ReadOptions::new(Commitment::Confirmed, None);
    let altered = ReadOptions::new(Commitment::Finalized, None);
    let fee = ExecutionObservation::message_fee(
        MessageFee {
            lamports: Some(5_000),
        },
        execution_context(
            ExecutionRequest::MessageFee {
                message: p.unsigned().transaction().message().clone(),
                options: altered,
            },
            101,
        )?,
    )?;
    let sim = ExecutionObservation::simulation(
        Simulation::new(ExecutionOutcome::Succeeded, None, None, None, None)?,
        execution_context(
            ExecutionRequest::Simulation {
                transaction: p.unsigned().transaction().clone(),
                options,
            },
            103,
        )?,
    )?;
    assert!(SwapEstimate::new(p.clone(), options, fee, sim.clone()).is_err());
    let mut changed = p.intent().data().clone();
    let mut fields = changed.build.value().data().clone();
    fields.blockhash = Hash::from_bytes([8; 32]);
    changed.build = observation(SwapBuild::new(fields)?)?;
    let different = PreparedSwap::new(SwapIntent::new(changed)?)?;
    let wrong_message = ExecutionObservation::message_fee(
        MessageFee {
            lamports: Some(5_000),
        },
        execution_context(
            ExecutionRequest::MessageFee {
                message: different.unsigned().transaction().message().clone(),
                options,
            },
            101,
        )?,
    )?;
    assert!(SwapEstimate::new(p.clone(), options, wrong_message, sim.clone()).is_err());
    let wrong_network = ExecutionObservation::message_fee(
        MessageFee {
            lamports: Some(5_000),
        },
        ExecutionContext::new(
            Network::new(Hash::from_bytes([7; 32]), "wrong")?,
            ExecutionRequest::MessageFee {
                message: p.unsigned().transaction().message().clone(),
                options,
            },
            Some(101),
            Source::new("rpc", "fixture", "1")?,
            Timestamp::from_unix_seconds(311),
        )?,
    )?;
    assert!(SwapEstimate::new(p.clone(), options, wrong_network, sim).is_err());
    Ok(())
}

#[test]
fn explicit_other_instruction_placement_changes_bytes_and_review_even_for_empty_groups()
-> TestResult {
    let mut data = build_data()?;
    let program = Pubkey::from_bytes([6; 32]);
    data.setup = vec![instruction(program, vec![41])?];
    data.other = vec![
        instruction(program, vec![42])?,
        instruction(program, vec![44])?,
    ];
    data.cleanup = Some(instruction(program, vec![43])?);
    let build = observation(SwapBuild::new(data)?)?;
    let mut values = Vec::new();
    for (placement, expected) in [
        (
            OtherInstructionPlacement::BeforeSetup,
            vec![42, 44, 41, 1, 43],
        ),
        (
            OtherInstructionPlacement::BeforeSwap,
            vec![41, 42, 44, 1, 43],
        ),
        (
            OtherInstructionPlacement::BeforeCleanup,
            vec![41, 1, 42, 44, 43],
        ),
        (
            OtherInstructionPlacement::AfterCleanup,
            vec![41, 1, 43, 42, 44],
        ),
    ] {
        let mut settings = settings();
        settings.other_instruction_placement = placement;
        let p = PreparedSwap::new(SwapIntent::new(SwapIntentData {
            build: build.clone(),
            settings,
        })?)?;
        let solana_message::VersionedMessage::V0(message) =
            p.unsigned().transaction().message().decoded_message()
        else {
            return Err("version".into());
        };
        assert_eq!(
            message.instructions[2..]
                .iter()
                .map(|i| i.data[0])
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            values
                .iter()
                .all(|prior: &PreparedSwap| prior.unsigned() != p.unsigned())
        );
        values.push(p);
    }
    let first = prepared()?;
    let mut review = first.intent().data().clone();
    review.settings.other_instruction_placement = OtherInstructionPlacement::BeforeSwap;
    let second = PreparedSwap::new(SwapIntent::new(review)?)?;
    assert_eq!(first.unsigned(), second.unsigned());
    assert_ne!(first.intent(), second.intent());
    let mut encoded = serde_json::to_value(first.intent())?;
    encoded["settings"]
        .as_object_mut()
        .ok_or("settings")?
        .remove("other_instruction_placement");
    assert!(serde_json::from_value::<SwapIntent>(encoded).is_err());
    Ok(())
}

#[test]
fn build_source_labels_respect_explicit_case_sensitive_include_exclude_filters() -> TestResult {
    let data = build_data()?;
    for filter in [
        DexFilter::Include(vec![Label::new("Fixture")?]),
        DexFilter::Exclude(vec![Label::new("fixture")?]),
    ] {
        let mut request = data.request.data().clone();
        request.dex_filter = filter;
        let mut wrong = data.clone();
        wrong.request = BuildRequest::new(request)?;
        assert!(SwapBuild::new(wrong.clone()).is_err());
        assert!(serde_json::from_value::<SwapBuild>(serde_json::to_value(wrong)?).is_err());
    }
    let mut request = data.request.data().clone();
    request.dex_filter = DexFilter::Include(vec![Label::new("fixture")?]);
    let mut allowed = data;
    allowed.request = BuildRequest::new(request)?;
    assert!(SwapBuild::new(allowed).is_ok());
    Ok(())
}

#[test]
fn optional_source_expiry_literal_has_no_invented_format_or_ttl() -> TestResult {
    let literal = ExpiryLiteral::new("Wed, 07 Oct 2026 08:00:00 GMT")?;
    assert_eq!(literal.as_str(), "Wed, 07 Oct 2026 08:00:00 GMT");
    assert_eq!(
        serde_json::from_value::<ExpiryLiteral>(serde_json::to_value(&literal)?)?,
        literal
    );
    assert!(!format!("{literal:?}").contains("GMT"));
    for bad in [String::new(), "x".repeat(129), "private\n".into()] {
        assert!(ExpiryLiteral::new(&bad).is_err());
        assert!(serde_json::from_value::<ExpiryLiteral>(json!(bad)).is_err());
    }
    Ok(())
}
