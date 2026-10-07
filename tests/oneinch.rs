// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Runtime-independent Classic Swap v6.1 identity, graph and immutable review invariants.
#![cfg(feature = "oneinch")]
#[path = "fixtures/oneinch/support.rs"]
mod fixture;
use fixture::{
    DST, SRC, asset, context, graph, network, prepared, quote, request, settings, spender,
    swap_request, transaction,
};
use regit_web3::{
    domain::{
        Address, ChainId, ExactDecimal, NetworkId,
        evm::Data,
        oneinch::{
            Asset, AssetKind, Limits, Method, PreparedSwap, ProtocolId, Quote, QuoteRequest,
            ReturnPolicy, RouteGraph, SlippageBps, SourceText, SourceTransaction, SwapRequest,
        },
    },
    error::Error,
    wallets::{HandoffId, HandoffRequest, PreparedRequest},
};
use serde_json::json;
type TestError = Box<dyn std::error::Error>;
#[test]
fn native_identity_has_only_provider_sentinel_and_exact_units() -> Result<(), TestError> {
    let native = Asset::new(ChainId::from(1_u64), AssetKind::Native)?;
    assert_eq!(native.provider_address().bytes(), [0xee; 20]);
    for a in [
        Address::from_bytes([0; 20]),
        Address::from_bytes([0xee; 20]),
    ] {
        assert!(Asset::new(ChainId::from(1_u64), AssetKind::Erc20(a)).is_err());
    }
    assert!(Asset::new(ChainId::from(0_u64), AssetKind::Native).is_err());
    let r = request()?;
    assert_eq!(r.amount().value().to_string(), "9007199254740993");
    assert_eq!(
        serde_json::from_str::<QuoteRequest>(&serde_json::to_string(&r)?)?,
        r
    );
    let mut fields = serde_json::to_value(r)?;
    fields["source"]["chain_id"] = json!("2");
    assert!(serde_json::from_value::<QuoteRequest>(fields).is_err());
    Ok(())
}
#[test]
fn explicit_routing_filters_and_collections_are_validated() -> Result<(), TestError> {
    let r = request()?;
    let mut s = r.settings().clone();
    s.protocols = vec![
        ProtocolId::new("UNISWAP_V3")?,
        ProtocolId::new("UNISWAP_V3")?,
    ];
    assert!(QuoteRequest::new(network()?, r.source(), r.destination(), r.amount(), s).is_err());
    let mut s = r.settings().clone();
    s.protocols = vec![ProtocolId::new("UNISWAP_V3")?];
    s.excluded_protocols = s.protocols.clone();
    assert!(QuoteRequest::new(network()?, r.source(), r.destination(), r.amount(), s).is_err());
    let mut s = r.settings().clone();
    s.connector_tokens = vec![Asset::new(ChainId::from(2_u64), AssetKind::Native)?];
    assert!(QuoteRequest::new(network()?, r.source(), r.destination(), r.amount(), s).is_err());
    assert!(ProtocolId::new("comma,injection").is_err());
    assert!(Limits::new(0, 1).is_err());
    assert!(Limits::new(4097, 1).is_err());
    assert!(
        serde_json::from_value::<Limits>(json!({"route_items":1,"liquidity_sources":513})).is_err()
    );
    Ok(())
}
#[test]
fn graph_keeps_fractional_shares_and_nonindex_terminal_ids_without_invented_sums()
-> Result<(), TestError> {
    let g = graph()?;
    assert_eq!(g.item_count(), 3);
    assert_eq!(g.groups()[0].hops[0].to_token_id, 77);
    assert_eq!(
        g.groups()[0].hops[0].protocols[0].percent.canonical(),
        "33.000000000000000001"
    );
    assert_eq!(
        serde_json::from_str::<RouteGraph>(&serde_json::to_string(&g)?)?,
        g
    );
    assert!(g.check_limits(Limits::new(2, 1)?).is_err());
    Ok(())
}
#[test]
fn graph_constructor_and_serde_reject_conflicting_ids_chains_or_shares() -> Result<(), TestError> {
    for n in ["-1", "100.000000000000000001"] {
        let mut groups = graph()?.groups().to_vec();
        groups[0].hops[0].percent = ExactDecimal::parse(n)?;
        assert!(RouteGraph::new(network()?, groups).is_err());
    }
    let mut groups = graph()?.groups().to_vec();
    let mut second = groups[0].clone();
    second.token = asset(DST)?;
    groups.push(second);
    assert!(RouteGraph::new(network()?, groups).is_err());
    let mut fields = serde_json::to_value(graph()?)?;
    fields["groups"][0]["hops"][0]["percent"] = json!("101");
    assert!(serde_json::from_value::<RouteGraph>(fields).is_err());
    Ok(())
}
#[test]
fn quote_checks_source_asset_and_context_without_fake_expiry() -> Result<(), TestError> {
    let q = quote(Method::Quote, 1000)?;
    let mut data = q.data().clone();
    data.source_token.asset = asset(DST)?;
    assert!(Quote::new(request()?, data, context(Method::Quote)?).is_err());
    let mut c = context(Method::Quote)?;
    c.network = NetworkId::new(ChainId::from(1_u64), "different-qualification")?;
    assert!(Quote::new(request()?, q.data().clone(), c).is_err());
    assert!(Quote::new(request()?, q.data().clone(), context(Method::Spender)?).is_err());
    assert_eq!(
        serde_json::from_str::<Quote>(&serde_json::to_string(&q)?)?,
        q
    );
    Ok(())
}
#[test]
fn caller_return_policy_and_native_value_are_not_implicitly_invented() -> Result<(), TestError> {
    assert_eq!(SlippageBps::new(123)?.percent(), "1.23");
    assert_eq!(SlippageBps::new(5000)?.percent(), "50.00");
    assert!(SlippageBps::new(5001).is_err());
    let mut s = settings()?;
    s.return_policy = ReturnPolicy::Minimum(899_u64.into());
    assert!(SwapRequest::new(quote(Method::Quote, 1000)?, s).is_err());
    let mut s = settings()?;
    s.native_value = 1_u64.into();
    assert!(SwapRequest::new(quote(Method::Quote, 1000)?, s).is_err());
    let mut fields = serde_json::to_value(swap_request()?)?;
    fields["settings"]["output_floor"] = json!("0");
    assert!(serde_json::from_value::<SwapRequest>(fields).is_err());
    Ok(())
}
#[test]
fn fresh_review_correlates_payload_floor_and_separate_spender() -> Result<(), TestError> {
    let p = prepared()?;
    assert_eq!(
        p.request()
            .selection()
            .data()
            .destination_amount
            .value()
            .to_string(),
        "1000"
    );
    assert_eq!(
        p.fresh_quote()
            .data()
            .destination_amount
            .value()
            .to_string(),
        "950"
    );
    assert_eq!(
        p.transaction().data().gas_price.value().to_string(),
        "9007199254740993"
    );
    assert!(
        PreparedSwap::new(
            swap_request()?,
            quote(Method::Swap, 899)?,
            transaction()?,
            spender()?
        )
        .is_err()
    );
    for member in ["from", "to", "value"] {
        let mut fields = serde_json::to_value(&p)?;
        fields["transaction"]["data"][member] = if member == "value" {
            json!("1")
        } else {
            json!(SRC)
        };
        assert!(serde_json::from_value::<PreparedSwap>(fields).is_err());
    }
    let mut fields = serde_json::to_value(&p)?;
    fields["spender"]["address"] = json!(SRC);
    assert!(serde_json::from_value::<PreparedSwap>(fields).is_err());
    assert_eq!(
        serde_json::from_str::<PreparedSwap>(&serde_json::to_string(&p)?)?,
        p
    );
    Ok(())
}
#[test]
fn unsigned_payload_is_bounded_opaque_and_not_semantically_verified() -> Result<(), TestError> {
    let t = transaction()?;
    assert!(!format!("{t:?}").contains("deadbeef"));
    let mut d = t.data().clone();
    d.data = Data::new(vec![1; 65_537])?;
    assert!(SourceTransaction::new(network()?, d).is_err());
    let mut d = t.data().clone();
    d.data = Data::parse("0x123456")?;
    assert!(SourceTransaction::new(network()?, d).is_err());
    let mut d = t.data().clone();
    d.data = Data::parse("0x99999999")?;
    let t = SourceTransaction::new(network()?, d)?;
    assert!(PreparedSwap::new(swap_request()?, quote(Method::Swap, 950)?, t, spender()?).is_ok());
    Ok(())
}
#[test]
fn preparation_composes_with_immutable_generic_review_and_external_handoff() -> Result<(), TestError>
{
    let p = prepared()?;
    let prepared = PreparedRequest::new(p.clone())?;
    let review = prepared.review();
    assert_eq!(review.intent(), p.request());
    assert_eq!(review.unsigned_payload(), p.transaction());
    assert_eq!(review.network(), p.request().network());
    let handoff = HandoffRequest::new(HandoffId::new("caller-lifecycle-id")?, prepared);
    assert!(!format!("{handoff:?}").contains("deadbeef"));
    assert_eq!(
        serde_json::from_str::<HandoffRequest<PreparedSwap>>(&serde_json::to_string(&handoff)?)?,
        handoff
    );
    Ok(())
}
#[test]
fn source_text_preserves_display_controls_and_byte_bounds() -> Result<(), TestError> {
    let t = SourceText::new("symbol\u{8}é")?;
    assert_eq!(
        serde_json::from_str::<SourceText>(&serde_json::to_string(&t)?)?,
        t
    );
    assert!(SourceText::new("é".repeat(2049)).is_err());
    assert!(!format!("{t:?}").contains('\u{8}'));
    let mut q = quote(Method::Quote, 1000)?.data().clone();
    q.source_token.fee_on_transfer = Some(true);
    let q = Quote::new(request()?, q, context(Method::Quote)?)?;
    assert_eq!(
        SwapRequest::new(q, settings()?),
        Err(Error::UnsupportedCapability)
    );
    Ok(())
}

#[test]
fn reported_liquidity_ids_must_match_exact_caller_filters_in_constructor_and_serde()
-> Result<(), TestError> {
    let source = quote(Method::Quote, 1000)?;
    for (allowed, excluded) in [
        (vec![], vec!["UNISWAP_V3"]),
        (vec!["OTHER"], vec![]),
        (vec!["uniswap_v3"], vec![]),
    ] {
        let mut routing = source.request().settings().clone();
        routing.protocols = allowed
            .iter()
            .map(|id| ProtocolId::new(*id))
            .collect::<Result<Vec<_>, Error>>()?;
        routing.excluded_protocols = excluded
            .iter()
            .map(|id| ProtocolId::new(*id))
            .collect::<Result<Vec<_>, Error>>()?;
        let request = QuoteRequest::new(
            network()?,
            source.request().source(),
            source.request().destination(),
            source.request().amount(),
            routing,
        )?;
        assert!(
            Quote::new(
                request.clone(),
                source.data().clone(),
                context(Method::Quote)?
            )
            .is_err()
        );
        let mut forged = serde_json::to_value(&source)?;
        forged["request"] = serde_json::to_value(request)?;
        assert!(serde_json::from_value::<Quote>(forged).is_err());
    }
    let mut routing = source.request().settings().clone();
    routing.protocols = vec![ProtocolId::new("UNISWAP_V3")?];
    routing.excluded_protocols = vec![ProtocolId::new("OTHER")?];
    let request = QuoteRequest::new(
        network()?,
        source.request().source(),
        source.request().destination(),
        source.request().amount(),
        routing,
    )?;
    assert!(Quote::new(request, source.data().clone(), context(Method::Quote)?).is_ok());
    Ok(())
}
#[test]
fn empty_source_graph_is_retained_without_filter_completeness_or_execution_claim()
-> Result<(), TestError> {
    let source = quote(Method::Quote, 1000)?;
    let mut routing = source.request().settings().clone();
    routing.protocols = vec![ProtocolId::new("OTHER")?];
    let request = QuoteRequest::new(
        network()?,
        source.request().source(),
        source.request().destination(),
        source.request().amount(),
        routing,
    )?;
    let mut data = source.data().clone();
    data.routes = RouteGraph::new(network()?, vec![])?;
    let source = Quote::new(request, data, context(Method::Quote)?)?;
    assert!(source.data().routes.groups().is_empty());
    assert_eq!(source.data().routes.item_count(), 0);
    assert_eq!(
        serde_json::from_str::<Quote>(&serde_json::to_string(&source)?)?,
        source
    );
    Ok(())
}
