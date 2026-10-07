// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Actual public XRPL HTTP contracts through bounded loopback exchanges.

#![cfg(feature = "xrpl-http")]

use std::{
    future::Future,
    task::{Context, Poll, Waker},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use regit_web3::{
    chains::xrpl::{XrplClient, XrplHttpConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::xrpl::{
        Address, Hash, HexData, HistoryMarker, HistoryRequest, Ledger, LedgerRange, Marker,
        Network, NetworkId, PageRequest,
    },
    error::{Error, ProviderError},
};
use serde_json::{Value, json};

#[path = "support/rpc_server.rs"]
mod server;
use server::{Fixture, Framing, Reply};

const ACCOUNT: &str = "r9cZA1mLK5R5Am25ArfXFmqgNwjZgnfk59";
const PEER: &str = "rGWrZyQqhTp9Xu7G5Pkayo7bXjH4k4QYpf";
const LEDGER: &str = "0101010101010101010101010101010101010101010101010101010101010101";

fn config(
    endpoint: &str,
    retries: u8,
    timeout: Duration,
    cap: usize,
) -> Result<XrplHttpConfig, Error> {
    Ok(XrplHttpConfig::new(
        Network::new(NetworkId::new(0), "fixture")?,
        HttpConfig::new(
            RpcEndpoint::new(endpoint)?,
            RpcLimits::new(timeout, timeout, cap, retries)?,
            "local",
        )?,
    ))
}
fn standard(endpoint: &str) -> Result<XrplHttpConfig, Error> {
    config(endpoint, 0, Duration::from_secs(2), 1_048_576)
}
fn result(mut value: Value) -> Result<Reply, Box<dyn std::error::Error>> {
    value
        .as_object_mut()
        .ok_or("expected fixture object")?
        .insert("status".into(), json!("success"));
    let mut reply = Reply::json(&json!({"result":value}))?;
    reply.echo_id = false;
    Ok(reply)
}
fn network() -> Result<Reply, Box<dyn std::error::Error>> {
    result(json!({"info":{"network_id":0,"build_version":"fixture"}}))
}
fn ledger() -> Result<Reply, Box<dyn std::error::Error>> {
    result(
        json!({"ledger_hash":LEDGER,"ledger_index":100,"validated":true,"ledger":{"ledger_hash":LEDGER,"ledger_index":"100","closed":true}}),
    )
}
fn account() -> Value {
    json!({"account_data":{"Account":ACCOUNT,"Balance":"9007199254740993","Sequence":7,"OwnerCount":2,"Flags":0,"LedgerEntryType":"AccountRoot"},"ledger_hash":LEDGER,"ledger_index":100,"validated":true})
}
fn line() -> Value {
    json!({"account":PEER,"balance":"-0.00111","currency":"USD","limit":"9007199254740993","limit_peer":"1e-81","quality_in":0,"quality_out":u32::MAX,"freeze_peer":true})
}
fn lines() -> Value {
    json!({"account":ACCOUNT,"lines":[line()],"ledger_hash":LEDGER,"ledger_index":100,"validated":true,"limit":10,"marker":"next"})
}
fn fee() -> Value {
    json!({"drops":{"base_fee":"10","minimum_fee":"12","median_fee":"15","open_ledger_fee":"9007199254740993"},"ledger_current_index":101,"levels":{"reference_level":"256"}})
}

fn history_fixture() -> Result<Value, serde_json::Error> {
    serde_json::from_str(include_str!("fixtures/xrpl_binary_history.json"))
}
fn binary_transaction() -> Result<Value, Box<dyn std::error::Error>> {
    let captured = history_fixture()?;
    let mut entry = captured["result"]["transactions"][0].clone();
    let hash =
        HexData::parse(entry["tx_blob"].as_str().ok_or("missing payload")?)?.transaction_hash();
    entry["hash"] = json!(hash);
    entry["ledger_hash"] = json!(LEDGER);
    Ok(entry)
}

#[tokio::test]
async fn actual_binary_history_entries_recompute_ids_with_no_invented_ledger_hash()
-> Result<(), Box<dyn std::error::Error>> {
    let captured = history_fixture()?;
    let reply = Reply::raw(200, serde_json::to_vec(&captured)?);
    let fixture = Fixture::start(vec![network()?, network()?, reply]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    let account = Address::parse(
        captured["result"]["account"]
            .as_str()
            .ok_or("missing account")?,
    )?;
    let request = HistoryRequest::new(LedgerRange::new(32_570, 107_483_958)?, 1, false, None)?;
    let observed = client.get_account_history(account, request).await?;
    let entry = &observed.value().transactions()[0];
    assert_eq!(
        entry.hash(),
        Hash::parse("AA422A0AF62C58042BE3A243954CC1BD2560E2345F475E577F8E21F4D455BEA5")?
    );
    assert_eq!(entry.ledger().map(Ledger::index), Some(106_836_736));
    assert_eq!(entry.ledger().and_then(Ledger::hash), None);
    assert!(entry.ledger().is_some_and(Ledger::validated));
    assert!(observed.context().ledger().is_none());
    assert_eq!(
        observed.value().next_marker(),
        Some(HistoryMarker::new(106_836_736, 27)?)
    );
    let requests = fixture.requests()?;
    assert_eq!(requests[2].body["method"], "account_tx");
    assert_eq!(requests[2].body["params"][0]["binary"], true);
    assert_eq!(requests[2].body["params"][0]["ledger_index_min"], 32_570);
    assert_eq!(
        requests[2].body["params"][0]["ledger_index_max"],
        107_483_958
    );
    Ok(())
}

#[tokio::test]
async fn transaction_retrieval_recomputes_exact_bytes_and_rejects_identity_mismatch()
-> Result<(), Box<dyn std::error::Error>> {
    let body = binary_transaction()?;
    let hash = Hash::parse(body["hash"].as_str().ok_or("missing hash")?)?;
    let fixture = Fixture::start(vec![network()?, network()?, result(body.clone())?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    let observed = client.get_transaction(hash).await?;
    assert_eq!(observed.value().payload().transaction_hash(), hash);
    assert_eq!(observed.context().ledger(), observed.value().ledger());
    assert_eq!(
        observed.context().ledger().and_then(Ledger::hash),
        Some(Hash::parse(LEDGER)?)
    );
    assert_eq!(fixture.requests()?[2].body["params"][0]["binary"], true);
    for change in [
        json!({"hash":Hash::from_bytes([2;32])}),
        json!({"tx_blob":"00"}),
        json!({"meta_blob":null}),
        json!({"ledger_index":null}),
        json!({"ledger_index":0}),
    ] {
        let mut malformed = body.clone();
        for (key, value) in change.as_object().ok_or("expected object")? {
            malformed[key] = value.clone();
        }
        let fixture = Fixture::start(vec![network()?, network()?, result(malformed)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client.get_transaction(hash).await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}

#[tokio::test]
async fn status_reports_validated_cost_failure_separately_from_success_and_pending()
-> Result<(), Box<dyn std::error::Error>> {
    let hash = Hash::from_bytes([1; 32]);
    for code in ["tesSUCCESS", "tecPATH_DRY"] {
        let body = json!({"hash":hash,"validated":true,"ledger_index":100,"ledger_hash":LEDGER,"tx_json":{"TransactionType":"Payment"},"meta":{"TransactionResult":code,"AffectedNodes":[]}});
        let fixture = Fixture::start(vec![network()?, network()?, result(body)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        let observed = client.get_transaction_status(hash).await?;
        assert!(observed.value().validated());
        assert_eq!(
            observed
                .value()
                .execution()
                .ok_or("missing execution")?
                .is_success(),
            code == "tesSUCCESS"
        );
        assert_eq!(fixture.requests()?[2].body["params"][0]["binary"], false);
    }
    let fixture = Fixture::start(vec![
        network()?,
        network()?,
        result(json!({"hash":hash,"tx_json":{"TransactionType":"OfferCreate"}}))?,
    ])
    .await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    let observed = client.get_transaction_status(hash).await?;
    assert!(observed.value().execution().is_none());
    assert!(observed.value().ledger().is_none());
    assert!(observed.context().ledger().is_none());
    assert!(!observed.value().validated());
    for change in [
        json!({"hash":Hash::from_bytes([2;32])}),
        json!({"meta":null}),
        json!({"meta":{"TransactionResult":"terQUEUED"}}),
        json!({"tx_json":null}),
        json!({"ledger_index":null}),
    ] {
        let mut body = json!({"hash":hash,"validated":true,"ledger_index":100,"ledger_hash":LEDGER,"tx_json":{},"meta":{"TransactionResult":"tesSUCCESS"}});
        for (key, value) in change.as_object().ok_or("expected object")? {
            body[key] = value.clone();
        }
        let fixture = Fixture::start(vec![network()?, network()?, result(body)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client.get_transaction_status(hash).await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}

#[tokio::test]
async fn history_rejects_duplicate_out_of_range_unvalidated_and_nonadvancing_entries()
-> Result<(), Box<dyn std::error::Error>> {
    let captured = history_fixture()?;
    let account = Address::parse(
        captured["result"]["account"]
            .as_str()
            .ok_or("missing account")?,
    )?;
    for change in [
        json!({"transactions":[captured["result"]["transactions"][0],captured["result"]["transactions"][0]]}),
        json!({"account":PEER}),
        json!({"validated":false}),
        json!({"ledger_index_min":1}),
        json!({"limit":2}),
    ] {
        let mut body = captured["result"].clone();
        for (key, value) in change.as_object().ok_or("expected object")? {
            body[key] = value.clone();
        }
        let fixture = Fixture::start(vec![network()?, network()?, result(body)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client
                .get_account_history(
                    account,
                    HistoryRequest::new(LedgerRange::new(32_570, 107_483_958)?, 1, false, None)?
                )
                .await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    for (field, value) in [
        ("ledger_index", json!(107_483_959)),
        ("validated", json!(false)),
        ("hash", json!(Hash::from_bytes([2; 32]))),
        ("meta_blob", Value::Null),
    ] {
        let mut body = captured["result"].clone();
        body["transactions"][0][field] = value;
        let fixture = Fixture::start(vec![network()?, network()?, result(body)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client
                .get_account_history(
                    account,
                    HistoryRequest::new(LedgerRange::new(32_570, 107_483_958)?, 1, false, None)?
                )
                .await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    let reply = Reply::raw(200, serde_json::to_vec(&captured)?);
    let fixture = Fixture::start(vec![network()?, network()?, reply]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client
            .get_account_history(
                account,
                HistoryRequest::new(
                    LedgerRange::new(32_570, 107_483_958)?,
                    1,
                    false,
                    Some(HistoryMarker::new(106_836_736, 27)?)
                )?
            )
            .await,
        Err(Error::Provider(ProviderError::InvalidResponse))
    );
    Ok(())
}

#[tokio::test]
async fn balance_uses_exact_validated_hash_and_correct_xrpl_http_envelope()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture =
        Fixture::start(vec![network()?, network()?, ledger()?, result(account())?]).await?;
    let before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    let observed = client
        .get_account_balance(Address::parse(ACCOUNT)?, None)
        .await?;
    assert_eq!(observed.value().balance().raw(), 9_007_199_254_740_993);
    assert_eq!(observed.value().sequence(), 7);
    assert_eq!(
        observed
            .context()
            .ledger()
            .and_then(regit_web3::domain::xrpl::Ledger::hash),
        Some(Hash::parse(LEDGER)?)
    );
    assert!(
        observed
            .context()
            .ledger()
            .is_some_and(regit_web3::domain::xrpl::Ledger::validated)
    );
    assert_eq!(observed.context().source().provider_id(), "local");
    assert_eq!(observed.context().source().method(), "account_info");
    assert_eq!(
        observed.context().source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!(observed.context().retrieved_at().unix_seconds() >= before);
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[0].body,
        json!({"method":"server_info","params":[{"api_version":2}]})
    );
    assert_eq!(requests[2].body["params"][0]["ledger_index"], "validated");
    assert_eq!(requests[3].body["method"], "account_info");
    assert_eq!(requests[3].body["params"][0]["ledger_hash"], LEDGER);
    assert_eq!(requests[3].body["params"][0]["account"], ACCOUNT);
    assert!(requests.iter().all(|request| {
        request.target == "/rpc"
            && request
                .headers
                .to_ascii_lowercase()
                .contains("content-type: application/json")
            && request.body.get("jsonrpc").is_none()
            && request.body.get("id").is_none()
    }));
    Ok(())
}

#[tokio::test]
async fn issued_balances_preserve_signed_exact_values_settings_and_hash_bound_marker()
-> Result<(), Box<dyn std::error::Error>> {
    let mut continued = lines();
    continued
        .as_object_mut()
        .ok_or("expected object")?
        .remove("marker");
    continued["lines"] = json!([]);
    let fixture = Fixture::start(vec![
        network()?,
        network()?,
        ledger()?,
        result(lines())?,
        network()?,
        ledger()?,
        result(continued)?,
    ])
    .await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    let page = client
        .get_trust_lines(Address::parse(ACCOUNT)?, PageRequest::new(10, None, None)?)
        .await?;
    assert_eq!(
        page.value().lines()[0].balance().value().canonical(),
        "-0.00111"
    );
    assert_eq!(
        page.value().lines()[0].limit().value().canonical(),
        "9007199254740993"
    );
    assert!(bool::from(page.value().lines()[0].settings().freeze_peer));
    let request = PageRequest::new(
        10,
        page.context()
            .ledger()
            .and_then(regit_web3::domain::xrpl::Ledger::hash),
        page.value().next_marker().cloned(),
    )?;
    let next = client
        .get_trust_lines(Address::parse(ACCOUNT)?, request)
        .await?;
    assert!(next.value().lines().is_empty());
    assert!(next.value().next_marker().is_none());
    let requests = fixture.requests()?;
    assert_eq!(requests[5].body["params"][0]["ledger_hash"], LEDGER);
    assert!(requests[5].body["params"][0].get("ledger_index").is_none());
    assert_eq!(requests[6].body["params"][0]["marker"], "next");
    assert_eq!(requests[6].body["params"][0]["ledger_hash"], LEDGER);
    assert_eq!(requests[6].body["params"][0]["ignore_default"], false);
    Ok(())
}

#[tokio::test]
async fn fee_context_is_actual_open_ledger_not_invented_finality()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![network()?, network()?, result(fee())?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    let observed = client.get_fee_estimate().await?;
    assert_eq!(
        observed.value().open_ledger_fee().raw(),
        9_007_199_254_740_993
    );
    assert_eq!(
        observed
            .context()
            .ledger()
            .map(regit_web3::domain::xrpl::Ledger::index)
            .ok_or("missing ledger")?,
        101
    );
    assert!(
        observed
            .context()
            .ledger()
            .and_then(regit_web3::domain::xrpl::Ledger::hash)
            .is_none()
    );
    assert!(
        !observed
            .context()
            .ledger()
            .is_some_and(regit_web3::domain::xrpl::Ledger::validated)
    );
    assert_eq!(observed.context().source().method(), "fee");
    Ok(())
}

#[tokio::test]
async fn documented_http_root_warnings_are_accepted_but_unknown_root_fields_are_not()
-> Result<(), Box<dyn std::error::Error>> {
    let warning = Reply::raw(
        200,
        serde_json::to_vec(
            &json!({"api_version":2,"status":"success","type":"response","result":{"status":"success","info":{"network_id":0}},"warnings":[{"id":2001,"message":"SECRET_REMOTE_MESSAGE"}],"forwarded":false}),
        )?,
    );
    let fixture = Fixture::start(vec![warning, network()?, result(fee())?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    assert!(client.get_fee_estimate().await.is_ok());
    let warning = Reply::raw(200,b"{\"result\":{\"status\":\"success\",\"info\":{\"network_id\":0}},\"warnings\":[],\"warnings\":[]}".to_vec());
    let fixture = Fixture::start(vec![warning]).await?;
    assert_eq!(
        XrplClient::connect(standard(&fixture.endpoint)?)
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::InvalidResponse)
    );
    Ok(())
}

#[tokio::test]
async fn absent_or_changed_network_never_defaults_to_mainnet()
-> Result<(), Box<dyn std::error::Error>> {
    for (info, expected) in [
        (json!({}), Error::UnsupportedCapability),
        (
            json!({"network_id":null}),
            Error::Provider(ProviderError::InvalidResponse),
        ),
        (
            json!({"network_id":1}),
            Error::Provider(ProviderError::ChainMismatch),
        ),
        (
            json!({"network_id":4_294_967_296_u64}),
            Error::Provider(ProviderError::InvalidResponse),
        ),
    ] {
        let fixture = Fixture::start(vec![result(json!({"info":info}))?]).await?;
        assert_eq!(
            XrplClient::connect(standard(&fixture.endpoint)?)
                .await
                .unwrap_err(),
            expected
        );
    }
    let fixture =
        Fixture::start(vec![network()?, result(json!({"info":{"network_id":2}}))?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.get_fee_estimate().await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    assert_eq!(fixture.requests()?.len(), 2);
    Ok(())
}

#[tokio::test]
async fn account_identity_and_ledger_mismatches_are_invalid_not_zero()
-> Result<(), Box<dyn std::error::Error>> {
    for malformed in [
        json!({"ledger_hash": "0202020202020202020202020202020202020202020202020202020202020202"}),
        json!({"ledger_index":101}),
        json!({"validated":false}),
        json!({"account_data":{"Account":PEER}}),
        json!({"account_data":{"Balance":"100000000000000001"}}),
        json!({"account_data":{"Balance":"-1"}}),
        json!({"account_data":{"Balance":null}}),
        json!({"account_data":{"LedgerEntryType":"Offer"}}),
    ] {
        let mut body = account();
        for (key, value) in malformed.as_object().ok_or("expected object")? {
            if key == "account_data" {
                for (field, value) in value.as_object().ok_or("expected object")? {
                    body[key][field] = value.clone();
                }
            } else {
                body[key] = value.clone();
            }
        }
        let fixture =
            Fixture::start(vec![network()?, network()?, ledger()?, result(body)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client
                .get_account_balance(Address::parse(ACCOUNT)?, None)
                .await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}

#[tokio::test]
async fn ledger_resolution_requires_validation_and_never_falls_back()
-> Result<(), Box<dyn std::error::Error>> {
    for fields in [
        json!({"ledger_hash":LEDGER,"ledger_index":100,"validated":false}),
        json!({"ledger_hash":LEDGER,"ledger_index":0,"validated":true}),
    ] {
        let fixture = Fixture::start(vec![network()?, network()?, result(fields)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client
                .get_account_balance(Address::parse(ACCOUNT)?, None)
                .await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    let fixture = Fixture::start(vec![network()?, network()?, ledger()?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client
            .get_account_balance(Address::parse(ACCOUNT)?, Some(Hash::from_bytes([2; 32])))
            .await,
        Err(Error::Provider(ProviderError::InvalidResponse))
    );
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test]
async fn duplicate_or_wrong_http_control_and_amount_fields_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let account_body = serde_json::to_string(
        &json!({"result":{"status":"success","account_data":account()["account_data"],"ledger_hash":LEDGER,"ledger_index":100,"validated":true}}),
    )?;
    let malformed = [
        account_body.replacen(
            "\"Balance\":\"9007199254740993\"",
            "\"Balance\":\"1\",\"Balance\":\"9007199254740993\"",
            1,
        ),
        account_body.replacen(
            "\"status\":\"success\"",
            "\"status\":\"success\",\"status\":\"success\"",
            1,
        ),
        account_body.replacen(
            "\"ledger_index\":100",
            "\"ledger_index\":100,\"ledger_index\":100",
            1,
        ),
        account_body.replacen(
            "\"status\":\"success\"",
            "\"status\":\"success\",\"error\":null",
            1,
        ),
        "{\"result\":null}".to_owned(),
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"status\":\"success\"}}".to_owned(),
    ];
    for body in malformed {
        let fixture = Fixture::start(vec![
            network()?,
            network()?,
            ledger()?,
            Reply::raw(200, body.into_bytes()),
        ])
        .await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client
                .get_account_balance(Address::parse(ACCOUNT)?, None)
                .await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}

#[tokio::test]
async fn trustline_pages_reject_duplicate_assets_overflow_and_nonadvancing_cursor()
-> Result<(), Box<dyn std::error::Error>> {
    for (body, request) in [
        (
            json!({"lines":[line(),line()]}),
            PageRequest::new(10, None, None)?,
        ),
        (json!({"account":PEER}), PageRequest::new(10, None, None)?),
        (
            json!({"lines":[{"account":PEER,"balance":"12345678901234567","currency":"USD","limit":"1","limit_peer":"1","quality_in":0,"quality_out":0}]}),
            PageRequest::new(10, None, None)?,
        ),
        (json!({"limit":11}), PageRequest::new(10, None, None)?),
        (
            json!({"marker":"next"}),
            PageRequest::new(10, Some(Hash::parse(LEDGER)?), Some(Marker::new("next")?))?,
        ),
        (
            json!({"lines":vec![line();11]}),
            PageRequest::new(10, None, None)?,
        ),
    ] {
        let mut value = lines();
        for (key, field) in body.as_object().ok_or("expected object")? {
            value[key] = field.clone();
        }
        let fixture =
            Fixture::start(vec![network()?, network()?, ledger()?, result(value)?]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client
                .get_trust_lines(Address::parse(ACCOUNT)?, request)
                .await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}

#[tokio::test]
async fn remote_errors_have_fixed_mapping_and_never_contain_messages()
-> Result<(), Box<dyn std::error::Error>> {
    for (name, expected) in [
        ("actNotFound", Error::UnavailableData),
        ("lgrNotFound", Error::UnavailableData),
        ("txnNotFound", Error::UnavailableData),
        ("unknownCmd", Error::UnsupportedCapability),
        ("slowDown", Error::Provider(ProviderError::RateLimited)),
        ("arbitrary", Error::Provider(ProviderError::Rpc)),
    ] {
        let reply = Reply::raw(
            200,
            serde_json::to_vec(
                &json!({"result":{"status":"error","error":name,"error_code":19,"error_message":"SECRET_REMOTE_BODY","request":{"secret":"SECRET_REMOTE_BODY"}}}),
            )?,
        );
        let fixture = Fixture::start(vec![network()?, network()?, ledger()?, reply]).await?;
        let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
        let actual = client
            .get_account_balance(Address::parse(ACCOUNT)?, None)
            .await
            .unwrap_err();
        assert_eq!(actual, expected);
        assert!(!format!("{actual:?} {actual}").contains("SECRET_REMOTE_BODY"));
    }
    Ok(())
}

#[tokio::test]
async fn safe_read_retries_reuse_identical_hash_account_and_body()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        network()?,
        network()?,
        ledger()?,
        Reply::raw(503, b"SECRET".to_vec()),
        result(account())?,
    ])
    .await?;
    let client = XrplClient::connect(config(
        &fixture.endpoint,
        1,
        Duration::from_secs(2),
        1_048_576,
    )?)
    .await?;
    assert!(
        client
            .get_account_balance(Address::parse(ACCOUNT)?, None)
            .await
            .is_ok()
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[3].body, requests[4].body);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.body["method"] == "ledger")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn total_deadline_covers_network_ledger_and_read_not_each_request()
-> Result<(), Box<dyn std::error::Error>> {
    let mut verify = network()?;
    // Both delayed stages fit individually, but consume one shared deadline.
    verify.delay = Duration::from_millis(1200);
    let mut selected = ledger()?;
    selected.delay = Duration::from_millis(1200);
    let fixture = Fixture::start(vec![network()?, verify, selected, result(account())?]).await?;
    let client = XrplClient::connect(config(
        &fixture.endpoint,
        0,
        Duration::from_secs(2),
        1_048_576,
    )?)
    .await?;
    assert_eq!(
        client
            .get_account_balance(Address::parse(ACCOUNT)?, None)
            .await,
        Err(Error::Timeout)
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].body["method"], "ledger");
    Ok(())
}

#[tokio::test]
async fn bounded_response_handles_every_framing_without_secret_exposure()
-> Result<(), Box<dyn std::error::Error>> {
    for framing in [
        Framing::ContentLength,
        Framing::CloseDelimited,
        Framing::Chunked,
    ] {
        let mut oversized = Reply::raw(200, vec![b'x'; 2048]);
        oversized.framing = framing;
        let fixture = Fixture::start(vec![network()?, network()?, oversized]).await?;
        let client =
            XrplClient::connect(config(&fixture.endpoint, 0, Duration::from_secs(2), 512)?).await?;
        assert_eq!(
            client.get_fee_estimate().await,
            Err(Error::Provider(ProviderError::ResponseTooLarge))
        );
    }
    let fixture =
        Fixture::start(vec![network()?, network()?, Reply::result(&json!(null))?]).await?;
    let client = XrplClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.get_fee_estimate().await,
        Err(Error::Provider(ProviderError::InvalidResponse))
    );
    Ok(())
}

#[test]
fn backend_without_runtime_returns_configuration_error_and_diagnostics_redact_endpoint()
-> Result<(), Box<dyn std::error::Error>> {
    let config = standard("https://user:SECRET@node.invalid/path?token=SECRET")?;
    assert!(!format!("{config:?}").contains("SECRET"));
    let mut future = std::pin::pin!(XrplClient::connect(config));
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}
