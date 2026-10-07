// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact provider-scoped mappings, bounded lookup and deserialization parity.

#![cfg(test)]

use std::cell::Cell;

use regit_web3::domain::{
    Address, Asset, AssetId, ChainId, NetworkId, Source, Timestamp,
    asset_binding::{AssetMarketBinding, AssetMarketRegistry, BindingError, MarketListing},
    market::Identifier,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

fn id(value: &str) -> Identifier {
    Identifier::parse(value).unwrap()
}

fn native_key(chain: u64) -> AssetId {
    Asset::native(
        NetworkId::new(ChainId::from(chain), "configured-network").unwrap(),
        18,
        Some("TOKEN".into()),
    )
    .unwrap()
    .identity()
}

fn binding<A>(asset_key: A, provider: &str, listing: &str) -> AssetMarketBinding<A> {
    AssetMarketBinding::new(
        asset_key,
        MarketListing::new(id(provider), id(listing)),
        Source::new("local-registry", "configured-mapping", "v1").unwrap(),
        Timestamp::from_unix_seconds(100),
    )
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn exact_lookup_preserves_provider_namespace_listing_case_and_mapping_attribution() {
    let asset = native_key(1);
    let first = binding(asset, "coingecko", "ethereum");
    let second = binding(asset, "other-provider", "Ethereum");
    let registry = AssetMarketRegistry::new(2, vec![first.clone(), second.clone()]).unwrap();

    assert_eq!(registry.lookup(&asset, &id("coingecko")), Some(&first));
    assert_eq!(
        registry.lookup(&asset, &id("other-provider")),
        Some(&second)
    );
    assert_eq!(registry.lookup(&asset, &id("CoinGecko")), None);
    assert_eq!(registry.lookup(&native_key(2), &id("coingecko")), None);
    assert_eq!(first.listing().listing_id().as_str(), "ethereum");
    assert_eq!(second.listing().listing_id().as_str(), "Ethereum");
    assert_eq!(first.source().provider_id(), "local-registry");
    assert_eq!(first.recorded_at().unix_seconds(), 100);
    assert_eq!(registry.capacity(), 2);
    assert_eq!(registry.bindings(), [first, second]);
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn identical_listing_text_in_different_providers_is_not_a_conflict() {
    let asset = native_key(1);
    let registry = AssetMarketRegistry::new(
        2,
        vec![
            binding(asset, "provider-a", "coin"),
            binding(asset, "provider-b", "coin"),
        ],
    )
    .unwrap();
    assert_ne!(
        registry.bindings()[0].listing(),
        registry.bindings()[1].listing()
    );
    assert!(registry.lookup(&asset, &id("provider-c")).is_none());
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn technical_native_keys_ignore_display_metadata_without_flattening_networks() {
    let original = native_key(1);
    let renamed = Asset::native(
        NetworkId::new(ChainId::from(1), "different-alias").unwrap(),
        6,
        Some("OTHER".into()),
    )
    .unwrap()
    .identity();
    assert_eq!(original, renamed);
    assert_eq!(
        AssetMarketRegistry::new(
            2,
            vec![
                binding(original, "coingecko", "ethereum"),
                binding(renamed, "coingecko", "ethereum")
            ]
        ),
        Err(BindingError::DuplicateMapping)
    );

    let registry = AssetMarketRegistry::new(
        2,
        vec![
            binding(original, "coingecko", "ethereum"),
            binding(native_key(2), "coingecko", "ethereum"),
        ],
    )
    .unwrap();
    assert_eq!(registry.bindings().len(), 2);
    assert_ne!(
        registry.bindings()[0].asset_key(),
        registry.bindings()[1].asset_key()
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn token_contract_keys_retain_the_full_chain_identifier_without_numeric_narrowing() {
    let contract = Address::from_bytes([7; 20]);
    let first = (ChainId::from(1), contract);
    let second = (
        ChainId::from_decimal("18446744073709551617").unwrap(),
        contract,
    );
    let registry = AssetMarketRegistry::new(
        2,
        vec![
            binding(first, "coingecko", "first-token"),
            binding(second, "coingecko", "second-token"),
        ],
    )
    .unwrap();
    assert_eq!(
        registry
            .lookup(&second, &id("coingecko"))
            .unwrap()
            .listing()
            .listing_id()
            .as_str(),
        "second-token"
    );
    let restored: AssetMarketRegistry<(ChainId, Address)> =
        serde_json::from_str(&serde_json::to_string(&registry).unwrap()).unwrap();
    assert_eq!(restored, registry);
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn duplicate_and_conflicting_mappings_reject_new_source_metadata() {
    let asset = native_key(1);
    let first = binding(asset, "coingecko", "ethereum");
    let repeated = AssetMarketBinding::new(
        asset,
        first.listing().clone(),
        Source::new("another-source", "another-method", "v2").unwrap(),
        Timestamp::from_unix_seconds(101),
    );
    assert_eq!(
        AssetMarketRegistry::new(2, vec![first.clone(), repeated]),
        Err(BindingError::DuplicateMapping)
    );
    assert_eq!(
        AssetMarketRegistry::new(
            2,
            vec![first, binding(asset, "coingecko", "different-coin")]
        ),
        Err(BindingError::ConflictingMapping)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn configured_capacity_and_absolute_ceiling_preserve_an_explicit_empty_registry() {
    let empty = AssetMarketRegistry::<AssetId>::new(1, vec![]).unwrap();
    assert_eq!(empty.lookup(&native_key(1), &id("coingecko")), None);
    assert!(empty.bindings().is_empty());
    assert_eq!(
        AssetMarketRegistry::<AssetId>::new(0, vec![]),
        Err(BindingError::InvalidCapacity)
    );
    assert_eq!(
        AssetMarketRegistry::<AssetId>::new(
            AssetMarketRegistry::<AssetId>::MAX_BINDINGS + 1,
            vec![]
        ),
        Err(BindingError::InvalidCapacity)
    );
    assert_eq!(
        AssetMarketRegistry::new(
            1,
            vec![
                binding(native_key(1), "coingecko", "one"),
                binding(native_key(2), "coingecko", "two")
            ]
        ),
        Err(BindingError::CapacityExceeded)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn serialization_reapplies_identity_capacity_and_mapping_validation() {
    let registry =
        AssetMarketRegistry::new(2, vec![binding(native_key(1), "coingecko", "ethereum")]).unwrap();
    let encoded = serde_json::to_value(&registry).unwrap();
    let restored: AssetMarketRegistry<AssetId> = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored, registry);

    let mut duplicate = encoded.clone();
    duplicate["bindings"]
        .as_array_mut()
        .unwrap()
        .push(encoded["bindings"][0].clone());
    assert!(serde_json::from_value::<AssetMarketRegistry<AssetId>>(duplicate).is_err());

    for changed in [
        json!({"capacity": 0, "bindings": []}),
        json!({"capacity": 1, "bindings": [], "extra": true}),
        json!({"bindings": []}),
    ] {
        assert!(serde_json::from_value::<AssetMarketRegistry<AssetId>>(changed).is_err());
    }
    let mut bad_listing = encoded["bindings"][0].clone();
    bad_listing["listing"]["listing_id"] = json!("../wrong");
    assert!(serde_json::from_value::<AssetMarketBinding<AssetId>>(bad_listing).is_err());
    let mut bad_key = encoded["bindings"][0].clone();
    bad_key["asset_key"]["chain_id"] = json!("01");
    assert!(serde_json::from_value::<AssetMarketBinding<AssetId>>(bad_key).is_err());
    let mut extra_binding = encoded["bindings"][0].clone();
    extra_binding["extra"] = json!(true);
    assert!(serde_json::from_value::<AssetMarketBinding<AssetId>>(extra_binding).is_err());
}

thread_local! {
    static DECODED_KEYS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Eq, PartialEq, Serialize)]
struct CountingKey(u16);

impl<'de> Deserialize<'de> for CountingKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        DECODED_KEYS.with(|count| count.set(count.get() + 1));
        u16::deserialize(deserializer).map(Self)
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn bounded_streaming_deserialization_stops_before_decoding_an_overflow_key() {
    let maximum = AssetMarketRegistry::<CountingKey>::MAX_BINDINGS;
    let bindings = (0..maximum)
        .map(|key| binding(CountingKey(key), "provider", "coin"))
        .collect();
    let registry = AssetMarketRegistry::new(maximum, bindings).unwrap();
    let encoded = serde_json::to_string(&registry).unwrap();
    DECODED_KEYS.with(|count| count.set(0));
    let restored: AssetMarketRegistry<CountingKey> = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored.bindings().len(), usize::from(maximum));
    assert_eq!(DECODED_KEYS.with(Cell::get), usize::from(maximum));

    let mut excessive: Value = serde_json::from_str(&encoded).unwrap();
    let mut last = excessive["bindings"][0].clone();
    last["asset_key"] = json!("must-not-decode");
    excessive["bindings"].as_array_mut().unwrap().push(last);
    let streamed = serde_json::to_string(&excessive).unwrap();
    DECODED_KEYS.with(|count| count.set(0));
    let error = serde_json::from_str::<AssetMarketRegistry<CountingKey>>(&streamed)
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("asset binding capacity exceeded")
    );
    assert!(!error.to_string().contains("must-not-decode"));
    assert_eq!(DECODED_KEYS.with(Cell::get), usize::from(maximum));
}

#[cfg(feature = "solana")]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn non_evm_and_evm_keys_coexist_without_losing_genesis_mint_or_program_identity() {
    use regit_web3::domain::solana::{Hash, Pubkey, TokenIdentity};

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "family", content = "identity", rename_all = "snake_case")]
    enum AssetKey {
        Evm(AssetId),
        Solana(TokenIdentity),
    }

    let token = TokenIdentity::new(
        Hash::from_bytes([1; 32]),
        Pubkey::from_bytes([2; 32]),
        Pubkey::from_bytes([3; 32]),
    );
    let other_network = TokenIdentity::new(
        Hash::from_bytes([4; 32]),
        token.mint(),
        token.token_program(),
    );
    let other_program = TokenIdentity::new(
        token.genesis_hash(),
        token.mint(),
        Pubkey::from_bytes([5; 32]),
    );
    let keys = [
        AssetKey::Evm(native_key(1)),
        AssetKey::Solana(token),
        AssetKey::Solana(other_network),
        AssetKey::Solana(other_program),
    ];
    let registry = AssetMarketRegistry::new(
        4,
        keys.iter()
            .cloned()
            .map(|key| binding(key, "coingecko", "declared-listing"))
            .collect(),
    )
    .unwrap();
    for key in &keys {
        assert_eq!(
            registry.lookup(key, &id("coingecko")).unwrap().asset_key(),
            key
        );
    }
    let restored: AssetMarketRegistry<AssetKey> =
        serde_json::from_str(&serde_json::to_string(&registry).unwrap()).unwrap();
    assert_eq!(restored, registry);
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn diagnostics_and_debug_do_not_include_caller_identifiers_or_keys() {
    let registry = AssetMarketRegistry::new(
        1,
        vec![binding(
            native_key(42),
            "sensitive-label",
            "sensitive-listing",
        )],
    )
    .unwrap();
    for debug in [
        format!("{registry:?}"),
        format!("{:?}", registry.bindings()[0]),
        format!("{:?}", registry.bindings()[0].listing()),
    ] {
        assert!(!debug.contains("sensitive"));
        assert!(!debug.contains("configured-network"));
    }
    assert_eq!(
        BindingError::ConflictingMapping.to_string(),
        "conflicting asset market mapping"
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn typed_errors_roundtrip_stable_names_and_reject_unknown_variants() {
    for (error, name) in [
        (BindingError::InvalidCapacity, "invalid_capacity"),
        (BindingError::CapacityExceeded, "capacity_exceeded"),
        (BindingError::DuplicateMapping, "duplicate_mapping"),
        (BindingError::ConflictingMapping, "conflicting_mapping"),
    ] {
        assert_eq!(serde_json::to_value(error).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<BindingError>(json!(name)).unwrap(),
            error
        );
    }
    for unknown in [
        "unknown_mapping",
        "ConflictingMapping",
        "conflicting-mapping",
    ] {
        assert!(serde_json::from_value::<BindingError>(json!(unknown)).is_err());
    }
}
