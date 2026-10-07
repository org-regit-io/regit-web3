// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Canonical/indexed Bitcoin retrieval over bounded real loopback HTTP fixtures.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "bitcoin-esplora")]

use std::time::Duration;

use regit_web3::{
    chains::bitcoin::{EsploraClient, EsploraConfig, TransactionReader},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::bitcoin::{
        Network, NetworkId, Observation, Operation, Satoshis, Transaction, TransactionBody,
        TransactionStatus, Txid,
    },
    error::{Error, ProviderError},
};
use serde_json::{Value, json};

#[path = "support/esplora.rs"]
mod support;
use support::{Fixture, Reply};

const GENESIS_HEX: &str = include_str!("fixtures/bitcoin/genesis.hex");
const GENESIS_JSON: &str = include_str!("fixtures/bitcoin/genesis.json");
const GENESIS_TXID: &str = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";
const SEGWIT_HEX: &str = include_str!("fixtures/bitcoin/segwit.hex");
const SEGWIT_JSON: &str = include_str!("fixtures/bitcoin/segwit.json");
const SEGWIT_TXID: &str = "14396c6a212fce4a794501b5840c718a700daa9e462427fed22a7f38d5197b1e";

fn genesis() -> Reply {
    Reply::ok(Network::Mainnet.genesis_hash().to_string())
}
fn replies(hex: &str, indexed: impl Into<String>) -> Vec<Reply> {
    vec![genesis(), genesis(), Reply::ok(hex), Reply::ok(indexed)]
}
async fn client(
    fixture: &Fixture,
    retries: u8,
    timeout: Duration,
    cap: usize,
) -> Result<EsploraClient, Error> {
    let limits = RpcLimits::new(timeout, timeout, cap, retries)?;
    EsploraClient::connect(EsploraConfig::new(
        NetworkId::new(Network::Mainnet, "bitcoin-fixture")?,
        HttpConfig::new(
            RpcEndpoint::new(fixture.endpoint())?,
            limits,
            "esplora-fixture",
        )?,
    ))
    .await
}
async fn standard_client(fixture: &Fixture) -> Result<EsploraClient, Error> {
    client(fixture, 0, Duration::from_secs(2), 16 * 1024).await
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

#[tokio::test]
async fn canonical_genesis_and_segwit_reads_match_indexed_values_and_public_serde()
-> Result<(), Box<dyn std::error::Error>> {
    for (hex, indexed, txid, fee, coinbase) in [
        (GENESIS_HEX, GENESIS_JSON, GENESIS_TXID, 0, true),
        (SEGWIT_HEX, SEGWIT_JSON, SEGWIT_TXID, 380, false),
    ] {
        let fixture = Fixture::start(replies(hex, indexed)).await?;
        let client = standard_client(&fixture).await?;
        let txid = Txid::parse(txid)?;
        let observation = TransactionReader::get_transaction(&client, txid).await?;
        assert_eq!(
            observation.context().operation(),
            &Operation::Transaction { txid }
        );
        assert_eq!(observation.context().network(), client.config().network());
        assert_eq!(observation.context().source().method(), "tx-with-hex");
        assert_eq!(
            observation.context().source().provider_id(),
            "esplora-fixture"
        );
        assert_eq!(observation.value().body().txid(), txid);
        assert_eq!(observation.value().reported_fee(), Some(Satoshis::new(fee)));
        assert_eq!(observation.value().body().is_coinbase(), coinbase);
        assert!(matches!(
            observation.value().status(),
            TransactionStatus::Confirmed(_)
        ));
        assert_eq!(
            observation.value().fee_from_previous_outputs(),
            (!coinbase).then_some(Satoshis::new(fee))
        );
        assert_eq!(
            serde_json::from_str::<Observation<Transaction>>(&serde_json::to_string(
                &observation
            )?)?,
            observation
        );
        let requests = fixture.requests()?;
        assert_eq!(requests.len(), 4);
        assert!(requests[1].starts_with("GET /api/block-height/0 "));
        assert!(requests[2].starts_with(&format!("GET /api/tx/{txid}/hex ")));
        assert!(requests[3].starts_with(&format!("GET /api/tx/{txid} ")));
    }
    Ok(())
}

#[tokio::test]
async fn unavailable_indexed_fee_and_ordinary_prevout_are_not_zero_or_coinbase()
-> Result<(), Box<dyn std::error::Error>> {
    let mut indexed: Value = serde_json::from_str(SEGWIT_JSON)?;
    indexed
        .as_object_mut()
        .ok_or(Error::Configuration)?
        .remove("fee");
    indexed["vin"][0]
        .as_object_mut()
        .ok_or(Error::Configuration)?
        .remove("prevout");
    let fixture = Fixture::start(replies(SEGWIT_HEX, indexed.to_string())).await?;
    let observation = standard_client(&fixture)
        .await?
        .get_transaction(Txid::parse(SEGWIT_TXID)?)
        .await?;
    assert_eq!(observation.value().reported_fee(), None);
    assert_eq!(observation.value().previous_outputs(), &[None]);
    assert_eq!(observation.value().fee_from_previous_outputs(), None);
    assert!(!observation.value().body().is_coinbase());
    Ok(())
}

#[tokio::test]
async fn partial_prevout_index_rejects_known_funding_above_reported_total()
-> Result<(), Box<dyn std::error::Error>> {
    let original = TransactionBody::from_hex(SEGWIT_HEX)?;
    let mut raw: bitcoin::Transaction = bitcoin::consensus::deserialize(&original.to_bytes())?;
    let mut second = raw.input[0].clone();
    second.previous_output.vout = 2;
    raw.input.push(second);
    let body = TransactionBody::from_bytes(&bitcoin::consensus::serialize(&raw))?;
    let mut indexed: Value = serde_json::from_str(SEGWIT_JSON)?;
    let mut second = indexed["vin"][0].clone();
    second["vout"] = json!(2);
    second["prevout"] = Value::Null;
    indexed["vin"]
        .as_array_mut()
        .ok_or(Error::Configuration)?
        .push(second);
    indexed["txid"] = json!(body.txid().to_string());
    indexed["size"] = json!(body.size());
    indexed["weight"] = json!(body.weight());
    let hex = serde_json::to_value(&body)?;
    let hex = hex.as_str().ok_or(Error::Configuration)?;
    for (fee, valid) in [(0, false), (379, false), (380, true), (381, true)] {
        indexed["fee"] = json!(fee);
        let fixture = Fixture::start(replies(hex, indexed.to_string())).await?;
        let result = standard_client(&fixture)
            .await?
            .get_transaction(body.txid())
            .await;
        if valid {
            let observation = result?;
            assert_eq!(observation.value().previous_outputs()[1], None);
            assert_eq!(observation.value().fee_from_previous_outputs(), None);
            assert_eq!(observation.value().reported_fee(), Some(Satoshis::new(fee)));
        } else {
            assert_eq!(result, Err(invalid()));
        }
    }
    Ok(())
}

#[tokio::test]
async fn either_transaction_resource_404_is_unavailable_not_unconfirmed()
-> Result<(), Box<dyn std::error::Error>> {
    for replies in [
        vec![genesis(), genesis(), Reply::status(404, "secret raw error")],
        vec![
            genesis(),
            genesis(),
            Reply::ok(GENESIS_HEX),
            Reply::status(404, "secret indexed error"),
        ],
    ] {
        let fixture = Fixture::start(replies).await?;
        assert_eq!(
            standard_client(&fixture)
                .await?
                .get_transaction(Txid::parse(GENESIS_TXID)?)
                .await,
            Err(Error::UnavailableData)
        );
    }
    Ok(())
}

#[tokio::test]
async fn changed_genesis_and_mismatched_raw_id_stop_before_the_indexed_read()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis(),
        Reply::ok(Network::Testnet4.genesis_hash().to_string()),
    ])
    .await?;
    assert_eq!(
        standard_client(&fixture)
            .await?
            .get_transaction(Txid::parse(GENESIS_TXID)?)
            .await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    assert_eq!(fixture.requests()?.len(), 2);
    let fixture = Fixture::start(replies(GENESIS_HEX, SEGWIT_JSON)).await?;
    assert_eq!(
        standard_client(&fixture)
            .await?
            .get_transaction(Txid::parse(SEGWIT_TXID)?)
            .await,
        Err(invalid())
    );
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test]
async fn immutable_json_raw_mismatches_and_invalid_index_facts_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let original: Value = serde_json::from_str(SEGWIT_JSON)?;
    let mut variants = Vec::new();
    for (key, value) in [
        ("txid", json!("00".repeat(32))),
        ("version", json!(1)),
        ("version", json!(2_147_483_648_u64)),
        ("locktime", json!(u64::from(u32::MAX) + 1)),
        ("size", json!(304)),
        ("weight", json!(883)),
        ("fee", json!(381)),
        ("fee", json!(u64::MAX)),
    ] {
        let mut altered = original.clone();
        altered[key] = value;
        variants.push(altered);
    }
    for (path, value) in [
        ("/vout/0/value", json!(u64::MAX)),
        ("/vout/0/scriptpubkey", json!("6a")),
        ("/vin/0/vout", json!(u64::from(u32::MAX) + 1)),
        ("/vin/0/is_coinbase", json!(true)),
        ("/vin/0/sequence", json!(u64::from(u32::MAX) + 1)),
        ("/vin/0/scriptsig", json!("51")),
        ("/vin/0/witness/0", json!("00")),
        ("/vin/0/prevout/value", json!(1)),
        ("/status/block_hash", Value::Null),
    ] {
        let mut altered = original.clone();
        *altered.pointer_mut(path).ok_or(Error::Configuration)? = value;
        variants.push(altered);
    }
    for altered in variants {
        let fixture = Fixture::start(replies(SEGWIT_HEX, altered.to_string())).await?;
        assert_eq!(
            standard_client(&fixture)
                .await?
                .get_transaction(Txid::parse(SEGWIT_TXID)?)
                .await,
            Err(invalid())
        );
    }
    let mut altered = original;
    altered["vout"][0]["scriptpubkey_address"] = json!("1BoatSLRHtKNngkdXEeobR76b53LETtpyT");
    let fixture = Fixture::start(replies(SEGWIT_HEX, altered.to_string())).await?;
    assert_eq!(
        standard_client(&fixture)
            .await?
            .get_transaction(Txid::parse(SEGWIT_TXID)?)
            .await,
        Err(invalid())
    );
    Ok(())
}

#[tokio::test]
async fn relevant_duplicate_fields_are_rejected_before_map_overwrite()
-> Result<(), Box<dyn std::error::Error>> {
    for indexed in [
        SEGWIT_JSON.replacen("\"fee\":380", "\"fee\":380,\"fee\":380", 1),
        SEGWIT_JSON.replacen(
            "\"is_coinbase\":false",
            "\"is_coinbase\":false,\"is_coinbase\":false",
            1,
        ),
        SEGWIT_JSON.replacen(
            "\"confirmed\":true",
            "\"confirmed\":true,\"confirmed\":true",
            1,
        ),
    ] {
        assert_ne!(indexed, SEGWIT_JSON);
        let fixture = Fixture::start(replies(SEGWIT_HEX, indexed)).await?;
        assert_eq!(
            standard_client(&fixture)
                .await?
                .get_transaction(Txid::parse(SEGWIT_TXID)?)
                .await,
            Err(invalid())
        );
    }
    Ok(())
}

#[tokio::test]
async fn malformed_raw_and_body_bounds_use_fixed_secret_safe_errors()
-> Result<(), Box<dyn std::error::Error>> {
    for hex in [
        "secret-provider-body".to_owned(),
        format!("{GENESIS_HEX}00"),
        GENESIS_HEX.to_ascii_uppercase(),
    ] {
        let fixture = Fixture::start(replies(&hex, GENESIS_JSON)).await?;
        let error = standard_client(&fixture)
            .await?
            .get_transaction(Txid::parse(GENESIS_TXID)?)
            .await
            .err()
            .ok_or(Error::Configuration)?;
        assert_eq!(error, invalid());
        assert!(!format!("{error:?} {error}").contains("secret-provider-body"));
    }
    let fixture = Fixture::start(replies(&"00".repeat(1024), GENESIS_JSON)).await?;
    let client = client(&fixture, 0, Duration::from_secs(2), 1024).await?;
    assert_eq!(
        client.get_transaction(Txid::parse(GENESIS_TXID)?).await,
        Err(Error::Provider(ProviderError::ResponseTooLarge))
    );
    Ok(())
}

#[tokio::test]
async fn one_deadline_covers_genesis_raw_and_indexed_resources()
-> Result<(), Box<dyn std::error::Error>> {
    // All three resources fit a fresh budget; only their total exceeds it.
    let delay = Duration::from_millis(1200);
    let fixture = Fixture::start(vec![
        genesis(),
        Reply::delayed(Network::Mainnet.genesis_hash().to_string(), delay),
        Reply::delayed(GENESIS_HEX, delay),
        Reply::delayed(GENESIS_JSON, delay),
    ])
    .await?;
    let client = client(&fixture, 0, Duration::from_secs(3), 16 * 1024).await?;
    assert_eq!(
        client.get_transaction(Txid::parse(GENESIS_TXID)?).await,
        Err(Error::Timeout)
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert!(requests[2].starts_with(&format!("GET /api/tx/{GENESIS_TXID}/hex ")));
    assert!(requests[3].starts_with(&format!("GET /api/tx/{GENESIS_TXID} ")));
    Ok(())
}

#[tokio::test]
async fn raw_resource_retry_keeps_the_requested_id_without_rechecking_genesis()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis(),
        genesis(),
        Reply::status(503, "discarded error body"),
        Reply::ok(GENESIS_HEX),
        Reply::ok(GENESIS_JSON),
    ])
    .await?;
    let client = client(&fixture, 1, Duration::from_secs(2), 16 * 1024).await?;
    let txid = Txid::parse(GENESIS_TXID)?;
    assert_eq!(
        client.get_transaction(txid).await?.value().body().txid(),
        txid
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 5);
    assert!(requests[2].starts_with(&format!("GET /api/tx/{txid}/hex ")));
    assert_eq!(requests[2], requests[3]);
    Ok(())
}
