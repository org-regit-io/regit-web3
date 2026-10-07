// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact Cardano indexed records, original CBOR and explicit payment review.

#![cfg(feature = "cardano")]

#[path = "support/cardano.rs"]
mod support;
use pallas_codec::minicbor;
use regit_web3::{
    domain::{
        U256,
        cardano::{
            AddressBalance, AddressData, AddressDetails, AssetEntry, AssetHolder, DrepId, Hash,
            IndexPage, IndexedAddressKind, Lovelace, MetadataAvailability, Order, PageRequest,
            PageTarget, PaymentEstimate, PaymentIntent, PaymentOutput, PaymentPreparation, PoolId,
            ProtocolParameters, SignedSubmission, SourceText, StakeAddress, TransactionCbor,
            TransactionStatus, asset_fingerprint,
        },
    },
    error::{Error, ValidationError},
    wallets::{Preparation, PreparedRequest},
};
use serde_json::json;

fn assert_send<T: Send>(_: T) {}

// Independently splice the transmitted components into the ledger's three-field
// encoding. In particular, do not re-encode their original CBOR through Pallas.
fn principal_components(encoded: &[u8]) -> Result<[Vec<u8>; 3], Error> {
    let mut decoder = minicbor::Decoder::new(encoded);
    if decoder.array().map_err(|_| Error::Configuration)? != Some(4) {
        return Err(Error::Configuration);
    }
    let body_start = decoder.position();
    decoder.skip().map_err(|_| Error::Configuration)?;
    let witness_start = decoder.position();
    decoder.skip().map_err(|_| Error::Configuration)?;
    let witness_end = decoder.position();
    if !decoder.bool().map_err(|_| Error::Configuration)? {
        return Err(Error::Configuration);
    }
    let auxiliary_start = decoder.position();
    decoder.skip().map_err(|_| Error::Configuration)?;
    if decoder.position() != encoded.len() {
        return Err(Error::Configuration);
    }
    Ok([
        encoded[body_start..witness_start].to_vec(),
        encoded[witness_start..witness_end].to_vec(),
        encoded[auxiliary_start..].to_vec(),
    ])
}

fn ledger_encoding(encoded: &[u8]) -> Result<Vec<u8>, Error> {
    let mut ledger = vec![0x83];
    for component in principal_components(encoded)? {
        ledger.extend(component);
    }
    Ok(ledger)
}

fn parameters_for_size(size: u32) -> Result<ProtocolParameters, Error> {
    let mut parameters = support::parameters()?.data().clone();
    parameters.max_transaction_bytes = size;
    ProtocolParameters::new(support::network()?, parameters)
}

fn alternate_witness_framing(encoded: &[u8], indefinite: bool) -> Result<Vec<u8>, Error> {
    let [body, witness, auxiliary] = principal_components(encoded)?;
    if witness.first() != Some(&0xa1) {
        return Err(Error::Configuration);
    }
    let mut alternate = vec![0x84];
    alternate.extend(body);
    if indefinite {
        alternate.push(0xbf);
        alternate.extend(&witness[1..]);
        alternate.push(0xff);
    } else {
        alternate.extend([0xb8, 1]);
        alternate.extend(&witness[1..]);
    }
    alternate.push(0xf5);
    alternate.extend(auxiliary);
    Ok(alternate)
}

#[test]
fn preparation_encodes_exact_selected_intent_and_review_without_witnesses() -> Result<(), Error> {
    let prep = support::preparation()?;
    prep.validate()?;
    let tx: pallas_primitives::conway::Tx<'_> =
        minicbor::decode(prep.unsigned_payload().bytes()).unwrap();
    assert_eq!(tx.transaction_body.fee, 500_000);
    assert_eq!(tx.transaction_body.ttl, Some(200_000_000));
    assert_eq!(
        tx.transaction_body.validity_interval_start,
        Some(199_000_000)
    );
    assert!(tx.transaction_witness_set.vkeywitness.is_none());
    assert_eq!(tx.transaction_body.inputs.len(), 1);
    assert_eq!(tx.transaction_body.outputs.len(), 1);
    assert_eq!(prep.intent(), &support::intent(500_000)?);
    assert_eq!(
        prep.transaction_id(),
        prep.unsigned_payload().transaction_id()
    );
    assert!(prep.estimate().minimum_fee() < 500_000);
    assert_eq!(
        prep.estimate().minimum_fee(),
        155_381 + 44 * u64::from(prep.estimate().ledger_size_bytes())
    );
    assert!(prep.estimate().output_minimum_lovelaces()[0] > 1_000_000);
    let reviewed = PreparedRequest::new(prep.clone())?;
    assert_eq!(reviewed.preparation(), &prep);
    assert_eq!(
        serde_json::from_value::<PaymentPreparation>(serde_json::to_value(&prep).unwrap()).unwrap(),
        prep
    );
    Ok(())
}

#[test]
fn ledger_fee_and_maximum_size_use_three_fields_with_exact_boundaries() -> Result<(), Error> {
    let initial = support::preparation()?;
    let transmitted = support::signed_bytes(&initial, 1)?;
    let ledger = ledger_encoding(&transmitted)?;
    assert_eq!(ledger[0], 0x83);
    assert_eq!(transmitted[0], 0x84);
    assert_eq!(transmitted.len(), ledger.len() + 1);
    let ledger_size = u32::try_from(ledger.len()).map_err(|_| Error::Configuration)?;
    let minimum_fee = 155_381 + 44 * u64::from(ledger_size);
    assert_eq!(initial.estimate().ledger_size_bytes(), ledger_size);
    assert_eq!(initial.estimate().minimum_fee(), minimum_fee);
    assert_eq!(155_381 + 44 * transmitted.len() as u64, minimum_fee + 44);

    // These fee/output values retain the same widths as the original vector.
    let preparation = PaymentPreparation::new(
        support::intent(minimum_fee)?,
        parameters_for_size(ledger_size)?,
    )?;
    let bytes = support::signed_bytes(&preparation, 1)?;
    assert_eq!(ledger_encoding(&bytes)?.len(), ledger.len());
    assert_eq!(preparation.estimate().minimum_fee(), minimum_fee);
    let signed = SignedSubmission::new(preparation, TransactionCbor::from_bytes(bytes)?)?;
    assert_eq!(signed.transaction().body_fee(), Some(minimum_fee));
    // Full transmitted size is deliberately above the ledger maximum by one.
    assert_eq!(signed.transaction().bytes().len(), ledger.len() + 1);
    assert!(
        PaymentPreparation::new(
            support::intent(minimum_fee - 1)?,
            parameters_for_size(ledger_size)?
        )
        .is_err()
    );
    assert!(
        PaymentPreparation::new(
            support::intent(minimum_fee)?,
            parameters_for_size(ledger_size - 1)?
        )
        .is_err()
    );
    let estimate = signed.preparation().estimate();
    for field in ["signed_size_bytes", "ledger_size_bytes"] {
        let mut forged = serde_json::to_value(estimate).unwrap();
        forged[field] = json!(forged[field].as_u64().unwrap() + 1);
        assert!(serde_json::from_value::<PaymentEstimate>(forged).is_err());
    }
    Ok(())
}

#[test]
fn signed_ledger_size_retains_original_nonminimal_and_indefinite_witness_framing()
-> Result<(), Error> {
    let initial = support::preparation()?;
    let bytes = support::signed_bytes(&initial, 1)?;
    let canonical_size =
        u32::try_from(ledger_encoding(&bytes)?.len()).map_err(|_| Error::Configuration)?;
    let actual_size = canonical_size + 1;
    let actual_fee = 155_381 + 44 * u64::from(actual_size);
    for indefinite in [false, true] {
        let preparation = PaymentPreparation::new(
            support::intent(actual_fee)?,
            parameters_for_size(actual_size)?,
        )?;
        let original = support::signed_bytes(&preparation, 1)?;
        let alternate = alternate_witness_framing(&original, indefinite)?;
        assert_eq!(alternate.len(), original.len() + 1);
        assert_eq!(ledger_encoding(&alternate)?.len(), actual_size as usize);
        let transaction = TransactionCbor::from_bytes(alternate.clone())?;
        assert_eq!(transaction.bytes(), &alternate);
        assert_eq!(
            transaction.body_bytes(),
            preparation.unsigned_payload().body_bytes()
        );
        assert_eq!(transaction.transaction_id(), preparation.transaction_id());
        let signed = SignedSubmission::new(preparation, transaction)?;
        assert_eq!(
            serde_json::from_value::<SignedSubmission>(serde_json::to_value(&signed).unwrap())
                .unwrap(),
            signed
        );

        let too_small = PaymentPreparation::new(
            support::intent(actual_fee)?,
            parameters_for_size(canonical_size)?,
        )?;
        let bytes = alternate_witness_framing(&support::signed_bytes(&too_small, 1)?, indefinite)?;
        assert!(SignedSubmission::new(too_small, TransactionCbor::from_bytes(bytes)?).is_err());

        // Placeholder witnesses fit and the fee exceeds the placeholder minimum,
        // but the exact original witness encoding needs one extra byte's fee.
        let underpaid = PaymentPreparation::new(
            support::intent(actual_fee - 1)?,
            parameters_for_size(actual_size)?,
        )?;
        let bytes = alternate_witness_framing(&support::signed_bytes(&underpaid, 1)?, indefinite)?;
        assert!(SignedSubmission::new(underpaid, TransactionCbor::from_bytes(bytes)?).is_err());
    }

    // The ledger's outer wrapper is canonical even if the transmitted wrapper
    // itself uses a nonminimal length encoding; principal components stay exact.
    let preparation = PaymentPreparation::new(
        support::intent(actual_fee - 44)?,
        parameters_for_size(canonical_size)?,
    )?;
    let original = support::signed_bytes(&preparation, 1)?;
    let mut alternate = vec![0x98, 4];
    alternate.extend(&original[1..]);
    assert_eq!(alternate.len(), original.len() + 1);
    assert_eq!(ledger_encoding(&alternate)?.len(), canonical_size as usize);
    SignedSubmission::new(preparation, TransactionCbor::from_bytes(alternate)?)?;
    Ok(())
}

#[test]
fn original_transmitted_cbor_has_an_independent_64_kib_resource_bound() -> Result<(), Error> {
    let mut encoder = minicbor::Encoder::new(Vec::new());
    encoder
        .array(4)
        .and_then(|e| e.map(3))
        .and_then(|e| e.u8(0))
        .and_then(|e| e.array(0))
        .and_then(|e| e.u8(1))
        .and_then(|e| e.array(0))
        .and_then(|e| e.u8(2))
        .and_then(|e| e.u8(0))
        .and_then(|e| e.map(1))
        .and_then(|e| e.u8(0))
        .and_then(|e| e.bytes(&vec![0; TransactionCbor::MAX_BYTES - 15]))
        .and_then(|e| e.bool(true))
        .and_then(|e| e.null())
        .map_err(|_| Error::Configuration)?;
    let bytes = encoder.into_writer();
    assert_eq!(bytes.len(), TransactionCbor::MAX_BYTES);
    assert_eq!(
        ledger_encoding(&bytes)?.len(),
        TransactionCbor::MAX_BYTES - 1
    );
    TransactionCbor::from_bytes(bytes.clone())?;
    let mut too_large = bytes;
    // Expand only the outer wrapper; the component facts themselves stay equal.
    too_large.splice(0..1, [0x98, 4]);
    assert_eq!(too_large.len(), TransactionCbor::MAX_BYTES + 1);
    assert!(TransactionCbor::from_bytes(too_large).is_err());
    Ok(())
}

#[test]
fn estimate_does_not_mutate_fee_or_change_and_rejects_forged_serialized_totals() -> Result<(), Error>
{
    let estimate = PaymentEstimate::new(support::intent(1)?, support::parameters()?)?;
    assert!(estimate.minimum_fee() > estimate.intent().fee());
    assert_eq!(estimate.intent().fee(), 1);
    assert!(PaymentPreparation::new(estimate.intent().clone(), support::parameters()?).is_err());
    let mut forged = serde_json::to_value(&estimate).unwrap();
    forged["minimum_fee"] = json!(0);
    assert!(serde_json::from_value::<PaymentEstimate>(forged).is_err());
    let mut forged = serde_json::to_value(support::preparation()?).unwrap();
    forged["unsigned_transaction"] = json!("82a0a0");
    assert!(serde_json::from_value::<PaymentPreparation>(forged).is_err());
    Ok(())
}

#[test]
fn conservation_intervals_duplicates_and_witness_policy_have_constructor_serde_parity()
-> Result<(), Error> {
    let intent = support::intent(500_000)?;
    for (field, value) in [
        ("fee", json!(500_001)),
        ("invalid_hereafter", json!(199_000_000)),
        ("witness_count", json!(0)),
        ("inputs", json!([intent.inputs()[0], intent.inputs()[0]])),
    ] {
        let mut forged = serde_json::to_value(&intent).unwrap();
        forged[field] = value;
        assert!(
            serde_json::from_value::<PaymentIntent>(forged).is_err(),
            "{field}"
        );
    }
    assert!(
        PaymentIntent::new(
            support::network()?,
            intent.inputs().to_vec(),
            intent.outputs().to_vec(),
            500_001,
            Some(199_000_000),
            200_000_000,
            1
        )
        .is_err()
    );
    let repeated = vec![intent.inputs()[0].clone(); 101];
    assert!(
        PaymentIntent::new(
            support::network()?,
            repeated,
            intent.outputs().to_vec(),
            500_000,
            None,
            200_000_000,
            1
        )
        .is_err()
    );
    assert!(
        PaymentOutput::new(
            support::address(2)?,
            vec![support::assets(1, 7)?[1].clone()]
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn parameter_profile_minimum_ada_value_size_transaction_size_and_arithmetic_are_checked()
-> Result<(), Error> {
    for change in [
        "max_value_bytes",
        "max_transaction_bytes",
        "coins_per_utxo_size",
        "protocol_major",
        "min_fee_coefficient",
    ] {
        let mut p = support::parameters()?.data().clone();
        match change {
            "max_value_bytes" => p.max_value_bytes = Some(1),
            "max_transaction_bytes" => p.max_transaction_bytes = 1,
            "coins_per_utxo_size" => p.coins_per_utxo_size = Some(u64::MAX),
            "protocol_major" => p.protocol_major = 11,
            _ => p.min_fee_coefficient = u64::MAX,
        }
        assert!(
            PaymentEstimate::new(
                support::intent(500_000)?,
                ProtocolParameters::new(support::network()?, p)?
            )
            .is_err(),
            "{change}"
        );
    }
    let mut p = support::parameters()?.data().clone();
    p.coins_per_utxo_size = None;
    assert_eq!(
        PaymentEstimate::new(
            support::intent(500_000)?,
            ProtocolParameters::new(support::network()?, p)?
        )
        .unwrap_err(),
        Error::UnsupportedCapability
    );
    Ok(())
}

#[test]
fn signed_submission_checks_actual_body_keys_and_size_without_claiming_signature_verification()
-> Result<(), Error> {
    let signed = support::signed()?;
    assert_eq!(
        signed.transaction().transaction_id(),
        signed.preparation().transaction_id()
    );
    assert_eq!(
        signed.transaction().bytes().len(),
        signed.preparation().estimate().signed_size_bytes() as usize
    );
    assert_eq!(
        serde_json::from_value::<SignedSubmission>(serde_json::to_value(&signed).unwrap()).unwrap(),
        signed
    );
    let prep = support::preparation()?;
    let wrong = TransactionCbor::from_bytes(support::signed_bytes(&prep, 2).unwrap())?;
    assert!(SignedSubmission::new(prep.clone(), wrong).is_err());
    let different = PaymentPreparation::new(support::intent(500_001)?, support::parameters()?)?;
    assert!(
        SignedSubmission::new(
            prep,
            TransactionCbor::from_bytes(support::signed_bytes(&different, 1).unwrap())?
        )
        .is_err()
    );
    // Zero signature bytes are only a shape fixture, never a cryptographic proof.
    assert!(!format!("{signed:?}").contains(&"00".repeat(64)));
    Ok(())
}

#[test]
fn original_body_hash_uses_source_bytes_and_unsupported_witness_keys_are_not_accepted()
-> Result<(), Error> {
    let prep = support::preparation()?;
    let original = prep.unsigned_payload();
    let body = original.body_bytes();
    let offset = body
        .windows(5)
        .position(|part| part == [0x1a, 0, 7, 0xa1, 0x20])
        .unwrap();
    let mut changed = body[..offset].to_vec();
    changed.extend([0x1b, 0, 0, 0, 0, 0, 7, 0xa1, 0x20]);
    changed.extend(&body[offset + 5..]);
    let mut encoded = vec![0x84];
    encoded.extend(changed);
    encoded.extend([0xa0, 0xf5, 0xf6]);
    let alternate = TransactionCbor::from_bytes(encoded)?;
    assert_eq!(alternate.body_fee(), original.body_fee());
    assert_ne!(alternate.transaction_id(), original.transaction_id());
    assert_eq!(
        alternate.transaction_id(),
        Hash::from_bytes(*pallas_crypto::hash::Hasher::<256>::hash(
            alternate.body_bytes()
        ))
    );
    let mut unsupported = vec![0x84];
    unsupported.extend(body);
    unsupported.extend([0xa1, 0x18, 0x63, 0x80, 0xf5, 0xf6]);
    let parsed = TransactionCbor::from_bytes(unsupported)?;
    assert_eq!(
        SignedSubmission::new(prep, parsed).unwrap_err(),
        Error::UnsupportedCapability
    );
    Ok(())
}

#[test]
fn cbor_framing_checks_duplicates_trailing_map_parity_depth_and_item_limits() -> Result<(), Error> {
    let valid = support::preparation()?.unsigned_payload().bytes().to_vec();
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(TransactionCbor::from_bytes(trailing).is_err());
    for bytes in [
        vec![0x84, 0xa4, 0, 0x80, 1, 0x80, 2, 0, 2, 0, 0xa0, 0xf5, 0xf6],
        vec![
            0x84, 0xa3, 0, 0x80, 1, 0x80, 2, 0, 0xa0, 0xf5, 0xbf, 0, 0xff,
        ],
        vec![
            0x84, 0xa3, 0, 0x80, 1, 0x80, 2, 0, 0xa2, 0, 0x80, 0, 0x80, 0xf5, 0xf6,
        ],
        vec![0x82, 0x82, 0x80, 0x80, 0x80],
        vec![0x82, 0xa3, 0, 0x80, 1, 0x80, 2, 0, 0x80],
        vec![0x82, 0x83, 0x80, 0x80, 0xa0, 0],
        vec![0x84, 0xa3, 0, 0x80, 1, 0x80, 2, 0, 0xa0, 0xf5, 0],
    ] {
        assert!(TransactionCbor::from_bytes(bytes).is_err());
    }
    let mut deep = vec![0x84, 0xa3, 0, 0x80, 1, 0x80, 2, 0, 0xa0, 0xf5];
    deep.extend([0x81; 66]);
    deep.push(0);
    assert!(TransactionCbor::from_bytes(deep).is_err());
    assert!(TransactionCbor::from_bytes(vec![0; 65_537]).is_err());
    let mut items = vec![
        0x84, 0xa3, 0, 0x80, 1, 0x80, 2, 0, 0xa0, 0xf5, 0x99, 0x20, 0,
    ];
    items.extend([0; 8192]);
    assert!(TransactionCbor::from_bytes(items).is_err());
    Ok(())
}

#[test]
fn exact_lovelace_metadata_and_staking_identities_preserve_units_and_controls() -> Result<(), Error>
{
    let amount = Lovelace::parse("18446744073709551616")?;
    assert_eq!(amount.raw(), U256::from(u64::MAX) + U256::from(1));
    assert!(amount.payment_coin().is_err());
    for text in ["1e3", "1.0", "+1", "01", "-1"] {
        assert!(Lovelace::parse(text).is_err());
    }
    let text = SourceText::new("\u{0008}\nmetadata")?;
    assert_eq!(
        serde_json::from_value::<SourceText>(serde_json::to_value(&text).unwrap()).unwrap(),
        text
    );
    assert!(!format!("{text:?}").contains('\u{0008}'));
    let modern = bech32::encode::<bech32::Bech32>(
        bech32::Hrp::parse("drep").unwrap(),
        &[vec![0x22], vec![1; 28]].concat(),
    )
    .unwrap();
    assert_eq!(DrepId::parse(&modern)?.as_str(), modern);
    let invalid = bech32::encode::<bech32::Bech32>(
        bech32::Hrp::parse("drep").unwrap(),
        &[vec![0x21], vec![1; 28]].concat(),
    )
    .unwrap();
    assert!(DrepId::parse(&invalid).is_err());
    let pool =
        bech32::encode::<bech32::Bech32>(bech32::Hrp::parse("pool").unwrap(), &[1; 28]).unwrap();
    assert_eq!(PoolId::parse(&pool)?.as_str(), pool);
    assert!(PoolId::parse(&modern).is_err());
    let fingerprint = asset_fingerprint(&support::token()?)?;
    let (hrp, bytes) = bech32::decode(&fingerprint).unwrap();
    assert_eq!(hrp.as_str(), "asset");
    assert_eq!(bytes.len(), 20);
    assert_ne!(MetadataAvailability::Unreported, MetadataAvailability::Null);
    Ok(())
}

#[test]
fn bounded_pages_validate_target_quantities_duplicate_identity_and_serialized_status()
-> Result<(), Error> {
    let target = PageTarget::Assets;
    let page = PageRequest::new(1, 1, Order::Asc)?;
    let entry = AssetEntry {
        asset: support::token()?,
        quantity: regit_web3::domain::Amount::new(U256::from(7), None),
    };
    let valid = IndexPage::new(
        support::network()?,
        target.clone(),
        page,
        vec![entry.clone()],
    )?;
    assert!(
        IndexPage::new(
            support::network()?,
            target,
            page,
            vec![entry.clone(), entry]
        )
        .is_err()
    );
    let mut forged = serde_json::to_value(&valid).unwrap();
    forged["status"] = json!("complete");
    assert!(serde_json::from_value::<IndexPage<AssetEntry>>(forged).is_err());
    let holder = AssetHolder {
        address: support::address(2)?,
        quantity: regit_web3::domain::Amount::new(U256::from(7), Some(6)),
    };
    assert!(
        IndexPage::new(
            support::network()?,
            PageTarget::AssetHolders {
                asset: support::token()?
            },
            page,
            vec![holder]
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn address_metadata_correlates_actual_header_and_base_stake_credential() -> Result<(), Error> {
    let balance = AddressBalance::new(
        support::network()?,
        support::address(1)?,
        support::assets(10_000_000, 7)?,
    )?;
    let data = AddressData {
        balance,
        stake_address: None,
        address_kind: IndexedAddressKind::Shelley,
        script: false,
    };
    AddressDetails::new(support::network()?, data.clone())?;
    let mut script = data.clone();
    script.script = true;
    assert!(AddressDetails::new(support::network()?, script).is_err());
    let mut byron = data;
    byron.address_kind = IndexedAddressKind::Byron;
    assert!(AddressDetails::new(support::network()?, byron).is_err());
    let mut bytes = vec![1];
    bytes.extend([1; 28]);
    bytes.extend([2; 28]);
    let base = regit_web3::domain::cardano::PaymentAddress::from_bytes(&bytes)?;
    let balance = AddressBalance::new(support::network()?, base, support::assets(10_000_000, 7)?)?;
    let mut stake = vec![0xe1];
    stake.extend([2; 28]);
    let good = AddressData {
        balance,
        stake_address: Some(StakeAddress::from_bytes(&stake)?),
        address_kind: IndexedAddressKind::Shelley,
        script: false,
    };
    AddressDetails::new(support::network()?, good.clone())?;
    let mut wrong = serde_json::to_value(AddressDetails::new(support::network()?, good)?).unwrap();
    stake[1] = 3;
    wrong["data"]["stake_address"] = json!(StakeAddress::from_bytes(&stake)?);
    assert!(serde_json::from_value::<AddressDetails>(wrong).is_err());
    let status = TransactionStatus::NotIndexed {
        network: support::network()?,
        transaction_id: Hash::from_bytes([9; 32]),
    };
    assert_eq!(
        serde_json::from_value::<TransactionStatus>(serde_json::to_value(&status).unwrap())
            .unwrap(),
        status
    );
    assert_eq!(
        TransactionCbor::parse("credential-secret").unwrap_err(),
        Error::Validation(ValidationError::InvalidCardanoTransaction)
    );
    Ok(())
}

#[test]
fn failed_script_paid_collateral_is_separate_from_declared_body_fee() -> Result<(), Error> {
    use regit_web3::domain::cardano::{Transaction, TransactionData};
    let mut encoder = minicbor::Encoder::new(Vec::new());
    encoder
        .array(4)
        .unwrap()
        .map(4)
        .unwrap()
        .u8(0)
        .unwrap()
        .array(0)
        .unwrap()
        .u8(1)
        .unwrap()
        .array(0)
        .unwrap()
        .u8(2)
        .unwrap()
        .u64(500_000)
        .unwrap()
        .u8(17)
        .unwrap()
        .u64(750_000)
        .unwrap()
        .map(0)
        .unwrap()
        .bool(false)
        .unwrap()
        .null()
        .unwrap();
    let cbor = TransactionCbor::from_bytes(encoder.into_writer())?;
    let data = TransactionData {
        transaction_id: cbor.transaction_id(),
        block: Hash::from_bytes([5; 32]),
        block_height: 1,
        block_unix_seconds: 1700,
        slot: 5,
        index: 0,
        output_amount: vec![],
        fees: Lovelace::parse("750000")?,
        deposit: regit_web3::domain::ExactDecimal::parse("0")?,
        size_bytes: cbor.bytes().len().try_into().unwrap(),
        invalid_before: None,
        invalid_hereafter: None,
        utxo_count: 1,
        valid_contract: false,
        treasury_donation: Lovelace::parse("0")?,
    };
    assert_eq!(cbor.body_fee(), Some(500_000));
    assert_eq!(cbor.total_collateral(), Some(750_000));
    let transaction = Transaction::new(support::network()?, data.clone(), cbor.clone())?;
    assert_eq!(
        serde_json::from_value::<Transaction>(serde_json::to_value(&transaction).unwrap()).unwrap(),
        transaction
    );
    // A body without total collateral leaves paid indexed fees source-reported.
    let mut encoder = minicbor::Encoder::new(Vec::new());
    encoder
        .array(4)
        .unwrap()
        .map(3)
        .unwrap()
        .u8(0)
        .unwrap()
        .array(0)
        .unwrap()
        .u8(1)
        .unwrap()
        .array(0)
        .unwrap()
        .u8(2)
        .unwrap()
        .u64(500_000)
        .unwrap()
        .map(0)
        .unwrap()
        .bool(false)
        .unwrap()
        .null()
        .unwrap();
    let without_total = TransactionCbor::from_bytes(encoder.into_writer())?;
    let mut separate = data.clone();
    separate.transaction_id = without_total.transaction_id();
    separate.size_bytes = without_total.bytes().len().try_into().unwrap();
    assert_eq!(without_total.total_collateral(), None);
    assert_eq!(
        Transaction::new(support::network()?, separate, without_total)?
            .data()
            .fees
            .raw(),
        U256::from(750_000)
    );
    let mut incorrect = data;
    incorrect.fees = Lovelace::parse("500000")?;
    assert!(Transaction::new(support::network()?, incorrect, cbor).is_err());
    Ok(())
}

#[test]
fn published_cip14_vectors_use_exact_policy_and_asset_name_bytes() -> Result<(), Error> {
    use regit_web3::domain::cardano::{AssetId, AssetName, NetworkId, PolicyId};
    let policy = PolicyId::parse("7eae28af2208be856f7a119668ae52a49b73725e326dc16579dcc373")?;
    for (name, expected) in [
        ("", "asset1rjklcrnsdzqp65wjgrg55sy9723kw09mlgvlc3"),
        (
            "504154415445",
            "asset13n25uv0yaf5kus35fm2k86cqy60z58d9xmde92",
        ),
        (
            "0000000000000000000000000000000000000000000000000000000000000000",
            "asset1pkpwyknlvul7az0xx8czhl60pyel45rpje4z8w",
        ),
    ] {
        let asset = AssetId::token(NetworkId::mainnet(), policy, AssetName::parse(name)?);
        assert_eq!(asset_fingerprint(&asset)?, expected);
    }
    Ok(())
}

#[test]
fn ordinary_preparation_rejects_script_or_datum_spends_and_supports_generic_external_handoff()
-> Result<(), Error> {
    use regit_web3::{
        domain::cardano::{OutputData, PaymentAddress, Utxo},
        wallets::{HandoffId, HandoffRequest},
    };
    let intent = support::intent(500_000)?;
    let input = &intent.inputs()[0];
    let mut script_address = input.address().bytes().to_vec();
    script_address[0] = 0x71;
    for (address, data) in [
        (
            PaymentAddress::from_bytes(&script_address)?,
            OutputData::new(None, None, None),
        ),
        (
            input.address().clone(),
            OutputData::new(Some(Hash::from_bytes([9; 32])), None, None),
        ),
    ] {
        let unsupported = Utxo::new(
            input.network().clone(),
            address,
            input.transaction_id(),
            input.output_index(),
            input.assets().to_vec(),
            input.creation_block(),
            data,
        )?;
        assert_eq!(
            PaymentIntent::new(
                support::network()?,
                vec![unsupported],
                intent.outputs().to_vec(),
                intent.fee(),
                None,
                intent.invalid_hereafter(),
                1
            )
            .unwrap_err(),
            Error::UnsupportedCapability
        );
    }
    let prep = support::preparation()?;
    let prepared = PreparedRequest::new(prep.clone())?;
    let handoff = HandoffRequest::new(HandoffId::new("caller-cardano-001")?, prepared);
    assert_eq!(handoff.prepared().review().intent(), prep.intent());
    assert_eq!(
        serde_json::from_value::<HandoffRequest<PaymentPreparation>>(
            serde_json::to_value(&handoff).unwrap()
        )
        .unwrap(),
        handoff
    );
    assert!(
        !format!("{handoff:?}").contains(&const_hex::encode(prep.unsigned_payload().body_bytes()))
    );
    assert_send(handoff);
    Ok(())
}
