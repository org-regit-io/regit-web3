// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Blockfrost indexed operations and separate exact CBOR one-shot submission.

#![cfg(feature = "blockfrost-http")]

#[path = "support/cardano.rs"]
mod cardano;
#[path = "support/blockfrost.rs"]
mod fixture;
use fixture::{Fixture, Reply, genesis};
use regit_web3::wallets::Preparation;
use regit_web3::{
    chains::cardano::{CardanoReader, CardanoSubmitter},
    domain::{
        U256,
        cardano::{
            EpochSelector, Hash, MetadataAvailability, Observation, Operation, Order, PageRequest,
            PageStatus, PaymentPreparation, StakeAddress, TransactionStatus, asset_fingerprint,
        },
    },
    error::{Error, ProviderError, SubmissionFailure, ValidationError},
    providers::blockfrost::BlockfrostClient,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{fmt::Debug, time::Duration};

fn pool() -> Result<String, Error> {
    bech32::encode::<bech32::Bech32>(
        bech32::Hrp::parse("pool").map_err(|_| Error::Configuration)?,
        &[1; 28],
    )
    .map_err(|_| Error::Configuration)
}
fn stake() -> Result<StakeAddress, Error> {
    let mut bytes = vec![0xe1];
    bytes.extend([1; 28]);
    StakeAddress::from_bytes(&bytes)
}
fn epoch() -> Value {
    json!({"epoch":600,"start_time":1_700_000_000,"end_time":1_700_432_000,"first_block_time":1_700_000_020,"last_block_time":1_700_100_000,"block_count":5000,"tx_count":7000,"output":"18446744073709551616","fees":"1000000","active_stake":null})
}
fn supply() -> Value {
    json!({"supply":{"max":"45000000000000000","total":"37000000000000000","circulating":"35000000000000000","locked":"1000000","treasury":"1000000","reserves":"1000000"},"stake":{"live":"24000000000000000","active":"23000000000000000"}})
}
fn account() -> Result<Value, Error> {
    Ok(
        json!({"stake_address":stake()?,"active":true,"registered":true,"active_epoch":599,"controlled_amount":"10000000","rewards_sum":"18446744073709551616","withdrawals_sum":"0","reserves_sum":"0","treasury_sum":"0","withdrawable_amount":"1234","pool_id":pool()?,"drep_id":"drep_always_abstain"}),
    )
}
fn references() -> Value {
    json!([{"tx_hash":"04".repeat(32),"tx_index":1,"block_height":12_000_000,"block_time":1_700_000_000}])
}
fn asset() -> Result<Value, Error> {
    Ok(
        json!({"asset":format!("{}00ff","03".repeat(28)),"policy_id":"03".repeat(28),"asset_name":"00ff","fingerprint":asset_fingerprint(&cardano::token()?)?,"quantity":"18446744073709551616","initial_mint_tx_hash":"04".repeat(32),"mint_or_burn_count":2,"metadata":{"name":"\u{0008}coin","description":"fixture\ntext","ticker":null,"url":null,"decimals":null},"onchain_metadata":{"provider_nested":true}}),
    )
}
fn utxos(hash: Hash) -> Result<Value, Error> {
    Ok(
        json!({"hash":hash,"inputs":[{"address":cardano::address(1)?,"amount":fixture::units(10_000_000),"tx_hash":"04".repeat(32),"output_index":0,"data_hash":null,"inline_datum":null,"reference_script_hash":null,"collateral":false,"reference":false}],"outputs":[{"address":cardano::address(2)?,"amount":fixture::units(9_500_000),"output_index":0,"data_hash":null,"inline_datum":null,"reference_script_hash":null,"collateral":false,"consumed_by_tx":null}]}),
    )
}
fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(
    v: &T,
) -> Result<(), serde_json::Error> {
    assert_eq!(&serde_json::from_slice::<T>(&serde_json::to_vec(v)?)?, v);
    Ok(())
}
fn observed<T: regit_web3::domain::cardano::ObservationValue>(v: &Observation<T>, op: Operation) {
    assert_eq!(v.context().operation(), op);
    assert_eq!(v.context().source().provider_id(), "fixture-blockfrost");
    assert_eq!(
        v.context().network().identity(),
        regit_web3::domain::cardano::NetworkId::mainnet()
    );
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

#[tokio::test]
async fn recorded_protocol_eleven_parameters_support_ordinary_fee_and_unsigned_review()
-> Result<(), Box<dyn std::error::Error>> {
    let parameters: Value = serde_json::from_str(include_str!(
        "fixtures/blockfrost/epoch_660_payment_parameters.json"
    ))?;
    let server = Fixture::start(vec![genesis(), genesis(), Reply::json(&parameters)]).await?;
    let client = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    let intent = cardano::intent(500_000)?;
    let observation = client
        .estimate_payment_fee(intent.clone(), EpochSelector::Number(660))
        .await?;
    observed(&observation, Operation::PaymentEstimate);
    assert_eq!(observation.value().intent(), &intent);
    assert_eq!(observation.value().parameters().data().epoch, 660);
    assert_eq!(observation.value().parameters().data().protocol_major, 11);
    let preparation = PaymentPreparation::new(intent, observation.value().parameters().clone())?;
    preparation.validate()?;
    let baseline = cardano::preparation()?;
    assert_eq!(preparation.unsigned_payload(), baseline.unsigned_payload());
    assert_eq!(
        preparation.estimate().minimum_fee(),
        baseline.estimate().minimum_fee()
    );
    assert_eq!(
        preparation.estimate().output_minimum_lovelaces(),
        baseline.estimate().output_minimum_lovelaces()
    );
    roundtrip(&observation)?;
    roundtrip(&preparation)?;
    let requests = server.requests()?;
    assert_eq!(requests.len(), 3);
    assert!(
        requests[2]
            .headers
            .starts_with("GET /api/v0/epochs/660/parameters ")
    );
    Ok(())
}

#[tokio::test]
async fn network_epoch_parameters_address_and_staking_operations_preserve_typed_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let address = json!({"address":cardano::address(1)?,"amount":fixture::units(10_000_000),"stake_address":null,"type":"shelley","script":false});
    let rewards =
        json!([{"epoch":599,"amount":"9007199254740993","pool_id":pool()?,"type":"member"}]);
    let mut replies = vec![genesis()];
    for value in [
        supply(),
        epoch(),
        fixture::params(),
        address,
        references(),
        account()?,
        rewards,
    ] {
        replies.extend([genesis(), Reply::json(&value)]);
    }
    let server = Fixture::start(replies).await?;
    let c = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    let page = PageRequest::new(2, 2, Order::Asc)?;
    let v = CardanoReader::get_network_data(&c).await?;
    observed(&v, Operation::NetworkData);
    assert_eq!(
        v.value().data().maximum.raw(),
        U256::from(45_000_000_000_000_000_u64)
    );
    roundtrip(&v)?;
    let v = c.get_epoch(EpochSelector::Number(600)).await?;
    observed(&v, Operation::Epoch);
    assert_eq!(v.value().data().active_stake, None);
    assert_eq!(
        v.value().data().output.raw(),
        U256::from(u64::MAX) + U256::from(1)
    );
    roundtrip(&v)?;
    let v = c.get_protocol_parameters(EpochSelector::Latest).await?;
    observed(&v, Operation::ProtocolParameters);
    assert_eq!(
        v.value()
            .data()
            .script_memory_price
            .as_ref()
            .unwrap()
            .canonical(),
        "0.0577"
    );
    assert_eq!(
        v.value()
            .data()
            .script_step_price
            .as_ref()
            .unwrap()
            .canonical(),
        "0.0000721"
    );
    roundtrip(&v)?;
    let v = c.get_address_details(cardano::address(1)?).await?;
    observed(&v, Operation::AddressDetails);
    assert_eq!(v.value().data().stake_address, None);
    roundtrip(&v)?;
    let v = c
        .get_address_transactions(cardano::address(1)?, page)
        .await?;
    observed(&v, Operation::AddressTransactions);
    assert_eq!(v.value().status(), PageStatus::ShortPage);
    roundtrip(&v)?;
    let v = c.get_stake_account(stake()?).await?;
    observed(&v, Operation::StakeAccount);
    assert_eq!(
        v.value().data().rewards.raw(),
        U256::from(u64::MAX) + U256::from(1)
    );
    roundtrip(&v)?;
    let v = c.get_stake_rewards(stake()?, page).await?;
    observed(&v, Operation::Rewards);
    assert_eq!(
        v.value().items()[0].amount.raw(),
        U256::from(9_007_199_254_740_993_u64)
    );
    roundtrip(&v)?;
    let requests = server.requests()?;
    assert_eq!(requests.len(), 15);
    assert!(
        requests
            .iter()
            .all(|r| r.headers.contains(fixture::CREDENTIAL))
    );
    assert!(requests[10].headers.starts_with(&format!(
        "GET /api/v0/addresses/{}/transactions?page=2&count=2&order=asc ",
        cardano::address(1)?
    )));
    assert!(requests[4].headers.starts_with("GET /api/v0/epochs/600 "));
    Ok(())
}

#[tokio::test]
async fn asset_catalogue_metadata_transactions_and_holders_are_exact_and_bounded()
-> Result<(), Box<dyn std::error::Error>> {
    let unit = format!("{}00ff", "03".repeat(28));
    let mut replies = vec![genesis()];
    for value in [
        json!([{"asset":unit,"quantity":"18446744073709551616"}]),
        asset()?,
        references(),
        json!([{"address":cardano::address(2)?,"quantity":"7"}]),
    ] {
        replies.extend([genesis(), Reply::json(&value)]);
    }
    let server = Fixture::start(replies).await?;
    let c = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    let page = PageRequest::new(1, 2, Order::Desc)?;
    let v = c.get_assets(page).await?;
    observed(&v, Operation::Assets);
    assert_eq!(v.value().items()[0].quantity.decimals(), None);
    roundtrip(&v)?;
    let v = c.get_asset(cardano::token()?).await?;
    observed(&v, Operation::AssetDetails);
    assert_eq!(
        v.value().data().onchain_metadata,
        MetadataAvailability::PresentUninterpreted
    );
    assert_eq!(
        v.value().data().metadata.as_ref().unwrap().name.as_str(),
        "\u{0008}coin"
    );
    roundtrip(&v)?;
    let v = c.get_asset_transactions(cardano::token()?, page).await?;
    observed(&v, Operation::AssetTransactions);
    roundtrip(&v)?;
    let v = c.get_asset_holders(cardano::token()?, page).await?;
    observed(&v, Operation::AssetHolders);
    assert_eq!(v.value().items()[0].quantity.raw(), U256::from(7));
    roundtrip(&v)?;
    assert_eq!(server.requests()?.len(), 9);
    assert!(server.requests()?[8].headers.starts_with(&format!(
        "GET /api/v0/assets/{unit}/addresses?page=1&count=2&order=desc "
    )));
    Ok(())
}

#[tokio::test]
async fn original_transaction_utxo_status_and_estimate_are_separate_typed_operations()
-> Result<(), Box<dyn std::error::Error>> {
    let signed = cardano::signed()?;
    let cbor = signed.transaction();
    let id = cbor.transaction_id();
    let summary = fixture::tx_summary(cbor);
    let payload = json!({"cbor":const_hex::encode(cbor.bytes())});
    let server = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::json(&summary.clone()),
        Reply::json(&payload.clone()),
        genesis(),
        Reply::json(&utxos(id)?),
        genesis(),
        Reply::json(&summary),
        Reply::json(&payload),
        genesis(),
        Reply::raw(404, "{\"private\":\"fixture-credential\"}".into()),
        genesis(),
        Reply::json(&fixture::params()),
    ])
    .await?;
    let c = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    let v = c.get_transaction(id).await?;
    observed(&v, Operation::Transaction);
    assert_eq!(v.value().cbor(), cbor);
    assert_eq!(v.value().data().fees.raw(), U256::from(500_000));
    roundtrip(&v)?;
    let v = c.get_transaction_utxos(id).await?;
    observed(&v, Operation::TransactionUtxos);
    assert_eq!(v.value().inputs()[0].reference, Some(false));
    assert_eq!(v.value().outputs()[0].consumed_by, None);
    roundtrip(&v)?;
    let v = c.get_transaction_status(id).await?;
    assert!(matches!(v.value(), TransactionStatus::Indexed { .. }));
    roundtrip(&v)?;
    let absent = Hash::from_bytes([9; 32]);
    let v = c.get_transaction_status(absent).await?;
    assert_eq!(
        v.value(),
        &TransactionStatus::NotIndexed {
            network: cardano::network()?,
            transaction_id: absent
        }
    );
    roundtrip(&v)?;
    let v = c
        .estimate_payment_fee(cardano::intent(500_000)?, EpochSelector::Number(600))
        .await?;
    observed(&v, Operation::PaymentEstimate);
    assert_eq!(v.value().intent().fee(), 500_000);
    roundtrip(&v)?;
    assert_eq!(server.requests()?.len(), 13);
    Ok(())
}

#[tokio::test]
async fn missing_payload_does_not_turn_known_indexed_transaction_into_not_indexed()
-> Result<(), Box<dyn std::error::Error>> {
    let signed = cardano::signed()?;
    let server = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::json(&fixture::tx_summary(signed.transaction())),
        Reply::raw(404, "{}".into()),
    ])
    .await?;
    let c = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    assert_eq!(
        c.get_transaction_status(signed.transaction().transaction_id())
            .await
            .unwrap_err(),
        Error::UnavailableData
    );
    assert_eq!(server.requests()?.len(), 4);
    Ok(())
}

#[tokio::test]
async fn wrong_genesis_or_local_network_fails_before_any_data_or_submission()
-> Result<(), Box<dyn std::error::Error>> {
    let server = Fixture::start(vec![
        genesis(),
        Reply::json(&json!({"network_magic":2,"system_start":1_506_203_091})),
    ])
    .await?;
    let c = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    assert_eq!(
        c.get_network_data().await.unwrap_err(),
        Error::Provider(ProviderError::ChainMismatch)
    );
    assert_eq!(server.requests()?.len(), 2);
    let mut raw = cardano::address(1)?.bytes().to_vec();
    raw[0] = 0x60;
    let wrong = regit_web3::domain::cardano::PaymentAddress::from_bytes(&raw)?;
    assert_eq!(
        c.get_address_details(wrong).await.unwrap_err(),
        Error::Validation(ValidationError::NetworkMismatch)
    );
    assert_eq!(server.requests()?.len(), 2);
    Ok(())
}

#[tokio::test]
async fn pages_reject_excess_duplicates_wrong_order_and_frozen_retries_keep_exact_request()
-> Result<(), Box<dyn std::error::Error>> {
    let page = PageRequest::new(7, 2, Order::Asc)?;
    let mut row = references()[0].clone();
    row["tx_hash"] = json!("06".repeat(32));
    row["tx_index"] = json!(0);
    let replies = vec![
        genesis(),
        genesis(),
        Reply::json(&json!([references()[0].clone(), row])),
        genesis(),
        Reply::json(&json!([references()[0].clone(), references()[0].clone()])),
        genesis(),
        Reply::json(&json!(vec![references()[0].clone(); 3])),
        genesis(),
        Reply::raw(503, "private-error-secret".into()),
        Reply::json(&references()),
    ];
    let server = Fixture::start(replies).await?;
    let c = BlockfrostClient::connect(fixture::config(
        &server.endpoint,
        1,
        Duration::from_secs(10),
        2 * 1024 * 1024,
    )?)
    .await?;
    for _ in 0..3 {
        assert_eq!(
            c.get_address_transactions(cardano::address(1)?, page)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    c.get_address_transactions(cardano::address(1)?, page)
        .await?;
    let r = server.requests()?;
    assert_eq!(r.len(), 10);
    assert_eq!(r[8].headers.lines().next(), r[9].headers.lines().next());
    assert_eq!(r[8].body, r[9].body);
    Ok(())
}

#[tokio::test]
async fn malformed_nullable_duplicate_exact_decimal_and_selected_epoch_fail_safely()
-> Result<(), Box<dyn std::error::Error>> {
    let mut wrong = fixture::params();
    wrong["epoch"] = json!(601);
    let mut missing = fixture::params();
    missing.as_object_mut().unwrap().remove("price_mem");
    let mut spoof = fixture::params();
    spoof["price_mem"] = json!({"$serde_json::private::Number":"0.0577"});
    let duplicate = fixture::params()
        .to_string()
        .replacen('{', "{\"epoch\":600,", 1);
    let mut replies = vec![genesis()];
    for reply in [
        Reply::json(&wrong),
        Reply::json(&missing),
        Reply::json(&spoof),
        Reply::raw(200, duplicate),
    ] {
        replies.extend([genesis(), reply]);
    }
    let server = Fixture::start(replies).await?;
    let c = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    for _ in 0..4 {
        let error = c
            .get_protocol_parameters(EpochSelector::Number(600))
            .await
            .unwrap_err();
        assert_eq!(error, invalid());
        assert!(!format!("{error:?} {error}").contains("private::Number"));
    }
    Ok(())
}

#[tokio::test]
async fn source_transaction_hash_paid_fee_validity_and_utxo_duplicates_are_correlated()
-> Result<(), Box<dyn std::error::Error>> {
    let signed = cardano::signed()?;
    let id = signed.transaction().transaction_id();
    let payload = json!({"cbor":const_hex::encode(signed.transaction().bytes())});
    let mut fee = fixture::tx_summary(signed.transaction());
    fee["fees"] = json!("500001");
    let mut validity = fixture::tx_summary(signed.transaction());
    validity["valid_contract"] = json!(false);
    let mut duplicate = utxos(id)?;
    duplicate["inputs"] = json!([
        duplicate["inputs"][0].clone(),
        duplicate["inputs"][0].clone()
    ]);
    let server = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::json(&fee),
        Reply::json(&payload.clone()),
        genesis(),
        Reply::json(&validity),
        Reply::json(&payload),
        genesis(),
        Reply::json(&duplicate),
    ])
    .await?;
    let c = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    assert_eq!(c.get_transaction(id).await.unwrap_err(), invalid());
    assert_eq!(c.get_transaction(id).await.unwrap_err(), invalid());
    assert_eq!(c.get_transaction_utxos(id).await.unwrap_err(), invalid());
    Ok(())
}

#[tokio::test]
async fn whole_operation_budget_covers_genesis_and_later_body_without_per_phase_reset()
-> Result<(), Box<dyn std::error::Error>> {
    let mut delayed_genesis = genesis();
    delayed_genesis.delay = Duration::from_secs(3);
    let mut delayed_data = Reply::json(&supply());
    delayed_data.delay = Duration::from_secs(3);
    let server = Fixture::start(vec![genesis(), delayed_genesis, delayed_data]).await?;
    let c = BlockfrostClient::connect(fixture::config(
        &server.endpoint,
        0,
        Duration::from_secs(5),
        2 * 1024 * 1024,
    )?)
    .await?;
    assert_eq!(c.get_network_data().await.unwrap_err(), Error::Timeout);
    assert_eq!(server.requests()?.len(), 3);
    Ok(())
}

#[tokio::test]
async fn configured_body_bound_is_enforced_before_json_mapping()
-> Result<(), Box<dyn std::error::Error>> {
    let server = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::raw(200, "x".repeat(1025)),
    ])
    .await?;
    let c = BlockfrostClient::connect(fixture::config(
        &server.endpoint,
        0,
        Duration::from_secs(10),
        1024,
    )?)
    .await?;
    assert_eq!(
        c.get_network_data().await.unwrap_err(),
        Error::Provider(ProviderError::ResponseTooLarge)
    );
    Ok(())
}

#[tokio::test]
async fn exact_raw_cbor_submission_is_once_and_acknowledgement_is_not_execution()
-> Result<(), Box<dyn std::error::Error>> {
    let signed = cardano::signed()?;
    let id = signed.transaction().transaction_id();
    let server = Fixture::start(vec![genesis(), genesis(), Reply::json(&json!(id))]).await?;
    let c = BlockfrostClient::connect(fixture::config(
        &server.endpoint,
        3,
        Duration::from_secs(10),
        2 * 1024 * 1024,
    )?)
    .await?;
    let v = CardanoSubmitter::submit_signed(&c, signed.clone()).await?;
    observed(&v, Operation::Submission);
    roundtrip(&v)?;
    assert_eq!(v.value().acknowledged_transaction(), id);
    let r = server.requests()?;
    assert_eq!(r.len(), 3);
    assert!(r[2].headers.starts_with("POST /api/v0/tx/submit "));
    assert!(
        r[2].headers
            .to_ascii_lowercase()
            .contains("content-type: application/cbor")
    );
    assert_eq!(r[2].body, signed.transaction().bytes());
    Ok(())
}

#[tokio::test]
async fn dispatched_rate_limit_mempool_full_http_error_malformed_and_wrong_ack_remain_ambiguous()
-> Result<(), Box<dyn std::error::Error>> {
    for (reply, cause) in [
        (
            Reply::raw(429, "secret-source-credential".into()),
            SubmissionFailure::RateLimited,
        ),
        (
            Reply::raw(425, "secret-source-credential".into()),
            SubmissionFailure::HttpStatus,
        ),
        (
            Reply::raw(500, "secret-source-credential".into()),
            SubmissionFailure::HttpStatus,
        ),
        (
            Reply::raw(200, "not-json-secret-source-credential".into()),
            SubmissionFailure::InvalidResponse,
        ),
        (
            Reply::json(&json!("09".repeat(32))),
            SubmissionFailure::InvalidResponse,
        ),
    ] {
        let server = Fixture::start(vec![genesis(), genesis(), reply]).await?;
        let c = BlockfrostClient::connect(fixture::config(
            &server.endpoint,
            3,
            Duration::from_secs(10),
            2 * 1024 * 1024,
        )?)
        .await?;
        let error = c.submit_signed(cardano::signed()?).await.unwrap_err();
        assert_eq!(error, Error::SubmissionOutcomeUnknown(cause));
        assert!(!format!("{error:?} {error}").contains("secret"));
        assert_eq!(server.requests()?.len(), 3);
    }
    Ok(())
}

#[tokio::test]
async fn submission_deadline_before_dispatch_is_safe_but_after_dispatch_retains_ambiguity()
-> Result<(), Box<dyn std::error::Error>> {
    let mut slow = genesis();
    slow.delay = Duration::from_secs(6);
    let server = Fixture::start(vec![genesis(), slow]).await?;
    let c = BlockfrostClient::connect(fixture::config(
        &server.endpoint,
        3,
        Duration::from_secs(5),
        2 * 1024 * 1024,
    )?)
    .await?;
    assert_eq!(
        c.submit_signed(cardano::signed()?).await.unwrap_err(),
        Error::Timeout
    );
    assert_eq!(server.requests()?.len(), 2);
    let mut slow = Reply::json(&json!(cardano::signed()?.transaction().transaction_id()));
    slow.delay = Duration::from_secs(6);
    let server = Fixture::start(vec![genesis(), genesis(), slow]).await?;
    let c = BlockfrostClient::connect(fixture::config(
        &server.endpoint,
        3,
        Duration::from_secs(5),
        2 * 1024 * 1024,
    )?)
    .await?;
    assert_eq!(
        c.submit_signed(cardano::signed()?).await.unwrap_err(),
        Error::SubmissionOutcomeUnknown(SubmissionFailure::Timeout)
    );
    assert_eq!(server.requests()?.len(), 3);
    Ok(())
}

#[tokio::test]
async fn native_asset_lookup_and_oversized_configuration_fail_without_remote_data_fetch()
-> Result<(), Box<dyn std::error::Error>> {
    let server = Fixture::start(vec![genesis()]).await?;
    let c = BlockfrostClient::connect(fixture::standard(&server.endpoint)?).await?;
    assert_eq!(
        c.get_asset(regit_web3::domain::cardano::AssetId::native(
            regit_web3::domain::cardano::NetworkId::mainnet()
        ))
        .await
        .unwrap_err(),
        Error::UnsupportedCapability
    );
    assert_eq!(server.requests()?.len(), 1);
    let huge = fixture::config(
        &server.endpoint,
        0,
        Duration::from_secs(5),
        2 * 1024 * 1024 + 1,
    )?;
    assert_eq!(
        BlockfrostClient::connect(huge).await.unwrap_err(),
        Error::Configuration
    );
    assert_eq!(server.requests()?.len(), 1);
    Ok(())
}
