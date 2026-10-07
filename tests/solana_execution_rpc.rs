// SPDX-License-Identifier: Apache-2.0
// Copyright (client) 2026 Regit

//! Deterministic Solana execution/read and isolated one-shot write contracts.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "solana-http")]

use base64::{Engine as _, engine::general_purpose::STANDARD};
use regit_web3::{
    chains::solana::{SolanaClient, SolanaHttpConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::solana::*,
    error::{Error, ProviderError},
};
use serde_json::{Value, json};
use std::time::Duration;
#[path = "support/rpc_server.rs"]
mod server;
use server::{Fixture, Framing, Reply};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
fn config(
    endpoint: &str,
    retries: u8,
    timeout: Duration,
    cap: usize,
) -> Result<SolanaHttpConfig, Error> {
    Ok(SolanaHttpConfig::new(
        Network::new(Hash::parse(GENESIS)?, "fixture")?,
        HttpConfig::new(
            RpcEndpoint::new(endpoint)?,
            RpcLimits::new(timeout, timeout, cap, retries)?,
            "local",
        )?,
    ))
}
fn standard_config(endpoint: &str) -> Result<SolanaHttpConfig, Error> {
    config(endpoint, 0, Duration::from_secs(5), 1024 * 1024)
}
fn genesis() -> Result<Reply, serde_json::Error> {
    Reply::result(&json!(GENESIS))
}
fn response(value: &Value, slot: u64) -> Result<Reply, serde_json::Error> {
    Reply::result(&json!({"context":{"slot":slot,"apiVersion":"4.3.0"},"value":value}))
}
fn options() -> ReadOptions {
    ReadOptions::new(Commitment::Confirmed, Some(100))
}
fn source_transaction() -> Result<Value, serde_json::Error> {
    serde_json::from_str(include_str!(
        "fixtures/solana_execution/mainnet_v1_transaction.json"
    ))
}
fn signed() -> Result<SignedTransaction, Box<dyn std::error::Error>> {
    Ok(SignedTransaction::from_bytes(const_hex::decode(
        include_str!("fixtures/solana_execution/mainnet_v1_transaction.hex").trim(),
    )?)?)
}
fn unsigned() -> Result<UnsignedTransaction, Box<dyn std::error::Error>> {
    Ok(UnsignedTransaction::from_message(
        signed()?.message().clone(),
    )?)
}
async fn client_fixture(
    reply: Reply,
) -> Result<(SolanaClient, Fixture), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![genesis()?, genesis()?, reply]).await?;
    let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
    Ok((client, fixture))
}

#[tokio::test(flavor = "current_thread")]
async fn retrieves_real_v1_bytes_exact_metadata_and_only_supported_lookup_controls() -> TestResult {
    let tx = signed()?;
    let read_options = TransactionReadOptions::new(Commitment::Confirmed, 1)?;
    let (client, fixture) = client_fixture(Reply::result(&source_transaction()?)?).await?;
    let observed = client.get_transaction(tx.signature(), read_options).await?;
    let value = observed
        .value()
        .transaction
        .as_ref()
        .ok_or("missing transaction")?;
    assert_eq!(value.body(), &tx);
    assert_eq!(value.inclusion_slot(), 454_111_150);
    assert_eq!(value.transaction_index(), Some(1064));
    let message = value.metadata().ok_or("missing metadata")?.data();
    assert_eq!(message.fee_lamports, 5410);
    assert_eq!(message.compute_units_consumed, Some(16988));
    assert_eq!(message.pre_token_balances.as_ref().map(Vec::len), Some(16));
    assert_eq!(message.outcome, ExecutionOutcome::Succeeded);
    assert_eq!(observed.context().evaluation_slot(), None);
    assert_eq!(observed.context().source().method(), "getTransaction");
    assert_eq!(
        serde_json::from_value::<ExecutionObservation<TransactionLookup>>(serde_json::to_value(
            &observed
        )?)?,
        observed
    );
    assert_eq!(fixture.requests()?[2].target, "/rpc");
    assert!(
        fixture.requests()?[2]
            .headers
            .to_ascii_lowercase()
            .contains("content-type: application/json")
    );
    assert_eq!(
        fixture.requests()?[2].body["params"],
        json!([tx.signature(),{"encoding":"base64","commitment":"confirmed","maxSupportedTransactionVersion":1}])
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn transaction_absence_is_unknown_and_metadata_absence_is_not_zero_fee() -> TestResult {
    let signature = signed()?.signature();
    let read_options = TransactionReadOptions::new(Commitment::Finalized, 1)?;
    let (client, fixture) = client_fixture(Reply::result(&Value::Null)?).await?;
    assert!(
        client
            .get_transaction(signature, read_options)
            .await?
            .value()
            .transaction
            .is_none()
    );
    assert_eq!(fixture.requests()?.len(), 3);
    let mut value = source_transaction()?;
    value["meta"] = Value::Null;
    let (client, _fixture) = client_fixture(Reply::result(&value)?).await?;
    assert!(
        client
            .get_transaction(signature, read_options)
            .await?
            .value()
            .transaction
            .as_ref()
            .and_then(Transaction::metadata)
            .is_none()
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn transaction_rejects_query_version_signature_structure_and_metadata_mismatch() -> TestResult
{
    let signature = signed()?.signature();
    let read_options = TransactionReadOptions::new(Commitment::Confirmed, 1)?;
    let original = source_transaction()?;
    let mut bads = Vec::new();
    let mut value = original.clone();
    value["version"] = json!(0);
    bads.push(value);
    let mut value = original.clone();
    value["meta"]["postBalances"]
        .as_array_mut()
        .ok_or("balances not array")?
        .pop();
    bads.push(value);
    let mut value = original.clone();
    value["meta"]["preTokenBalances"][0]["accountIndex"] = json!(255);
    bads.push(value);
    let mut value = original.clone();
    value["meta"]["loadedAddresses"]["writable"] = json!([Pubkey::from_bytes([1; 32])]);
    bads.push(value);
    let mut value = original.clone();
    value["meta"]["status"] = json!({"Err":"AccountNotFound"});
    bads.push(value);
    let mut value = original.clone();
    value["transaction"][1] = json!("json");
    bads.push(value);
    let mut value = original.clone();
    value["transaction"][0] = json!(format!(
        "{}AA==",
        value["transaction"][0].as_str().ok_or("not encoded")?
    ));
    bads.push(value);
    let mut value = original.clone();
    value["meta"]
        .as_object_mut()
        .ok_or("not meta")?
        .remove("err");
    bads.push(value);
    for value in bads {
        let (client, fixture) = client_fixture(Reply::result(&value)?).await?;
        assert_eq!(
            client
                .get_transaction(signature, read_options)
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    let (client, _fixture) = client_fixture(Reply::result(&original)?).await?;
    assert_eq!(
        client
            .get_transaction(Signature::from_bytes([9; 64]), read_options)
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::InvalidResponse)
    );
    let (client, _fixture) = client_fixture(Reply::result(&original)?).await?;
    assert_eq!(
        client
            .get_transaction(
                signature,
                TransactionReadOptions::new(Commitment::Confirmed, 0)?
            )
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::InvalidResponse)
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn status_retains_actual_execution_confirmation_and_evaluation_slot_separately() -> TestResult
{
    let signature = signed()?.signature();
    let read_options = StatusOptions {
        search_transaction_history: true,
    };
    let wire = json!([{"slot":99,"confirmations":1,"err":{"InstructionError":[0,{"Custom":6000}]},"status":{"Err":{"InstructionError":[0,{"Custom":6000}]}},"confirmationStatus":"confirmed"}]);
    let (client, fixture) = client_fixture(response(&wire, 101)?).await?;
    let observed = client
        .get_transaction_status(signature, read_options)
        .await?;
    assert_eq!(observed.context().evaluation_slot(), Some(101));
    let status = observed.value().status.as_ref().ok_or("missing status")?;
    assert_eq!(status.inclusion_slot, 99);
    assert_eq!(status.confirmations, Some(1));
    assert!(matches!(status.outcome, ExecutionOutcome::Failed { .. }));
    assert_eq!(
        fixture.requests()?[2].body["params"],
        json!([[signature],{"searchTransactionHistory":true}])
    );
    assert_eq!(
        serde_json::from_value::<ExecutionObservation<StatusLookup>>(serde_json::to_value(
            &observed
        )?)?,
        observed
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn status_null_cache_lookup_does_not_invent_pending_or_confirmation() -> TestResult {
    let (client, _fixture) = client_fixture(response(&json!([null]), 100)?).await?;
    let observed = client
        .get_transaction_status(
            signed()?.signature(),
            StatusOptions {
                search_transaction_history: false,
            },
        )
        .await?;
    assert!(observed.value().status.is_none());
    let (client, _fixture) = client_fixture(response(
        &json!([{"slot":90,"confirmations":null,"err":null}]),
        100,
    )?)
    .await?;
    let observed = client
        .get_transaction_status(
            signed()?.signature(),
            StatusOptions {
                search_transaction_history: true,
            },
        )
        .await?;
    assert_eq!(
        observed
            .value()
            .status
            .as_ref()
            .and_then(|signature| signature.confirmation_status),
        None
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn status_rejects_wrong_width_missing_error_slot_and_inconsistent_legacy_result() -> TestResult
{
    for value in [
        json!([]),
        json!([null, null]),
        json!([{"slot":90,"confirmations":1}]),
        json!([{"slot":110,"confirmations":1,"err":null}]),
        json!([{"slot":90,"confirmations":1,"err":null,"status":{"Err":"AccountNotFound"}}]),
        json!([{"slot":90,"confirmations":1,"err":null,"confirmationStatus":"finalized"}]),
    ] {
        let (client, _fixture) = client_fixture(response(&value, 100)?).await?;
        assert_eq!(
            client
                .get_transaction_status(
                    signed()?.signature(),
                    StatusOptions {
                        search_transaction_history: true
                    }
                )
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn latest_blockhash_retains_block_height_distinct_from_context_slot() -> TestResult {
    let blockhash = Hash::from_bytes([8; 32]);
    let (client, fixture) = client_fixture(response(
        &json!({"blockhash":blockhash,"lastValidBlockHeight":9_007_199_254_740_993_u64}),
        101,
    )?)
    .await?;
    let observed = client.get_latest_blockhash(options()).await?;
    assert_eq!(
        observed.value().lifetime.last_valid_block_height,
        9_007_199_254_740_993
    );
    assert_eq!(observed.context().evaluation_slot(), Some(101));
    assert_eq!(
        fixture.requests()?[2].body["params"],
        json!([{"commitment":"confirmed","minContextSlot":100}])
    );
    assert_eq!(
        serde_json::from_value::<ExecutionObservation<LatestBlockhash>>(serde_json::to_value(
            &observed
        )?)?,
        observed
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn validity_and_block_height_keep_their_actual_method_contexts() -> TestResult {
    let blockhash = Hash::from_bytes([8; 32]);
    let (client, fixture) = client_fixture(response(&json!(false), 100)?).await?;
    let observed = client.is_blockhash_valid(blockhash, options()).await?;
    assert!(!observed.value().valid);
    assert_eq!(observed.value().blockhash, blockhash);
    assert_eq!(
        fixture.requests()?[2].body["params"],
        json!([blockhash,{"commitment":"confirmed","minContextSlot":100}])
    );
    let (client, fixture) = client_fixture(Reply::result(&json!(u64::MAX))?).await?;
    let observed = client.get_block_height(Commitment::Finalized).await?;
    assert_eq!(observed.value().height, u64::MAX);
    assert_eq!(observed.context().evaluation_slot(), None);
    assert_eq!(
        fixture.requests()?[2].body["params"],
        json!([{"commitment":"finalized"}])
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn exact_fee_null_unavailable_and_retry_preserve_original_message_bytes() -> TestResult {
    let message = unsigned()?.message().clone();
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        Reply::raw(503, b"FAKE_SECRET".to_vec()),
        response(&json!(9_007_199_254_740_993_u64), 100)?,
    ])
    .await?;
    let client = SolanaClient::connect(config(
        &fixture.endpoint,
        1,
        Duration::from_secs(5),
        1024 * 1024,
    )?)
    .await?;
    let observed = client
        .get_fee_for_message(message.clone(), options())
        .await?;
    assert_eq!(observed.value().lamports, Some(9_007_199_254_740_993));
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].body, requests[3].body);
    assert_eq!(
        requests[2].body["params"][0],
        STANDARD.encode(message.bytes())
    );
    let (client, _fixture) = client_fixture(response(&Value::Null, 100)?).await?;
    assert_eq!(
        client
            .get_fee_for_message(message, options())
            .await?
            .value()
            .lamports,
        None
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn simulation_preserves_exact_bytes_fixed_controls_source_failure_logs_and_return_data()
-> TestResult {
    let transaction = unsigned()?;
    let value = json!({"err":"InsufficientFundsForFee","logs":["","FAKE_SECRET\n\u{0000}"],"unitsConsumed":9_007_199_254_740_993_u64,"fee":5000,"returnData":{"programId":Pubkey::from_bytes([2;32]),"data":["AQID","base64"]}});
    let (client, fixture) = client_fixture(response(&value, 100)?).await?;
    let observed = client
        .simulate_transaction(transaction.clone(), options())
        .await?;
    assert!(matches!(
        observed.value().outcome(),
        ExecutionOutcome::Failed { .. }
    ));
    assert_eq!(
        observed.value().units_consumed(),
        Some(9_007_199_254_740_993)
    );
    assert_eq!(
        observed.value().return_data().map(|d| d.data.bytes()),
        Some([1, 2, 3].as_slice())
    );
    assert!(!format!("{observed:?}").contains("FAKE_SECRET"));
    assert_eq!(
        fixture.requests()?[2].body["params"],
        json!([STANDARD.encode(transaction.bytes()),{"encoding":"base64","commitment":"confirmed","minContextSlot":100,"sigVerify":false,"replaceRecentBlockhash":false}])
    );
    assert_eq!(
        serde_json::from_value::<ExecutionObservation<Simulation>>(serde_json::to_value(
            &observed
        )?)?,
        observed
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn minimum_context_and_fixed_safe_source_errors_are_enforced_across_execution_reads()
-> TestResult {
    let (client, _fixture) = client_fixture(response(&json!(true), 99)?).await?;
    assert_eq!(
        client
            .is_blockhash_valid(Hash::from_bytes([1; 32]), options())
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::InvalidResponse)
    );
    let source = Reply::json(
        &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32601,"message":"FAKE_SECRET","data":{"secret":"FAKE_KEY"}}}),
    )?;
    let (client, fixture) = client_fixture(source).await?;
    let error = client.get_latest_blockhash(options()).await.unwrap_err();
    assert_eq!(error, Error::UnsupportedCapability);
    assert!(!format!("{error:?} {error}").contains("FAKE_"));
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn full_transaction_body_limits_and_shared_network_read_deadline_are_enforced() -> TestResult
{
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        Reply::result(&source_transaction()?)?,
    ])
    .await?;
    let client =
        SolanaClient::connect(config(&fixture.endpoint, 0, Duration::from_secs(5), 512)?).await?;
    assert_eq!(
        client
            .get_transaction(
                signed()?.signature(),
                TransactionReadOptions::new(Commitment::Confirmed, 1)?
            )
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::ResponseTooLarge)
    );
    assert_eq!(fixture.requests()?.len(), 3);
    let mut verification = genesis()?;
    verification.delay = Duration::from_millis(1200);
    let mut read = response(&json!(true), 100)?;
    read.delay = Duration::from_millis(1200);
    let fixture = Fixture::start(vec![genesis()?, verification, read]).await?;
    let client = SolanaClient::connect(config(
        &fixture.endpoint,
        0,
        Duration::from_secs(2),
        1024 * 1024,
    )?)
    .await?;
    assert_eq!(
        client
            .is_blockhash_valid(Hash::from_bytes([1; 32]), options())
            .await
            .unwrap_err(),
        Error::Timeout
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].body["method"], "isBlockhashValid");
    Ok(())
}
fn submit_options() -> SubmitOptions {
    SubmitOptions {
        preflight_commitment: Commitment::Confirmed,
        minimum_context_slot: Some(100),
        skip_preflight: false,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn streaming_response_framings_preserve_values_and_enforce_body_caps() -> TestResult {
    for framing in [Framing::Chunked, Framing::CloseDelimited] {
        let mut reply = response(&json!(true), 100)?;
        reply.framing = framing;
        let (client, _fixture) = client_fixture(reply).await?;
        assert!(
            client
                .is_blockhash_valid(Hash::from_bytes([1; 32]), options())
                .await?
                .value()
                .valid
        );
    }
    for framing in [Framing::Chunked, Framing::CloseDelimited] {
        let mut reply = Reply::result(&source_transaction()?)?;
        reply.framing = framing;
        let fixture = Fixture::start(vec![genesis()?, genesis()?, reply]).await?;
        let client =
            SolanaClient::connect(config(&fixture.endpoint, 0, Duration::from_secs(5), 512)?)
                .await?;
        assert_eq!(
            client
                .get_transaction(
                    signed()?.signature(),
                    TransactionReadOptions::new(Commitment::Confirmed, 1)?
                )
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::ResponseTooLarge)
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn caller_signed_submission_sends_once_with_zero_node_retries_matching_ack_only() -> TestResult
{
    let transaction = signed()?;
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        Reply::result(&json!(transaction.signature()))?,
    ])
    .await?;
    let client = SolanaClient::connect(config(
        &fixture.endpoint,
        3,
        Duration::from_secs(5),
        1024 * 1024,
    )?)
    .await?;
    let observed = client
        .submit_signed(transaction.clone(), submit_options())
        .await?;
    assert_eq!(observed.value().signature, transaction.signature());
    assert_eq!(observed.context().evaluation_slot(), None);
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].body["method"], "sendTransaction");
    assert_eq!(
        requests[2].body["params"],
        json!([STANDARD.encode(transaction.bytes()),{"encoding":"base64","preflightCommitment":"confirmed","minContextSlot":100,"skipPreflight":false,"maxRetries":0}])
    );
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn submission_source_failures_malformed_and_wrong_identity_retain_unknown_once() -> TestResult
{
    let mut replies = vec![
        Reply::raw(429, b"FAKE_SECRET".to_vec()),
        Reply::raw(503, b"FAKE_SECRET".to_vec()),
        Reply::result(&Value::Null)?,
        Reply::result(&json!(Signature::from_bytes([7; 64])))?,
        Reply::raw(200, b"FAKE_SECRET".to_vec()),
        Reply::json(
            &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32002,"message":"FAKE_SECRET","data":{"err":"AccountNotFound"}}}),
        )?,
    ];
    let mut disconnected = Reply::raw(200, vec![]);
    disconnected.close_connection = true;
    replies.push(disconnected);
    for reply in replies {
        let fixture = Fixture::start(vec![genesis()?, genesis()?, reply]).await?;
        let client = SolanaClient::connect(config(
            &fixture.endpoint,
            3,
            Duration::from_secs(5),
            1024 * 1024,
        )?)
        .await?;
        let error = client
            .submit_signed(signed()?, submit_options())
            .await
            .unwrap_err();
        assert!(matches!(error, Error::SubmissionOutcomeUnknown(_)));
        assert!(!format!("{error:?} {error}").contains("FAKE_"));
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn changed_genesis_prevents_submission_and_local_invalid_bytes_never_dispatch() -> TestResult
{
    let fixture = Fixture::start(vec![
        genesis()?,
        Reply::result(&json!(Hash::from_bytes([9; 32])))?,
    ])
    .await?;
    let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
    assert_eq!(
        client
            .submit_signed(signed()?, submit_options())
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::ChainMismatch)
    );
    assert!(SignedTransaction::from_bytes(unsigned()?.bytes().to_vec()).is_err());
    assert_eq!(fixture.requests()?.len(), 2);
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
async fn post_dispatch_body_timeout_is_possible_submission_and_never_retried() -> TestResult {
    let mut ack = Reply::result(&json!(signed()?.signature()))?;
    ack.body_delay = Duration::from_secs(3);
    let fixture = Fixture::start(vec![genesis()?, genesis()?, ack]).await?;
    let client = SolanaClient::connect(config(
        &fixture.endpoint,
        3,
        Duration::from_secs(2),
        1024 * 1024,
    )?)
    .await?;
    assert!(matches!(
        client
            .submit_signed(signed()?, submit_options())
            .await
            .unwrap_err(),
        Error::SubmissionOutcomeUnknown(_)
    ));
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn transaction_nested_lists_duplicates_and_malformed_units_are_terminal() -> TestResult {
    let original = source_transaction()?;
    let mut cases = Vec::new();
    let mut value = original.clone();
    value["meta"]["preBalances"] = json!(vec![0_u64; 257]);
    cases.push(value);
    let mut value = original.clone();
    value["meta"]["preTokenBalances"] =
        json!(vec![value["meta"]["preTokenBalances"][0].clone(); 257]);
    cases.push(value);
    let mut value = original.clone();
    value["meta"]["preTokenBalances"][0]["uiTokenAmount"]["amount"] = json!("18446744073709551616");
    cases.push(value);
    let mut value = original.clone();
    value["meta"]["preTokenBalances"][0]["uiTokenAmount"]["amount"] = json!("01");
    cases.push(value);
    let mut value = original.clone();
    value["meta"]["fee"] = json!(1.5);
    cases.push(value);
    let mut value = original.clone();
    value["meta"]["returnData"] =
        json!({"programId": Pubkey::from_bytes([1;32]), "data":["AQID","base58"]});
    cases.push(value);
    let mut value = original.clone();
    value["meta"]["logMessages"] = json!(["FAKE_SECRET".repeat(2048)]);
    cases.push(value);
    for value in cases {
        let (client, fixture) = client_fixture(Reply::result(&value)?).await?;
        let error = client
            .get_transaction(
                signed()?.signature(),
                TransactionReadOptions::new(Commitment::Confirmed, 1)?,
            )
            .await
            .unwrap_err();
        assert_eq!(error, Error::Provider(ProviderError::InvalidResponse));
        assert!(!format!("{error:?} {error}").contains("FAKE_"));
        assert_eq!(fixture.requests()?.len(), 3);
    }
    let result =
        serde_json::to_string(&original)?.replacen("\"fee\":5410", "\"fee\":5410,\"fee\":6000", 1);
    let raw = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{result}}}");
    let (client, fixture) = client_fixture(Reply::raw(200, raw.into_bytes())).await?;
    assert_eq!(
        client
            .get_transaction(
                signed()?.signature(),
                TransactionReadOptions::new(Commitment::Confirmed, 1)?
            )
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::InvalidResponse)
    );
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_after_observed_submission_dispatch_leaves_no_detached_retry() -> TestResult {
    let mut reply = Reply::result(&json!(signed()?.signature()))?;
    reply.delay = Duration::from_secs(3);
    let fixture = Fixture::start(vec![genesis()?, genesis()?, reply]).await?;
    let client = SolanaClient::connect(config(
        &fixture.endpoint,
        3,
        Duration::from_secs(5),
        1024 * 1024,
    )?)
    .await?;
    let mut submission = Box::pin(client.submit_signed(signed()?, submit_options()));
    let observed = tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            tokio::select! {
                result = &mut submission => return Err(format!("unexpected submission completion: {result:?}")),
                () = tokio::time::sleep(Duration::from_millis(10)) => {
                    if fixture.requests().map_err(|e|e.to_string())?.len() == 3 { return Ok(()); }
                }
            }
        }
    }).await?;
    observed.map_err(std::io::Error::other)?;
    assert_eq!(fixture.requests()?[2].body["method"], "sendTransaction");
    // Dropping the future cancels local polling only; the captured write remains possible.
    drop(submission);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn source_reported_simulation_replacement_contradicts_frozen_hash_controls() -> TestResult {
    let value = json!({"err":null,"logs":[],"replacementBlockhash":{"blockhash":Hash::from_bytes([9;32]),"lastValidBlockHeight":200}});
    let (client, fixture) = client_fixture(response(&value, 100)?).await?;
    assert_eq!(
        client
            .simulate_transaction(unsigned()?, options())
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::InvalidResponse)
    );
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn legacy_and_v0_retrieval_preserve_source_loaded_accounts_and_exact_metadata() -> TestResult
{
    let prepared = TransferPreparation::new(
        Network::new(Hash::parse(GENESIS)?, "fixture")?,
        TransferIntent::Native {
            fee_payer: Pubkey::from_bytes([1; 32]),
            sender: Pubkey::from_bytes([1; 32]),
            recipient: Pubkey::from_bytes([2; 32]),
            lamports: 1,
        },
        BlockhashLifetime {
            blockhash: Hash::from_bytes([3; 32]),
            last_valid_block_height: 100,
        },
    )?;
    let solana_message::VersionedMessage::Legacy(legacy) =
        prepared.unsigned_transaction().message().decoded_message()
    else {
        return Err("expected legacy".into());
    };
    let v0 = solana_message::v0::Message {
        header: legacy.header,
        account_keys: legacy.account_keys.clone(),
        recent_blockhash: legacy.recent_blockhash,
        instructions: legacy.instructions.clone(),
        address_table_lookups: vec![solana_message::v0::MessageAddressTableLookup {
            account_key: solana_address::Address::new_from_array([4; 32]),
            writable_indexes: vec![5],
            readonly_indexes: vec![6],
        }],
    };
    for (message, wire_version, loaded) in [
        (
            solana_message::VersionedMessage::Legacy(legacy),
            json!("legacy"),
            json!({"writable":[],"readonly":[]}),
        ),
        (
            solana_message::VersionedMessage::V0(v0),
            json!(0),
            json!({"writable":[Pubkey::from_bytes([8;32])],"readonly":[Pubkey::from_bytes([9;32])]}),
        ),
    ] {
        let transaction = solana_transaction::versioned::VersionedTransaction {
            message,
            signatures: vec![solana_signature::Signature::from([7; 64])],
        };
        let body = SignedTransaction::from_bytes(wincode::serialize(&transaction)?)?;
        let count = body.message().account_count();
        let value = json!({"transaction":[STANDARD.encode(body.bytes()),"base64"],"slot":90,"blockTime":null,"version":wire_version,"meta":{"err":null,"fee":9_007_199_254_740_993_u64,"preBalances":vec![0_u64;count],"postBalances":vec![0_u64;count],"loadedAddresses":loaded,"innerInstructions":[{"index":0,"instructions":[{"programIdIndex":2,"accounts":[0,1],"data":"Ldp","stackHeight":2}]}],"preTokenBalances":[],"postTokenBalances":[],"logMessages":[],"rewards":[]}});
        let (client, _fixture) = client_fixture(Reply::result(&value)?).await?;
        let observed = client
            .get_transaction(
                body.signature(),
                TransactionReadOptions::new(Commitment::Confirmed, 1)?,
            )
            .await?;
        let actual = observed
            .value()
            .transaction
            .as_ref()
            .ok_or("missing transaction")?;
        assert_eq!(actual.body(), &body);
        let metadata = actual.metadata().ok_or("missing metadata")?.data();
        assert_eq!(metadata.fee_lamports, 9_007_199_254_740_993);
        assert_eq!(metadata.pre_token_balances, Some(vec![]));
        assert_eq!(metadata.inner_instructions.as_ref().map(Vec::len), Some(1));
        assert_eq!(
            serde_json::from_value::<ExecutionObservation<TransactionLookup>>(
                serde_json::to_value(&observed)?
            )?,
            observed
        );
    }
    Ok(())
}
