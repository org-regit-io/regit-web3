// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Public Cardano contracts independent of a provider or runtime.

#![cfg(feature = "cardano")]

use regit_web3::{
    chains::cardano::BalanceReader,
    domain::{
        Source, Timestamp, U256,
        cardano::{
            AddressBalance, AssetAmount, AssetId, AssetName, Context, Hash, HexData, Network,
            NetworkId, Observation, Operation, Order, OutputData, PageRequest, PageStatus,
            PaymentAddress, PolicyId, StakeAddress, Utxo, UtxoPage,
        },
    },
    error::{Error, ValidationError},
};
use serde_json::json;
use std::{
    future::{Future, ready},
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

// Published CIP-19 vectors.
const MAIN: &str = "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x";
const TEST: &str = "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgs68faae";
const STAKE: &str = "stake1uyehkck0lajq8gr28t9uxnuvgcqrc6070x3k9r8048z8y5gh6ffgw";
const BYRON: &str = "37btjrVyb4KDXBNC4haBVPCrro8AQPHwvCMp3RFhhSVWwfFmZ6wwzSK6JK1hY6wHNmtrpTf1kdbva8TCneM2YsiXT7mrzT21EacHnPpz5YyUdj64na";

fn network(alias: &str) -> Result<Network, Error> {
    Network::new(NetworkId::mainnet(), alias)
}
fn native(raw: U256) -> Result<AssetAmount, Error> {
    AssetAmount::new(AssetId::native(NetworkId::mainnet()), raw, Some(6))
}
fn token(raw: U256) -> Result<AssetAmount, Error> {
    AssetAmount::new(
        AssetId::token(
            NetworkId::mainnet(),
            PolicyId::from_bytes([3; 28]),
            AssetName::from_bytes(vec![0, 255])?,
        ),
        raw,
        None,
    )
}
fn output(index: u16, assets: Vec<AssetAmount>) -> Result<Utxo, Error> {
    Utxo::new(
        network("output")?,
        PaymentAddress::parse(MAIN)?,
        Hash::from_bytes([1; 32]),
        index,
        assets,
        Hash::from_bytes([2; 32]),
        OutputData::new(None, None, None),
    )
}
fn balance() -> Result<AddressBalance, Error> {
    AddressBalance::new(
        network("value")?,
        PaymentAddress::parse(MAIN)?,
        vec![
            native(U256::from(9_007_199_254_740_993_u64))?,
            token(U256::from(7))?,
        ],
    )
}
fn context(operation: Operation) -> Result<Context, Error> {
    Ok(Context::new(
        operation,
        network("context")?,
        Source::new("fixture", "indexed", "v0")?,
        Timestamp::from_unix_seconds(1700),
    ))
}

#[test]
fn published_vectors_roundtrip_with_conventional_roles_and_encoding() {
    for text in [MAIN, TEST, BYRON] {
        let parsed = PaymentAddress::parse(text).unwrap();
        assert_eq!(parsed.as_str(), text);
        assert_eq!(PaymentAddress::from_bytes(parsed.bytes()).unwrap(), parsed);
        assert_eq!(
            serde_json::from_value::<PaymentAddress>(json!(text)).unwrap(),
            parsed
        );
    }
    let stake = StakeAddress::parse(STAKE).unwrap();
    assert_eq!(StakeAddress::from_bytes(stake.bytes()).unwrap(), stake);
    assert!(PaymentAddress::parse(STAKE).is_err());
    assert!(StakeAddress::parse(MAIN).is_err());
    assert!(StakeAddress::parse(BYRON).is_err());
    assert_eq!(
        PaymentAddress::parse(&MAIN.to_uppercase())
            .unwrap()
            .as_str(),
        MAIN
    );
}

#[test]
fn address_validation_checks_byron_crc_surplus_bytes_and_pointer_canonicality() {
    let parsed = PaymentAddress::parse(MAIN).unwrap();
    let mut surplus = parsed.bytes().to_vec();
    surplus.push(0);
    assert!(PaymentAddress::from_bytes(&surplus).is_err());
    let mut byron = pallas_addresses::byron::ByronAddress::from_base58(BYRON).unwrap();
    byron.crc ^= 1;
    assert_eq!(
        PaymentAddress::parse(&byron.to_base58()).unwrap_err(),
        Error::Validation(ValidationError::InvalidCardanoAddress)
    );
    for text in [
        "",
        "secret-credential",
        "addr1invalid",
        &const_hex::encode(parsed.bytes()),
    ] {
        assert_eq!(
            PaymentAddress::parse(text).unwrap_err().to_string(),
            "invalid Cardano address"
        );
    }
    let mut pointer = vec![0x40];
    pointer.extend([0; 28]);
    pointer.extend([0x80, 0, 0, 0]);
    assert!(PaymentAddress::from_bytes(&pointer).is_err());
}

fn byron_with_attributes(
    attributes: Vec<pallas_addresses::byron::AddrAttrProperty>,
) -> Result<String, pallas_addresses::Error> {
    let mut payload = pallas_addresses::byron::ByronAddress::from_base58(BYRON)?.decode()?;
    payload.attributes = attributes.into();
    // The maintained codec generates canonical outer CBOR and recomputes CRC.
    Ok(pallas_addresses::byron::ByronAddress::from_decoded(payload).to_base58())
}

#[test]
fn byron_inner_network_magic_requires_canonical_full_cbor_even_with_valid_crc() {
    use pallas_addresses::byron::AddrAttrProperty;
    let canonical =
        byron_with_attributes(vec![AddrAttrProperty::NetworkTag(vec![1].into())]).unwrap();
    let parsed = PaymentAddress::parse(&canonical).unwrap();
    assert!(parsed.is_compatible_with(NetworkId::preprod()));
    assert!(!parsed.is_compatible_with(NetworkId::preview()));
    assert_eq!(
        serde_json::from_value::<PaymentAddress>(json!(canonical)).unwrap(),
        parsed
    );
    for inner in [vec![0x18, 1], vec![1, 0], vec![0x20]] {
        let encoded =
            byron_with_attributes(vec![AddrAttrProperty::NetworkTag(inner.into())]).unwrap();
        let raw = pallas_addresses::byron::ByronAddress::from_base58(&encoded).unwrap();
        assert_eq!(
            crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC).checksum(raw.payload.0.as_ref()),
            raw.crc
        );
        assert_eq!(
            PaymentAddress::parse(&encoded).unwrap_err(),
            Error::Validation(ValidationError::InvalidCardanoAddress)
        );
        assert!(serde_json::from_value::<PaymentAddress>(json!(encoded)).is_err());
    }
}

#[test]
fn byron_derivation_payload_is_canonical_bytes_without_claiming_ciphertext_validity() {
    use pallas_addresses::byron::AddrAttrProperty;
    for inner in [vec![0x40], vec![0x42, 0, 255]] {
        let encoded =
            byron_with_attributes(vec![AddrAttrProperty::DerivationPath(inner.into())]).unwrap();
        let parsed = PaymentAddress::parse(&encoded).unwrap();
        assert!(parsed.is_compatible_with(NetworkId::mainnet()));
        assert_eq!(
            serde_json::from_value::<PaymentAddress>(json!(encoded)).unwrap(),
            parsed
        );
    }
    for inner in [
        vec![1],
        vec![0x58, 0],
        vec![0x41, 0, 0],
        vec![0x5f, 0x40, 0xff],
    ] {
        let encoded =
            byron_with_attributes(vec![AddrAttrProperty::DerivationPath(inner.into())]).unwrap();
        let raw = pallas_addresses::byron::ByronAddress::from_base58(&encoded).unwrap();
        assert_eq!(
            crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC).checksum(raw.payload.0.as_ref()),
            raw.crc
        );
        assert!(PaymentAddress::parse(&encoded).is_err());
        assert!(serde_json::from_value::<PaymentAddress>(json!(encoded)).is_err());
    }
}

#[test]
fn byron_attributes_are_strictly_ordered_unique_and_in_the_supported_profile() {
    use pallas_addresses::byron::{AddrAttrProperty, AddrDistr};
    let path = AddrAttrProperty::DerivationPath(vec![0x42, 0, 255].into());
    let magic = AddrAttrProperty::NetworkTag(vec![1].into());
    let canonical = byron_with_attributes(vec![path.clone(), magic.clone()]).unwrap();
    let accepted = PaymentAddress::parse(&canonical).unwrap();
    assert!(accepted.is_compatible_with(NetworkId::preprod()));
    assert_eq!(
        serde_json::from_value::<PaymentAddress>(json!(canonical)).unwrap(),
        accepted
    );
    for attributes in [
        vec![path.clone(), path.clone()],
        vec![magic.clone(), path],
        vec![magic.clone(), magic],
        vec![AddrAttrProperty::AddrDistr(
            AddrDistr::BootstrapEraDistribution,
        )],
    ] {
        let encoded = byron_with_attributes(attributes).unwrap();
        let raw = pallas_addresses::byron::ByronAddress::from_base58(&encoded).unwrap();
        assert_eq!(
            crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC).checksum(raw.payload.0.as_ref()),
            raw.crc
        );
        assert_eq!(
            PaymentAddress::parse(&encoded).unwrap_err(),
            Error::Validation(ValidationError::InvalidCardanoAddress)
        );
        assert!(serde_json::from_value::<PaymentAddress>(json!(encoded)).is_err());
    }
}

#[test]
fn network_magic_distinguishes_testnets_without_inventing_address_evidence() {
    let address = PaymentAddress::parse(TEST).unwrap();
    assert!(address.is_compatible_with(NetworkId::preprod()));
    assert!(address.is_compatible_with(NetworkId::preview()));
    assert!(!address.is_compatible_with(NetworkId::mainnet()));
    assert_ne!(NetworkId::preprod(), NetworkId::preview());
    for (tag, magic) in [(1, 1), (0, 764_824_073), (2, 3)] {
        assert!(NetworkId::new(tag, magic).is_err());
        assert!(
            serde_json::from_value::<NetworkId>(json!({"network_tag":tag,"network_magic":magic}))
                .is_err()
        );
    }
    assert!(NetworkId::new(0, 1234).is_ok());
    assert_eq!(
        network("a").unwrap().identity(),
        network("b").unwrap().identity()
    );
    assert!(Network::new(NetworkId::mainnet(), "https://secret").is_err());
}

#[test]
fn hashes_and_binary_asset_names_have_exact_width_and_canonical_hex() {
    assert_eq!(
        Hash::parse(&"AB".repeat(32)).unwrap().to_string(),
        "ab".repeat(32)
    );
    assert!(Hash::parse(&"ab".repeat(31)).is_err());
    assert!(AssetName::parse("").unwrap().bytes().is_empty());
    let binary = AssetName::from_bytes(vec![255; 32]).unwrap();
    assert_eq!(
        serde_json::from_value::<AssetName>(json!("ff".repeat(32))).unwrap(),
        binary
    );
    for invalid in ["0x", "0x00", "a", &"ff".repeat(33)] {
        assert!(AssetName::parse(invalid).is_err());
    }
    assert_eq!(
        AssetId::from_unit(NetworkId::mainnet(), &format!("{}00ff", "03".repeat(28))).unwrap(),
        token(U256::from(1)).unwrap().asset_id().clone()
    );
}

#[test]
fn aggregate_balances_preserve_uint256_and_unknown_token_precision() {
    let wide = U256::from(u64::MAX) + U256::from(1);
    let value = AddressBalance::new(
        network("wide").unwrap(),
        PaymentAddress::parse(MAIN).unwrap(),
        vec![native(wide).unwrap(), token(U256::from(7)).unwrap()],
    )
    .unwrap();
    assert_eq!(value.native_amount().unwrap().raw(), wide);
    assert_eq!(value.assets()[1].amount().formatted(), None);
    assert_eq!(
        serde_json::from_value::<AddressBalance>(serde_json::to_value(&value).unwrap()).unwrap(),
        value
    );
    let empty = AddressBalance::new(
        network("empty").unwrap(),
        PaymentAddress::parse(MAIN).unwrap(),
        vec![],
    )
    .unwrap();
    assert!(empty.native_amount().is_none());
    assert!(AssetAmount::new(AssetId::native(NetworkId::mainnet()), U256::from(1), None).is_err());
}

#[test]
fn constructors_and_serde_reject_duplicate_assets_precision_and_network_bypass() {
    assert!(
        AddressBalance::new(
            network("bad").unwrap(),
            PaymentAddress::parse(MAIN).unwrap(),
            vec![
                native(U256::from(1)).unwrap(),
                native(U256::from(2)).unwrap()
            ]
        )
        .is_err()
    );
    let mut fields = serde_json::to_value(balance().unwrap()).unwrap();
    fields["assets"][0]["amount"]["decimals"] = json!(18);
    fields["assets"][0]["amount"]["formatted"] = json!("0.009007199254740993");
    assert!(serde_json::from_value::<AddressBalance>(fields).is_err());
    let mut fields = serde_json::to_value(balance().unwrap()).unwrap();
    fields["assets"][1] = fields["assets"][0].clone();
    assert!(serde_json::from_value::<AddressBalance>(fields).is_err());
    let mut fields = serde_json::to_value(balance().unwrap()).unwrap();
    fields["assets"][1]["asset_id"]["network"] =
        serde_json::to_value(NetworkId::preview()).unwrap();
    assert!(serde_json::from_value::<AddressBalance>(fields).is_err());
}

#[test]
fn output_width_positive_tokens_and_required_nullable_fields_are_enforced() {
    let wide = U256::from(u64::MAX) + U256::from(1);
    assert_eq!(
        output(0, vec![native(wide).unwrap()]).unwrap_err(),
        Error::Validation(ValidationError::CardanoAmountOverflow)
    );
    assert!(
        output(
            0,
            vec![native(U256::from(1)).unwrap(), token(U256::ZERO).unwrap()]
        )
        .is_err()
    );
    assert!(output(0, vec![token(U256::from(1)).unwrap()]).is_err());
    let value = output(u16::MAX, vec![native(U256::from(u64::MAX)).unwrap()]).unwrap();
    assert_eq!(value.output_index(), u16::MAX);
    let mut fields = serde_json::to_value(&value).unwrap();
    fields["assets"][0]["amount"]["raw"] = json!(wide.to_string());
    fields["assets"][0]["amount"]["formatted"] = json!("18446744073709.551616");
    assert!(serde_json::from_value::<Utxo>(fields).is_err());
    let mut fields = serde_json::to_value(value).unwrap();
    fields["data"]
        .as_object_mut()
        .unwrap()
        .remove("inline_datum");
    assert!(serde_json::from_value::<Utxo>(fields).is_err());
}

#[test]
fn pages_preserve_bounds_request_completeness_and_unique_outpoints() {
    let request = PageRequest::new(4, 1, Order::Desc).unwrap();
    let item = output(0, vec![native(U256::from(1)).unwrap()]).unwrap();
    let page = UtxoPage::new(
        network("page").unwrap(),
        PaymentAddress::parse(MAIN).unwrap(),
        request,
        vec![item.clone()],
    )
    .unwrap();
    assert_eq!(page.status(), PageStatus::MayHaveMore);
    assert_eq!(page.requested_page(), request);
    assert_eq!(
        UtxoPage::new(
            network("page").unwrap(),
            PaymentAddress::parse(MAIN).unwrap(),
            request,
            vec![]
        )
        .unwrap()
        .status(),
        PageStatus::ShortPage
    );
    assert!(
        UtxoPage::new(
            network("page").unwrap(),
            PaymentAddress::parse(MAIN).unwrap(),
            PageRequest::new(1, 2, Order::Asc).unwrap(),
            vec![item.clone(), item]
        )
        .is_err()
    );
    let mut fields = serde_json::to_value(page).unwrap();
    fields["status"] = json!("short_page");
    assert!(serde_json::from_value::<UtxoPage>(fields).is_err());
    for (number, count) in [(0, 1), (21_474_837, 1), (1, 0), (1, 101)] {
        assert!(PageRequest::new(number, count, Order::Asc).is_err());
        assert!(
            serde_json::from_value::<PageRequest>(
                json!({"page":number,"count":count,"order":"asc"})
            )
            .is_err()
        );
    }
}

#[test]
fn observations_check_network_operation_schema_unknown_and_raw_duplicate_keys() {
    let value =
        Observation::balance(balance().unwrap(), context(Operation::Balance).unwrap()).unwrap();
    assert_eq!(value.value().network().alias(), "value");
    assert_eq!(value.context().network().alias(), "context");
    assert!(Observation::balance(balance().unwrap(), context(Operation::Utxos).unwrap()).is_err());
    let mut fields = serde_json::to_value(&value).unwrap();
    fields["schema_version"] = json!(2);
    assert!(serde_json::from_value::<Observation<AddressBalance>>(fields).is_err());
    let mut fields = serde_json::to_value(&value).unwrap();
    fields["context"]["evaluation_block"] = json!("invented");
    assert!(serde_json::from_value::<Observation<AddressBalance>>(fields).is_err());
    let encoded = serde_json::to_string(&value).unwrap();
    let duplicate = encoded.replacen('{', "{\"schema_version\":1,", 1);
    assert!(serde_json::from_str::<Observation<AddressBalance>>(&duplicate).is_err());
    let mut fields = serde_json::to_value(value).unwrap();
    fields["context"]["network"]["identity"] = serde_json::to_value(NetworkId::preview()).unwrap();
    assert!(serde_json::from_value::<Observation<AddressBalance>>(fields).is_err());
}

#[test]
fn opaque_data_has_explicit_absence_application_bounds_and_redacted_debug() {
    let present = HexData::parse("19a6aa").unwrap();
    assert_eq!(present.bytes(), [0x19, 0xa6, 0xaa]);
    assert!(!format!("{present:?}").contains("19a6aa"));
    assert!(HexData::parse("0x").is_err());
    assert!(HexData::from_bytes(vec![0; HexData::MAX_BYTES + 1]).is_err());
    assert!(serde_json::from_value::<HexData>(json!("00".repeat(HexData::MAX_BYTES + 1))).is_err());
    let data = OutputData::new(None, Some(HexData::from_bytes(vec![]).unwrap()), None);
    assert!(data.inline_datum().unwrap().bytes().is_empty());
    assert_eq!(serde_json::to_value(data).unwrap()["inline_datum"], "");
}

#[test]
fn caller_backend_is_runtime_independent_without_send_sync_supertraits() {
    struct Reader(Rc<Network>);
    impl BalanceReader for Reader {
        fn network(&self) -> &Network {
            &self.0
        }
        fn get_balance(
            &self,
            _address: PaymentAddress,
        ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + Send {
            ready(
                balance()
                    .and_then(|value| Observation::balance(value, context(Operation::Balance)?)),
            )
        }
    }
    fn read<R: BalanceReader>(
        reader: &R,
        address: PaymentAddress,
    ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + Send {
        reader.get_balance(address)
    }
    let reader = Reader(Rc::new(network("caller").unwrap()));
    let future = read(&reader, PaymentAddress::parse(MAIN).unwrap());
    let mut future = std::pin::pin!(future);
    let mut task = TaskContext::from_waker(Waker::noop());
    match future.as_mut().poll(&mut task) {
        Poll::Ready(Ok(value)) => assert_eq!(value.value().assets().len(), 2),
        _ => panic!("ready custom backend did not return"),
    }
}
