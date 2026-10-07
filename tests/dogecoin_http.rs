// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Actual public Rust indexed APIs over deterministic offline loopback fixtures.
#![cfg(feature = "dogecoin-http")]
#[path = "support/market_server.rs"]
mod market_server;
use market_server::{Fixture, Reply};
use regit_web3::{
    chains::dogecoin::{BlockCypherClient, BlockCypherConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::dogecoin::*,
    error::{Error, ProviderError, ValidationError},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{fmt::Debug, time::Duration};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const ADDRESS: &str = "DLAznsPDLDRgsVcTFWRMYMG5uH6GddDtv8";
const TXID: &str = "5f7e779f7600f54e528686e91d5891f3ae226ee907f461692519e549105f521c";
const GENESIS: &str = include_str!("fixtures/dogecoin/genesis.json");
const BALANCE: &str = include_str!("fixtures/dogecoin/balance.json");
const HISTORY: &str = include_str!("fixtures/dogecoin/history.json");
const CHAIN: &str = include_str!("fixtures/dogecoin/chain.json");
const TRANSACTION: &str = include_str!("fixtures/dogecoin/transaction.json");
fn address() -> Result<Address, Error> {
    Address::parse(ADDRESS, Network::Mainnet)
}
fn txid() -> Result<Txid, Error> {
    Txid::parse(TXID)
}
fn bad_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn config(
    url: &str,
    bytes: usize,
    retries: u8,
    timeout: Duration,
) -> Result<BlockCypherConfig, Error> {
    BlockCypherConfig::new(
        NetworkId::new(Network::Mainnet, "mainnet")?,
        HttpConfig::new(
            RpcEndpoint::new(url)?.with_header("x-fixture-secret", "private-fixture-key")?,
            RpcLimits::new(timeout, timeout, bytes, retries)?,
            "blockcypher-fixture",
        )?,
    )
}
async fn client(url: &str) -> Result<BlockCypherClient, Error> {
    BlockCypherClient::connect(config(url, 1024 * 1024, 0, Duration::from_secs(3))?).await
}
fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(
    value: &T,
) -> Result<(), serde_json::Error> {
    assert_eq!(
        &serde_json::from_str::<T>(&serde_json::to_string(value)?)?,
        value
    );
    Ok(())
}
async fn transaction_result(
    body: &str,
) -> Result<Result<Observation<Transaction>, Error>, Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::json(body),
    ])
    .await?;
    Ok(client(&fixture.endpoint)
        .await?
        .get_transaction(txid()?, 20)
        .await)
}
async fn history_result(
    body: &str,
    request: HistoryRequest,
) -> Result<Result<Observation<HistoryPage>, Error>, Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::json(body),
    ])
    .await?;
    Ok(client(&fixture.endpoint)
        .await?
        .get_address_history(address()?, request)
        .await)
}
#[tokio::test(flavor = "current_thread")]
async fn all_five_reads_preserve_actual_family_source_and_exact_indexed_fields() -> TestResult {
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::json(BALANCE),
        Reply::json(GENESIS),
        Reply::json(HISTORY),
        Reply::json(GENESIS),
        Reply::json(CHAIN),
        Reply::json(GENESIS),
        Reply::json(TRANSACTION),
        Reply::json(GENESIS),
        Reply::json(TRANSACTION),
    ])
    .await?;
    let reader = client(&fixture.endpoint).await?;
    let balance = reader.get_address_balance(address()?).await?;
    assert_eq!(balance.value().data().confirmed.raw(), 5_656_308_500);
    assert_eq!(
        balance.context().network().genesis_hash(),
        &Network::Mainnet.genesis_hash()
    );
    assert_eq!(
        balance.context().source().provider_id(),
        "blockcypher-fixture"
    );
    assert_eq!(
        balance.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    roundtrip(&balance)?;
    let history = reader
        .get_address_history(address()?, HistoryRequest::new(None, 10, 100)?)
        .await?;
    assert_eq!(history.value().confirmed().len(), 10);
    assert_eq!(history.value().has_more(), Some(true));
    assert!(history.value().next_before_height().is_some());
    assert!(history.value().confirmed()[0].double_spend);
    roundtrip(&history)?;
    let fees = reader.get_fee_estimates().await?;
    assert_eq!(fees.value().high_per_kilobyte.raw(), 221_771_683);
    assert!(fees.value().tip_height > 0);
    roundtrip(&fees)?;
    let transaction = reader.get_transaction(txid()?, 20).await?;
    assert_eq!(transaction.value().data().inputs.len(), 1);
    assert_eq!(transaction.value().data().outputs.len(), 1);
    assert_eq!(transaction.value().data().status.txid(), txid()?);
    assert_eq!(
        transaction.value().data().total_output.raw(),
        6_841_600_000_000
    );
    assert_eq!(
        transaction
            .value()
            .data()
            .raw
            .as_ref()
            .ok_or_else(bad_response)?
            .as_slice()
            .len(),
        109
    );
    assert!(transaction.value().data().lock_time.is_none());
    assert!(transaction.value().derived_fee().is_none());
    assert!(transaction.value().data().outputs[0].spent_by.is_some());
    roundtrip(&transaction)?;
    let status = reader.get_transaction_status(txid()?).await?;
    assert_eq!(
        status.value().inclusion().ok_or_else(bad_response)?.height,
        1
    );
    assert!(status.value().confirmations() > 0);
    roundtrip(&status)?;
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 11);
    assert!(requests[0].starts_with("GET /api/v3/blocks/0?limit=1 "));
    assert!(requests[4].contains("?limit=10&includeScript=true "));
    assert!(requests[8].contains("?limit=20&includeHex=true "));
    assert!(requests[10].contains("?limit=1 "));
    assert!(!format!("{reader:?}").contains("private-fixture-key"));
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn expected_genesis_and_chain_name_are_verified_per_read_with_local_network_preflight()
-> TestResult {
    let mut wrong: Value = serde_json::from_str(GENESIS)?;
    wrong["hash"] = json!(Network::Testnet.genesis_hash());
    let fixture =
        Fixture::start(vec![Reply::json(GENESIS), Reply::json(&wrong.to_string())]).await?;
    let reader = client(&fixture.endpoint).await?;
    assert_eq!(
        reader.get_address_balance(address()?).await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    assert_eq!(fixture.requests()?.len(), 2);
    let test = NetworkId::new(Network::Testnet, "testnet")?;
    let http = HttpConfig::new(
        RpcEndpoint::new(&fixture.endpoint)?,
        RpcLimits::new(
            Duration::from_secs(3),
            Duration::from_secs(3),
            1024 * 1024,
            0,
        )?,
        "fixture",
    )?;
    assert!(matches!(
        BlockCypherConfig::new(test, http),
        Err(Error::Configuration)
    ));
    let fixture = Fixture::start(vec![Reply::json(GENESIS)]).await?;
    let reader = client(&fixture.endpoint).await?;
    let mut bytes = [7; 21];
    bytes[0] = 113;
    let wrong_address = Address::parse(
        &bs58::encode(bytes).with_check().into_string(),
        Network::Testnet,
    )?;
    assert_eq!(
        reader.get_address_balance(wrong_address).await,
        Err(Error::Validation(ValidationError::NetworkMismatch))
    );
    assert!(reader.get_transaction(txid()?, 0).await.is_err());
    assert_eq!(fixture.requests()?.len(), 1);
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn exact_balance_arithmetic_above_float_precision_and_negative_unconfirmed_delta_survive()
-> TestResult {
    let mut body: Value = serde_json::from_str(BALANCE)?;
    body["total_received"] = json!(9_007_199_254_740_993_u64);
    body["total_sent"] = json!(0);
    body["balance"] = json!(9_007_199_254_740_993_u64);
    body["unconfirmed_balance"] = json!(-1);
    body["final_balance"] = json!(9_007_199_254_740_992_u64);
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::json(&body.to_string()),
    ])
    .await?;
    let value = client(&fixture.endpoint)
        .await?
        .get_address_balance(address()?)
        .await?;
    assert_eq!(value.value().data().confirmed.raw(), 9_007_199_254_740_993);
    assert_eq!(value.value().data().unconfirmed.raw(), -1);
    roundtrip(&value)?;
    for field in ["balance", "final_balance", "final_n_tx"] {
        let mut malformed = body.clone();
        malformed[field] = json!(999);
        let fixture = Fixture::start(vec![
            Reply::json(GENESIS),
            Reply::json(GENESIS),
            Reply::json(&malformed.to_string()),
        ])
        .await?;
        assert_eq!(
            client(&fixture.endpoint)
                .await?
                .get_address_balance(address()?)
                .await,
            Err(bad_response())
        );
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn history_boundary_blocks_continuations_duplicates_and_capacity_are_checked_without_truncation()
-> TestResult {
    let mut body: Value = serde_json::from_str(HISTORY)?;
    let first = body["txrefs"][0].clone();
    let mut second = first.clone();
    second["tx_output_n"] = json!(1);
    body["txrefs"] = json!([first.clone(), second]);
    let value = history_result(&body.to_string(), HistoryRequest::new(None, 1, 2)?).await??;
    assert_eq!(value.value().confirmed().len(), 2);
    let before = value
        .value()
        .next_before_height()
        .ok_or_else(bad_response)?;
    assert_eq!(
        history_result(&body.to_string(), HistoryRequest::new(None, 1, 1)?).await?,
        Err(bad_response())
    );
    assert_eq!(
        history_result(&body.to_string(), HistoryRequest::new(Some(before), 1, 2)?).await?,
        Err(bad_response())
    );
    body["txrefs"] = json!([first.clone(), first]);
    assert_eq!(
        history_result(&body.to_string(), HistoryRequest::new(None, 1, 2)?).await?,
        Err(bad_response())
    );
    body["txrefs"] = json!([]);
    body["hasMore"] = json!(true);
    assert_eq!(
        history_result(&body.to_string(), HistoryRequest::new(None, 1, 2)?).await?,
        Err(bad_response())
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn full_transactions_reject_incomplete_arrays_remote_continuation_wrong_id_and_size()
-> TestResult {
    for key in ["vin_sz", "vout_sz"] {
        let mut body: Value = serde_json::from_str(TRANSACTION)?;
        body[key] = json!(2);
        assert_eq!(
            transaction_result(&body.to_string()).await?,
            Err(Error::UnavailableData)
        );
    }
    let mut body: Value = serde_json::from_str(TRANSACTION)?;
    body["next_outputs"] = json!("https://private-host.invalid/next?token=private-token");
    assert_eq!(
        transaction_result(&body.to_string()).await?,
        Err(Error::UnavailableData)
    );
    let mut body: Value = serde_json::from_str(TRANSACTION)?;
    body["hash"] = json!(Txid::from_display_bytes([3; 32]));
    assert_eq!(
        transaction_result(&body.to_string()).await?,
        Err(bad_response())
    );
    let mut body: Value = serde_json::from_str(TRANSACTION)?;
    body["size"] = json!(999);
    assert_eq!(
        transaction_result(&body.to_string()).await?,
        Err(bad_response())
    );
    let duplicate = TRANSACTION.replacen('{', &format!("{{\"hash\":\"{TXID}\","), 1);
    assert_eq!(transaction_result(&duplicate).await?, Err(bad_response()));
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn nullable_source_fields_and_opaque_family_payloads_remain_explicit() -> TestResult {
    let mut body: Value = serde_json::from_str(TRANSACTION)?;
    body.as_object_mut().ok_or_else(bad_response)?.remove("hex");
    body.as_object_mut()
        .ok_or_else(bad_response)?
        .remove("vsize");
    body["fees"] = Value::Null;
    let value = transaction_result(&body.to_string()).await??;
    assert!(value.value().data().raw.is_none());
    assert!(value.value().data().virtual_size.is_none());
    assert!(value.value().data().reported_fee.is_none());
    body["inputs"][0]["witness"] = json!(["aa"]);
    assert!(transaction_result(&body.to_string()).await?.is_err());
    body["inputs"][0]["output_index"] = json!(-2);
    assert_eq!(
        transaction_result(&body.to_string()).await?,
        Err(bad_response())
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn known_previous_output_lower_bound_and_family_money_limits_reject_impossible_indexed_fees()
-> TestResult {
    let mut body: Value = serde_json::from_str(TRANSACTION)?;
    body["hex"] = Value::Null;
    body["total"] = json!(5);
    body["outputs"][0]["value"] = json!(5);
    body["fees"] = json!(1);
    body["vin_sz"] = json!(2);
    body["inputs"] = json!([{"prev_hash":Txid::from_display_bytes([2;32]),"output_index":0,"output_value":10,"sequence":4_294_967_295_u32}, {"prev_hash":Txid::from_display_bytes([3;32]),"output_index":0}]);
    assert_eq!(
        transaction_result(&body.to_string()).await?,
        Err(bad_response())
    );
    body["inputs"][0]["output_value"] = json!(6);
    let value = transaction_result(&body.to_string()).await??;
    assert!(value.value().derived_fee().is_none());
    roundtrip(&value)?;
    body["outputs"][0]["value"] = json!(1_000_000_000_000_000_000_u64 + 1);
    body["total"] = json!(1_000_000_000_000_000_000_u64 + 1);
    assert_eq!(
        transaction_result(&body.to_string()).await?,
        Err(bad_response())
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn safe_read_retries_freeze_queries_and_unknown_transactions_are_unavailable() -> TestResult {
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::status(503),
        Reply::json(HISTORY),
    ])
    .await?;
    let reader = BlockCypherClient::connect(config(
        &fixture.endpoint,
        1024 * 1024,
        1,
        Duration::from_secs(3),
    )?)
    .await?;
    let value = reader
        .get_address_history(address()?, HistoryRequest::new(None, 10, 100)?)
        .await?;
    assert_eq!(value.value().confirmed().len(), 10);
    let requests = fixture.requests()?;
    assert_eq!(requests[2], requests[3]);
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::status(404),
    ])
    .await?;
    assert_eq!(
        client(&fixture.endpoint)
            .await?
            .get_transaction_status(txid()?)
            .await,
        Err(Error::UnavailableData)
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn genesis_and_resource_reads_share_one_deadline_and_error_bodies_remain_private()
-> TestResult {
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply {
            status: 200,
            body: GENESIS.to_owned(),
            delay: Duration::from_millis(1200),
        },
        Reply {
            status: 200,
            body: BALANCE.to_owned(),
            delay: Duration::from_millis(1200),
        },
    ])
    .await?;
    let reader = BlockCypherClient::connect(config(
        &fixture.endpoint,
        1024 * 1024,
        0,
        Duration::from_secs(2),
    )?)
    .await?;
    assert_eq!(
        reader.get_address_balance(address()?).await,
        Err(Error::Timeout)
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert!(requests[2].starts_with(&format!("GET /api/v3/addrs/{ADDRESS}/balance ")));

    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::status(429),
    ])
    .await?;
    let error = client(&fixture.endpoint)
        .await?
        .get_address_balance(address()?)
        .await
        .err()
        .ok_or_else(bad_response)?;
    assert!(!format!("{error:?} {error}").contains("private-provider-body"));
    assert_eq!(error, Error::Provider(ProviderError::RateLimited));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn missing_known_history_and_bounded_body_nested_addresses_and_source_chain_fail_explicitly()
-> TestResult {
    let mut body: Value = serde_json::from_str(HISTORY)?;
    body.as_object_mut()
        .ok_or_else(bad_response)?
        .remove("txrefs");
    assert_eq!(
        history_result(&body.to_string(), HistoryRequest::new(None, 10, 100)?).await?,
        Err(Error::UnavailableData)
    );
    let mut body: Value = serde_json::from_str(TRANSACTION)?;
    body["outputs"][0]["addresses"] = json!(vec![ADDRESS; 101]);
    assert_eq!(
        transaction_result(&body.to_string()).await?,
        Err(bad_response())
    );
    let mut chain: Value = serde_json::from_str(CHAIN)?;
    chain["name"] = json!("BTC.main");
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::json(&chain.to_string()),
    ])
    .await?;
    assert_eq!(
        client(&fixture.endpoint).await?.get_fee_estimates().await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::json(&"x".repeat(4097)),
    ])
    .await?;
    let reader =
        BlockCypherClient::connect(config(&fixture.endpoint, 4096, 0, Duration::from_secs(3))?)
            .await?;
    assert_eq!(
        reader.get_address_balance(address()?).await,
        Err(Error::Provider(ProviderError::ResponseTooLarge))
    );
    Ok(())
}

async fn status_result(
    body: &str,
) -> Result<Result<Observation<TransactionStatus>, Error>, Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        Reply::json(GENESIS),
        Reply::json(GENESIS),
        Reply::json(body),
    ])
    .await?;
    Ok(client(&fixture.endpoint)
        .await?
        .get_transaction_status(txid()?)
        .await)
}
#[tokio::test(flavor = "current_thread")]
async fn explicit_pending_height_is_required_in_status_and_complete_transaction_resources()
-> TestResult {
    let mut body: Value = serde_json::from_str(TRANSACTION)?;
    body["block_height"] = json!(-1);
    body["confirmations"] = json!(0);
    for key in ["block_hash", "block_index", "confirmed"] {
        body.as_object_mut().ok_or_else(bad_response)?.remove(key);
    }
    let status = status_result(&body.to_string()).await??;
    assert!(status.value().inclusion().is_none());
    assert_eq!(status.value().confirmations(), 0);
    let transaction = transaction_result(&body.to_string()).await??;
    assert!(transaction.value().data().status.inclusion().is_none());
    roundtrip(&status)?;
    roundtrip(&transaction)?;
    for omitted in [true, false] {
        let mut invalid = body.clone();
        if omitted {
            invalid
                .as_object_mut()
                .ok_or_else(bad_response)?
                .remove("block_height");
        } else {
            invalid["block_height"] = Value::Null;
        }
        assert_eq!(
            status_result(&invalid.to_string()).await?,
            Err(bad_response())
        );
        assert_eq!(
            transaction_result(&invalid.to_string()).await?,
            Err(bad_response())
        );
    }
    body["block_height"] = json!(-2);
    assert_eq!(status_result(&body.to_string()).await?, Err(bad_response()));
    body["block_height"] = json!(9_223_372_036_854_775_808_u64);
    assert_eq!(
        transaction_result(&body.to_string()).await?,
        Err(bad_response())
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn documented_coinbase_omission_and_explicit_sentinel_remain_source_only_with_mixed_null_rejection()
-> TestResult {
    let mut body: Value = serde_json::from_str(TRANSACTION)?;
    body["inputs"][0]
        .as_object_mut()
        .ok_or_else(bad_response)?
        .remove("output_index");
    let value = transaction_result(&body.to_string()).await??;
    assert!(value.value().data().inputs[0].previous_output.is_none());
    assert_eq!(
        value.value().data().inputs[0].coinbase_source,
        Some(CoinbaseSource::OmittedPrevoutFields)
    );
    assert!(value.value().derived_fee().is_none());
    roundtrip(&value)?;
    let mut sentinel = body.clone();
    sentinel["inputs"][0]["output_index"] = json!(-1);
    sentinel["inputs"][0]["prev_hash"] = json!(Txid::from_display_bytes([0; 32]));
    let value = transaction_result(&sentinel.to_string()).await??;
    assert_eq!(
        value.value().data().inputs[0].coinbase_source,
        Some(CoinbaseSource::ExplicitSentinel)
    );
    for key in ["prev_hash", "output_index", "output_value"] {
        let mut null = body.clone();
        null["inputs"][0][key] = Value::Null;
        assert_eq!(
            transaction_result(&null.to_string()).await?,
            Err(bad_response())
        );
    }
    let mut mixed = body.clone();
    mixed["inputs"][0]["prev_hash"] = json!(Txid::from_display_bytes([2; 32]));
    assert_eq!(
        transaction_result(&mixed.to_string()).await?,
        Err(bad_response())
    );
    let mut mixed = body.clone();
    mixed["inputs"][0]["output_index"] = json!(0);
    assert_eq!(
        transaction_result(&mixed.to_string()).await?,
        Err(bad_response())
    );
    let mut mixed = body.clone();
    mixed["inputs"][0]["output_value"] = json!(0);
    assert_eq!(
        transaction_result(&mixed.to_string()).await?,
        Err(bad_response())
    );
    sentinel["inputs"][0]["prev_hash"] = json!(Txid::from_display_bytes([2; 32]));
    assert_eq!(
        transaction_result(&sentinel.to_string()).await?,
        Err(bad_response())
    );
    Ok(())
}
