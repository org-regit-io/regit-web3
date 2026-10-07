// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit
//! Actual bounded TON API-v2 requests through loopback fixtures.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "ton-http")]

use regit_web3::{
    chains::ton::{TonClient, TonHttpConfig, TonReader, TonSubmitter},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::ton::*,
    error::{Error, ProviderError, SubmissionFailure},
};
use serde_json::{Value, json};
use std::{
    future::Future,
    task::{Context, Poll, Waker},
    time::Duration,
};
use tycho_types::{
    boc::Boc as Codec,
    cell::{Cell, CellBuilder, CellFamily, HashBytes},
    models::{ExtInMsgInfo, IntAddr, MsgInfo, OwnedMessage, StdAddr},
};
#[path = "support/rpc_server.rs"]
mod server;
use server::{Fixture, Framing, Reply};

fn network() -> Result<Network, Error> {
    Network::new(
        NetworkCategory::Mainnet,
        ZeroState::new(
            -1,
            Hash::parse("F6OpKZKqvqeFp6CQmFomXNMfMj2EnaUSOXN+Mh+wVWk=")?,
            Hash::parse("XplPz01CXAps5qeSWUtxcyBfdAo5zVb1N979KLSKD24=")?,
        )?,
    )
}
fn address() -> Result<Address, Error> {
    Address::parse("EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrHF")
}
fn config(url: &str, retries: u8, total: Duration, cap: usize) -> Result<TonHttpConfig, Error> {
    TonHttpConfig::new(
        network()?,
        HttpConfig::new(
            RpcEndpoint::new(url)?,
            RpcLimits::new(total.min(Duration::from_secs(5)), total, cap, retries)?,
            "ton-fixture",
        )?,
    )
}
fn standard(url: &str) -> Result<TonHttpConfig, Error> {
    config(url, 0, Duration::from_secs(20), 2 * 1024 * 1024)
}
fn result(value: Value) -> Result<Reply, serde_json::Error> {
    let mut envelope = json!({"ok":true});
    envelope["result"] = value;
    let mut reply = Reply::json(&envelope)?;
    reply.echo_id = false;
    Ok(reply)
}
fn master() -> Result<Value, serde_json::Error> {
    let value: Value = serde_json::from_str(include_str!("fixtures/ton/masterchain.json"))?;
    Ok(value["result"].clone())
}
fn master_reply() -> Result<Reply, serde_json::Error> {
    result(master()?)
}
fn rows() -> Result<Value, serde_json::Error> {
    let value: Value = serde_json::from_str(include_str!("fixtures/ton/transactions.json"))?;
    Ok(value["result"].clone())
}
fn account() -> Result<Value, Box<dyn std::error::Error>> {
    let mut value: Value = serde_json::from_str(include_str!("fixtures/ton/account.json"))?;
    value["result"]["block_id"] = master()?["last"].clone();
    Ok(value["result"].clone())
}
fn cursor() -> Result<Cursor, Box<dyn std::error::Error>> {
    let row = rows()?[0].clone();
    Ok(Cursor::new(
        LogicalTime::parse(row["transaction_id"]["lt"].as_str().ok_or("lt")?)?,
        Hash::parse(row["transaction_id"]["hash"].as_str().ok_or("hash")?)?,
    )?)
}
fn empty() -> Result<Boc, Error> {
    Boc::from_bytes(Codec::encode(Cell::empty_cell()))
}
fn fee_request() -> Result<FeeRequest, Error> {
    FeeRequest::new(network()?, address()?, empty()?, None, None, true)
}
fn fees() -> Value {
    json!({"source_fees":{"in_fwd_fee":1,"storage_fee":2,"gas_fee":9_007_199_254_740_993_u64,"fwd_fee":4},"destination_fees":[{"in_fwd_fee":"5","storage_fee":"6","gas_fee":"7","fwd_fee":"8"}]})
}
fn submission() -> Result<SignedSubmission, Box<dyn std::error::Error>> {
    let address = address()?;
    let root = CellBuilder::build_from(OwnedMessage {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            dst: IntAddr::Std(StdAddr::new(
                address.workchain(),
                HashBytes(*address.account().as_bytes()),
            )),
            ..ExtInMsgInfo::default()
        }),
        init: None,
        body: Cell::empty_cell().into(),
        layout: None,
    })?;
    Ok(SignedSubmission::new(
        network()?,
        address,
        Boc::from_bytes(Codec::encode(root))?,
    )?)
}

#[tokio::test]
async fn eight_actual_methods_preserve_exact_source_identity_and_payloads()
-> Result<(), Box<dyn std::error::Error>> {
    let net = master()?;
    let account = account()?;
    let txs = rows()?;
    let submit = submission()?;
    let hash = submit.message_hash();
    let fixture = Fixture::start_routed(move |r| {
        let reply = if r.target.contains("getMasterchainInfo") {
            result(net.clone())
        } else if r.target.contains("getAddressInformation") {
            result(account.clone())
        } else if r.target.contains("getTransactions") {
            if r.target.contains("limit=1") {
                result(json!([txs[0].clone()]))
            } else {
                result(txs.clone())
            }
        } else if r.target.contains("estimateFee") {
            result(fees())
        } else if r.target.contains("sendBocReturnHash") {
            result(json!({"hash":hash.to_base64()}))
        } else {
            return Err(std::io::Error::other("unexpected path"));
        };
        reply.map_err(std::io::Error::other)
    })
    .await?;
    let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
    let network = client.get_network_data().await?;
    assert_eq!(network.value().network(), self::network()?);
    let balance = client.get_account_balance(address()?).await?;
    assert_eq!(balance.value().balance().native().raw(), 739_241_685_386);
    assert_eq!(balance.context().block(), Some(network.value().last()));
    let page = client
        .get_account_history(HistoryRequest::new(address()?, Some(cursor()?), 2, true)?)
        .await?;
    assert_eq!(page.value().transactions().len(), 2);
    assert!(page.context().block().is_none());
    let tx = client.get_transaction(address()?, cursor()?).await?;
    assert_eq!(tx.value().cursor(), cursor()?);
    let status = client.get_transaction_status(address()?, cursor()?).await?;
    assert!(matches!(
        status.value().transaction().execution(),
        Execution::Ordinary { .. }
    ));
    let message_hash = tx.value().incoming().ok_or("incoming")?.hash()?;
    let message = client
        .get_message_status(
            message_hash,
            HistoryRequest::new(address()?, Some(cursor()?), 2, true)?,
        )
        .await?;
    assert_eq!(message.value().observed(), &[cursor()?]);
    let request = fee_request()?;
    let estimate = client.estimate_fee(request.clone()).await?;
    assert_eq!(estimate.value().request(), &request);
    assert_eq!(
        estimate.value().source().gas_fee.raw(),
        9_007_199_254_740_993
    );
    let accepted = client.submit_signed(submit).await?;
    assert_eq!(accepted.value().source_message_hash(), hash);
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 16);
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.target.contains("getMasterchainInfo"))
            .count(),
        9
    );
    let get = requests
        .iter()
        .find(|r| r.target.contains("getAddressInformation"))
        .ok_or("account request")?;
    assert!(
        get.target
            .contains(&format!("seqno={}", network.value().last().seqno()))
    );
    let post = requests
        .iter()
        .find(|r| r.target.contains("estimateFee"))
        .ok_or("estimate request")?;
    assert_eq!(post.body["ignore_chksig"], true);
    assert!(post.body.get("init_code").is_none());
    assert!(post.headers.starts_with("POST "));
    drop(fixture);
    Ok(())
}
#[tokio::test]
async fn wrong_genesis_fails_before_account_or_write() -> Result<(), Box<dyn std::error::Error>> {
    let mut wrong = master()?;
    wrong["init"]["file_hash"] = json!(Hash::from_bytes([1; 32]).to_base64());
    let fixture = Fixture::start(vec![master_reply()?, result(wrong)?]).await?;
    let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
    assert!(matches!(
        client.submit_signed(submission()?).await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    ));
    assert_eq!(fixture.requests()?.len(), 2);
    drop(fixture);
    Ok(())
}
#[tokio::test]
async fn account_retry_retains_frozen_sequence_and_full_block_match()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        master_reply()?,
        master_reply()?,
        Reply::raw(503, Vec::new()),
        result(account()?)?,
    ])
    .await?;
    let client = TonClient::connect(config(
        &fixture.endpoint,
        2,
        Duration::from_secs(20),
        2 * 1024 * 1024,
    )?)
    .await?;
    client.get_account_balance(address()?).await?;
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].target, requests[3].target);
    assert_eq!(requests[2].body, requests[3].body);
    drop(fixture);
    Ok(())
}
#[tokio::test]
async fn equal_sequence_different_block_hash_is_rejected() -> Result<(), Box<dyn std::error::Error>>
{
    let mut altered = account()?;
    altered["block_id"]["root_hash"] = json!(Hash::from_bytes([2; 32]).to_base64());
    let fixture = Fixture::start(vec![master_reply()?, master_reply()?, result(altered)?]).await?;
    let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
    assert!(matches!(
        client.get_account_balance(address()?).await,
        Err(Error::Provider(ProviderError::InvalidResponse))
    ));
    drop(fixture);
    Ok(())
}
#[tokio::test]
async fn transaction_boc_identity_and_echo_disagreement_fails()
-> Result<(), Box<dyn std::error::Error>> {
    for field in ["hash", "utime", "fee", "incoming"] {
        let mut altered = rows()?[0].clone();
        match field {
            "hash" => {
                altered["transaction_id"]["hash"] = json!(Hash::from_bytes([3; 32]).to_base64());
            }
            "utime" => altered["utime"] = json!(1),
            "fee" => altered["fee"] = json!("26"),
            _ => altered["in_msg"]["hash"] = json!(Hash::from_bytes([4; 32]).to_base64()),
        }
        let fixture = Fixture::start(vec![
            master_reply()?,
            master_reply()?,
            result(json!([altered]))?,
        ])
        .await?;
        let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
        assert!(matches!(
            client.get_transaction(address()?, cursor()?).await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        ));
        drop(fixture);
    }
    Ok(())
}
#[tokio::test]
async fn broken_linkage_and_page_overflow_fail_whole() -> Result<(), Box<dyn std::error::Error>> {
    for (data, limit) in [
        (json!([rows()?[1].clone(), rows()?[0].clone()]), 2),
        (rows()?, 1),
    ] {
        let fixture = Fixture::start(vec![master_reply()?, master_reply()?, result(data)?]).await?;
        let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
        assert!(matches!(
            client
                .get_account_history(HistoryRequest::new(address()?, None, limit, true)?)
                .await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        ));
        drop(fixture);
    }
    Ok(())
}
#[tokio::test]
async fn empty_lookup_is_unavailable_without_fabricated_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture =
        Fixture::start(vec![master_reply()?, master_reply()?, result(json!([]))?]).await?;
    let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
    assert!(matches!(
        client.get_transaction_status(address()?, cursor()?).await,
        Err(Error::UnavailableData)
    ));
    drop(fixture);
    Ok(())
}
#[tokio::test]
async fn duplicate_envelope_and_nested_identity_fields_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let duplicate = format!("{{\"ok\":true,\"ok\":true,\"result\":{}}}", master()?);
    let normal = serde_json::to_string(&master()?)?;
    let last = serde_json::to_string(&master()?["last"])?;
    let nested = normal.replace(
        &last,
        &last.replacen("\"seqno\":", "\"seqno\":1,\"seqno\":", 1),
    );
    for body in [duplicate, format!("{{\"ok\":true,\"result\":{nested}}}")] {
        let fixture = Fixture::start(vec![Reply::raw(200, body.into_bytes())]).await?;
        assert!(matches!(
            TonClient::connect(standard(&fixture.endpoint)?).await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        ));
        drop(fixture);
    }
    Ok(())
}
#[tokio::test]
async fn estimate_uses_frozen_post_retries_and_lexical_exact_integers()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        master_reply()?,
        master_reply()?,
        Reply::raw(503, Vec::new()),
        result(fees())?,
    ])
    .await?;
    let client = TonClient::connect(config(
        &fixture.endpoint,
        2,
        Duration::from_secs(20),
        2 * 1024 * 1024,
    )?)
    .await?;
    client.estimate_fee(fee_request()?).await?;
    let requests = fixture.requests()?;
    assert_eq!(requests[2].target, requests[3].target);
    assert_eq!(requests[2].body, requests[3].body);
    drop(fixture);
    for value in [
        json!(1.0),
        json!({"$serde_json::private::Number":"1"}),
        json!("01"),
    ] {
        let mut invalid = fees();
        invalid["source_fees"]["gas_fee"] = value;
        let fixture =
            Fixture::start(vec![master_reply()?, master_reply()?, result(invalid)?]).await?;
        let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
        assert!(matches!(
            client.estimate_fee(fee_request()?).await,
            Err(Error::Provider(ProviderError::InvalidResponse))
        ));
        drop(fixture);
    }
    Ok(())
}
#[tokio::test]
async fn write_failures_are_ambiguous_and_never_retried() -> Result<(), Box<dyn std::error::Error>>
{
    for status in [429, 500] {
        let fixture = Fixture::start(vec![
            master_reply()?,
            master_reply()?,
            Reply::raw(status, b"private provider secret".to_vec()),
        ])
        .await?;
        let client = TonClient::connect(config(
            &fixture.endpoint,
            3,
            Duration::from_secs(20),
            2 * 1024 * 1024,
        )?)
        .await?;
        let error = client
            .submit_signed(submission()?)
            .await
            .err()
            .ok_or("error")?;
        assert!(matches!(
            error,
            Error::SubmissionOutcomeUnknown(
                SubmissionFailure::RateLimited | SubmissionFailure::HttpStatus
            )
        ));
        assert!(!format!("{error:?} {error}").contains("secret"));
        assert_eq!(fixture.requests()?.len(), 3);
        drop(fixture);
    }
    Ok(())
}
#[tokio::test]
async fn mismatched_submit_acknowledgement_preserves_uncertain_outcome()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        master_reply()?,
        master_reply()?,
        result(json!({"hash":Hash::from_bytes([9;32]).to_base64()}))?,
    ])
    .await?;
    let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
    assert!(matches!(
        client.submit_signed(submission()?).await,
        Err(Error::SubmissionOutcomeUnknown(
            SubmissionFailure::InvalidResponse
        ))
    ));
    drop(fixture);
    Ok(())
}
#[tokio::test]
async fn test_only_mainnet_addresses_fail_before_any_request()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![master_reply()?]).await?;
    let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
    let test = Address::parse(&address()?.to_friendly(
        FriendlyFlags {
            bounceable: true,
            test_only: true,
        },
        true,
    ))?;
    assert!(client.get_account_balance(test).await.is_err());
    assert_eq!(fixture.requests()?.len(), 1);
    drop(fixture);
    Ok(())
}
#[tokio::test]
async fn total_deadline_covers_network_and_account_and_body_limit()
-> Result<(), Box<dyn std::error::Error>> {
    let mut network = master_reply()?;
    network.delay = Duration::from_secs(2);
    let mut account = result(account()?)?;
    account.body_delay = Duration::from_secs(2);
    let fixture = Fixture::start(vec![master_reply()?, network, account]).await?;
    let client = TonClient::connect(config(
        &fixture.endpoint,
        0,
        Duration::from_secs(3),
        2 * 1024 * 1024,
    )?)
    .await?;
    assert!(matches!(
        client.get_account_balance(address()?).await,
        Err(Error::Timeout)
    ));
    assert_eq!(fixture.requests()?.len(), 3);
    drop(fixture);
    for framing in [
        Framing::ContentLength,
        Framing::Chunked,
        Framing::CloseDelimited,
    ] {
        let mut response = result(self::account()?)?;
        response.framing = framing;
        let fixture = Fixture::start(vec![master_reply()?, master_reply()?, response]).await?;
        let client =
            TonClient::connect(config(&fixture.endpoint, 0, Duration::from_secs(20), 1024)?)
                .await?;
        assert!(matches!(
            client.get_account_balance(address()?).await,
            Err(Error::Provider(ProviderError::ResponseTooLarge))
        ));
        drop(fixture);
    }
    Ok(())
}
#[test]
fn connect_without_runtime_is_typed_failure() -> Result<(), Box<dyn std::error::Error>> {
    let config = standard("http://127.0.0.1:1/api/v2")?;
    let mut future = std::pin::pin!(TonClient::connect(config));
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}
#[tokio::test]
async fn unrelated_rpc_envelope_and_private_provider_error_are_safe()
-> Result<(), Box<dyn std::error::Error>> {
    let unrelated = Reply::result(&master()?)?;
    let private = Reply::raw(
        200,
        b"{\"ok\":false,\"error\":\"private provider secret\",\"code\":500}".to_vec(),
    );
    for reply in [unrelated, private] {
        let fixture = Fixture::start(vec![reply]).await?;
        let error = TonClient::connect(standard(&fixture.endpoint)?)
            .await
            .err()
            .ok_or("expected error")?;
        assert!(matches!(
            error,
            Error::Provider(ProviderError::InvalidResponse | ProviderError::Rpc)
        ));
        assert!(!format!("{error:?} {error}").contains("secret"));
        drop(fixture);
    }
    Ok(())
}
#[tokio::test]
async fn caller_spacing_reserves_distinct_slots_for_concurrent_reads_and_retries()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![master_reply()?, master_reply()?, master_reply()?]).await?;
    let settings = standard(&fixture.endpoint)?.with_request_spacing(Duration::from_millis(500))?;
    assert_eq!(settings.request_spacing(), Duration::from_millis(500));
    assert!(
        standard(&fixture.endpoint)?
            .with_request_spacing(Duration::from_secs(61))
            .is_err()
    );
    let client = TonClient::connect(settings).await?;
    let started = std::time::Instant::now();
    let (first, second) = tokio::join!(client.get_network_data(), client.get_network_data());
    first?;
    second?;
    assert!(started.elapsed() >= Duration::from_millis(950));
    // No retry in this client's first config: use an explicit retry-enabled client.
    drop(client);
    drop(fixture);
    let fixture = Fixture::start(vec![
        master_reply()?,
        master_reply()?,
        Reply::raw(503, Vec::new()),
        result(account()?)?,
    ])
    .await?;
    let config = config(
        &fixture.endpoint,
        2,
        Duration::from_secs(20),
        2 * 1024 * 1024,
    )?
    .with_request_spacing(Duration::from_millis(500))?;
    let client = TonClient::connect(config).await?;
    let started = std::time::Instant::now();
    client.get_account_balance(address()?).await?;
    assert!(started.elapsed() >= Duration::from_millis(1450));
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].target, requests[3].target);
    assert_eq!(requests[2].body, requests[3].body);
    Ok(())
}
#[tokio::test]
async fn spacing_and_retry_wait_share_one_total_deadline() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::start(vec![
        master_reply()?,
        master_reply()?,
        Reply::raw(503, Vec::new()),
    ])
    .await?;
    let settings = config(
        &fixture.endpoint,
        3,
        Duration::from_secs(5),
        2 * 1024 * 1024,
    )?
    .with_request_spacing(Duration::from_secs(2))?;
    let client = TonClient::connect(settings).await?;
    assert!(matches!(
        client.get_account_balance(address()?).await,
        Err(Error::Timeout)
    ));
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert!(requests[2].target.contains("getAddressInformation"));
    Ok(())
}
#[tokio::test]
async fn dispatched_write_body_timeout_and_rpc_error_remain_ambiguous()
-> Result<(), Box<dyn std::error::Error>> {
    let mut late = result(json!({"hash":submission()?.message_hash().to_base64()}))?;
    late.body_delay = Duration::from_secs(4);
    let fixture = Fixture::start(vec![master_reply()?, master_reply()?, late]).await?;
    let client = TonClient::connect(config(
        &fixture.endpoint,
        3,
        Duration::from_secs(3),
        2 * 1024 * 1024,
    )?)
    .await?;
    assert!(matches!(
        client.submit_signed(submission()?).await,
        Err(Error::SubmissionOutcomeUnknown(SubmissionFailure::Timeout))
    ));
    assert_eq!(fixture.requests()?.len(), 3);
    drop(fixture);
    let fixture = Fixture::start(vec![
        master_reply()?,
        master_reply()?,
        Reply::raw(
            200,
            b"{\"ok\":false,\"error\":\"private remote detail\"}".to_vec(),
        ),
    ])
    .await?;
    let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
    assert!(matches!(
        client.submit_signed(submission()?).await,
        Err(Error::SubmissionOutcomeUnknown(SubmissionFailure::Rpc))
    ));
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test]
async fn outgoing_aggregate_fee_is_not_decoded_total_fees() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture: Value = serde_json::from_str(include_str!("fixtures/ton/outgoing.json"))?;
    let row = fixture["result"][0].clone();
    let cursor = Cursor::new(
        LogicalTime::parse(row["transaction_id"]["lt"].as_str().ok_or("lt")?)?,
        Hash::parse(row["transaction_id"]["hash"].as_str().ok_or("hash")?)?,
    )?;
    let server = Fixture::start(vec![
        master_reply()?,
        master_reply()?,
        result(json!([row.clone()]))?,
    ])
    .await?;
    let client = TonClient::connect(standard(&server.endpoint)?).await?;
    let tx = client.get_transaction(address()?, cursor).await?;
    assert_eq!(tx.value().fees().native().raw(), 442_944);
    assert_eq!(
        tx.value()
            .provider_fees()
            .ok_or("provider fees")?
            .aggregate()
            .raw(),
        507_479
    );
    assert_eq!(tx.value().outgoing().len(), 1);
    assert_eq!(
        serde_json::from_slice::<Observation<Transaction>>(&serde_json::to_vec(&tx)?)?,
        tx
    );
    drop(server);
    for field in ["aggregate", "sum", "hash", "forwarding", "duplicate_fee"] {
        let mut altered = row.clone();
        match field {
            "aggregate" => {
                altered["fee"] = json!("507480");
                altered["other_fee"] = json!("496804");
            }
            "sum" => altered["other_fee"] = json!("496804"),
            "hash" => altered["out_msgs"][0]["hash"] = json!(Hash::from_bytes([1; 32]).to_base64()),
            "forwarding" => {
                altered["out_msgs"][0]["fwd_fee"] = json!("64536");
                altered["fee"] = json!("507480");
                altered["other_fee"] = json!("496804");
            }
            "duplicate_fee" => {}
            _ => return Err("field".into()),
        }
        let mut reply = result(json!([altered]))?;
        if field == "duplicate_fee" {
            let body = String::from_utf8(reply.body)?;
            reply.body = body
                .replacen(
                    "\"fwd_fee\":\"64535\"",
                    "\"fwd_fee\":\"64535\",\"fwd_fee\":\"64535\"",
                    1,
                )
                .into_bytes();
        }
        let server = Fixture::start(vec![master_reply()?, master_reply()?, reply]).await?;
        let client = TonClient::connect(standard(&server.endpoint)?).await?;
        assert!(
            matches!(
                client.get_transaction(address()?, cursor).await,
                Err(Error::Provider(ProviderError::InvalidResponse))
            ),
            "{field}"
        );
        drop(server);
    }
    Ok(())
}

#[tokio::test]
async fn signed_source_currency_id_preserves_protocol_bits()
-> Result<(), Box<dyn std::error::Error>> {
    let mut account = account()?;
    account["extra_currencies"] = json!([{"id": -1, "amount": "9007199254740993"}, {"id": -2_147_483_648_i32, "amount": "7"}]);
    let fixture = Fixture::start(vec![master_reply()?, master_reply()?, result(account)?]).await?;
    let client = TonClient::connect(standard(&fixture.endpoint)?).await?;
    let balance = client.get_account_balance(address()?).await?;
    assert_eq!(balance.value().balance().extra()[0].id, u32::MAX);
    assert_eq!(
        balance.value().balance().extra()[0]
            .amount
            .raw()
            .to_string(),
        "9007199254740993"
    );
    assert_eq!(balance.value().balance().extra()[1].id, 1u32 << 31);
    assert_eq!(
        serde_json::from_slice::<Observation<AccountBalance>>(&serde_json::to_vec(&balance)?)?,
        balance
    );
    drop(fixture);
    Ok(())
}
