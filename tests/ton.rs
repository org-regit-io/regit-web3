// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit
//! Pure TON identity, maintained BOC, account linkage and wallet review behavior.
#![cfg(test)]
#![cfg(feature = "ton")]

use regit_web3::{
    domain::{Amount, Source, Timestamp, ton::*},
    error::Error,
    wallets::{Preparation, PreparedRequest},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tycho_types::{
    boc::Boc as Codec,
    cell::{Cell, CellBuilder, CellFamily, HashBytes},
    models::{ExtInMsgInfo, IntAddr, MsgInfo, OwnedMessage, StdAddr},
};

fn network() -> Result<Network, Error> {
    Network::new(
        NetworkCategory::Mainnet,
        ZeroState::new(
            -1,
            Hash::parse("F6OpKZKqvqeFp6CQmFomXNMfMj2EnaUSOXN+Mh+wVWk=")?,
            Hash::parse("XplPz01CXAps5qeSWUtxcyBfdAo5zVb1N979KLSKD24=")?,
        )?,
    )
}
fn address() -> Result<Address, Error> {
    Address::parse("EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrHF")
}
fn rows() -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    let data: Value = serde_json::from_str(include_str!("fixtures/ton/transactions.json"))?;
    data["result"]
        .as_array()
        .ok_or("rows")?
        .iter()
        .map(|r| {
            Ok(Transaction::decode(
                address()?,
                Boc::parse(r["data"].as_str().ok_or("data")?)?,
            )?)
        })
        .collect()
}
fn roundtrip<T: Serialize + DeserializeOwned + Eq + std::fmt::Debug>(
    v: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(&serde_json::from_slice::<T>(&serde_json::to_vec(v)?)?, v);
    Ok(())
}
fn empty() -> Result<Boc, Error> {
    Boc::from_bytes(Codec::encode(Cell::empty_cell()))
}
fn external(destination: Address) -> Result<Boc, Box<dyn std::error::Error>> {
    let root = CellBuilder::build_from(OwnedMessage {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            dst: IntAddr::Std(StdAddr::new(
                destination.workchain(),
                HashBytes(*destination.account().as_bytes()),
            )),
            ..ExtInMsgInfo::default()
        }),
        init: None,
        body: Cell::empty_cell().into(),
        layout: None,
    })?;
    Ok(Boc::from_bytes(Codec::encode(root))?)
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn addresses_preserve_flags_crc_and_identity() -> Result<(), Box<dyn std::error::Error>> {
    let raw = address()?.to_raw();
    let address = Address::parse(&raw)?;
    for bounceable in [true, false] {
        for test_only in [true, false] {
            for url in [true, false] {
                let flags = FriendlyFlags {
                    bounceable,
                    test_only,
                };
                let encoded = address.to_friendly(flags, url);
                let parsed = Address::parse(&encoded)?;
                assert!(parsed.same_account(address));
                assert_eq!(parsed.format(), AddressFormat::Friendly(flags));
                roundtrip(&parsed)?;
                assert_eq!(parsed.check_network(network()?).is_err(), test_only);
            }
        }
    }
    for malformed in [
        "+0:0000000000000000000000000000000000000000000000000000000000000000",
        "0:00",
        "128:0000000000000000000000000000000000000000000000000000000000000000",
        "EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrH0",
    ] {
        assert!(Address::parse(malformed).is_err());
    }
    let zero = Address::new(0, Hash::ZERO, AddressFormat::Raw);
    assert!(
        Address::parse(&zero.to_friendly(
            FriendlyFlags {
                bounceable: false,
                test_only: false
            },
            true
        ))?
        .same_account(zero)
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn network_categories_check_full_zero_state() -> Result<(), Box<dyn std::error::Error>> {
    let n = network()?;
    roundtrip(&n)?;
    assert!(Network::new(NetworkCategory::Custom, n.zero_state()).is_err());
    let wrong = ZeroState::new(-1, Hash::from_bytes([1; 32]), Hash::from_bytes([2; 32]))?;
    assert!(Network::new(NetworkCategory::Mainnet, wrong).is_err());
    assert!(ZeroState::new(0, Hash::from_bytes([1; 32]), Hash::from_bytes([2; 32])).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn exact_nano_and_extra_currency_bounds() -> Result<(), Box<dyn std::error::Error>> {
    let max = Nanotons::new((1u128 << 120) - 1)?;
    roundtrip(&max)?;
    assert_eq!(
        Nanotons::parse("9007199254740993")?
            .amount()?
            .formatted()
            .as_deref(),
        Some("9007199.254740993")
    );
    assert!(Nanotons::new(1u128 << 120).is_err());
    for text in ["", "-1", "+1", "01", "1.0", "1e2", " 1"] {
        assert!(Nanotons::parse(text).is_err());
    }
    let extra = ExtraCurrency {
        id: 1,
        amount: Amount::from_decimal("1", None)?,
    };
    assert!(Currency::new(max, vec![extra.clone(), extra]).is_err());
    assert!(
        Currency::new(
            max,
            vec![ExtraCurrency {
                id: 1,
                amount: Amount::from_decimal("1", Some(0))?
            }]
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
fn boc_strict_framing_hash_and_privacy() -> Result<(), Box<dyn std::error::Error>> {
    let b = empty()?;
    roundtrip(&b)?;
    assert_eq!(
        b.hash()?.to_hex(),
        "96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7"
    );
    let mut trailing = b.as_bytes().to_vec();
    trailing.push(0);
    assert!(Boc::from_bytes(trailing).is_err());
    let mut reserved = b.as_bytes().to_vec();
    reserved[4] |= 8;
    assert!(Boc::from_bytes(reserved).is_err());
    assert!(Boc::from_bytes(vec![0; Boc::MAX_BYTES + 1]).is_err());
    assert!(!format!("{b:?}").contains(&b.to_base64()));
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn real_transaction_vectors_decode_hash_time_and_fees() -> Result<(), Box<dyn std::error::Error>> {
    let txs = rows()?;
    assert_eq!(txs.len(), 2);
    assert_eq!(txs[0].cursor().logical_time().raw(), 107_688_363_000_020);
    assert_eq!(
        txs[0].cursor().hash(),
        Hash::parse("HiUo0gLKMkSkCgEsSPGcwRsKjHjcUKyLfzJY+YynwAQ=")?
    );
    assert_eq!(txs[0].fees().native().raw(), 25);
    assert_eq!(txs[0].previous(), Some(txs[1].cursor()));
    assert!(matches!(txs[0].execution(), Execution::Ordinary { .. }));
    assert!(
        matches!(txs[0].incoming().ok_or("incoming")?.info()?,MessageInfo::Internal{value,..}if value.native().raw()==1)
    );
    for tx in txs {
        roundtrip(&tx)?;
    }
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn transaction_serde_and_account_correlation_reject_forgery()
-> Result<(), Box<dyn std::error::Error>> {
    let tx = rows()?.remove(0);
    let wrong = Address::new(0, Hash::from_bytes([1; 32]), AddressFormat::Raw);
    assert!(Transaction::decode(wrong, tx.boc().clone()).is_err());
    let mut value = serde_json::to_value(&tx)?;
    value["created_unix_seconds"] = json!(1);
    assert!(serde_json::from_value::<Transaction>(value).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn linked_history_cursor_does_not_repeat_inclusive_oldest() -> Result<(), Box<dyn std::error::Error>>
{
    let txs = rows()?;
    let request = HistoryRequest::new(address()?, Some(txs[0].cursor()), 2, true)?;
    let page = HistoryPage::new(request, txs.clone())?;
    assert_eq!(page.next(), txs[1].previous());
    assert!(!page.reaches_account_origin());
    roundtrip(&page)?;
    assert!(HistoryPage::new(request, vec![txs[1].clone(), txs[0].clone()]).is_err());
    assert!(HistoryPage::new(HistoryRequest::new(address()?, None, 1, true)?, txs).is_err());
    assert!(HistoryRequest::new(address()?, None, 101, true).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn message_scan_is_explicitly_page_bounded() -> Result<(), Box<dyn std::error::Error>> {
    let txs = rows()?;
    let hash = txs[0].incoming().ok_or("incoming")?.hash()?;
    let page = HistoryPage::new(HistoryRequest::new(address()?, None, 2, true)?, txs)?;
    let scan = MessageStatus::scan(hash, page.clone())?;
    assert_eq!(scan.observed().len(), 1);
    roundtrip(&scan)?;
    assert!(MessageStatus::scan(Hash::from_bytes([3; 32]), page)?.not_observed_within_page());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn preparation_binds_internal_fields_and_outer_policy_for_review()
-> Result<(), Box<dyn std::error::Error>> {
    let sender = address()?;
    let destination = Address::new(0, Hash::ZERO, AddressFormat::Raw);
    let intent = TransferIntent::new(
        network()?,
        sender,
        destination,
        Nanotons::parse("1234567890")?,
        false,
        None,
        Some(2_000_000_000),
    )?;
    let prep = TransferPreparation::new(intent.clone())?;
    prep.validate()?;
    roundtrip(&prep)?;
    let request = PreparedRequest::new(prep.clone())?;
    assert_eq!(request.review().intent(), &intent);
    assert_eq!(request.review().unsigned_payload(), prep.unsigned_payload());
    let root = Codec::decode(prep.unsigned_payload().as_bytes())?;
    let msg = root.parse::<tycho_types::models::RelaxedMessage<'_>>()?;
    let tycho_types::models::RelaxedMsgInfo::Int(info) = msg.info else {
        return Err("internal".into());
    };
    assert!(info.src.is_none());
    assert_eq!(info.value.tokens.into_inner(), 1_234_567_890);
    assert_eq!(info.dst.as_std().ok_or("std")?.address, HashBytes::ZERO);
    assert!(
        SignedSubmission::new(network()?, destination, prep.unsigned_payload().clone()).is_err()
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn preparation_serde_recomputes_payload() -> Result<(), Box<dyn std::error::Error>> {
    let intent = TransferIntent::new(
        network()?,
        address()?,
        address()?,
        Nanotons::parse("1")?,
        true,
        None,
        None,
    )?;
    let prep = TransferPreparation::new(intent)?;
    let mut value = serde_json::to_value(prep)?;
    value["unsigned_payload"] = serde_json::to_value(empty()?)?;
    assert!(serde_json::from_value::<TransferPreparation>(value).is_err());
    assert!(
        TransferIntent::new(
            network()?,
            address()?,
            address()?,
            Nanotons::ZERO,
            true,
            None,
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
fn signed_submission_checks_external_structure_and_hash_only()
-> Result<(), Box<dyn std::error::Error>> {
    let destination = address()?;
    let submit = SignedSubmission::new(network()?, destination, external(destination)?)?;
    roundtrip(&submit)?;
    let result = SubmissionResult::new(submit.clone(), submit.message_hash())?;
    roundtrip(&result)?;
    assert!(SubmissionResult::new(submit, Hash::from_bytes([1; 32])).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn observation_rejects_wrong_operation_and_schema() -> Result<(), Box<dyn std::error::Error>> {
    let tx = rows()?.remove(0);
    let source = Source::new("fixture", "getTransactions", "0.1.0")?;
    let context = Context::new(
        Operation::Transaction,
        network()?,
        None,
        source.clone(),
        Timestamp::from_unix_seconds(1),
    );
    let observation = Observation::new(tx.clone(), context)?;
    roundtrip(&observation)?;
    let wrong = Context::new(
        Operation::AccountHistory,
        network()?,
        None,
        source,
        Timestamp::from_unix_seconds(1),
    );
    assert!(Observation::new(tx, wrong).is_err());
    let mut value = serde_json::to_value(observation)?;
    value["schema_version"] = json!(2);
    assert!(serde_json::from_value::<Observation<Transaction>>(value).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn boc_crc_multiroot_and_depth_limits_are_enforced() -> Result<(), Box<dyn std::error::Error>> {
    let tx = rows()?.remove(0);
    let mut bytes = tx.boc().as_bytes().to_vec();
    let last = bytes.last_mut().ok_or("crc")?;
    *last ^= 1;
    assert!(Boc::from_bytes(bytes).is_err());
    assert!(Boc::from_bytes(Codec::encode_pair((Cell::empty_cell(), Cell::empty_cell()))).is_err());
    let mut cell = Cell::empty_cell();
    for _ in 0..257 {
        let mut builder = CellBuilder::new();
        builder.store_reference(cell)?;
        cell = builder.build()?;
    }
    assert!(Boc::from_bytes(Codec::encode(cell)).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn unsupported_execution_retains_description_and_known_malformed_execution_fails()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = rows()?.remove(0);
    let root = Codec::decode(fixture.boc().as_bytes())?;
    let mut tx = root.parse::<tycho_types::models::Transaction>()?;
    let mut description = CellBuilder::new();
    description.store_small_uint(2, 3)?;
    tx.info = tycho_types::cell::Lazy::from_raw(description.build()?)?;
    let boc = Boc::from_bytes(Codec::encode(CellBuilder::build_from(&tx)?))?;
    let parsed = Transaction::decode(address()?, boc)?;
    assert!(matches!(
        parsed.execution(),
        Execution::Uninterpreted { prefix: 2, .. }
    ));
    roundtrip(&parsed)?;
    let mut description = CellBuilder::new();
    description.store_small_uint(0, 4)?;
    tx.info = tycho_types::cell::Lazy::from_raw(description.build()?)?;
    let boc = Boc::from_bytes(Codec::encode(CellBuilder::build_from(&tx)?))?;
    assert!(Transaction::decode(address()?, boc).is_err());
    assert_eq!(
        regit_web3::domain::ton::Message::new(empty()?).info()?,
        MessageInfo::Uninterpreted
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn next_cursor_and_fee_collection_serde_enforce_constructor_parity()
-> Result<(), Box<dyn std::error::Error>> {
    let txs = rows()?;
    let page = HistoryPage::new(HistoryRequest::new(address()?, None, 2, true)?, txs.clone())?;
    let mut data = serde_json::to_value(page)?;
    data["next"] = serde_json::to_value(txs[0].cursor())?;
    assert!(serde_json::from_value::<HistoryPage>(data).is_err());
    let request = FeeRequest::new(network()?, address()?, empty()?, None, None, true)?;
    let part = FeePart {
        incoming_forwarding_fee: Nanotons::ZERO,
        storage_fee: Nanotons::ZERO,
        gas_fee: Nanotons::ZERO,
        forwarding_fee: Nanotons::ZERO,
    };
    let estimate = FeeEstimate::new(request.clone(), part, vec![part])?;
    roundtrip(&estimate)?;
    assert!(FeeEstimate::new(request, part, vec![part; 257]).is_err());
    let empty_page = HistoryPage::new(HistoryRequest::new(address()?, None, 1, true)?, Vec::new())?;
    assert!(!empty_page.reaches_account_origin());
    assert!(Cursor::new(LogicalTime::new(1)?, Hash::ZERO).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn external_message_with_referenced_body_rejects_trailing_structure()
-> Result<(), Box<dyn std::error::Error>> {
    use tycho_types::cell::Store as _;
    let destination = address()?;
    let message = OwnedMessage {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            dst: IntAddr::Std(StdAddr::new(
                destination.workchain(),
                HashBytes(*destination.account().as_bytes()),
            )),
            ..ExtInMsgInfo::default()
        }),
        init: None,
        body: Cell::empty_cell().into(),
        layout: Some(tycho_types::models::MessageLayout {
            init_to_cell: false,
            body_to_cell: true,
        }),
    };
    let mut builder = CellBuilder::new();
    message.store_into(&mut builder, Cell::empty_context())?;
    let valid = Boc::from_bytes(Codec::encode(builder.clone().build()?))?;
    SignedSubmission::new(network()?, destination, valid)?;
    builder.store_bit(true)?;
    let trailing = Boc::from_bytes(Codec::encode(builder.build()?))?;
    assert!(SignedSubmission::new(network()?, destination, trailing).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn outgoing_provider_fees_remain_separate_and_constructor_serde_bound()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/ton/outgoing.json"))?;
    let row = &fixture["result"][0];
    let decoded =
        Transaction::decode(address()?, Boc::parse(row["data"].as_str().ok_or("data")?)?)?;
    assert!(decoded.provider_fees().is_none());
    assert_eq!(decoded.fees().native().raw(), 442_944);
    assert_eq!(decoded.outgoing().len(), 1);
    assert_eq!(
        decoded.outgoing()[0].forwarding_fee()?,
        Some(Nanotons::new(64_535)?)
    );
    let source = SourceMessageFees {
        message_hash: decoded.outgoing()[0].hash()?,
        forwarding_fee: Nanotons::new(64_535)?,
        source_ihr_fee: Nanotons::ZERO,
    };
    let fees = ProviderFees::new(
        Nanotons::new(507_479)?,
        Nanotons::new(10_676)?,
        Nanotons::new(496_803)?,
        vec![source.clone()],
    )?;
    roundtrip(&fees)?;
    let attributed = decoded.clone().with_provider_fees(fees)?;
    assert_eq!(attributed.fees().native(), decoded.fees().native());
    assert_eq!(
        attributed.provider_fees().ok_or("fees")?.aggregate().raw(),
        507_479
    );
    roundtrip(&attributed)?;
    assert!(
        ProviderFees::new(
            Nanotons::new(507_479)?,
            Nanotons::new(10_676)?,
            Nanotons::new(496_804)?,
            vec![source.clone()]
        )
        .is_err()
    );
    let mut wrong_hash = source.clone();
    wrong_hash.message_hash = Hash::from_bytes([1; 32]);
    assert!(
        decoded
            .clone()
            .with_provider_fees(ProviderFees::new(
                Nanotons::new(507_479)?,
                Nanotons::new(10_676)?,
                Nanotons::new(496_803)?,
                vec![wrong_hash]
            )?)
            .is_err()
    );
    let mut wrong_forwarding = source.clone();
    wrong_forwarding.forwarding_fee = Nanotons::new(64_536)?;
    assert!(
        decoded
            .clone()
            .with_provider_fees(ProviderFees::new(
                Nanotons::new(507_480)?,
                Nanotons::new(10_676)?,
                Nanotons::new(496_804)?,
                vec![wrong_forwarding]
            )?)
            .is_err()
    );
    // This source-labelled IHR value is not reinterpreted as decoded modern flags.
    let mut reported_ihr = source;
    reported_ihr.source_ihr_fee = Nanotons::new(2)?;
    let historical_source = decoded.with_provider_fees(ProviderFees::new(
        Nanotons::new(507_481)?,
        Nanotons::new(10_676)?,
        Nanotons::new(496_805)?,
        vec![reported_ihr],
    )?)?;
    assert_eq!(
        historical_source.provider_fees().ok_or("fees")?.outgoing()[0]
            .source_ihr_fee
            .raw(),
        2
    );
    roundtrip(&historical_source)?;
    let mut forged = serde_json::to_value(&attributed)?;
    forged["provider_fees"]["outgoing"][0]["message_hash"] = json!(Hash::from_bytes([1; 32]));
    assert!(serde_json::from_value::<Transaction>(forged).is_err());
    let mut forged = serde_json::to_value(&attributed)?;
    forged["provider_fees"]["aggregate"] = json!("507480");
    assert!(serde_json::from_value::<Transaction>(forged).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn uninterpreted_forwarding_header_keeps_source_fees_without_decoding_ihr()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/ton/outgoing.json"))?;
    let boc = Boc::parse(fixture["result"][0]["data"].as_str().ok_or("data")?)?;
    let root = Codec::decode(boc.as_bytes())?;
    let mut raw = root.parse::<tycho_types::models::Transaction>()?;
    // Synthetic unsupported message evidence; no transfer outcome is asserted.
    raw.out_msgs
        .set(tycho_types::num::Uint15::new(0), Cell::empty_cell())?;
    let tx = Transaction::decode(
        address()?,
        Boc::from_bytes(Codec::encode(CellBuilder::build_from(raw)?))?,
    )?;
    assert_eq!(tx.outgoing()[0].info()?, MessageInfo::Uninterpreted);
    assert_eq!(tx.outgoing()[0].forwarding_fee()?, None);
    let aggregate = Nanotons::new(tx.fees().native().raw() + 11)?;
    let fees = ProviderFees::new(
        aggregate,
        Nanotons::ZERO,
        aggregate,
        vec![SourceMessageFees {
            message_hash: tx.outgoing()[0].hash()?,
            forwarding_fee: Nanotons::new(9)?,
            source_ihr_fee: Nanotons::new(2)?,
        }],
    )?;
    let attributed = tx.with_provider_fees(fees)?;
    assert_eq!(attributed.outgoing()[0].forwarding_fee()?, None);
    roundtrip(&attributed)?;
    Ok(())
}
