// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Family identity, exact native values and immutable indexed constructor/serde contracts.
#![cfg(feature = "dogecoin")]
use regit_web3::{
    domain::{Source, Timestamp, dogecoin::*},
    error::{Error, ValidationError},
};
use serde_json::{Value, json};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const ADDRESS: &str = "DLAznsPDLDRgsVcTFWRMYMG5uH6GddDtv8";
const GENESIS: &str = "1a91e3dace36e2be3bf030a65679fe821aa1d6ef92e7c9902eb318182c355691";
fn address() -> Result<Address, Error> {
    Address::parse(ADDRESS, Network::Mainnet)
}
fn txid(byte: u8) -> Txid {
    Txid::from_display_bytes([byte; 32])
}
fn balance() -> Result<BalanceData, Error> {
    Ok(BalanceData {
        total_received: Koinu::new(11),
        total_sent: Koinu::new(1),
        confirmed: Koinu::new(10),
        unconfirmed: MempoolDelta::new(-1)?,
        final_balance: Koinu::new(9),
        confirmed_transactions: 2,
        unconfirmed_transactions: 1,
        final_transactions: 3,
    })
}
fn transaction() -> Result<TransactionData, Error> {
    Ok(TransactionData {
        status: TransactionStatus::new(txid(1), None, 0, false, None)?,
        version: 2,
        lock_time: None,
        size: 100,
        virtual_size: None,
        total_output: Koinu::new(10),
        reported_fee: Some(Koinu::new(1)),
        inputs: vec![TransactionInput {
            previous_output: Some(OutPoint {
                txid: txid(2),
                output_index: 1,
            }),
            coinbase_source: None,
            previous_output_value: Some(Koinu::new(11)),
            sequence: Some(u32::MAX),
            script: None,
            witness: None,
            addresses: Some(vec![address()?]),
            script_type: None,
        }],
        outputs: vec![TransactionOutput {
            value: Koinu::new(10),
            script: Bytes::new(Vec::new())?,
            addresses: None,
            spent_by: None,
            script_type: None,
        }],
        raw: None,
    })
}
fn reference(id: u8, index: u32, height: u64) -> TransactionReference {
    TransactionReference {
        txid: txid(id),
        direction: ReferenceDirection::Output { index },
        value: Koinu::new(1),
        inclusion: Some(ReferenceInclusion {
            height,
            block_hash: None,
            transaction_index: None,
            confirmed_at: None,
        }),
        confirmations: 1,
        balance_at_reference: None,
        spent: Some(false),
        spent_by: None,
        double_spend: false,
        double_spend_transaction: None,
        script: None,
    }
}

#[test]
fn standard_genesis_identity_is_independent_of_display_alias_and_serde_checks_it() -> TestResult {
    assert_eq!(Network::Mainnet.genesis_hash().to_string(), GENESIS);
    assert_ne!(
        Network::Mainnet.genesis_hash(),
        Network::Testnet.genesis_hash()
    );
    assert_ne!(
        Network::Testnet.genesis_hash(),
        Network::Regtest.genesis_hash()
    );
    let identity = NetworkId::new(Network::Mainnet, "mainnet")?;
    let mut wire = serde_json::to_value(&identity)?;
    wire["genesis_hash"] = json!(Network::Testnet.genesis_hash());
    assert!(serde_json::from_value::<NetworkId>(wire).is_err());
    assert!(NetworkId::new(Network::Mainnet, "bad alias").is_err());
    Ok(())
}
#[test]
fn maintained_address_checksums_network_width_and_constructor_serde_agree() -> TestResult {
    let value = address()?;
    assert_eq!(value.to_string(), ADDRESS);
    assert!(Address::parse(ADDRESS, Network::Testnet).is_err());
    let mut bad = ADDRESS.to_string();
    bad.push('1');
    assert!(Address::parse(&bad, Network::Mainnet).is_err());
    let malformed = bs58::encode([30; 22]).with_check().into_string();
    assert!(Address::parse(&malformed, Network::Mainnet).is_err());
    assert!(Address::parse("private-invalid-secret", Network::Mainnet).is_err());
    let error = Address::parse("private-invalid-secret", Network::Mainnet)
        .err()
        .ok_or(Error::Configuration)?;
    assert!(!format!("{error:?} {error}").contains("private-invalid-secret"));
    let wire = serde_json::to_value(&value)?;
    assert_eq!(serde_json::from_value::<Address>(wire.clone())?, value);
    let mut wrong = wire;
    wrong["network"] = json!("testnet");
    assert!(serde_json::from_value::<Address>(wrong).is_err());
    Ok(())
}
#[test]
fn exact_native_units_and_signed_deltas_preserve_values_above_javascript_precision() -> TestResult {
    let amount = Koinu::from_decimal("9007199254740993")?;
    assert_eq!(amount.raw(), 9_007_199_254_740_993);
    assert_eq!(
        amount.amount().formatted().as_deref(),
        Some("90071992.54740993")
    );
    assert_eq!(Koinu::unit(), "koinu");
    assert_eq!(serde_json::to_string(&amount)?, "\"9007199254740993\"");
    assert!(Koinu::from_decimal("01").is_err());
    assert!(Koinu::from_decimal("18446744073709551616").is_err());
    assert!(MempoolDelta::from_decimal("-0").is_err());
    assert!(MempoolDelta::from_decimal("1.5").is_err());
    let delta = MempoolDelta::from_decimal("-9007199254740993")?;
    assert_eq!(
        serde_json::from_str::<MempoolDelta>(&serde_json::to_string(&delta)?)?,
        delta
    );
    Ok(())
}
#[test]
fn balance_lifetime_totals_are_distinct_from_transaction_money_limits() -> TestResult {
    let mut data = balance()?;
    data.total_received = Koinu::new(u64::MAX);
    data.total_sent = Koinu::new(u64::MAX - 10);
    let value = AddressBalance::new(address()?, data)?;
    assert_eq!(value.data().confirmed.raw(), 10);
    let mut wire = serde_json::to_value(&value)?;
    wire["data"]["final_balance"] = json!("10");
    assert!(serde_json::from_value::<AddressBalance>(wire).is_err());
    let mut data = balance()?;
    data.final_transactions = u64::MAX;
    assert!(AddressBalance::new(address()?, data).is_err());
    let mut data = balance()?;
    data.unconfirmed = MempoolDelta::new(-11)?;
    assert!(AddressBalance::new(address()?, data).is_err());
    Ok(())
}
#[test]
fn height_pages_keep_distinct_same_transaction_references_and_complete_boundary_blocks()
-> TestResult {
    let request = HistoryRequest::new(None, 1, 3)?;
    let value = HistoryPage::new(
        address()?,
        request,
        balance()?,
        vec![reference(1, 0, 10), reference(1, 1, 10)],
        Vec::new(),
        Some(true),
    )?;
    assert_eq!(value.confirmed().len(), 2);
    assert_eq!(value.next_before_height(), Some(10));
    assert_eq!(
        serde_json::from_str::<HistoryPage>(&serde_json::to_string(&value)?)?,
        value
    );
    assert!(
        HistoryPage::new(
            address()?,
            request,
            balance()?,
            vec![reference(1, 0, 10), reference(1, 0, 10)],
            Vec::new(),
            Some(true)
        )
        .is_err()
    );
    assert!(
        HistoryPage::new(
            address()?,
            HistoryRequest::new(Some(10), 1, 3)?,
            balance()?,
            vec![reference(1, 0, 10)],
            Vec::new(),
            Some(true)
        )
        .is_err()
    );
    assert!(
        HistoryPage::new(
            address()?,
            request,
            balance()?,
            vec![reference(1, 0, 10), reference(2, 0, 11)],
            Vec::new(),
            Some(true)
        )
        .is_err()
    );
    assert!(HistoryRequest::new(None, 2001, 3000).is_err());
    assert!(HistoryRequest::new(None, 2, 1).is_err());
    assert!(
        HistoryPage::new(
            address()?,
            HistoryRequest::new(None, 1, 1)?,
            balance()?,
            vec![reference(1, 0, 10), reference(1, 1, 10)],
            Vec::new(),
            Some(true)
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn immutable_transactions_enforce_family_money_ranges_and_exact_known_prevout_fees() -> TestResult {
    let value = Transaction::new(Network::Mainnet, transaction()?)?;
    assert_eq!(value.derived_fee().map(Koinu::raw), Some(1));
    let mut data = transaction()?;
    data.inputs[0].previous_output_value = None;
    let value = Transaction::new(Network::Mainnet, data.clone())?;
    assert!(value.derived_fee().is_none());
    data.inputs.push(transaction()?.inputs.remove(0));
    data.inputs[1].previous_output = Some(OutPoint {
        txid: txid(3),
        output_index: 2,
    });
    data.inputs[1].previous_output_value = Some(Koinu::new(12));
    assert!(Transaction::new(Network::Mainnet, data).is_err());
    let mut data = transaction()?;
    data.outputs[0].value = Koinu::new(1_000_000_000_000_000_000);
    data.total_output = Koinu::new(1_000_000_000_000_000_000);
    data.inputs[0].previous_output_value = None;
    data.reported_fee = Some(Koinu::new(0));
    assert!(Transaction::new(Network::Mainnet, data.clone()).is_ok());
    data.outputs[0].value = Koinu::new(1_000_000_000_000_000_000 + 1);
    data.total_output = Koinu::new(1_000_000_000_000_000_000 + 1);
    assert!(Transaction::new(Network::Mainnet, data).is_err());
    let mut data = transaction()?;
    data.outputs[0].value = Koinu::new(u64::MAX);
    data.outputs.push(data.outputs[0].clone());
    assert!(Transaction::new(Network::Mainnet, data).is_err());
    let mut wire = serde_json::to_value(value)?;
    wire["data"]["outputs"][0]["value"] = json!("999");
    assert!(serde_json::from_value::<Transaction>(wire).is_err());
    Ok(())
}
#[test]
fn coinbase_missing_fields_and_family_witness_semantics_are_not_synthesized() -> TestResult {
    let mut data = transaction()?;
    data.inputs[0].previous_output = None;
    data.inputs[0].coinbase_source = Some(CoinbaseSource::ExplicitSentinel);
    data.inputs[0].previous_output_value = None;
    data.reported_fee = Some(Koinu::new(0));
    let value = Transaction::new(Network::Mainnet, data.clone())?;
    assert!(value.derived_fee().is_none());
    assert!(value.data().lock_time.is_none());
    assert!(value.data().virtual_size.is_none());
    data.inputs.push(data.inputs[0].clone());
    assert!(Transaction::new(Network::Mainnet, data).is_err());
    let mut data = transaction()?;
    data.inputs[0].witness = Some(vec![Bytes::from_hex("aa")?]);
    assert!(Transaction::new(Network::Mainnet, data).is_err());
    Ok(())
}
#[test]
fn source_status_schema_query_and_family_network_correlations_remain_checked() -> TestResult {
    assert!(TransactionStatus::new(txid(1), None, 1, false, None).is_err());
    let context = Context::new(
        NetworkId::new(Network::Mainnet, "mainnet")?,
        Operation::TransactionStatus { txid: txid(1) },
        Source::new("fixture", "status", "0.1.0")?,
        Timestamp::from_unix_seconds(1),
    )?;
    let value = Observation::transaction_status(
        TransactionStatus::new(txid(1), None, 0, false, None)?,
        context.clone(),
    )?;
    assert!(
        Observation::transaction_status(
            TransactionStatus::new(txid(2), None, 0, false, None)?,
            context
        )
        .is_err()
    );
    let mut wire = serde_json::to_value(&value)?;
    wire["context"]["schema_version"] = json!(2);
    assert!(serde_json::from_value::<Observation<TransactionStatus>>(wire).is_err());
    let wrong = Address::parse(
        &bs58::encode([113; 21]).with_check().into_string(),
        Network::Testnet,
    )?;
    assert_eq!(
        Context::new(
            NetworkId::new(Network::Mainnet, "mainnet")?,
            Operation::AddressBalance { address: wrong },
            Source::new("fixture", "balance", "0.1.0")?,
            Timestamp::from_unix_seconds(1)
        ),
        Err(Error::Validation(ValidationError::NetworkMismatch))
    );
    Ok(())
}
#[test]
fn opaque_payloads_are_bounded_and_redacted_without_claiming_canonical_decoding() -> TestResult {
    let bytes = Bytes::from_hex("70726976617465")?;
    assert!(!format!("{bytes:?}").contains("70726976617465"));
    assert!(Bytes::from_hex("0xaa").is_err());
    assert!(Bytes::from_hex("abc").is_err());
    assert!(Bytes::new(vec![0; Bytes::MAXIMUM_BYTES + 1]).is_err());
    let text = SourceText::new("private-source-metadata")?;
    assert!(!format!("{text:?}").contains("private-source-metadata"));
    assert!(SourceText::new("line\ncontrol").is_err());
    assert!(Txid::parse(&"0".repeat(63)).is_err());
    assert!(Txid::parse(&format!("0x{}", "0".repeat(64))).is_err());
    let value: Value = serde_json::to_value(Transaction::new(Network::Mainnet, transaction()?)?)?;
    assert!(value["data"]["raw"].is_null());
    Ok(())
}

#[test]
fn dogecoin_has_distinct_regtest_versions_and_no_segwit_address_form() {
    let mut raw = [7_u8; 21];
    raw[0] = 111;
    let regression = bs58::encode(raw).with_check().into_string();
    assert!(Address::parse(&regression, Network::Regtest).is_ok());
    assert!(Address::parse(&regression, Network::Testnet).is_err());
    assert!(
        Address::parse(
            "ltc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv2arq5",
            Network::Mainnet
        )
        .is_err()
    );
    assert!(Address::parse("DD4KSSuBJqcjuTcvUg1CgUKeurPUFeEZkE", Network::Mainnet).is_ok());
}

#[test]
fn null_outpoints_and_nested_resource_bounds_are_enforced_before_constructor_work() -> TestResult {
    fn copy_value<T: Copy>(value: T) -> T {
        value
    }
    assert_eq!(copy_value(Koinu::new(1)).raw(), 1);
    assert_eq!(copy_value(MempoolDelta::new(-1)?).raw(), -1);
    let mut data = transaction()?;
    data.inputs[0].previous_output = Some(OutPoint {
        txid: txid(0),
        output_index: u32::MAX,
    });
    assert!(Transaction::new(Network::Mainnet, data).is_err());
    let value = Transaction::new(Network::Mainnet, transaction()?)?;
    let mut wire = serde_json::to_value(value)?;
    wire["data"]["inputs"][0]["addresses"] = json!(vec![address()?; 101]);
    let error = serde_json::from_value::<Transaction>(wire.clone())
        .err()
        .ok_or(Error::Configuration)?;
    assert!(error.to_string().contains("indexed record limit exceeded"));
    wire["data"]["inputs"][0]["addresses"] = Value::Null;
    wire["data"]["inputs"][0]["witness"] = json!(vec![""; 10_001]);
    let error = serde_json::from_value::<Transaction>(wire)
        .err()
        .ok_or(Error::Configuration)?;
    assert!(error.to_string().contains("indexed record limit exceeded"));
    Ok(())
}

#[test]
fn source_coinbase_classification_is_explicit_correlated_and_checked_through_serde() -> TestResult {
    let mut data = transaction()?;
    data.inputs[0].previous_output = None;
    data.inputs[0].previous_output_value = None;
    data.inputs[0].coinbase_source = Some(CoinbaseSource::OmittedPrevoutFields);
    data.reported_fee = Some(Koinu::new(0));
    let value = Transaction::new(Network::Mainnet, data.clone())?;
    assert_eq!(
        value.data().inputs[0].coinbase_source,
        Some(CoinbaseSource::OmittedPrevoutFields)
    );
    assert!(value.derived_fee().is_none());
    let wire = serde_json::to_value(&value)?;
    assert_eq!(serde_json::from_value::<Transaction>(wire.clone())?, value);
    let mut wrong = wire;
    wrong["data"]["inputs"][0]["coinbase_source"] = Value::Null;
    assert!(serde_json::from_value::<Transaction>(wrong).is_err());
    data.inputs[0].coinbase_source = None;
    assert!(Transaction::new(Network::Mainnet, data).is_err());
    let mut ordinary = transaction()?;
    ordinary.inputs[0].coinbase_source = Some(CoinbaseSource::ExplicitSentinel);
    assert!(Transaction::new(Network::Mainnet, ordinary).is_err());
    Ok(())
}
