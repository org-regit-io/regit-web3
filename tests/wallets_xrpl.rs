// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Actual XRPL Payment intent derives field-based preparation without signing.

#![cfg(feature = "xrpl")]

use regit_web3::{
    domain::xrpl::{
        Address, Destination, Drops, Network, NetworkId, PaymentAmount, PaymentRequest,
    },
    wallets::{HandoffId, HandoffRequest, Preparation, PreparedRequest, XrplPaymentPreparation},
};
use serde_json::{Value, json};

type TestError = Box<dyn std::error::Error>;

fn payment() -> Result<PaymentRequest, regit_web3::error::Error> {
    PaymentRequest::new(
        Network::new(NetworkId::new(1025), "fixture-sensitive-network")?,
        Address::parse("r9cZA1mLK5R5Am25ArfXFmqgNwjZgnfk59")?,
        Destination::new(
            NetworkId::new(1025),
            Address::parse("rGWrZyQqhTp9Xu7G5Pkayo7bXjH4k4QYpf")?,
            Some(0),
        ),
        PaymentAmount::Xrp(Drops::new(9_007_199_254_740_993)?),
        Drops::new(12)?,
        1,
        100,
    )
}

#[test]
fn xrpl_review_and_handoff_retain_validated_intent_and_exact_unsigned_fields()
-> Result<(), TestError> {
    let intent = payment()?;
    let adapter = XrplPaymentPreparation::new(intent.clone());
    let request = HandoffRequest::new(
        HandoffId::new("caller-generated")?,
        PreparedRequest::new(adapter)?,
    );
    let review = request.prepared().review();
    assert_eq!(review.network(), intent.network());
    assert_eq!(review.intent(), &intent);
    assert_eq!(review.unsigned_payload().request(), &intent);
    let fields: Value = serde_json::to_value(review.unsigned_payload())?;
    assert_eq!(fields["Amount"], "9007199254740993");
    assert_eq!(fields["Fee"], "12");
    assert_eq!(fields["NetworkID"], 1025);
    assert_eq!(fields["DestinationTag"], 0);
    assert!(fields.get("SigningPubKey").is_none());
    assert!(fields.get("TxnSignature").is_none());
    let restored: HandoffRequest<XrplPaymentPreparation> =
        serde_json::from_str(&serde_json::to_string(&request)?)?;
    assert_eq!(restored, request);
    assert_eq!(
        serde_json::to_value(restored.prepared().preparation().unsigned_payload())?,
        fields
    );
    assert!(!format!("{request:?}").contains("fixture-sensitive-network"));
    assert!(!format!("{:?}", request.prepared().preparation()).contains("r9cZA"));
    Ok(())
}

#[test]
fn xrpl_adapter_deserialization_rebuilds_payload_from_validated_intent() -> Result<(), TestError> {
    let intent = payment()?;
    let mut invalid = serde_json::to_value(&intent)?;
    invalid["fee"] = json!("0");
    assert!(serde_json::from_value::<XrplPaymentPreparation>(invalid).is_err());
    let mut supplied_payload = serde_json::to_value(&intent)?;
    supplied_payload["unsigned"] = json!({"Amount":"1"});
    assert!(serde_json::from_value::<XrplPaymentPreparation>(supplied_payload).is_err());
    let adapter = XrplPaymentPreparation::new(intent.clone());
    assert_eq!(
        serde_json::to_value(&adapter)?,
        serde_json::to_value(&intent)?
    );
    let restored =
        serde_json::from_value::<XrplPaymentPreparation>(serde_json::to_value(&adapter)?)?;
    assert_eq!(restored.intent(), &intent);
    assert_eq!(restored.unsigned_payload(), &intent.prepare());
    Ok(())
}
