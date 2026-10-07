// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! All eight public `THORNode` operations and bounded failures over offline loopback.
#![cfg(feature = "thorchain-http")]
#[path = "support/market_server.rs"]
mod market_server;
use market_server::{Fixture, Reply};
use regit_web3::{
    chains::thorchain::{ThorchainClient, ThorchainHttpConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        U256,
        thorchain::{
            AccountPrefix, Address, Asset, BasisPoints, Chain, ChainAddress, CollectionLimit,
            Network, Observation, ProtocolAmount, QuoteInputResolution, StreamingParameters,
            SwapParameters, SwapQuote, SwapRequest, TransactionStatus, Txid,
        },
    },
    error::{Error, ProviderError, ValidationError},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{fmt::Debug, time::Duration};
const ACCOUNT: &str = "thor1dheycdevq39qlkxs2a6wuuzyn4aqxhve4qxtxt";
const TXID: &str = "A3F81568387CD3880AED812780799E8F6D3F970E071F7B1861B581B20399F21F";
const INFO: &str = include_str!("fixtures/thorchain/nodeinfo.json");
const POOL: &str = include_str!("fixtures/thorchain/pool.json");
const INTERNAL: &str = include_str!("fixtures/thorchain/internal_status.json");
const STATUS: &str = include_str!("fixtures/thorchain/external_status.json");
fn config(
    url: &str,
    bytes: usize,
    retries: u8,
    timeout: Duration,
) -> Result<ThorchainHttpConfig, Error> {
    Ok(ThorchainHttpConfig::new(
        Network::new("thorchain-1", AccountPrefix::Thor, "mainnet")?,
        HttpConfig::new(
            RpcEndpoint::new(url)?.with_header("x-fixture-secret", "private-fixture-token")?,
            RpcLimits::new(timeout, timeout, bytes, retries)?,
            "thor-fixture",
        )?,
    ))
}
async fn client(url: &str) -> Result<ThorchainClient, Error> {
    ThorchainClient::connect(config(url, 1024 * 1024, 0, Duration::from_secs(3))?).await
}
fn request() -> Result<SwapRequest, Error> {
    SwapRequest::new(
        Asset::parse("BTC.BTC")?,
        Asset::parse("ETH.ETH")?,
        ProtocolAmount::from_decimal("100000000")?,
        SwapParameters::default(),
    )
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn updated_quote() -> Result<String, serde_json::Error> {
    let mut v: Value = serde_json::from_str(include_str!("fixtures/thorchain/quote.json"))?;
    v["expiry"] = json!(9_999_999_999u64);
    serde_json::to_string(&v)
}
fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(
    value: &T,
) -> Result<(), serde_json::Error> {
    let wire = serde_json::to_string(value)?;
    assert_eq!(&serde_json::from_str::<T>(&wire)?, value);
    Ok(())
}
fn assert_quote(v: &Observation<SwapQuote>) -> Result<(), Box<dyn std::error::Error>> {
    let data = v.value().data();
    assert_eq!(data.input_resolution, QuoteInputResolution::Unreported);
    assert_eq!(data.fees.total.raw().to_string(), "6496742");
    assert_eq!(data.fees.asset.to_string(), "ETH.ETH");
    assert_eq!(
        data.dust_threshold.ok_or_else(invalid)?.raw().to_string(),
        "1000"
    );
    assert_eq!(
        data.recommended_min_amount_in
            .ok_or_else(invalid)?
            .raw()
            .to_string(),
        "6309"
    );
    assert_eq!(
        data.gas_rate_units.as_ref().ok_or_else(invalid)?.as_str(),
        "satsperbyte"
    );
    assert!(data.memo.is_none());
    roundtrip(v)?;
    Ok(())
}
fn assert_status(v: &Observation<TransactionStatus>) -> Result<(), Box<dyn std::error::Error>> {
    let value = v.value();
    let tx = value.transaction().ok_or_else(invalid)?;
    assert_eq!(tx.data().chain.as_str(), "ETH");
    assert_eq!(
        tx.data().coins.as_ref().ok_or_else(invalid)?[0].decimals,
        Some(6)
    );
    assert_eq!(
        value.planned_outbounds().ok_or_else(invalid)?[0]
            .data()
            .chain
            .as_str(),
        "BTC"
    );
    assert_eq!(
        value.outbounds().ok_or_else(invalid)?[0].data().id.as_str(),
        "16487E76DB13E0070F6259EB6F2A8366857E264DD93BFDA60A36213E974316B8"
    );
    assert!(
        value
            .stages()
            .outbound_signed
            .as_ref()
            .ok_or_else(invalid)?
            .completed
    );
    assert_eq!(v.context().network().chain_id(), "thorchain-1");
    assert_eq!(v.context().source().provider_id(), "thor-fixture");
    roundtrip(v)?;
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn all_eight_reads_preserve_exact_units_network_source_and_cross_chain_state()
-> Result<(), Box<dyn std::error::Error>> {
    let quote = updated_quote()?;
    let bodies = [
        include_str!("fixtures/thorchain/balance.json"),
        POOL,
        include_str!("fixtures/thorchain/pools.json"),
        include_str!("fixtures/thorchain/network.json"),
        &quote,
        include_str!("fixtures/thorchain/inbounds.json"),
        include_str!("fixtures/thorchain/lastblocks.json"),
        STATUS,
    ];
    let mut replies = vec![Reply::json(INFO)];
    for body in bodies {
        replies.push(Reply::json(INFO));
        replies.push(Reply::json(body));
    }
    let fixture = Fixture::start(replies).await?;
    let c = client(&fixture.endpoint).await?;
    let limit = CollectionLimit::new(100)?;
    let v = c.get_rune_balance(Address::parse(ACCOUNT)?).await?;
    assert_eq!(v.value().amount().raw().to_string(), "2532117741560096");
    assert_eq!(v.value().amount().amount().decimals(), Some(8));
    roundtrip(&v)?;
    let v = c.get_pool(Asset::parse("BTC.BTC")?).await?;
    assert_eq!(
        v.value().data().pool_units.raw().to_string(),
        "10169555322973480759703440"
    );
    assert!(v.value().data().decimals.is_none());
    roundtrip(&v)?;
    let v = c.get_pools(limit).await?;
    assert_eq!(v.value().items().len(), 1);
    roundtrip(&v)?;
    let v = c.get_network().await?;
    assert_eq!(
        v.value().total_reserve.raw().to_string(),
        "2532117713238871"
    );
    assert_eq!(v.value().native_tx_fee_rune.raw().to_string(), "2000000");
    roundtrip(&v)?;
    let v = c.get_swap_quote(request()?).await?;
    assert_quote(&v)?;
    let v = c.get_inbound_addresses(limit).await?;
    assert_eq!(v.value().items().len(), 2);
    assert!(
        v.value()
            .items()
            .iter()
            .any(|v| v.data().chain.as_str() == "ETH"
                && v.data().dust_threshold.raw() == U256::from(1000)
                && v.data().gas_rate_units.as_str() == "gwei")
    );
    roundtrip(&v)?;
    let v = c.get_last_blocks(limit).await?;
    assert_eq!(v.value().items().len(), 2);
    assert!(
        v.value()
            .items()
            .iter()
            .any(|v| v.last_observed_in() != v.thorchain())
    );
    roundtrip(&v)?;
    let v = c.get_transaction_status(Txid::parse(TXID)?, limit).await?;
    assert_status(&v)?;
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 17);
    assert!(requests.iter().all(|v|v.starts_with("GET ") && v.contains("x-fixture-secret: private-fixture-token")));
    assert_eq!(
        requests
            .iter()
            .filter(|v| v.contains("/cosmos/base/tendermint/v1beta1/node_info "))
            .count(),
        9
    );
    assert!(requests[2].contains(&format!(
        "/cosmos/bank/v1beta1/balances/{ACCOUNT}/by_denom?denom=rune "
    )));
    assert!(
        requests[10].contains(
            "/thorchain/quote/swap?from_asset=BTC.BTC&to_asset=ETH.ETH&amount=100000000 "
        )
    );
    for diagnostic in [format!("{c:?}"), format!("{:?}", c.config())] {
        assert!(!diagnostic.contains("private-fixture-token"));
        assert!(!diagnostic.contains(&fixture.endpoint));
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn absent_balances_are_unavailable_and_explicit_zero_is_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    for (body, expected) in [
        (r#"{"balance":null}"#, Some(Error::UnavailableData)),
        ("{}", Some(Error::UnavailableData)),
        (
            r#"{"balance":{"denom":"btc","amount":"0"}}"#,
            Some(invalid()),
        ),
        (
            r#"{"balance":{"denom":"rune","amount":"00"}}"#,
            Some(invalid()),
        ),
        (r#"{"balance":{"denom":"rune","amount":"0"}}"#, None),
    ] {
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::json(body),
        ])
        .await?;
        let c = client(&fixture.endpoint).await?;
        let value = c.get_rune_balance(Address::parse(ACCOUNT)?).await;
        if let Some(error) = expected {
            assert_eq!(value.unwrap_err(), error);
        } else {
            assert_eq!(value?.value().amount().raw(), U256::ZERO);
        }
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn expected_network_is_rechecked_per_read_and_mismatch_prevents_data_request()
-> Result<(), Box<dyn std::error::Error>> {
    let wrong = r#"{"default_node_info":{"network":"thorchain-other"}}"#;
    let fixture = Fixture::start(vec![Reply::json(INFO), Reply::json(wrong)]).await?;
    let c = client(&fixture.endpoint).await?;
    assert_eq!(
        c.get_network().await.unwrap_err(),
        Error::Provider(ProviderError::ChainMismatch)
    );
    assert_eq!(fixture.requests()?.len(), 2);
    let fixture = Fixture::start(vec![Reply::json(wrong)]).await?;
    assert_eq!(
        client(&fixture.endpoint).await.unwrap_err(),
        Error::Provider(ProviderError::ChainMismatch)
    );
    let fixture = Fixture::start(vec![Reply::json(INFO)]).await?;
    let c = client(&fixture.endpoint).await?;
    let stage = bech32::encode::<bech32::Bech32>(bech32::Hrp::parse("sthor")?, &[7u8; 20])?;
    assert_eq!(
        c.get_rune_balance(Address::parse(&stage)?)
            .await
            .unwrap_err(),
        Error::Validation(ValidationError::NetworkMismatch)
    );
    assert_eq!(fixture.requests()?.len(), 1);
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn quote_query_uses_exact_snake_case_settings_and_frozen_retry_inputs()
-> Result<(), Box<dyn std::error::Error>> {
    for (tolerance, liquidity, expected) in [
        (Some(10_000), None, "tolerance_bps=10000"),
        (None, Some(9999), "liquidity_tolerance_bps=9999"),
    ] {
        let quote = updated_quote()?;
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::status(503),
            Reply::json(&quote),
        ])
        .await?;
        let c = ThorchainClient::connect(config(
            &fixture.endpoint,
            1024 * 1024,
            1,
            Duration::from_secs(3),
        )?)
        .await?;
        let req = SwapRequest::new(
            Asset::parse("BTC.BTC")?,
            Asset::parse("ETH.ETH")?,
            ProtocolAmount::from_decimal("9007199254740993")?,
            SwapParameters {
                destination: Some(ChainAddress::new(
                    Chain::parse("ETH")?,
                    "0xPublicDestination",
                )?),
                refund_address: Some(ChainAddress::new(Chain::parse("BTC")?, "bc1qpublicrefund")?),
                streaming: Some(StreamingParameters {
                    interval: 0,
                    quantity: 0,
                }),
                tolerance_bps: tolerance.map(BasisPoints::new).transpose()?,
                liquidity_tolerance_bps: liquidity.map(BasisPoints::new).transpose()?,
            },
        )?;
        let result = c.get_swap_quote(req.clone()).await?;
        assert_eq!(result.value().request(), &req);
        let requests = fixture.requests()?;
        assert_eq!(requests[2], requests[3]);
        assert!(requests[2].contains(&format!("amount=9007199254740993&destination=0xPublicDestination&refund_address=bc1qpublicrefund&streaming_interval=0&streaming_quantity=0&{expected} ")));
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn quote_expiry_missing_facts_duplicate_fields_and_wrong_fees_fail_without_refresh()
-> Result<(), Box<dyn std::error::Error>> {
    let quote = updated_quote()?;
    let base: Value = serde_json::from_str(&quote)?;
    let mut bodies = vec![(
        include_str!("fixtures/thorchain/quote.json").to_owned(),
        Error::UnavailableData,
    )];
    let mut v = base.clone();
    v["fees"]["asset"] = json!("BTC.BTC");
    bodies.push((serde_json::to_string(&v)?, invalid()));
    let mut v = base.clone();
    v["fees"]["total"] = json!("1");
    bodies.push((serde_json::to_string(&v)?, invalid()));
    let mut v = base.clone();
    v.as_object_mut().unwrap().remove("expiry");
    bodies.push((serde_json::to_string(&v)?, invalid()));
    let mut v = base.clone();
    v["recommended_gas_rate"] = json!(null);
    bodies.push((serde_json::to_string(&v)?, invalid()));
    bodies.push((
        quote.replacen("\"expiry\":", "\"expiry\":9999999999,\"expiry\":", 1),
        invalid(),
    ));
    for (body, error) in bodies {
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::json(&body),
        ])
        .await?;
        let c = client(&fixture.endpoint).await?;
        assert_eq!(c.get_swap_quote(request()?).await.unwrap_err(), error);
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn pool_collection_rejects_duplicate_wrong_identity_sum_and_item_excess()
-> Result<(), Box<dyn std::error::Error>> {
    let mut malformed: Value = serde_json::from_str(POOL)?;
    malformed["pool_units"] = json!("1");
    let malformed = serde_json::to_string(&malformed)?;
    for body in [
        malformed,
        POOL.replace("BTC.BTC", "ETH.ETH"),
        POOL.replacen("\"asset\":", "\"asset\":\"BTC.BTC\",\"asset\":", 1),
    ] {
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::json(&body),
        ])
        .await?;
        assert_eq!(
            client(&fixture.endpoint)
                .await?
                .get_pool(Asset::parse("BTC.BTC")?)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    let duplicate = format!("[{POOL},{POOL}]");
    for limit in [1, 100] {
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::json(&duplicate),
        ])
        .await?;
        assert_eq!(
            client(&fixture.endpoint)
                .await?
                .get_pools(CollectionLimit::new(limit)?)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn internal_null_gas_blank_outbound_and_unseen_stage_absence_are_not_synthesized()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(INTERNAL),
    ])
    .await?;
    let c = client(&fixture.endpoint).await?;
    let txid = Txid::parse("A487E167659E0FC3681B60D1AEABFAB180D6C6917F5200AD48246D313F3469AC")?;
    let value = c
        .get_transaction_status(txid, CollectionLimit::new(100)?)
        .await?;
    assert!(value.value().transaction().unwrap().data().gas.is_none());
    assert_eq!(
        value
            .value()
            .transaction()
            .unwrap()
            .data()
            .coins
            .as_ref()
            .unwrap()[0]
            .asset
            .to_string(),
        "BCH~BCH"
    );
    assert!(value.value().outbounds().unwrap()[0].data().id.is_blank());
    assert!(value.value().stages().outbound_signed.is_none());
    roundtrip(&value)?;
    for body in [
        r#"{"tx":null,"planned_out_txs":null,"out_txs":null,"stages":{"inbound_observed":{"final_count":0,"completed":false}}}"#,
        r#"{"stages":{"inbound_observed":{"final_count":0,"completed":false}}}"#,
        r#"{"tx":null,"planned_out_txs":[],"out_txs":[],"stages":{"inbound_observed":{"final_count":0,"completed":false}}}"#,
    ] {
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::json(body),
        ])
        .await?;
        let c = client(&fixture.endpoint).await?;
        let value = c
            .get_transaction_status(Txid::parse(TXID)?, CollectionLimit::new(1)?)
            .await?;
        assert!(value.value().transaction().is_none());
        assert!(!value.value().stages().inbound_observed.completed);
        if body.contains("[]") {
            assert_eq!(value.value().planned_outbounds(), Some(&[][..]));
            assert_eq!(value.value().outbounds(), Some(&[][..]));
        } else {
            assert!(value.value().planned_outbounds().is_none());
            assert!(value.value().outbounds().is_none());
        }
        roundtrip(&value)?;
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn status_preserves_solana_case_and_hex_aliases_but_rejects_mismatch()
-> Result<(), Box<dyn std::error::Error>> {
    let signature = bs58::encode([4u8; 64]).into_string();
    let body=json!({"tx":{"id":signature,"chain":"SOL","from_address":"source-sol-address","to_address":"dest-sol-address","coins":[{"asset":"SOL.SOL","amount":"9007199254740993"}],"gas":[],"memo":""},"stages":{"inbound_observed":{"final_count":3,"completed":true}}}).to_string();
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(&body),
    ])
    .await?;
    let value = client(&fixture.endpoint)
        .await?
        .get_transaction_status(Txid::parse(&signature)?, CollectionLimit::new(10)?)
        .await?;
    assert_eq!(
        value.value().transaction().unwrap().data().id.as_str(),
        signature
    );
    assert!(fixture.requests()?[2].contains(&format!("/thorchain/tx/status/{signature} ")));
    roundtrip(&value)?;
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(STATUS),
    ])
    .await?;
    let alias = Txid::parse(&format!("0x{TXID}"))?;
    let value = client(&fixture.endpoint)
        .await?
        .get_transaction_status(alias.clone(), CollectionLimit::new(10)?)
        .await?;
    assert_eq!(value.value().query_id(), &alias);
    assert!(
        value
            .value()
            .transaction()
            .unwrap()
            .data()
            .id
            .same_transaction(&alias)
    );
    roundtrip(&value)?;
    let mismatch = STATUS.replace(TXID, &"cd".repeat(32));
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(&mismatch),
    ])
    .await?;
    assert_eq!(
        client(&fixture.endpoint)
            .await?
            .get_transaction_status(Txid::parse(TXID)?, CollectionLimit::new(10)?)
            .await
            .unwrap_err(),
        invalid()
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn source_status_validates_stages_nested_limits_and_thor_prefix()
-> Result<(), Box<dyn std::error::Error>> {
    let base: Value = serde_json::from_str(STATUS)?;
    let mut bodies = vec![];
    let mut v = base.clone();
    v["stages"]["inbound_confirmation_counted"] = json!({"chain":"BTC","external_observed_height":10,"external_confirmation_delay_height":9,"completed":false});
    bodies.push(serde_json::to_string(&v)?);
    let mut v = base.clone();
    v["tx"]["coins"] = json!([v["tx"]["coins"][0].clone(), v["tx"]["coins"][0].clone()]);
    bodies.push(serde_json::to_string(&v)?);
    let mut v = base;
    v["out_txs"] = json!([v["out_txs"][0].clone(), v["out_txs"][0].clone()]);
    bodies.push(serde_json::to_string(&v)?);
    for body in bodies {
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::json(&body),
        ])
        .await?;
        assert_eq!(
            client(&fixture.endpoint)
                .await?
                .get_transaction_status(Txid::parse(TXID)?, CollectionLimit::new(1)?)
                .await
                .unwrap_err(),
            invalid()
        );
    }
    let mut v: Value = serde_json::from_str(INTERNAL)?;
    let stage = bech32::encode::<bech32::Bech32>(bech32::Hrp::parse("sthor")?, &[7u8; 20])?;
    v["tx"]["from_address"] = json!(stage);
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(&serde_json::to_string(&v)?),
    ])
    .await?;
    assert_eq!(
        client(&fixture.endpoint)
            .await?
            .get_transaction_status(
                Txid::parse("A487E167659E0FC3681B60D1AEABFAB180D6C6917F5200AD48246D313F3469AC")?,
                CollectionLimit::new(10)?
            )
            .await
            .unwrap_err(),
        Error::Validation(ValidationError::ObservationOperationMismatch)
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn inbound_halts_and_external_heights_reject_incompatible_source_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let mut v: Value = serde_json::from_str(include_str!("fixtures/thorchain/inbounds.json"))?;
    v[0]["halted"] = json!(true);
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(&v.to_string()),
    ])
    .await?;
    assert_eq!(
        client(&fixture.endpoint)
            .await?
            .get_inbound_addresses(CollectionLimit::new(10)?)
            .await
            .unwrap_err(),
        invalid()
    );
    let body =
        r#"[{"chain":"BTC","last_observed_in":900000000,"last_signed_out":10,"thorchain":9}]"#;
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(body),
    ])
    .await?;
    assert_eq!(
        client(&fixture.endpoint)
            .await?
            .get_last_blocks(CollectionLimit::new(10)?)
            .await
            .unwrap_err(),
        invalid()
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn network_and_data_delays_share_one_budget_with_bounded_bodies_and_safe_errors()
-> Result<(), Box<dyn std::error::Error>> {
    // Each stage fits a fresh budget; their total exceeds the shared deadline.
    // Keep enough setup margin for slower Linux runners.
    let delay = Duration::from_millis(1200);
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply {
            delay,
            ..Reply::json(INFO)
        },
        Reply {
            delay,
            ..Reply::json(include_str!("fixtures/thorchain/network.json"))
        },
    ])
    .await?;
    let c = ThorchainClient::connect(config(
        &fixture.endpoint,
        1024 * 1024,
        0,
        Duration::from_secs(2),
    )?)
    .await?;
    assert_eq!(c.get_network().await.unwrap_err(), Error::Timeout);
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert!(requests[2].starts_with("GET /api/v3/thorchain/network "));
    let huge = "x".repeat(2000);
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(&huge),
    ])
    .await?;
    let c = ThorchainClient::connect(config(&fixture.endpoint, 1000, 0, Duration::from_secs(3))?)
        .await?;
    assert_eq!(
        c.get_network().await.unwrap_err(),
        Error::Provider(ProviderError::ResponseTooLarge)
    );
    for (status, expected) in [
        (404, Error::UnavailableData),
        (429, Error::Provider(ProviderError::RateLimited)),
        (401, Error::Provider(ProviderError::HttpStatus)),
    ] {
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::status(status),
        ])
        .await?;
        let error = client(&fixture.endpoint)
            .await?
            .get_network()
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?}: {error}").contains("private-provider-body"));
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn unsupported_quote_tolerance_modes_fail_before_network_setup()
-> Result<(), Box<dyn std::error::Error>> {
    for parameters in [
        SwapParameters {
            tolerance_bps: Some(BasisPoints::new(100)?),
            liquidity_tolerance_bps: Some(BasisPoints::new(20)?),
            ..SwapParameters::default()
        },
        SwapParameters {
            liquidity_tolerance_bps: Some(BasisPoints::new(10_000)?),
            ..SwapParameters::default()
        },
    ] {
        let fixture = Fixture::start(vec![Reply::json(INFO)]).await?;
        let outcome = async {
            let request = SwapRequest::new(
                Asset::parse("BTC.BTC")?,
                Asset::parse("ETH.ETH")?,
                ProtocolAmount::from_decimal("100000000")?,
                parameters,
            )?;
            client(&fixture.endpoint)
                .await?
                .get_swap_quote(request)
                .await
        }
        .await;
        assert_eq!(
            outcome.unwrap_err(),
            Error::Validation(ValidationError::InvalidThorchainRecord)
        );
        assert!(fixture.requests()?.is_empty());
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn observed_memo_controls_empty_null_and_missing_remain_distinct()
-> Result<(), Box<dyn std::error::Error>> {
    for expected in [Some("private-source-memo\n\0\t雪"), Some(""), None] {
        let mut body: Value = serde_json::from_str(STATUS)?;
        body["tx"]["memo"] = json!(expected);
        let fixture = Fixture::start(vec![
            Reply::json(INFO),
            Reply::json(INFO),
            Reply::json(&body.to_string()),
        ])
        .await?;
        let value = client(&fixture.endpoint)
            .await?
            .get_transaction_status(Txid::parse(TXID)?, CollectionLimit::new(10)?)
            .await?;
        let memo = value
            .value()
            .transaction()
            .ok_or_else(invalid)?
            .data()
            .memo
            .as_ref();
        assert_eq!(
            memo.map(regit_web3::domain::thorchain::TransactionMemo::as_str),
            expected
        );
        assert!(!format!("{value:?}").contains("private-source-memo"));
        roundtrip(&value)?;
    }
    let mut body: Value = serde_json::from_str(STATUS)?;
    body["tx"]
        .as_object_mut()
        .ok_or_else(invalid)?
        .remove("memo");
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(&body.to_string()),
    ])
    .await?;
    let value = client(&fixture.endpoint)
        .await?
        .get_transaction_status(Txid::parse(TXID)?, CollectionLimit::new(10)?)
        .await?;
    assert!(
        value
            .value()
            .transaction()
            .ok_or_else(invalid)?
            .data()
            .memo
            .is_none()
    );
    body["tx"]["memo"] = json!("x".repeat(251));
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(&body.to_string()),
    ])
    .await?;
    assert_eq!(
        client(&fixture.endpoint)
            .await?
            .get_transaction_status(Txid::parse(TXID)?, CollectionLimit::new(10)?)
            .await
            .unwrap_err(),
        invalid()
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn input_asset_shorthand_remains_requested_syntax_with_unreported_source_resolution()
-> Result<(), Box<dyn std::error::Error>> {
    let mut body: Value = serde_json::from_str(&updated_quote()?)?;
    body["inbound_address"] = json!("0xd16e79edc37cae2019ae049bbc37587a94405960");
    body["router"] = json!("0xD37BbE5744D730a1d98d8DC97c42F0Ca46aD7146");
    body["notes"] = json!("Source token router instructions");
    body["memo"] = Value::Null;
    body["recommended_gas_rate"] = json!("15");
    body["gas_rate_units"] = json!("gwei");
    body["dust_threshold"] = json!("1000");
    body["recommended_min_amount_in"] = json!("100000000");
    body["inbound_confirmation_blocks"] = json!(1);
    body["inbound_confirmation_seconds"] = json!(12);
    body["outbound_delay_blocks"] = json!(1);
    body["outbound_delay_seconds"] = json!(6);
    body["total_swap_seconds"] = json!(54);
    body["expected_amount_out"] = json!("320000");
    body["fees"] = json!({"asset":"ETH.ETH","affiliate":"0","outbound":"10000","liquidity":"3000","total":"13000","slippage_bps":90,"total_bps":390});
    let fixture = Fixture::start(vec![
        Reply::json(INFO),
        Reply::json(INFO),
        Reply::json(&body.to_string()),
    ])
    .await?;
    let request = SwapRequest::new(
        Asset::parse("ETH.USDC")?,
        Asset::parse("ETH.ETH")?,
        ProtocolAmount::from_decimal("1000000000")?,
        SwapParameters::default(),
    )?;
    let value = client(&fixture.endpoint)
        .await?
        .get_swap_quote(request.clone())
        .await?;
    assert_eq!(value.value().request(), &request);
    assert_eq!(
        value.value().data().input_resolution,
        QuoteInputResolution::Unreported
    );
    assert_eq!(value.value().data().fees.asset, Asset::parse("ETH.ETH")?);
    assert!(value.value().data().memo.is_none());
    assert_eq!(
        value
            .value()
            .data()
            .inbound_address
            .as_ref()
            .ok_or_else(invalid)?
            .chain()
            .as_str(),
        "ETH"
    );
    assert_eq!(
        value
            .value()
            .data()
            .gas_rate_units
            .as_ref()
            .ok_or_else(invalid)?
            .as_str(),
        "gwei"
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert!(requests[2].contains("from_asset=ETH.USDC&to_asset=ETH.ETH&amount=1000000000 "));
    roundtrip(&value)?;
    Ok(())
}
