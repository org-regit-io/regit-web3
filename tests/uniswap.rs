// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure V3 request, source observation and Universal Router 2.1.2 preparation behavior.
#![cfg(test)]
#![cfg(feature = "uniswap")]

use regit_web3::{
    domain::{
        Address, BlockContext, BlockHash, BlockSelector, ChainId, NetworkId, Source, Timestamp,
        U256,
        evm::{
            AccountNonce, FeeTerms, OperationContext, OperationObservation, Quantity,
            ReadOperation, ReadState,
        },
        uniswap::{
            CallSettings, CallSettingsData, PreparedSwap, RouteComparison, RouteComparisonData,
            RouteOutcome, SlippageBps, SwapIntent, SwapIntentData, V3Deployment, V3Path,
            V3PathData, V3Quote, V3QuoteData, V3QuoteObservation, V3QuoteRequest,
            V3QuoteRequestData, V3RouteRequest, V3RouteRequestData,
        },
    },
    error::Error,
    wallets::{HandoffId, HandoffRequest, Preparation, PreparedRequest},
};
use serde_json::json;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn settings() -> Result<CallSettings, Error> {
    CallSettings::new(CallSettingsData {
        from: Address::from_bytes([7; 20]),
        nonce: 9,
        gas_limit: 500_000,
        fees: FeeTerms::Legacy {
            gas_price: Quantity::from(10),
        },
    })
}
fn path(fee: u32) -> Result<V3Path, Error> {
    V3Path::new(V3PathData {
        tokens: vec![Address::from_bytes([3; 20]), Address::from_bytes([4; 20])],
        fees: vec![fee],
    })
}
fn request(fee: u32) -> Result<V3QuoteRequest, Error> {
    V3QuoteRequest::new(V3QuoteRequestData {
        deployment: V3Deployment::ethereum_mainnet()?,
        path: path(fee)?,
        amount_in: Quantity::from(1_000),
        settings: settings()?,
    })
}
fn context(operation: ReadOperation) -> Result<OperationContext, Error> {
    OperationContext::new(
        operation,
        NetworkId::new(ChainId::from(1), "fixture")?,
        ReadState::CanonicalHash {
            requested_selector: BlockSelector::Safe,
            block: BlockContext::new(
                42,
                BlockHash::from_bytes([5; 32]),
                Timestamp::from_unix_seconds(100),
            ),
        },
        Source::new("fixture", "eth_call", env!("CARGO_PKG_VERSION"))?,
        Timestamp::from_unix_seconds(110),
    )
}
fn quote(fee: u32, amount: Quantity) -> Result<V3QuoteObservation, Error> {
    V3QuoteObservation::new(
        V3Quote::new(V3QuoteData {
            request: request(fee)?,
            amount_out: amount,
            sqrt_price_x96_after: vec![Quantity::from(123)],
            initialized_ticks_crossed: vec![3],
            gas_estimate: Quantity::from(42_000),
        })?,
        context(ReadOperation::Call)?,
    )
}
fn intent() -> Result<SwapIntent, Error> {
    SwapIntent::new(SwapIntentData {
        quote: quote(500, Quantity::from(10_001))?,
        recipient: Address::from_bytes([9; 20]),
        slippage: SlippageBps::new(100)?,
        deadline: Timestamp::from_unix_seconds(200),
        settings: settings()?,
    })
}
fn route() -> Result<V3RouteRequest, Error> {
    V3RouteRequest::new(V3RouteRequestData {
        deployment: V3Deployment::ethereum_mainnet()?,
        paths: vec![path(500)?, path(3000)?],
        amount_in: Quantity::from(1_000),
        settings: settings()?,
    })
}
fn anchor() -> Result<OperationObservation<AccountNonce>, Error> {
    OperationObservation::new(
        AccountNonce {
            chain_id: ChainId::from(1),
            address: settings()?.data().from,
            nonce: 44,
        },
        context(ReadOperation::AccountNonce)?,
    )
}
fn word(bytes: &[u8], offset: usize) -> U256 {
    U256::from_be_slice(&bytes[offset..offset + 32])
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn deployment_versions_and_constructor_serde_validation_agree() -> TestResult {
    let deployed = V3Deployment::ethereum_mainnet()?;
    assert_eq!(deployed.chain_id(), ChainId::from(1));
    assert_eq!(
        deployed.data().universal_router_2_1_2.to_string(),
        "0x23617e59a5925b2a4bf75d73ff6711cd0b29de85"
    );
    let mut data = deployed.data().clone();
    data.quoter_v2 = data.factory;
    assert!(V3Deployment::new(data.clone()).is_err());
    assert!(serde_json::from_value::<V3Deployment>(serde_json::to_value(data)?).is_err());
    let mut wire = serde_json::to_value(&deployed)?;
    wire["unsupported_version"] = json!("v4");
    assert!(serde_json::from_value::<V3Deployment>(wire).is_err());
    assert_eq!(
        serde_json::from_value::<V3Deployment>(serde_json::to_value(&deployed)?)?,
        deployed
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn paths_preserve_forward_tokens_fees_repeats_and_exact_bounds() -> TestResult {
    let p = path(500)?;
    let bytes = p.encoded()?;
    assert_eq!(bytes.bytes().len(), 43);
    assert_eq!(&bytes.bytes()[..20], &[3; 20]);
    assert_eq!(&bytes.bytes()[20..23], &[0, 1, 244]);
    assert_eq!(&bytes.bytes()[23..], &[4; 20]);
    for bad in [
        V3PathData {
            tokens: vec![],
            fees: vec![],
        },
        V3PathData {
            tokens: vec![Address::from_bytes([3; 20]); 2],
            fees: vec![500],
        },
        V3PathData {
            tokens: vec![Address::from_bytes([0; 20]), Address::from_bytes([3; 20])],
            fees: vec![500],
        },
        V3PathData {
            tokens: p.data().tokens.clone(),
            fees: vec![1_000_000],
        },
    ] {
        assert!(V3Path::new(bad.clone()).is_err());
        assert!(serde_json::from_value::<V3Path>(serde_json::to_value(bad)?).is_err());
    }
    let tokens: Vec<_> = (1..=9).map(|b| Address::from_bytes([b; 20])).collect();
    assert_eq!(
        V3Path::new(V3PathData {
            tokens: tokens.clone(),
            fees: vec![0; 8]
        })?
        .hops(),
        8
    );
    assert!(
        V3Path::new(V3PathData {
            tokens,
            fees: vec![1; 9]
        })
        .is_err()
    );
    assert!(
        V3Path::new(V3PathData {
            tokens: vec![p.token_in(), p.token_out(), p.token_in()],
            fees: vec![500, 3000]
        })
        .is_ok()
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn exact_quote_calldata_and_signed_swap_amount_boundary() -> TestResult {
    let request = request(500)?;
    let call = request.call()?;
    let bytes = call.data().input.bytes();
    assert_eq!(&bytes[..4], &[0xcd, 0xca, 0x17, 0x53]);
    assert_eq!(word(bytes, 4), U256::from(64));
    assert_eq!(word(bytes, 36), U256::from(1_000));
    assert_eq!(word(bytes, 68), U256::from(43));
    assert_eq!(&bytes[100..143], request.data().path.encoded()?.bytes());
    assert!(bytes[143..].iter().all(|b| *b == 0));
    assert_eq!(
        call.data().to,
        Some(request.data().deployment.data().quoter_v2)
    );
    assert_eq!(call.data().value, Quantity::from(0));
    let mut data = request.data().clone();
    data.amount_in = Quantity::new(U256::MAX / U256::from(2));
    assert!(V3QuoteRequest::new(data.clone()).is_ok());
    for amount in [
        U256::ZERO,
        U256::MAX / U256::from(2) + U256::from(1),
        U256::MAX,
    ] {
        data.amount_in = Quantity::new(amount);
        assert!(V3QuoteRequest::new(data.clone()).is_err());
        assert!(serde_json::from_value::<V3QuoteRequest>(serde_json::to_value(&data)?).is_err());
    }
    let mut bad = settings()?.data().clone();
    bad.gas_limit = 0;
    assert!(CallSettings::new(bad.clone()).is_err());
    assert!(serde_json::from_value::<CallSettings>(serde_json::to_value(bad)?).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn quote_cardinality_width_network_and_attribution_cannot_be_fabricated() -> TestResult {
    let q = quote(500, Quantity::new(U256::MAX))?;
    assert_eq!(
        serde_json::from_value::<V3QuoteObservation>(serde_json::to_value(&q)?)?,
        q
    );
    for variant in 0..3 {
        let mut data = q.value().data().clone();
        match variant {
            0 => data.sqrt_price_x96_after.clear(),
            1 => data.initialized_ticks_crossed.push(0),
            _ => data.sqrt_price_x96_after[0] = Quantity::new(U256::from(1) << 160),
        }
        assert!(V3Quote::new(data.clone()).is_err());
        assert!(serde_json::from_value::<V3Quote>(serde_json::to_value(data)?).is_err());
    }
    for replacement in [json!({"kind":"unanchored"}), json!({"kind":"pending"})] {
        let mut wire = serde_json::to_value(&q)?;
        wire["context"]["state"] = replacement;
        assert!(serde_json::from_value::<V3QuoteObservation>(wire).is_err());
    }
    let mut wire = serde_json::to_value(&q)?;
    wire["context"]["network"]["chain_id"] = json!("2");
    assert!(serde_json::from_value::<V3QuoteObservation>(wire).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn supplied_route_comparison_has_stable_ties_and_honest_all_reverted_outcome() -> TestResult {
    let request = route()?;
    let outcomes = vec![
        RouteOutcome::Quoted {
            quote: Box::new(quote(500, Quantity::from(9))?),
        },
        RouteOutcome::Quoted {
            quote: Box::new(quote(3000, Quantity::from(9))?),
        },
    ];
    let record = RouteComparison::new(RouteComparisonData {
        request: request.clone(),
        anchor: anchor()?,
        outcomes,
    })?;
    assert_eq!(record.best_index(), Some(0));
    assert_eq!(
        serde_json::from_value::<RouteComparison>(serde_json::to_value(&record)?)?,
        record
    );
    let outcomes = (0..2)
        .map(|i| {
            Ok(RouteOutcome::Reverted {
                request: Box::new(request.candidate(i)?),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let record = RouteComparison::new(RouteComparisonData {
        request,
        anchor: anchor()?,
        outcomes,
    })?;
    assert_eq!(record.best_index(), None);
    assert!(record.best_quote().is_none());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn route_comparison_rejects_reordering_changed_candidates_and_source_state() -> TestResult {
    let request = route()?;
    let outcomes = vec![
        RouteOutcome::Quoted {
            quote: Box::new(quote(500, Quantity::from(9))?),
        },
        RouteOutcome::Quoted {
            quote: Box::new(quote(3000, Quantity::from(10))?),
        },
    ];
    let data = RouteComparisonData {
        request,
        anchor: anchor()?,
        outcomes,
    };
    assert_eq!(RouteComparison::new(data.clone())?.best_index(), Some(1));
    let mut reordered = data.clone();
    reordered.outcomes.reverse();
    assert!(RouteComparison::new(reordered).is_err());
    for variant in 0..4 {
        let mut wire = serde_json::to_value(&data)?;
        match variant {
            0 => wire["outcomes"][0]["quote"]["context"]["state"]["block"]["number"] = json!(43),
            1 => wire["outcomes"][0]["quote"]["context"]["source"]["provider_id"] = json!("other"),
            2 => {
                wire["anchor"]["value"]["address"] =
                    json!(Address::from_bytes([10; 20]).to_string());
            }
            _ => wire["outcomes"][0]["quote"]["value"]["request"]["amount_in"] = json!("1"),
        }
        assert!(serde_json::from_value::<RouteComparison>(wire).is_err());
    }
    let mut request = data.request.data().clone();
    request.paths = vec![path(500)?; 17];
    assert!(V3RouteRequest::new(request.clone()).is_err());
    request.paths = vec![path(500)?; 2];
    assert!(V3RouteRequest::new(request).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn slippage_uses_exact_floor_and_handles_entire_u256_width() -> TestResult {
    assert_eq!(
        SlippageBps::new(100)?.minimum_output(Quantity::from(10_001)),
        Quantity::from(9_900)
    );
    assert_eq!(
        SlippageBps::new(0)?.minimum_output(Quantity::new(U256::MAX)),
        Quantity::new(U256::MAX)
    );
    assert_eq!(
        SlippageBps::new(10_000)?.minimum_output(Quantity::new(U256::MAX)),
        Quantity::from(0)
    );
    assert_eq!(
        SlippageBps::new(5_000)?
            .minimum_output(Quantity::new(U256::MAX))
            .value(),
        U256::MAX / U256::from(2)
    );
    assert!(SlippageBps::new(10_001).is_err());
    assert!(serde_json::from_str::<SlippageBps>("10001").is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn router_2_1_2_six_field_command_binds_intent_recipient_output_deadline_and_allowances()
-> TestResult {
    let prepared = PreparedSwap::new(intent()?)?;
    prepared.validate()?;
    let unsigned = prepared.unsigned();
    let call = unsigned.call().data();
    let bytes = call.input.bytes();
    assert_eq!(&bytes[..4], &[0x35, 0x93, 0x56, 0x4c]);
    assert_eq!(bytes.len(), 580);
    assert_eq!(word(bytes, 4), U256::from(96));
    assert_eq!(word(bytes, 36), U256::from(160));
    assert_eq!(word(bytes, 68), U256::from(200));
    assert_eq!(word(bytes, 100), U256::from(1));
    assert!(bytes[132..164].iter().all(|b| *b == 0));
    assert_eq!(word(bytes, 164), U256::from(1));
    assert_eq!(word(bytes, 196), U256::from(32));
    assert_eq!(word(bytes, 228), U256::from(320));
    assert_eq!(&bytes[260..272], &[0; 12]);
    assert_eq!(&bytes[272..292], &[9; 20]);
    assert_eq!(word(bytes, 292), U256::from(1_000));
    assert_eq!(word(bytes, 324), U256::from(9_900));
    assert_eq!(word(bytes, 356), U256::from(192));
    assert_eq!(word(bytes, 388), U256::from(1));
    assert_eq!(word(bytes, 420), U256::from(288));
    assert_eq!(word(bytes, 452), U256::from(43));
    assert_eq!(&bytes[484..527], path(500)?.encoded()?.bytes());
    assert!(bytes[527..548].iter().all(|b| *b == 0));
    assert_eq!(word(bytes, 548), U256::ZERO);
    let d = prepared
        .intent()
        .data()
        .quote
        .value()
        .data()
        .request
        .data()
        .deployment
        .data();
    assert_eq!(call.to, Some(d.universal_router_2_1_2));
    assert_eq!(call.from, settings()?.data().from);
    assert_eq!(call.value, Quantity::from(0));
    assert_eq!(unsigned.allowances().erc20_spender, d.permit2);
    assert_eq!(
        unsigned.allowances().permit2_spender,
        d.universal_router_2_1_2
    );
    assert_eq!(unsigned.allowances().amount, Quantity::from(1_000));
    assert_eq!(
        serde_json::from_value::<PreparedSwap>(serde_json::to_value(&prepared)?)?,
        prepared
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn preparation_rejects_reserved_recipient_expired_quote_and_permit2_overflow() -> TestResult {
    let original = intent()?;
    for marker in [0, 1, 2] {
        let mut d = original.data().clone();
        let mut bytes = [0; 20];
        bytes[19] = marker;
        d.recipient = Address::from_bytes(bytes);
        assert!(SwapIntent::new(d.clone()).is_err());
        assert!(serde_json::from_value::<PreparedSwap>(serde_json::to_value(d)?).is_err());
    }
    let mut d = original.data().clone();
    d.deadline = Timestamp::from_unix_seconds(99);
    assert!(SwapIntent::new(d).is_err());
    let mut d = original.data().clone();
    d.deadline = Timestamp::from_unix_seconds(100);
    assert!(SwapIntent::new(d).is_ok());
    for allowed in [true, false] {
        let mut q = original.data().quote.value().data().clone();
        let mut r = q.request.data().clone();
        r.amount_in = Quantity::new(if allowed {
            (U256::from(1) << 160) - U256::from(1)
        } else {
            U256::from(1) << 160
        });
        q.request = V3QuoteRequest::new(r)?;
        let mut d = original.data().clone();
        d.quote = V3QuoteObservation::new(V3Quote::new(q)?, context(ReadOperation::Call)?)?;
        assert_eq!(SwapIntent::new(d).is_ok(), allowed);
    }
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn prepared_snapshot_handoff_is_immutable_and_redacts_payload_debug() -> TestResult {
    let first = PreparedSwap::new(intent()?)?;
    let request = PreparedRequest::new(first.clone())?;
    let handoff = HandoffRequest::new(HandoffId::new("caller-swap-1")?, request);
    let mut d = first.intent().data().clone();
    d.recipient = Address::from_bytes([11; 20]);
    let changed = PreparedSwap::new(SwapIntent::new(d)?)?;
    assert_ne!(first, changed);
    assert_ne!(first.unsigned(), changed.unsigned());
    for debug in [
        format!("{first:?}"),
        format!("{:?}", first.intent()),
        format!("{:?}", first.unsigned()),
        format!("{handoff:?}"),
    ] {
        assert!(!debug.contains("3593564c"));
        assert!(!debug.contains("caller-swap-1"));
    }
    let mut bad = serde_json::to_value(&first)?;
    bad["injected_payload"] = json!("SECRET");
    assert!(serde_json::from_value::<PreparedSwap>(bad).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn retained_live_observations_rebuild_exact_unsigned_intent_without_precision_defaults()
-> TestResult {
    let quote: V3QuoteObservation =
        serde_json::from_str(include_str!("fixtures/uniswap/quote_multihop.json"))?;
    let comparison: RouteComparison =
        serde_json::from_str(include_str!("fixtures/uniswap/routes.json"))?;
    let prepared: PreparedSwap =
        serde_json::from_str(include_str!("fixtures/uniswap/prepared_intent.json"))?;
    assert_eq!(quote.value().data().request.data().path.hops(), 2);
    assert_eq!(
        quote.value().data().amount_out,
        Quantity::from_decimal("2612327071403948745")?
    );
    assert_eq!(comparison.best_index(), Some(1));
    assert_eq!(
        comparison.best_quote().ok_or("missing best")?,
        &prepared.intent().data().quote
    );
    assert_eq!(
        prepared.intent().minimum_output(),
        Quantity::from_decimal("2586203800689909257")?
    );
    assert_eq!(prepared.unsigned().call().data().value, Quantity::from(0));
    assert_eq!(PreparedSwap::new(prepared.intent().clone())?, prepared);
    Ok(())
}
