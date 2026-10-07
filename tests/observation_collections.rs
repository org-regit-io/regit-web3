// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Ordered typed observations, explicit failures and bounded decoding.

#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};

use regit_web3::{
    domain::{
        Address, Amount, Asset, AssetId, Balance, BlockContext, BlockHash, BlockSelector, ChainId,
        Finality, NetworkId, Observation, ObservationContext, Source, Timestamp,
        collections::{CollectionError, CollectionLimit, ObservationCollection, ObservationItem},
    },
    error::{Error, ProviderError},
};
#[cfg(feature = "solana")]
use serde::Serialize;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

fn network(chain: u64, alias: &str) -> NetworkId {
    NetworkId::new(ChainId::from(chain), alias).unwrap()
}

fn native_key(chain: u64) -> AssetId {
    Asset::native(network(chain, "configured"), 18, Some("NATIVE".into()))
        .unwrap()
        .identity()
}

fn native_observation(chain: u64, hash_byte: u8, raw: &str) -> Observation<Balance> {
    let hash = BlockHash::from_bytes([hash_byte; 32]);
    Observation::native_balance(
        Balance::new(
            Address::from_bytes([3; 20]),
            Asset::native(network(chain, "value-network"), 18, Some("NATIVE".into())).unwrap(),
            Amount::from_decimal(raw, Some(18)).unwrap(),
        )
        .unwrap(),
        ObservationContext::new(
            network(chain, "context-network"),
            BlockSelector::Hash(hash),
            BlockContext::new(50, hash, Timestamp::from_unix_seconds(100)),
            Source::new("fixture", "native_balance", "v1").unwrap(),
            Timestamp::from_unix_seconds(200),
        )
        .unwrap()
        .with_finality(Finality::Unknown, None),
    )
    .unwrap()
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn limit_constructor_and_deserializer_accept_only_the_explicit_bounded_range() {
    assert_eq!(CollectionLimit::new(0), Err(CollectionError::InvalidLimit));
    assert_eq!(
        CollectionLimit::new(CollectionLimit::MAX_ITEMS + 1),
        Err(CollectionError::InvalidLimit)
    );
    for maximum in [1, 2, CollectionLimit::MAX_ITEMS] {
        let limit = CollectionLimit::new(maximum).unwrap();
        assert_eq!(limit.get(), maximum);
        assert_eq!(usize::from(limit), maximum);
        assert_eq!(serde_json::to_value(limit).unwrap(), json!(maximum));
        assert_eq!(
            serde_json::from_value::<CollectionLimit>(json!(maximum)).unwrap(),
            limit
        );
    }
    for invalid in [
        json!(0),
        json!(1025),
        json!(-1),
        json!(1.5),
        json!("1"),
        Value::Null,
    ] {
        assert!(serde_json::from_value::<CollectionLimit>(invalid).is_err());
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn collection_errors_have_fixed_stable_serialization() {
    for (error, spelling) in [
        (CollectionError::InvalidLimit, "invalid_limit"),
        (CollectionError::TooManyItems, "too_many_items"),
        (CollectionError::DuplicateKey, "duplicate_key"),
    ] {
        assert_eq!(serde_json::to_value(error).unwrap(), json!(spelling));
        assert_eq!(
            serde_json::from_value::<CollectionError>(json!(spelling)).unwrap(),
            error
        );
        assert!(!error.to_string().is_empty());
    }
    assert!(serde_json::from_str::<CollectionError>(r#""unknown_variant""#).is_err());
    assert!(
        serde_json::from_value::<CollectionError>(json!({"duplicate_key": "private"})).is_err()
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn typed_asset_keys_keep_order_exact_values_contexts_and_explicit_failures() {
    let observed = native_observation(1, 7, "9007199254740993");
    let missing = ObservationItem::new(native_key(2), Err(Error::Timeout));
    let successful = ObservationItem::new(native_key(1), Ok(observed.clone()));
    let unavailable = ObservationItem::new(
        native_key(3),
        Err(Error::Provider(ProviderError::Transport)),
    );
    let collection = ObservationCollection::new(
        CollectionLimit::new(3).unwrap(),
        vec![missing.clone(), successful.clone(), unavailable.clone()],
    )
    .unwrap();

    assert_eq!(
        collection.items(),
        [missing, successful.clone(), unavailable]
    );
    assert_eq!(collection.get(&native_key(1)), Some(&successful));
    assert!(collection.get(&native_key(4)).is_none());
    assert_eq!(collection.len(), 3);
    assert_eq!(collection.success_count(), 1);
    assert_eq!(collection.failure_count(), 2);
    assert!(!collection.all_succeeded());
    assert_eq!(collection.items()[0].failure(), Some(Error::Timeout));
    assert!(collection.items()[0].observation().is_none());
    assert!(successful.failure().is_none());
    let retained = collection.items()[1].observation().unwrap();
    assert_eq!(retained, &observed);
    assert_eq!(
        retained.value().amount().raw().to_string(),
        "9007199254740993"
    );
    assert_eq!(
        retained.context().block().hash(),
        &BlockHash::from_bytes([7; 32])
    );
    assert_eq!(
        retained.context().requested_selector(),
        &BlockSelector::Hash(BlockHash::from_bytes([7; 32]))
    );
    assert_eq!(retained.context().retrieved_at().unix_seconds(), 200);
    assert_eq!(retained.context().block().timestamp().unix_seconds(), 100);
    assert_eq!(retained.context().network().alias(), "context-network");
    assert_eq!(retained.value().asset().network().alias(), "value-network");
    assert_eq!(retained.context().finality(), Finality::Unknown);
    assert_eq!(retained.context().confirmations(), None);
    let encoded = serde_json::to_value(&collection).unwrap();
    assert_eq!(
        encoded["items"][0]["outcome"]["Err"],
        json!({"category": "timeout"})
    );
    assert_eq!(
        encoded["items"][1]["outcome"]["Ok"]["value"]["amount"]["raw"],
        "9007199254740993"
    );
    assert_eq!(
        serde_json::from_value::<ObservationCollection<AssetId, Observation<Balance>>>(encoded)
            .unwrap(),
        collection
    );
    let (key, outcome) = successful.into_parts();
    assert_eq!(key, native_key(1));
    assert_eq!(outcome, Ok(observed));
    let (limit, items) = collection.into_parts();
    assert_eq!(limit.get(), 3);
    assert_eq!(items.len(), 3);
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn collection_deserialization_preserves_family_observation_validation() {
    type Collection = ObservationCollection<AssetId, Observation<Balance>>;
    let collection = Collection::new(
        CollectionLimit::new(1).unwrap(),
        vec![ObservationItem::new(
            native_key(1),
            Ok(native_observation(1, 7, "9007199254740993")),
        )],
    )
    .unwrap();
    let original = serde_json::to_value(collection).unwrap();
    for (path, invalid) in [
        ("/items/0/outcome/Ok/network/chain_id", json!("2")),
        ("/items/0/outcome/Ok/schema_version", json!(2)),
        (
            "/items/0/outcome/Ok/requested_selector/value",
            json!(BlockHash::from_bytes([8; 32])),
        ),
        ("/items/0/outcome/Ok/value/amount/formatted", json!("0")),
        (
            "/items/0/outcome/Ok/source/provider_id",
            json!("https://invalid-source.example"),
        ),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(path).unwrap() = invalid;
        assert!(
            serde_json::from_value::<Collection>(value).is_err(),
            "{path}"
        );
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn exact_key_equality_retains_network_aliases_without_requiring_hashing() {
    let original = network(1, "display-A");
    let renamed = network(1, "display-a");
    let collection = ObservationCollection::new(
        CollectionLimit::new(2).unwrap(),
        vec![
            ObservationItem::<_, Observation<Balance>>::new(original.clone(), Err(Error::Timeout)),
            ObservationItem::new(renamed.clone(), Err(Error::UnavailableData)),
        ],
    )
    .unwrap();
    assert_eq!(collection.items()[0].key(), &original);
    assert_eq!(collection.items()[1].key(), &renamed);
    assert_eq!(
        collection.get(&original).unwrap().failure(),
        Some(Error::Timeout)
    );
    assert_eq!(
        collection.get(&renamed).unwrap().failure(),
        Some(Error::UnavailableData)
    );
    assert!(collection.get(&network(1, "display-B")).is_none());
    assert_eq!(
        ObservationCollection::new(
            CollectionLimit::new(2).unwrap(),
            vec![collection.items()[0].clone(), collection.items()[0].clone()],
        ),
        Err(CollectionError::DuplicateKey)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn duplicates_and_excess_items_are_rejected_without_truncation() {
    let first = ObservationItem::<_, Amount>::new(native_key(1), Err(Error::Timeout));
    let same_key_success =
        ObservationItem::new(native_key(1), Ok(Amount::from_decimal("0", None).unwrap()));
    assert_eq!(
        ObservationCollection::new(
            CollectionLimit::new(2).unwrap(),
            vec![first.clone(), same_key_success.clone()]
        ),
        Err(CollectionError::DuplicateKey)
    );
    assert_eq!(
        ObservationCollection::new(
            CollectionLimit::new(1).unwrap(),
            vec![first, same_key_success]
        ),
        Err(CollectionError::TooManyItems)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn success_counts_describe_only_supplied_outcomes_including_an_empty_set() {
    let empty = ObservationCollection::<AssetId, Observation<Balance>>::new(
        CollectionLimit::new(1).unwrap(),
        vec![],
    )
    .unwrap();
    assert!(empty.is_empty());
    assert!(empty.all_succeeded());
    assert_eq!(empty.success_count(), 0);
    assert_eq!(empty.failure_count(), 0);
    assert_eq!(empty.limit().get(), 1);
    let supplied = ObservationCollection::new(
        CollectionLimit::new(2).unwrap(),
        vec![ObservationItem::new(
            native_key(1),
            Ok(native_observation(1, 1, "0")),
        )],
    )
    .unwrap();
    assert!(!supplied.is_empty());
    assert_eq!(supplied.len(), 1);
    assert_eq!(supplied.success_count(), 1);
    assert_eq!(supplied.failure_count(), 0);
    assert!(supplied.all_succeeded());
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn deserialization_retains_constructor_parity_independently_of_field_order() {
    type Collection = ObservationCollection<u16, Amount>;
    let items = json!([
        {"key": 2, "outcome": {"Err": {"category": "timeout"}}},
        {"key": 1, "outcome": {"Ok": Amount::from_decimal("9007199254740993", None).unwrap()}}
    ]);
    let item_json = items.to_string();
    let limit_first = format!(r#"{{"limit":2,"items":{item_json}}}"#);
    let items_first = format!(r#"{{"items":{item_json},"limit":2}}"#);
    let from_limit_first: Collection = serde_json::from_str(&limit_first).unwrap();
    assert_eq!(
        serde_json::from_str::<Collection>(&items_first).unwrap(),
        from_limit_first
    );
    assert_eq!(from_limit_first.items()[0].key(), &2);
    assert_eq!(from_limit_first.items()[1].key(), &1);
    assert_eq!(
        from_limit_first.items()[1]
            .observation()
            .unwrap()
            .decimals(),
        None
    );
    assert_eq!(
        serde_json::from_str::<Collection>(&format!("[2,{item_json}]")).unwrap(),
        from_limit_first
    );
    for invalid in [
        format!(r#"{{"limit":1,"items":{item_json}}}"#),
        format!(r#"{{"items":{item_json},"limit":1}}"#),
        r#"{"limit":2,"items":[{"key":1,"outcome":{"Err":{"category":"timeout"}}},{"key":1,"outcome":{"Err":{"category":"unavailable_data"}}}]}"#.into(),
    ] {
        assert!(serde_json::from_str::<Collection>(&invalid).is_err());
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn strict_collection_and_item_fields_reject_ambiguous_or_untyped_input() {
    type Collection = ObservationCollection<u16, Amount>;
    for invalid in [
        "{}",
        r#"{"limit":1}"#,
        r#"{"items":[]}"#,
        r#"{"limit":null,"items":[]}"#,
        r#"{"limit":1,"items":null}"#,
        r#"{"limit":1,"limit":2,"items":[]}"#,
        r#"{"limit":1,"items":[],"items":[]}"#,
        r#"{"limit":1,"items":[],"extra":{"token":"private"}}"#,
        r#"{"limit":1,"items":[{"key":1}]}"#,
        r#"{"limit":1,"items":[{"outcome":{"Err":{"category":"timeout"}}}]}"#,
        r#"{"limit":1,"items":[{"key":1,"key":2,"outcome":{"Err":{"category":"timeout"}}}]}"#,
        r#"{"limit":1,"items":[{"key":1,"outcome":{"Err":{"category":"timeout"}},"extra":0}]}"#,
        r#"{"limit":1,"items":[{"key":1,"outcome":{"Err":{"category":"provider","reason":"private"}}}]}"#,
        r#"{"limit":1,"items":[{"key":1,"outcome":null}]}"#,
        "[1]",
        r#"[1,[],{"unexpected":[1,2,3]}]"#,
    ] {
        assert!(
            serde_json::from_str::<Collection>(invalid).is_err(),
            "{invalid}"
        );
    }
    let error = serde_json::from_str::<Collection>(r#"{"private-field-name":null}"#)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("unknown observation collection field"));
    assert!(!error.contains("private-field-name"));
}

static KEY_DECODES: AtomicUsize = AtomicUsize::new(0);
static OBSERVATION_DECODES: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Eq, PartialEq)]
struct CountedKey(u16);

impl<'de> Deserialize<'de> for CountedKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        KEY_DECODES.fetch_add(1, Ordering::Relaxed);
        u16::deserialize(deserializer).map(Self)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CountedObservation(u64);

impl<'de> Deserialize<'de> for CountedObservation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        OBSERVATION_DECODES.fetch_add(1, Ordering::Relaxed);
        u64::deserialize(deserializer).map(Self)
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn overflow_is_rejected_before_an_extra_key_or_observation_is_decoded() {
    type Collection = ObservationCollection<CountedKey, CountedObservation>;
    KEY_DECODES.store(0, Ordering::Relaxed);
    OBSERVATION_DECODES.store(0, Ordering::Relaxed);
    let extra = r#"{"limit":1,"items":[{"key":1,"outcome":{"Ok":1}},{"key":["never-decoded"],"outcome":{"Ok":"never-decoded"}}]}"#;
    let error = serde_json::from_str::<Collection>(extra)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("observation collection item limit exceeded"));
    assert_eq!(KEY_DECODES.load(Ordering::Relaxed), 1);
    assert_eq!(OBSERVATION_DECODES.load(Ordering::Relaxed), 1);

    KEY_DECODES.store(0, Ordering::Relaxed);
    OBSERVATION_DECODES.store(0, Ordering::Relaxed);
    let items = (0..CollectionLimit::MAX_ITEMS)
        .map(|index| format!(r#"{{"key":{index},"outcome":{{"Ok":1}}}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let absolute_overflow = format!(
        r#"{{"items":[{items},{{"key":"never-decoded","outcome":{{"Ok":null}}}}],"limit":1024}}"#
    );
    let error = serde_json::from_str::<Collection>(&absolute_overflow)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("observation collection item limit exceeded"));
    assert_eq!(
        KEY_DECODES.load(Ordering::Relaxed),
        CollectionLimit::MAX_ITEMS
    );
    assert_eq!(
        OBSERVATION_DECODES.load(Ordering::Relaxed),
        CollectionLimit::MAX_ITEMS
    );
    let exact = format!(r#"{{"items":[{items}],"limit":1024}}"#);
    assert_eq!(
        serde_json::from_str::<Collection>(&exact).unwrap().len(),
        CollectionLimit::MAX_ITEMS
    );
}

#[cfg(feature = "solana")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum FamilyKey {
    Evm(AssetId),
    Solana {
        genesis_hash: regit_web3::domain::solana::Hash,
        address: regit_web3::domain::solana::Pubkey,
    },
}

#[cfg(feature = "solana")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum FamilyObservation {
    Evm(Observation<Balance>),
    Solana(regit_web3::domain::solana::Observation<regit_web3::domain::solana::NativeBalance>),
}

#[cfg(feature = "solana")]
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn mixed_family_observations_keep_actual_hash_and_slot_contexts_independently() {
    use regit_web3::domain::solana::{
        Commitment, Context, Hash, NativeBalance, Network, Operation, Pubkey, ReadOptions,
    };

    let genesis = Hash::from_bytes([9; 32]);
    let address = Pubkey::from_bytes([4; 32]);
    let solana_key = FamilyKey::Solana {
        genesis_hash: genesis,
        address,
    };
    let solana = regit_web3::domain::solana::Observation::native_balance(
        NativeBalance::new(
            Network::new(genesis, "solana-value").unwrap(),
            address,
            9_007_199_254_740_993,
        ),
        Context::new(
            Operation::NativeBalance,
            Network::new(genesis, "solana-context").unwrap(),
            ReadOptions::new(Commitment::Confirmed, Some(100)),
            120,
            Source::new("solana-fixture", "getBalance", "v1").unwrap(),
            Timestamp::from_unix_seconds(300),
        )
        .unwrap(),
    )
    .unwrap();
    let evm = native_observation(1, 8, "9007199254740995");
    let evm_key = FamilyKey::Evm(native_key(1));
    let collection = ObservationCollection::new(
        CollectionLimit::new(3).unwrap(),
        vec![
            ObservationItem::new(
                solana_key.clone(),
                Ok(FamilyObservation::Solana(solana.clone())),
            ),
            ObservationItem::new(FamilyKey::Evm(native_key(2)), Err(Error::Timeout)),
            ObservationItem::new(evm_key.clone(), Ok(FamilyObservation::Evm(evm.clone()))),
        ],
    )
    .unwrap();
    assert_eq!(collection.items()[0].key(), &solana_key);
    assert_eq!(collection.items()[2].key(), &evm_key);
    assert!(
        collection
            .get(&FamilyKey::Solana {
                genesis_hash: Hash::from_bytes([10; 32]),
                address,
            })
            .is_none()
    );
    assert_eq!(
        collection.get(&evm_key).unwrap().observation(),
        Some(&FamilyObservation::Evm(evm))
    );
    assert_eq!(collection.success_count(), 2);
    assert_eq!(collection.failure_count(), 1);
    assert_eq!(
        solana.value().amount().raw().to_string(),
        "9007199254740993"
    );
    assert_eq!(solana.context().network().genesis_hash(), genesis);
    assert_eq!(solana.context().slot(), 120);
    assert_eq!(
        solana.context().requested_options().minimum_context_slot(),
        Some(100)
    );
    assert_eq!(
        solana.context().requested_options().commitment(),
        Commitment::Confirmed
    );
    assert_eq!(solana.context().retrieved_at().unix_seconds(), 300);
    assert_eq!(solana.context().source().provider_id(), "solana-fixture");
    assert_eq!(
        serde_json::from_str::<ObservationCollection<FamilyKey, FamilyObservation>>(
            &serde_json::to_string(&collection).unwrap()
        )
        .unwrap(),
        collection
    );
}
