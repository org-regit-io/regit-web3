// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Public contracts for exact values, validated identities, and observations.

#![cfg(test)]
use regit_web3::domain::{
    Address, Amount, Asset, AssetKind, Balance, BlockContext, BlockHash, BlockSelector, ChainId,
    Finality, MetadataOrigin, NetworkId, Observation, ObservationContext, Operation, Source,
    Timestamp,
};
use regit_web3::error::Error;
use serde_json::{Value, json};

const UINT256_MAX: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";
const UINT256_OVERFLOW: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639936";

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn amount_roundtrips_zero_large_integers_and_uint256_max_losslessly() {
    for raw in ["0", "9007199254740993", UINT256_MAX] {
        let amount = Amount::from_decimal(raw, None).unwrap();
        assert_eq!(amount.raw().to_string(), raw);
        assert_eq!(amount.decimals(), None);
        assert_eq!(amount.formatted(), None);

        let serialized = serde_json::to_value(amount).unwrap();
        assert_eq!(
            serialized,
            json!({"raw": raw, "decimals": null, "formatted": null})
        );
        let decoded: Amount = serde_json::from_value(serialized).unwrap();
        assert_eq!(decoded, amount);
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn exact_formatting_preserves_configured_fractional_places() {
    let cases = [
        ("0", 0, "0"),
        ("42", 0, "42"),
        ("10", 1, "1.0"),
        ("1234", 2, "12.34"),
        ("1", 18, "0.000000000000000001"),
        ("0", 18, "0.000000000000000000"),
        ("1000000000000000000", 18, "1.000000000000000000"),
        ("9007199254740993", 6, "9007199254.740993"),
        (UINT256_MAX, 0, UINT256_MAX),
    ];
    for (raw, decimals, expected) in cases {
        let amount = Amount::from_decimal(raw, Some(decimals)).unwrap();
        assert_eq!(amount.formatted().as_deref(), Some(expected));
        assert_eq!(
            serde_json::to_value(amount).unwrap(),
            json!({"raw": raw, "decimals": decimals, "formatted": expected})
        );
    }

    let amount = Amount::from_decimal("1", Some(u8::MAX)).unwrap();
    let expected = format!("0.{}1", "0".repeat(254));
    assert_eq!(amount.formatted(), Some(expected));
    assert_eq!(amount.raw().to_string(), "1");
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn decimal_amount_parser_rejects_noncanonical_or_out_of_range_input() {
    for raw in [
        "",
        "-1",
        "+1",
        "1.0",
        "1e3",
        "0x10",
        " 1",
        "1 ",
        "1\n",
        "00",
        "01",
        "1_000",
        "１２",
        UINT256_OVERFLOW,
    ] {
        assert!(Amount::from_decimal(raw, None).is_err(), "accepted {raw:?}");
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn amount_deserialization_cannot_bypass_raw_decimals_or_derived_format() {
    let valid = json!({"raw": "1", "decimals": 18, "formatted": "0.000000000000000001"});
    assert!(serde_json::from_value::<Amount>(valid.clone()).is_ok());
    for field in ["raw", "decimals", "formatted"] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Amount>(missing).is_err());
    }
    let unknown_precision = json!({"raw": "1", "decimals": null, "formatted": null});
    assert!(serde_json::from_value::<Amount>(unknown_precision.clone()).is_ok());
    for field in ["decimals", "formatted"] {
        let mut missing = unknown_precision.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Amount>(missing).is_err());
    }

    let invalid_fields = [
        ("raw", json!(1)),
        ("raw", json!(-1)),
        ("raw", serde_json::from_str::<Value>("1.5").unwrap()),
        ("raw", json!("1.0")),
        ("raw", json!(UINT256_OVERFLOW)),
        ("decimals", json!(-1)),
        ("decimals", json!(256)),
        ("formatted", json!(null)),
        ("formatted", json!("1")),
    ];
    for (field, bad_value) in invalid_fields {
        let mut input = valid.clone();
        input[field] = bad_value;
        assert!(
            serde_json::from_value::<Amount>(input).is_err(),
            "accepted invalid {field}"
        );
    }
    assert!(
        serde_json::from_value::<Amount>(json!({"raw": "1", "decimals": null, "formatted": "1"}))
            .is_err()
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn chain_identity_is_exact_and_distinguishes_networks() {
    let one = ChainId::from_decimal("1").unwrap();
    let other = ChainId::from_decimal("2").unwrap();
    assert_ne!(one, other);
    for raw in ["0", "1", "9007199254740993", UINT256_MAX] {
        let chain = ChainId::from_decimal(raw).unwrap();
        assert_eq!(chain.value().to_string(), raw);
        assert_eq!(
            serde_json::to_value(chain).unwrap(),
            Value::String(raw.to_owned())
        );
        let decoded: ChainId = serde_json::from_value(json!(raw)).unwrap();
        assert_eq!(decoded.value().to_string(), raw);
    }
    assert!(ChainId::from_decimal(UINT256_OVERFLOW).is_err());
    assert!(serde_json::from_value::<ChainId>(json!(9_007_199_254_740_993_u64)).is_err());
    assert!(serde_json::from_value::<ChainId>(json!("-1")).is_err());
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn addresses_follow_eip55_checksums_and_normalize_valid_input() {
    // Official mixed-case vectors: https://eips.ethereum.org/EIPS/eip-55.
    for input in [
        "0x52908400098527886E0F7030069857D2E4169EE7",
        "0x8617E340B3D01FA5F11F306F4090FD50E238070D",
        "0xde709f2102306220921060314715629080e2fb77",
        "0x27b1fdb04752bbc536007a920d24acb045561c26",
        "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
        "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
        "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
        "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
    ] {
        let address = Address::parse(input).unwrap();
        let normalized = input.to_ascii_lowercase();
        assert_eq!(address.to_string(), normalized);
        assert_eq!(serde_json::to_value(address).unwrap(), json!(normalized));
        assert_eq!(
            Address::parse(input).unwrap(),
            Address::parse(&normalized).unwrap()
        );
    }
    let bad_checksum = "0x5AAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
    assert!(Address::parse(bad_checksum).is_err());
    assert!(serde_json::from_value::<Address>(json!(bad_checksum)).is_err());
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn address_and_block_hash_deserialization_validate_length_hex_and_prefix() {
    let address = "0x0000000000000000000000000000000000000000";
    assert!(Address::parse(address).is_ok());
    for input in [
        "",
        "0x",
        "0000000000000000000000000000000000000000",
        "0x000000000000000000000000000000000000000",
        "0x00000000000000000000000000000000000000000",
        "0x000000000000000000000000000000000000000g",
        " 0x0000000000000000000000000000000000000000",
    ] {
        assert!(Address::parse(input).is_err());
        assert!(serde_json::from_value::<Address>(json!(input)).is_err());
    }

    let hash = format!("0x{}", "a".repeat(64));
    let upper_hex = format!("0x{}", "A".repeat(64));
    assert_eq!(
        BlockHash::parse(&hash).unwrap(),
        BlockHash::parse(&upper_hex).unwrap()
    );
    let decoded: BlockHash = serde_json::from_value(json!(upper_hex)).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), json!(hash));
    for input in [
        String::new(),
        "0x0".to_owned(),
        format!("0x{}", "a".repeat(63)),
        format!("0x{}", "a".repeat(65)),
        format!("0x{}g", "a".repeat(63)),
    ] {
        assert!(BlockHash::parse(&input).is_err());
        assert!(serde_json::from_value::<BlockHash>(json!(input)).is_err());
    }
}

fn network(chain_id: &str, alias: &str) -> Result<NetworkId, Error> {
    NetworkId::new(ChainId::from_decimal(chain_id)?, alias)
}

fn hash() -> Result<BlockHash, Error> {
    BlockHash::parse(&format!("0x{}", "ab".repeat(32)))
}

fn context(selector: BlockSelector) -> Result<ObservationContext, Error> {
    ObservationContext::new(
        network("1", "mainnet")?,
        selector,
        BlockContext::new(42, hash()?, Timestamp::from_unix_seconds(1_700_000_000)),
        Source::new("fixture", "eth_getBalance", "0.1.0")?,
        Timestamp::from_unix_seconds(1_700_000_030),
    )
}

fn balance() -> Result<Balance, Error> {
    Balance::new(
        Address::parse("0x0000000000000000000000000000000000000001")?,
        Asset::native(network("1", "mainnet")?, 18, Some("ETH".to_owned()))?,
        Amount::from_decimal("9007199254740993", Some(18))?,
    )
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn native_identity_excludes_display_metadata_and_includes_chain_identity() -> Result<(), Error> {
    let first = Asset::native(network("1", "mainnet")?, 18, Some("ETH".to_owned())).unwrap();
    let renamed = Asset::native(network("1", "alias")?, 18, Some("NATIVE".to_owned())).unwrap();
    let other_chain = Asset::native(network("2", "mainnet")?, 18, Some("ETH".to_owned())).unwrap();
    assert_eq!(first.identity(), renamed.identity());
    assert_ne!(first.identity(), other_chain.identity());
    assert_eq!(first.kind(), AssetKind::Native);
    assert_eq!(first.metadata_origin(), MetadataOrigin::CallerConfigured);
    assert_eq!(first.decimals(), 18);
    assert_eq!(first.symbol(), Some("ETH"));

    let without_symbol = Asset::native(network("1", "mainnet")?, 0, None).unwrap();
    let serialized = serde_json::to_value(&without_symbol).unwrap();
    assert_eq!(serialized["symbol"], Value::Null);
    assert_eq!(serialized["decimals"], 0);
    assert_eq!(serialized["metadata_origin"], "caller_configured");
    assert_eq!(
        serde_json::from_value::<Asset>(serialized).unwrap(),
        without_symbol
    );
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn native_asset_deserialization_rejects_invalid_metadata_and_identity() -> Result<(), Error> {
    let valid = serde_json::to_value(
        Asset::native(network("1", "mainnet")?, 18, Some("ETH".to_owned())).unwrap(),
    )
    .unwrap();
    for (field, invalid) in [
        ("kind", json!("unsupported")),
        ("metadata_origin", json!("unsupported")),
        ("decimals", json!(-1)),
        ("decimals", json!(256)),
        ("decimals", Value::Null),
        ("symbol", json!("https://user:password@example.invalid")),
    ] {
        let mut input = valid.clone();
        input[field] = invalid;
        assert!(serde_json::from_value::<Asset>(input).is_err());
    }
    let mut invalid_chain = valid;
    invalid_chain["network"]["chain_id"] = json!(UINT256_OVERFLOW);
    assert!(serde_json::from_value::<Asset>(invalid_chain).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn native_balance_requires_decimals_to_match_native_asset_metadata() -> Result<(), Error> {
    for decimals in [None, Some(0), Some(17), Some(19)] {
        assert!(
            Balance::new(
                Address::parse("0x0000000000000000000000000000000000000001").unwrap(),
                Asset::native(network("1", "mainnet")?, 18, None).unwrap(),
                Amount::from_decimal("0", decimals).unwrap(),
            )
            .is_err()
        );
    }
    for decimals in [0, 18, u8::MAX] {
        assert!(
            Balance::new(
                Address::parse("0x0000000000000000000000000000000000000001").unwrap(),
                Asset::native(network("1", "mainnet")?, decimals, None).unwrap(),
                Amount::from_decimal("0", Some(decimals)).unwrap(),
            )
            .is_ok()
        );
    }

    let mut input = serde_json::to_value(balance()?).unwrap();
    input["asset"]["decimals"] = json!(6);
    assert!(serde_json::from_value::<Balance>(input).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn observation_retains_selector_resolved_block_and_distinct_times() -> Result<(), Error> {
    let observed =
        Observation::native_balance(balance()?, context(BlockSelector::Latest)?).unwrap();
    let captured = observed.context();
    assert_eq!(captured.schema_version(), 1);
    assert_eq!(captured.operation(), Operation::NativeBalance);
    assert_eq!(captured.requested_selector(), &BlockSelector::Latest);
    assert_eq!(captured.block().number(), 42);
    assert_eq!(captured.block().hash(), &hash()?);
    assert_eq!(captured.block().timestamp().unix_seconds(), 1_700_000_000);
    assert_eq!(captured.retrieved_at().unix_seconds(), 1_700_000_030);
    assert_eq!(captured.finality(), Finality::Unknown);
    assert_eq!(captured.confirmations(), None);
    assert_eq!(observed.value(), &balance()?);

    let serialized = serde_json::to_value(&observed).unwrap();
    assert_eq!(serialized["schema_version"], 1);
    assert_eq!(serialized["operation"], "native_balance");
    assert_eq!(serialized["network"]["chain_id"], "1");
    assert_eq!(serialized["requested_selector"], json!({"kind": "latest"}));
    assert_eq!(serialized["block"]["hash"], json!(hash()?.to_string()));
    assert_eq!(serialized["block"]["timestamp"], 1_700_000_000_u64);
    assert_eq!(serialized["retrieved_at"], 1_700_000_030_u64);
    assert_eq!(
        serialized["source"],
        json!({
            "provider_id": "fixture", "method": "eth_getBalance", "integration_version": "0.1.0"
        })
    );
    assert_eq!(serialized["finality"], "unknown");
    assert_eq!(serialized["confirmations"], Value::Null);
    assert_eq!(serialized["value"]["amount"]["raw"], "9007199254740993");
    assert_eq!(
        serde_json::from_value::<Observation<Balance>>(serialized).unwrap(),
        observed
    );
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn finality_is_explicit_and_confirmations_do_not_establish_it() -> Result<(), Error> {
    for selector in [BlockSelector::Safe, BlockSelector::Finalized] {
        assert_eq!(context(selector)?.finality(), Finality::Unknown);
    }
    let unknown = context(BlockSelector::Latest)?.with_finality(Finality::Unknown, Some(500));
    assert_eq!(unknown.finality(), Finality::Unknown);
    assert_eq!(unknown.confirmations(), Some(500));
    let zero = context(BlockSelector::Latest)?.with_finality(Finality::Unknown, Some(0));
    let encoded =
        serde_json::to_value(Observation::native_balance(balance()?, zero).unwrap()).unwrap();
    assert_eq!(encoded["confirmations"], 0);
    for finality in [Finality::Safe, Finality::Finalized] {
        let captured = context(BlockSelector::Latest)?.with_finality(finality, None);
        assert_eq!(captured.finality(), finality);
        assert_eq!(captured.confirmations(), None);
        let observed = Observation::native_balance(balance()?, captured).unwrap();
        let roundtrip = serde_json::from_value::<Observation<Balance>>(
            serde_json::to_value(&observed).unwrap(),
        )
        .unwrap();
        assert_eq!(roundtrip, observed);
    }
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn explicit_block_selectors_require_matching_resolved_identity() -> Result<(), Error> {
    for selector in [BlockSelector::Number(42), BlockSelector::Hash(hash()?)] {
        let captured = context(selector)?;
        assert_eq!(captured.requested_selector(), &selector);
    }
    let other_hash = BlockHash::parse(&format!("0x{}", "cd".repeat(32))).unwrap();
    for selector in [BlockSelector::Number(43), BlockSelector::Hash(other_hash)] {
        assert!(
            ObservationContext::new(
                network("1", "mainnet")?,
                selector,
                BlockContext::new(42, hash()?, Timestamp::from_unix_seconds(1_700_000_000)),
                Source::new("fixture", "eth_getBalance", "0.1.0").unwrap(),
                Timestamp::from_unix_seconds(1_700_000_030),
            )
            .is_err()
        );
    }
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn each_state_read_selector_roundtrips_separately_from_the_resolved_block() -> Result<(), Error> {
    for (selector, encoded) in [
        (BlockSelector::Latest, json!({"kind":"latest"})),
        (BlockSelector::Safe, json!({"kind":"safe"})),
        (BlockSelector::Finalized, json!({"kind":"finalized"})),
        (
            BlockSelector::Number(42),
            json!({"kind":"number","value":42}),
        ),
        (
            BlockSelector::Hash(hash()?),
            json!({"kind":"hash","value":hash()?.to_string()}),
        ),
    ] {
        assert_eq!(serde_json::to_value(selector).unwrap(), encoded);
        let observed = Observation::native_balance(balance()?, context(selector)?).unwrap();
        let roundtrip: Observation<Balance> =
            serde_json::from_value(serde_json::to_value(&observed).unwrap()).unwrap();
        assert_eq!(roundtrip.context().requested_selector(), &selector);
        assert_eq!(roundtrip.context().block().hash(), &hash()?);
        assert_eq!(roundtrip, observed);
    }
    assert!(serde_json::from_value::<BlockSelector>(json!({"kind":"pending"})).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn balance_observation_rejects_a_different_chain() -> Result<(), Error> {
    let other = ObservationContext::new(
        network("2", "mainnet")?,
        BlockSelector::Latest,
        BlockContext::new(42, hash()?, Timestamp::from_unix_seconds(1_700_000_000)),
        Source::new("fixture", "eth_getBalance", "0.1.0").unwrap(),
        Timestamp::from_unix_seconds(1_700_000_030),
    )
    .unwrap();
    assert!(Observation::native_balance(balance()?, other).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn balance_observation_matches_chain_identity_and_retains_different_aliases() -> Result<(), Error> {
    let captured = ObservationContext::new(
        network("1", "provider-alias")?,
        BlockSelector::Latest,
        BlockContext::new(42, hash()?, Timestamp::from_unix_seconds(1_700_000_000)),
        Source::new("fixture", "eth_getBalance", "0.1.0")?,
        Timestamp::from_unix_seconds(1_700_000_030),
    )?;
    let observed = Observation::native_balance(balance()?, captured)?;
    assert_eq!(observed.context().network().alias(), "provider-alias");
    assert_eq!(observed.value().asset().network().alias(), "mainnet");
    let encoded = serde_json::to_value(&observed).unwrap();
    assert_eq!(encoded["network"]["alias"], "provider-alias");
    assert_eq!(encoded["value"]["asset"]["network"]["alias"], "mainnet");
    let decoded: Observation<Balance> = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, observed);
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn observation_deserialization_enforces_anchor_schema_and_identity() -> Result<(), Error> {
    let observed =
        Observation::native_balance(balance()?, context(BlockSelector::Latest)?).unwrap();
    let valid = serde_json::to_value(observed).unwrap();
    let mut absent_block = valid.clone();
    absent_block.as_object_mut().unwrap().remove("block");
    assert!(serde_json::from_value::<Observation<Balance>>(absent_block).is_err());
    let mut absent_confirmations = valid.clone();
    assert_eq!(absent_confirmations["confirmations"], Value::Null);
    absent_confirmations
        .as_object_mut()
        .unwrap()
        .remove("confirmations");
    assert!(serde_json::from_value::<Observation<Balance>>(absent_confirmations).is_err());

    for (field, invalid) in [
        ("block", Value::Null),
        ("schema_version", json!(0)),
        ("schema_version", json!(2)),
        ("operation", json!("unsupported")),
        ("requested_selector", json!({"kind": "number", "value": 43})),
        (
            "requested_selector",
            json!({"kind": "hash", "value": format!("0x{}", "cd".repeat(32))}),
        ),
    ] {
        let mut input = valid.clone();
        input[field] = invalid;
        assert!(
            serde_json::from_value::<Observation<Balance>>(input).is_err(),
            "accepted invalid {field}"
        );
    }
    for field in ["hash", "number", "timestamp"] {
        let mut input = valid.clone();
        input["block"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Observation<Balance>>(input).is_err());
    }
    let mut wrong_chain = valid.clone();
    wrong_chain["network"]["chain_id"] = json!("2");
    assert!(serde_json::from_value::<Observation<Balance>>(wrong_chain).is_err());
    let mut wrong_decimals = valid.clone();
    wrong_decimals["value"]["asset"]["decimals"] = json!(17);
    assert!(serde_json::from_value::<Observation<Balance>>(wrong_decimals).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn observation_deserialization_rejects_unknown_root_fields() -> Result<(), Error> {
    let observed = Observation::native_balance(balance()?, context(BlockSelector::Latest)?)?;
    let serialized = serde_json::to_string(&observed).unwrap();
    let fields = serialized.strip_prefix('{').unwrap();
    let unknown_field = format!("{{\"additional\":true,{fields}");
    assert!(serde_json::from_str::<Observation<Balance>>(&unknown_field).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn observation_deserialization_rejects_duplicate_root_keys_in_raw_json() -> Result<(), Error> {
    let observed = Observation::native_balance(balance()?, context(BlockSelector::Latest)?)?;
    let serialized = serde_json::to_string(&observed).unwrap();
    let fields = serialized.strip_prefix('{').unwrap();
    let duplicate_schema = format!("{{\"schema_version\":1,{fields}");
    assert!(serde_json::from_str::<Observation<Balance>>(&duplicate_schema).is_err());

    let value = serde_json::to_string(observed.value()).unwrap();
    let duplicate_value = format!("{{\"value\":{value},{fields}");
    assert!(serde_json::from_str::<Observation<Balance>>(&duplicate_value).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn network_and_source_deserialization_reject_malformed_labels() {
    assert!(NetworkId::new(ChainId::from_decimal("1").unwrap(), "a".repeat(64)).is_ok());
    assert!(NetworkId::new(ChainId::from_decimal("1").unwrap(), "a".repeat(65)).is_err());
    for alias in [
        "",
        "main net",
        "https://example.invalid",
        "Authorization: hidden",
        "main\nnet",
    ] {
        assert!(NetworkId::new(ChainId::from_decimal("1").unwrap(), alias).is_err());
        assert!(
            serde_json::from_value::<NetworkId>(json!({"chain_id":"1", "alias":alias})).is_err()
        );
    }
    for bad in [
        "",
        "https://user:password@example.invalid",
        "Bearer hidden",
        "method\nname",
    ] {
        assert!(Source::new(bad, "eth_getBalance", "0.1.0").is_err());
        assert!(Source::new("fixture", bad, "0.1.0").is_err());
        assert!(Source::new("fixture", "eth_getBalance", bad).is_err());
        for field in ["provider_id", "method", "integration_version"] {
            let mut input = json!({"provider_id": "fixture", "method": "eth_getBalance", "integration_version": "0.1.0"});
            input[field] = json!(bad);
            assert!(serde_json::from_value::<Source>(input).is_err());
        }
    }
    let boundary = "a".repeat(128);
    let too_long = "a".repeat(129);
    assert!(Source::new(&boundary, &boundary, &boundary).is_ok());
    assert!(Source::new(&too_long, "eth_getBalance", "0.1.0").is_err());
    assert!(Source::new("fixture", &too_long, "0.1.0").is_err());
    assert!(Source::new("fixture", "eth_getBalance", &too_long).is_err());
}
