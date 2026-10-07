// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure exact Rubic source records and immutable unsigned review boundaries.
#![cfg(test)]
#![cfg(feature = "rubic")]
use regit_web3::{
    domain::{
        Address, Amount, ChainId,
        rubic::{
            Account, Asset, AssetIdentifier, Catalogue, Chain, Family, FreshQuote, Identifier,
            Limits, Payload, PreparationRequest, PreparedSwap, ProviderStatus, Quote, QuoteRequest,
            Routes, SlippageBps, Status, StatusQuery, Text, TransactionId,
        },
        solana::{Hash, Network},
    },
    error::Error,
    wallets::{
        HandoffId, HandoffRequest, HandoffResponse, Preparation, PreparedRequest,
        SignedPayloadVerifier, VerificationDecision, verify_handoff,
    },
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const QUOTE: &str = include_str!("fixtures/rubic/quote_domain.json");
const PREPARED: &str = include_str!("fixtures/rubic/prepared_domain.json");
fn quote() -> Result<Quote, serde_json::Error> {
    serde_json::from_str(QUOTE)
}
fn prepared() -> Result<PreparedSwap, serde_json::Error> {
    serde_json::from_str(PREPARED)
}
fn roundtrip<T: Serialize + DeserializeOwned + Eq + std::fmt::Debug>(v: &T) -> TestResult {
    assert_eq!(&serde_json::from_str::<T>(&serde_json::to_string(v)?)?, v);
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn exact_gross_net_and_route_amounts_remain_separate() -> TestResult {
    let q = quote()?;
    assert_eq!(
        q.data().request.data().amount.raw().to_string(),
        "10000000000000000"
    );
    assert_eq!(
        q.data().input.amount().raw().to_string(),
        "9960000000000000"
    );
    assert_eq!(
        q.data().legs[0].path()[0].amount().raw().to_string(),
        "9958008000000000"
    );
    assert_eq!(
        q.data().estimate.data().output.raw().to_string(),
        "251507041920000000000"
    );
    assert_eq!(
        q.data().estimate.data().minimum_output.raw().to_string(),
        "248991971500800000000"
    );
    assert_eq!(q.data().fees.percent.value().canonical(), "0.4");
    roundtrip(&q)
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn fresh_preparation_retains_both_source_snapshots_and_caller_floor() -> TestResult {
    let p = prepared()?;
    assert_eq!(
        p.data()
            .fresh_quote
            .data()
            .input
            .as_ref()
            .ok_or("missing input")?
            .amount()
            .raw()
            .to_string(),
        "10000000000000000"
    );
    assert_ne!(
        p.data().request.data().quote.data().estimate,
        p.data().fresh_quote.data().estimate
    );
    assert_eq!(
        p.data().request.data().minimum_output.raw().to_string(),
        "1"
    );
    assert_eq!(p.data().source_foreign_filters.len(), 2);
    assert!(p.data().source_provider_ids.is_empty());
    assert!(matches!(p.data().payload, Payload::Evm(_)));
    let reviewed = PreparedRequest::new(p.clone())?;
    assert_eq!(reviewed.review().network(), p.network());
    assert_eq!(reviewed.review().intent(), p.data());
    roundtrip(&p)
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn identity_family_numbers_are_not_inferred_from_provider_catalogue() -> TestResult {
    let sol = Chain::new(
        Identifier::new("SOLANA")?,
        Some(7_565_164),
        Family::Solana {
            network: Network::new(Hash::from_bytes([1; 32]), "explicit")?,
        },
        false,
    )?;
    assert!(
        Asset::new(
            sol.clone(),
            AssetIdentifier::EvmToken(Address::from_bytes([1; 20])),
            18
        )
        .is_err()
    );
    assert!(
        Account::Evm(Address::from_bytes([1; 20]))
            .validate(&sol)
            .is_err()
    );
    let doge = Chain::new(
        Identifier::new("DOGECOIN")?,
        None,
        Family::Provider {
            kind: Identifier::new("DOGECOIN")?,
        },
        false,
    )?;
    assert_eq!(doge.provider_id(), None);
    assert!(Asset::new(doge, AssetIdentifier::Native, 8).is_ok());
    assert!(
        Chain::new(
            Identifier::new("ETH")?,
            Some(137),
            Family::Evm {
                chain_id: ChainId::from(1)
            },
            false
        )
        .is_err()
    );
    assert!(
        Chain::new(
            Identifier::new("ETH")?,
            Some(1),
            Family::Provider {
                kind: Identifier::new("EVM")?
            },
            false
        )
        .is_err()
    );
    roundtrip(&sol)
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn catalogue_and_capacity_construction_matches_deserialization() -> TestResult {
    let chain = quote()?.data().request.data().source.chain().clone();
    assert!(Catalogue::new(vec![chain.clone(), chain.clone()]).is_err());
    assert!(serde_json::from_value::<Catalogue>(json!([chain, chain])).is_err());
    for values in [
        [0, 1, 1, 1, 1],
        [513, 1, 1, 1, 1],
        [1, 129, 1, 1, 1],
        [1, 1, 0, 1, 1],
    ] {
        assert!(Limits::new(values[0], values[1], values[2], values[3], values[4]).is_err());
    }
    let v = Limits::new(512, 128, 128, 128, 128)?;
    roundtrip(&v)
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn request_precision_filters_and_slippage_have_no_defaults() -> TestResult {
    let q = quote()?;
    for bps in [0, 1, 100, 2500, 5000] {
        roundtrip(&SlippageBps::new(bps)?)?;
    }
    assert_eq!(SlippageBps::new(100)?.fraction(), "0.01");
    assert!(SlippageBps::new(5001).is_err());
    let mut data = q.data().request.data().clone();
    data.amount = Amount::from_decimal("100", None)?;
    assert!(QuoteRequest::new(data.clone()).is_err());
    assert!(serde_json::from_value::<QuoteRequest>(serde_json::to_value(data)?).is_err());
    let mut data = q.data().request.data().clone();
    data.excluded_providers = vec![Identifier::new("lifi")?, Identifier::new("lifi")?];
    assert!(QuoteRequest::new(data).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn quote_identity_precision_slippage_and_nested_limits_fail_closed() -> TestResult {
    for change in ["asset", "precision", "slippage", "provider", "capacity"] {
        let mut data = quote()?.data().clone();
        match change {
            "asset" => data.destination.asset = data.input.token().asset.clone(),
            "precision" => {
                let mut e = data.estimate.data().clone();
                e.output = Amount::new(e.output.raw(), Some(6));
                e.minimum_output = Amount::new(e.minimum_output.raw(), Some(6));
                data.estimate = regit_web3::domain::rubic::Estimate::new(e)?;
            }
            "slippage" => {
                let mut r = data.request.data().clone();
                r.slippage = SlippageBps::new(200)?;
                data.request = QuoteRequest::new(r)?;
            }
            "provider" => {
                let mut r = data.request.data().clone();
                r.excluded_providers = vec![Identifier::new("lifi")?];
                data.request = QuoteRequest::new(r)?;
            }
            _ => data.legs.clear(),
        }
        assert!(Quote::new(data.clone()).is_err(), "{change}");
        assert!(
            serde_json::from_value::<Quote>(serde_json::to_value(data)?).is_err(),
            "{change}"
        );
    }
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn duplicate_route_ids_and_request_mismatch_fail_in_constructor_and_serde() -> TestResult {
    let q = quote()?;
    let request = q.data().request.clone();
    let raw = json!({"request":request,"routes":[q,q]});
    assert!(Routes::new(request, vec![q.clone(), q]).is_err());
    assert!(serde_json::from_value::<Routes>(raw).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn changed_router_value_signers_route_identity_and_floor_do_not_qualify() -> TestResult {
    for change in ["router", "value", "id", "provider", "floor"] {
        let mut data = prepared()?.data().clone();
        match change {
            "router" => {
                if let Payload::Evm(p) = &mut data.payload {
                    p.to = Address::from_bytes([9; 20]);
                }
            }
            "value" => {
                if let Payload::Evm(p) = &mut data.payload {
                    p.value = regit_web3::domain::evm::Quantity::from_decimal("20000000000000001")?;
                }
            }
            "id" => {
                let mut q = data.fresh_quote.data().clone();
                q.id = Identifier::new("another-id")?;
                data.fresh_quote = FreshQuote::new(q)?;
            }
            "provider" => {
                let mut q = data.fresh_quote.data().clone();
                q.provider = Identifier::new("another-provider")?;
                data.fresh_quote = FreshQuote::new(q)?;
            }
            _ => {
                let mut r = data.request.data().clone();
                r.minimum_output = Amount::from_decimal("251503987850000000001", Some(18))?;
                data.request = PreparationRequest::new(r)?;
            }
        }
        assert!(PreparedSwap::new(data.clone()).is_err(), "{change}");
        assert!(
            serde_json::from_value::<PreparedSwap>(serde_json::to_value(data)?).is_err(),
            "{change}"
        );
    }
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn statuses_do_not_invent_execution_hashes_precision_or_finality() -> TestResult {
    let q = quote()?;
    let source = q.data().request.data().source.chain().clone();
    let dest = q.data().request.data().destination.chain().clone();
    let query = StatusQuery::new(
        Identifier::new("external-id")?,
        source,
        TransactionId::Evm(regit_web3::domain::evm::TransactionId::from_bytes([1; 32])),
        dest,
    )?;
    for state in [
        ProviderStatus::NotFound,
        ProviderStatus::Pending,
        ProviderStatus::Success,
        ProviderStatus::ReadyToClaim,
        ProviderStatus::Reverted,
    ] {
        let s = Status::new(query.clone(), state, None, None, None, None)?;
        assert_eq!(s.status(), state);
        assert!(s.destination().is_none());
        roundtrip(&s)?;
    }
    assert!(
        Status::new(
            query,
            ProviderStatus::Success,
            None,
            None,
            Some(Amount::from_decimal("10", Some(18))?),
            None
        )
        .is_err()
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn untrusted_text_and_opaque_payload_debug_are_redacted() -> TestResult {
    let p = prepared()?;
    assert!(!format!("{p:?}").contains("e1fcde8e"));
    assert!(!format!("{:?}", p.data()).contains("e1fcde8e"));
    assert!(!format!("{:?}", Identifier::new("SECRET")?).contains("SECRET"));
    assert!(!format!("{:?}", Text::new("SECRET")?).contains("SECRET"));
    for secret in ["", "SECRET\nvalue"] {
        assert!(Identifier::new(secret).is_err());
        assert!(Text::new(secret).is_err());
    }
    let mut v: Value = serde_json::to_value(p)?;
    v["payload"]["extra"] = json!("SECRET");
    assert!(serde_json::from_value::<PreparedSwap>(v).is_err());
    Ok(())
}
struct BindingVerifier {
    calls: AtomicUsize,
}
impl SignedPayloadVerifier<PreparedSwap, String> for BindingVerifier {
    fn verify_binding(
        &self,
        _: &PreparedRequest<PreparedSwap>,
        signed: &String,
    ) -> impl std::future::Future<Output = Result<VerificationDecision, Error>>
    + regit_web3::future::MaybeSend {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::future::ready(Ok(if signed == "verified-test-body" {
            VerificationDecision::Confirmed
        } else {
            VerificationDecision::Rejected
        }))
    }
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), tokio::test)]
async fn handoff_correlation_precedes_trusted_signed_content_verification() -> TestResult {
    let verifier = BindingVerifier {
        calls: AtomicUsize::new(0),
    };
    let prepared = PreparedRequest::new(prepared()?)?;
    let request = HandoffRequest::new(HandoffId::new("caller-id")?, prepared.clone());
    let mismatch = HandoffResponse::new(
        HandoffId::new("other-id")?,
        prepared.clone(),
        "verified-test-body".into(),
    );
    assert!(verify_handoff(&request, mismatch, &verifier).await.is_err());
    assert_eq!(verifier.calls.load(Ordering::SeqCst), 0);
    let bad = HandoffResponse::new(
        HandoffId::new("caller-id")?,
        prepared.clone(),
        "different-body".into(),
    );
    assert!(verify_handoff(&request, bad, &verifier).await.is_err());
    assert_eq!(verifier.calls.load(Ordering::SeqCst), 1);
    let good = HandoffResponse::new(
        HandoffId::new("caller-id")?,
        prepared,
        "verified-test-body".into(),
    );
    let qualified = verify_handoff(&request, good, &verifier).await?;
    assert_eq!(qualified.signed(), "verified-test-body");
    assert_eq!(verifier.calls.load(Ordering::SeqCst), 2);
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn fresh_metadata_absence_is_preserved_and_present_mismatches_are_rejected() -> TestResult {
    let p = prepared()?;
    let mut facts = p.data().fresh_quote.data().clone();
    facts.input = None;
    facts.destination = None;
    let mut data = p.data().clone();
    data.fresh_quote = FreshQuote::new(facts.clone())?;
    let p = PreparedSwap::new(data)?;
    assert!(p.data().fresh_quote.data().input.is_none());
    assert!(p.data().fresh_quote.data().destination.is_none());
    assert!(p.data().request.data().quote.data().input.amount().raw() > 0);
    roundtrip(&p)?;
    facts.destination = Some(p.data().request.data().quote.data().input.token().clone());
    assert!(FreshQuote::new(facts.clone()).is_err());
    assert!(serde_json::from_value::<FreshQuote>(serde_json::to_value(facts)?).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn native_solana_provider_sentinel_is_not_a_mint_or_wrapped_sol() -> TestResult {
    use regit_web3::domain::{rubic::SOLANA_NATIVE_ASSET_ADDRESS, solana::Pubkey};
    let chain = Chain::new(
        Identifier::new("SOLANA")?,
        Some(7_565_164),
        Family::Solana {
            network: Network::new(Hash::from_bytes([1; 32]), "explicit")?,
        },
        false,
    )?;
    let native = Asset::new(chain.clone(), AssetIdentifier::Native, 9)?;
    assert_eq!(native.source_address(), SOLANA_NATIVE_ASSET_ADDRESS);
    assert!(
        Asset::new(
            chain.clone(),
            AssetIdentifier::SolanaMint(Pubkey::parse(SOLANA_NATIVE_ASSET_ADDRESS)?),
            9
        )
        .is_err()
    );
    let invalid = json!({"chain":chain,"identifier":{"kind":"solana_mint","value":SOLANA_NATIVE_ASSET_ADDRESS},"decimals":9});
    assert!(serde_json::from_value::<Asset>(invalid).is_err());
    let wrapped = Asset::new(
        chain,
        AssetIdentifier::SolanaMint(Pubkey::parse(
            "So11111111111111111111111111111111111111112",
        )?),
        9,
    )?;
    assert_ne!(native, wrapped);
    assert_eq!(
        wrapped.source_address(),
        "So11111111111111111111111111111111111111112"
    );
    roundtrip(&native)?;
    roundtrip(&wrapped)
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn additional_source_data_has_exact_lexical_numbers_and_bounded_validated_serde() -> TestResult {
    use regit_web3::domain::rubic::SourceAdditionalData;
    let raw =
        "{\"provider\":\"SOURCE-SECRET\",\"number\":9007199254740993.12500,\"nested\":[null,true]}";
    let value = SourceAdditionalData::new(raw)?;
    assert_eq!(value.as_json(), raw);
    assert!(!format!("{value:?}").contains("SECRET"));
    roundtrip(&value)?;
    let deep = format!("{{\"a\":{}0{}}}", "[".repeat(33), "]".repeat(33));
    let nodes = json!({"a":vec![vec![0;3000];3]}).to_string();
    let collection = json!({"a":vec![0;4097]}).to_string();
    let text = json!({"a":"x".repeat(65_537)}).to_string();
    let key = json!({"x".repeat(4097):0}).to_string();
    for bad in [
        "[]".to_owned(),
        "null".to_owned(),
        "{\"a\":1,\"\\u0061\":2}".to_owned(),
        deep,
        nodes,
        collection,
        text,
        key,
    ] {
        assert!(SourceAdditionalData::new(&bad).is_err());
        assert!(serde_json::from_value::<SourceAdditionalData>(json!(bad)).is_err());
    }
    Ok(())
}
