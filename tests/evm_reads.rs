// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Deterministic ERC-20 and transaction/receipt/status HTTP qualification.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "evm-http")]

#[path = "support/rpc_server.rs"]
mod rpc_server;
use regit_web3::{
    chains::evm::{Erc20Reader, EvmClient, TransactionReader},
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::evm::{
        Erc20Metadata, ExecutionOutcome, MetadataUnavailable, MetadataValue, OperationObservation,
        ReadState, TransactionId, TransactionKind, TransactionState, Word,
    },
    domain::{Address, BlockHash, BlockSelector, ChainId, Finality, NetworkId, U256},
    error::{Error, ProviderError},
};
use rpc_server::{Fixture, Framing, Reply};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const CONTRACT: &str = "0x0000000000000000000000000000000000000001";
const OWNER: &str = "0x0000000000000000000000000000000000000002";
const SPENDER: &str = "0x0000000000000000000000000000000000000003";
fn hash() -> TransactionId {
    TransactionId::from_bytes([1; 32])
}
fn block_hash() -> String {
    format!("0x{}", "ab".repeat(32))
}
fn block() -> Value {
    json!({"number":"0x2a","hash":block_hash(),"timestamp":"0x6553f100"})
}
fn config(
    fixture: &Fixture,
    retries: u8,
    bytes: usize,
    timeout: Duration,
) -> Result<EvmConfig, Error> {
    EvmConfig::new(
        NetworkId::new(ChainId::from(1), "fixture")?,
        RpcEndpoint::new(&fixture.endpoint)?,
        18,
        None,
        BlockSelector::Safe,
        RpcLimits::new(Duration::from_secs(1).min(timeout), timeout, bytes, retries)?,
        "evm-fixture",
    )
}
async fn start_client(
    replies: Vec<Reply>,
) -> Result<(Fixture, EvmClient), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(replies).await?;
    let client = EvmClient::connect(config(
        &fixture,
        2,
        2 * 1024 * 1024,
        Duration::from_secs(3),
    )?)
    .await?;
    Ok((fixture, client))
}
fn prefix() -> Result<Vec<Reply>, serde_json::Error> {
    Ok(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&json!("0x1"))?,
    ])
}
fn token_prefix() -> Result<Vec<Reply>, serde_json::Error> {
    let mut replies = prefix()?;
    replies.push(Reply::result(&block())?);
    Ok(replies)
}
fn word(value: U256) -> String {
    format!("0x{value:064x}")
}
fn text(value: &[u8]) -> String {
    let mut bytes = U256::from(32).to_be_bytes::<32>().to_vec();
    bytes.extend_from_slice(&U256::from(value.len()).to_be_bytes::<32>());
    bytes.extend_from_slice(value);
    bytes.resize(64 + value.len().div_ceil(32) * 32, 0);
    const_hex::encode_prefixed(bytes)
}
fn rpc_error(code: i64) -> Result<Reply, serde_json::Error> {
    Reply::json(
        &json!({"jsonrpc":"2.0","id":1,"error":{"code":code,"message":"SECRET_REMOTE_DIAGNOSTIC","data":{"secret":"PRIVATE"}}}),
    )
}
fn transaction() -> Value {
    json!({"hash":hash().to_string(),"from":OWNER,"to":SPENDER,"nonce":"0x1","gas":"0x5208","value":format!("0x{:x}",U256::MAX),"input":"0xdeadbeef","type":"0x0","chainId":"0x1","gasPrice":format!("0x{:x}",U256::MAX),"v":"0x25","r":"0x1","s":"0x2","blockHash":block_hash(),"blockNumber":"0x2a","transactionIndex":"0x3","unknownFutureField":"allowed"})
}
fn log() -> Value {
    json!({"transactionHash":hash().to_string(),"transactionIndex":"0x3","blockHash":block_hash(),"blockNumber":"0x2a","logIndex":"0x9","address":CONTRACT,"data":"0xdeadbeef","topics":[format!("0x{}","cd".repeat(32))],"removed":false})
}
fn receipt() -> Value {
    json!({"transactionHash":hash().to_string(),"transactionIndex":"0x3","blockHash":block_hash(),"blockNumber":"0x2a","from":OWNER,"to":SPENDER,"contractAddress":null,"type":"0x0","status":"0x0","gasUsed":"0x5208","cumulativeGasUsed":"0x7530","effectiveGasPrice":format!("0x{:x}",U256::MAX),"logsBloom":format!("0x{}","00".repeat(256)),"logs":[log()]})
}

#[tokio::test]
async fn erc20_balance_and_allowance_use_exact_selectors_calldata_and_captured_hash() -> TestResult
{
    for allowance in [false, true] {
        let mut replies = token_prefix()?;
        replies.push(Reply::result(&json!(word(U256::MAX)))?);
        let (fixture, client) = start_client(replies).await?;
        let before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let state = if allowance {
            let o = Erc20Reader::get_erc20_allowance(
                &client,
                Address::parse(CONTRACT)?,
                Address::parse(OWNER)?,
                Address::parse(SPENDER)?,
                None,
            )
            .await?;
            assert_eq!(o.value().spender().to_string(), SPENDER);
            assert_eq!(o.value().amount().raw(), U256::MAX);
            assert_eq!(o.value().amount().decimals(), None);
            o.context().clone()
        } else {
            let o = Erc20Reader::get_erc20_balance(
                &client,
                Address::parse(CONTRACT)?,
                Address::parse(OWNER)?,
                None,
            )
            .await?;
            assert_eq!(o.value().owner().to_string(), OWNER);
            assert_eq!(o.value().amount().raw(), U256::MAX);
            assert_eq!(o.value().amount().decimals(), None);
            o.context().clone()
        };
        assert_eq!(
            state.state(),
            ReadState::CanonicalHash {
                requested_selector: BlockSelector::Safe,
                block: regit_web3::domain::BlockContext::new(
                    42,
                    BlockHash::parse(&block_hash())?,
                    regit_web3::domain::Timestamp::from_unix_seconds(1_700_000_000)
                )
            }
        );
        assert_eq!(state.finality(), Finality::Unknown);
        assert_eq!(state.confirmations(), None);
        assert_eq!(
            state.source().integration_version(),
            env!("CARGO_PKG_VERSION")
        );
        assert!(state.retrieved_at().unix_seconds() >= before);
        let calls = fixture.requests()?;
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[2].body["params"], json!(["safe", false]));
        let expected = if allowance {
            format!(
                "0xdd62ed3e{}{}",
                "0000000000000000000000000000000000000000000000000000000000000002",
                "0000000000000000000000000000000000000000000000000000000000000003"
            )
        } else {
            format!(
                "0x70a08231{}",
                "0000000000000000000000000000000000000000000000000000000000000002"
            )
        };
        assert_eq!(calls[3].body["method"], "eth_call");
        assert_eq!(
            calls[3].body["params"],
            json!([{"to":CONTRACT,"data":expected},{"blockHash":block_hash(),"requireCanonical":true}])
        );
        assert_eq!(calls[3].target, "/rpc");
        assert!(
            calls[3]
                .headers
                .to_ascii_lowercase()
                .contains("application/json")
        );
    }
    Ok(())
}
#[tokio::test]
async fn required_erc20_reads_reject_short_trailing_noncanonical_and_reverted_returns() -> TestResult
{
    for value in [
        "0x".to_owned(),
        "0x00".to_owned(),
        format!("{}00", word(U256::ZERO)),
        "0x0".to_owned(),
    ] {
        let mut replies = token_prefix()?;
        replies.push(Reply::result(&json!(value))?);
        let (_fixture, client) = start_client(replies).await?;
        assert_eq!(
            client
                .get_erc20_balance(Address::parse(CONTRACT)?, Address::parse(OWNER)?, None)
                .await
                .err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    let mut replies = token_prefix()?;
    replies.push(rpc_error(3)?);
    let (_fixture, client) = start_client(replies).await?;
    let error = client
        .get_erc20_allowance(
            Address::parse(CONTRACT)?,
            Address::parse(OWNER)?,
            Address::parse(SPENDER)?,
            None,
        )
        .await
        .err()
        .ok_or("expected revert")?;
    assert_eq!(error, Error::ExecutionReverted);
    assert!(!format!("{error:?} {error}").contains("SECRET"));
    Ok(())
}
#[tokio::test]
async fn metadata_supports_canonical_utf8_empty_text_and_uint8_boundary_without_defaults()
-> TestResult {
    let mut replies = token_prefix()?;
    replies.extend([
        Reply::result(&json!(text("Token 🦀".as_bytes())))?,
        Reply::result(&json!(text(b"")))?,
        Reply::result(&json!(word(U256::from(255))))?,
    ]);
    let (fixture, client) = start_client(replies).await?;
    let observed = client
        .get_erc20_metadata(Address::parse(CONTRACT)?, None)
        .await?;
    assert!(matches!(observed.value().name(),MetadataValue::Available(v)if v.as_str()=="Token 🦀"));
    assert!(
        matches!(observed.value().symbol(),MetadataValue::Available(v)if v.as_str().is_empty())
    );
    assert_eq!(observed.value().decimals(), &MetadataValue::Available(255));
    assert_eq!(
        serde_json::from_str::<OperationObservation<Erc20Metadata>>(&serde_json::to_string(
            &observed
        )?)?,
        observed
    );
    let requests = fixture.requests()?;
    for (i, selector) in [(3, "0x06fdde03"), (4, "0x95d89b41"), (5, "0x313ce567")] {
        assert_eq!(requests[i].body["params"][0]["data"], selector);
        assert_eq!(
            requests[i].body["params"][1],
            json!({"blockHash":block_hash(),"requireCanonical":true})
        );
    }
    Ok(())
}
#[tokio::test]
async fn optional_metadata_keeps_revert_unavailable_invalid_abi_and_unknown_rpc_distinct()
-> TestResult {
    let mut replies = token_prefix()?;
    replies.extend([
        rpc_error(3)?,
        rpc_error(-32601)?,
        Reply::result(&json!(word(U256::from(256))))?,
    ]);
    let (_fixture, client) = start_client(replies).await?;
    let observed = client
        .get_erc20_metadata(Address::parse(CONTRACT)?, None)
        .await?;
    assert_eq!(observed.value().name(), &MetadataValue::Reverted);
    assert_eq!(
        observed.value().symbol(),
        &MetadataValue::Unavailable(MetadataUnavailable::Unsupported)
    );
    assert_eq!(observed.value().decimals(), &MetadataValue::InvalidAbi);
    let mut replies = token_prefix()?;
    replies.extend([
        rpc_error(-32001)?,
        Reply::result(&Value::Null)?,
        Reply::result(&json!("0x"))?,
    ]);
    let (_fixture, client) = start_client(replies).await?;
    let observed = client
        .get_erc20_metadata(Address::parse(CONTRACT)?, None)
        .await?;
    assert_eq!(
        observed.value().name(),
        &MetadataValue::Unavailable(MetadataUnavailable::StateUnavailable)
    );
    assert_eq!(
        observed.value().symbol(),
        &MetadataValue::Unavailable(MetadataUnavailable::NullResult)
    );
    assert_eq!(observed.value().decimals(), &MetadataValue::InvalidAbi);
    for code in [-32000, -32603] {
        let mut replies = token_prefix()?;
        replies.push(rpc_error(code)?);
        let (fixture, client) = start_client(replies).await?;
        assert_eq!(
            client
                .get_erc20_metadata(Address::parse(CONTRACT)?, None)
                .await
                .err(),
            Some(Error::Provider(ProviderError::Rpc))
        );
        assert_eq!(fixture.requests()?.len(), 4);
    }
    Ok(())
}
#[tokio::test]
async fn metadata_string_decoder_rejects_alternate_offsets_lengths_padding_utf8_and_bytes32()
-> TestResult {
    let valid = const_hex::decode(text(b"abc").strip_prefix("0x").ok_or("prefix")?)?;
    let mut cases = vec![vec![0; 32], vec![], valid.clone()];
    cases[2][31] = 64;
    let mut bad = valid.clone();
    bad[31] = 0;
    cases.push(bad);
    let mut bad = valid.clone();
    bad[32..64].fill(255);
    cases.push(bad);
    let mut bad = valid.clone();
    bad[63] = 255;
    cases.push(bad);
    let mut bad = valid.clone();
    bad[95] = 1;
    cases.push(bad);
    let mut bad = valid.clone();
    bad[64] = 255;
    cases.push(bad);
    let mut bad = valid.clone();
    bad.push(0);
    cases.push(bad);
    let mut bad = valid;
    bad.pop();
    cases.push(bad);
    cases.push(const_hex::decode(
        text(&vec![b'x'; 4097]).strip_prefix("0x").ok_or("prefix")?,
    )?);
    for bad in cases {
        let mut replies = token_prefix()?;
        replies.extend([
            Reply::result(&json!(const_hex::encode_prefixed(bad)))?,
            Reply::result(&json!(text(b"ok")))?,
            Reply::result(&json!(word(U256::ZERO)))?,
        ]);
        let (_fixture, client) = start_client(replies).await?;
        let observed = client
            .get_erc20_metadata(Address::parse(CONTRACT)?, None)
            .await?;
        assert_eq!(observed.value().name(), &MetadataValue::InvalidAbi);
        assert_eq!(observed.value().decimals(), &MetadataValue::Available(0));
    }
    Ok(())
}
#[tokio::test]
async fn transient_token_retries_keep_exact_call_and_never_resolve_a_new_head() -> TestResult {
    let mut replies = token_prefix()?;
    replies.extend([
        Reply::raw(503, vec![]),
        Reply::result(&json!(text(b"name")))?,
        Reply::result(&json!(text(b"symbol")))?,
        Reply::result(&json!(word(U256::from(6))))?,
    ]);
    let (fixture, client) = start_client(replies).await?;
    let observed = client
        .get_erc20_metadata(Address::parse(CONTRACT)?, None)
        .await?;
    assert_eq!(observed.value().decimals(), &MetadataValue::Available(6));
    let requests = fixture.requests()?;
    assert_eq!(requests[3].body, requests[4].body);
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.body["method"] == "eth_getBlockByNumber")
            .count(),
        1
    );
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.body["method"] == "eth_chainId")
            .count(),
        2
    );
    Ok(())
}
#[tokio::test]
async fn token_operation_deadline_covers_all_metadata_stages() -> TestResult {
    // Each call fits a fresh budget, but all three calls exceed one deadline.
    // Leave enough setup margin to dispatch the final call on loaded runners.
    let mut replies = token_prefix()?;
    for bytes in [text(b"a"), text(b"b"), word(U256::from(6))] {
        let mut response = Reply::result(&json!(bytes))?;
        response.delay = Duration::from_millis(1500);
        replies.push(response);
    }
    let fixture = Fixture::start(replies).await?;
    let client = EvmClient::connect(config(&fixture, 2, 100_000, Duration::from_secs(4))?).await?;
    assert_eq!(
        client
            .get_erc20_metadata(Address::parse(CONTRACT)?, None)
            .await
            .err(),
        Some(Error::Timeout)
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 6);
    assert_eq!(requests[5].body["method"], "eth_call");
    assert_eq!(requests[5].body["params"][0]["data"], "0x313ce567");
    assert_eq!(
        requests[5].body["params"][1],
        json!({ "blockHash": block_hash(), "requireCanonical": true })
    );
    Ok(())
}
#[tokio::test]
async fn all_read_paths_reject_changed_chain_before_querying_contracts_or_transactions()
-> TestResult {
    for operation in 0..6 {
        let (fixture, client) = start_client(vec![
            Reply::result(&json!("0x1"))?,
            Reply::result(&json!("0x2"))?,
        ])
        .await?;
        let error = match operation {
            0 => client
                .get_erc20_balance(Address::parse(CONTRACT)?, Address::parse(OWNER)?, None)
                .await
                .err(),
            1 => client
                .get_erc20_allowance(
                    Address::parse(CONTRACT)?,
                    Address::parse(OWNER)?,
                    Address::parse(SPENDER)?,
                    None,
                )
                .await
                .err(),
            2 => client
                .get_erc20_metadata(Address::parse(CONTRACT)?, None)
                .await
                .err(),
            3 => client.get_transaction(hash()).await.err(),
            4 => client.get_receipt(hash()).await.err(),
            _ => client.get_transaction_status(hash()).await.err(),
        };
        assert_eq!(error, Some(Error::Provider(ProviderError::ChainMismatch)));
        assert_eq!(fixture.requests()?.len(), 2);
    }
    Ok(())
}
#[tokio::test]
async fn transaction_retrieval_retains_actual_u256_fields_and_pending_or_null_attribution()
-> TestResult {
    for state in 0..3 {
        let mut value = transaction();
        if state == 1 {
            for key in ["blockHash", "blockNumber", "transactionIndex"] {
                value[key] = Value::Null;
            }
        } else if state == 2 {
            value = Value::Null;
        }
        let mut replies = prefix()?;
        replies.push(Reply::result(&value)?);
        let (fixture, client) = start_client(replies).await?;
        let observed = TransactionReader::get_transaction(&client, hash()).await?;
        assert_eq!(observed.value().query(), hash());
        match state {
            0 => {
                let transaction = observed
                    .value()
                    .transaction()
                    .ok_or("missing transaction")?;
                assert_eq!(transaction.data().value.value(), U256::MAX);
                assert_eq!(transaction.data().input.bytes(), &[0xde, 0xad, 0xbe, 0xef]);
                assert!(matches!(
                    observed.context().state(),
                    ReadState::Included { .. }
                ));
            }
            1 => assert_eq!(observed.context().state(), ReadState::Pending),
            _ => {
                assert!(observed.value().transaction().is_none());
                assert_eq!(observed.context().state(), ReadState::Unanchored);
            }
        }
        assert_eq!(observed.context().finality(), Finality::Unknown);
        assert_eq!(
            observed.context().source().method(),
            "eth_getTransactionByHash"
        );
        assert_eq!(
            fixture.requests()?[2].body["params"],
            json!([hash().to_string()])
        );
        assert_eq!(
            serde_json::from_str::<OperationObservation<regit_web3::domain::evm::TransactionLookup>>(
                &serde_json::to_string(&observed)?
            )?,
            observed
        );
    }
    Ok(())
}
#[tokio::test]
async fn transaction_fields_reject_identity_chain_inclusion_quantity_parity_and_fee_mismatches()
-> TestResult {
    for (key, bad) in [
        (
            "hash",
            json!(TransactionId::from_bytes([2; 32]).to_string()),
        ),
        ("chainId", json!("0x2")),
        ("blockHash", Value::Null),
        ("value", json!("0x00")),
        ("nonce", json!("0xffffffffffffffff")),
        ("v", json!("0x27")),
        ("r", Value::Null),
    ] {
        let mut value = transaction();
        value[key] = bad;
        let mut replies = prefix()?;
        replies.push(Reply::result(&value)?);
        let (_fixture, client) = start_client(replies).await?;
        assert_eq!(
            client.get_transaction(hash()).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    for field in ["to", "blockHash", "blockNumber", "transactionIndex"] {
        let mut value = transaction();
        value.as_object_mut().ok_or("object")?.remove(field);
        let mut replies = prefix()?;
        replies.push(Reply::result(&value)?);
        let (_fixture, client) = start_client(replies).await?;
        assert_eq!(
            client.get_transaction(hash()).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    let mut value = transaction();
    value["type"] = json!("0x7e");
    let mut replies = prefix()?;
    replies.push(Reply::result(&value)?);
    let (_fixture, client) = start_client(replies).await?;
    assert_eq!(
        client.get_transaction(hash()).await.err(),
        Some(Error::UnsupportedCapability)
    );
    Ok(())
}
#[tokio::test]
async fn typed_transactions_preserve_legal_access_duplicates_and_actual_blob_authorization_fields()
-> TestResult {
    for kind in 1..=4 {
        let mut value = transaction();
        value["type"] = json!(format!("0x{kind:x}"));
        value["v"] = json!("0x1");
        value["yParity"] = json!("0x1");
        value["accessList"] = json!([{"address":CONTRACT,"storageKeys":[block_hash(),block_hash()]},{"address":CONTRACT,"storageKeys":[block_hash()]}]);
        if kind >= 2 {
            value["maxFeePerGas"] = json!(format!("0x{:x}", U256::MAX));
            value["maxPriorityFeePerGas"] = json!(format!("0x{:x}", U256::MAX));
        }
        if kind == 3 {
            value["maxFeePerBlobGas"] = json!(format!("0x{:x}", U256::MAX));
            value["blobVersionedHashes"] = json!([format!("0x01{}", "00".repeat(31))]);
        }
        if kind == 4 {
            value["authorizationList"] = json!([{"chainId":"0x0","address":CONTRACT,"nonce":"0x0","yParity":"0x1","r":"0x1","s":"0x2"}]);
        }
        let mut replies = prefix()?;
        replies.push(Reply::result(&value)?);
        let (_fixture, client) = start_client(replies).await?;
        let observed = client.get_transaction(hash()).await?;
        let transaction = observed.value().transaction().ok_or("missing")?;
        assert_eq!(transaction.data().kind.type_byte(), kind);
        match &transaction.data().kind {
            TransactionKind::AccessList { access_list, .. }
            | TransactionKind::DynamicFee { access_list, .. }
            | TransactionKind::Blob { access_list, .. }
            | TransactionKind::Authorization { access_list, .. } => {
                assert_eq!(access_list.len(), 2);
                assert_eq!(access_list[0].storage_keys.len(), 2);
            }
            TransactionKind::Legacy { .. } => return Err("wrong kind".into()),
        }
    }
    Ok(())
}
#[tokio::test]
async fn receipt_retrieval_keeps_failed_success_historical_unknown_and_null_distinct() -> TestResult
{
    for execution in 0..5 {
        let mut value = receipt();
        match execution {
            0 => {}
            1 => value["status"] = json!("0x1"),
            2 => {
                value.as_object_mut().ok_or("object")?.remove("status");
                value["root"] = json!(block_hash());
            }
            3 => {
                value.as_object_mut().ok_or("object")?.remove("status");
            }
            _ => value = Value::Null,
        }
        let mut replies = prefix()?;
        replies.push(Reply::result(&value)?);
        let (_fixture, client) = start_client(replies).await?;
        let observed = TransactionReader::get_receipt(&client, hash()).await?;
        if execution == 4 {
            assert!(observed.value().receipt().is_none());
            assert_eq!(observed.context().state(), ReadState::Unanchored);
        } else {
            let receipt = observed.value().receipt().ok_or("missing receipt")?;
            assert_eq!(
                receipt.execution(),
                match execution {
                    0 => ExecutionOutcome::Failed,
                    1 => ExecutionOutcome::Succeeded,
                    2 => ExecutionOutcome::PreByzantium(Word::parse(&block_hash())?),
                    _ => ExecutionOutcome::Unknown,
                }
            );
            assert_eq!(
                receipt
                    .data()
                    .effective_gas_price
                    .ok_or("gas price")?
                    .value(),
                U256::MAX
            );
            assert_eq!(receipt.data().logs.len(), 1);
            assert!(matches!(
                observed.context().state(),
                ReadState::Included { .. }
            ));
        }
    }
    Ok(())
}
#[tokio::test]
async fn receipt_rejects_invalid_status_log_identity_duplicate_positions_and_removed_logs()
-> TestResult {
    for variant in 0..9 {
        let mut value = receipt();
        match variant {
            0 => value["status"] = json!("0x2"),
            1 => value["root"] = json!(block_hash()),
            2 => {
                value["logs"][0]["transactionHash"] =
                    json!(TransactionId::from_bytes([2; 32]).to_string());
            }
            3 => value["logs"][0]["blockNumber"] = json!("0x2b"),
            4 => value["logs"][0]["removed"] = json!(true),
            5 => value["logs"] = json!([log(), log()]),
            6 => value["logs"][0]["topics"] = json!(vec![block_hash(); 5]),
            7 => value["logsBloom"] = json!("0x00"),
            _ => value["gasUsed"] = json!("0x7531"),
        }
        let mut replies = prefix()?;
        replies.push(Reply::result(&value)?);
        let (_fixture, client) = start_client(replies).await?;
        assert_eq!(
            client.get_receipt(hash()).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}
#[tokio::test]
async fn joint_status_distinguishes_absence_pending_included_and_matching_failed_execution()
-> TestResult {
    for state in 0..4 {
        let mut tx = transaction();
        let mut rec = receipt();
        match state {
            0 => {
                tx = Value::Null;
                rec = Value::Null;
            }
            1 => {
                for key in ["blockHash", "blockNumber", "transactionIndex"] {
                    tx[key] = Value::Null;
                }
                rec = Value::Null;
            }
            2 => rec = Value::Null,
            _ => {}
        }
        let mut replies = prefix()?;
        replies.extend([Reply::result(&tx)?, Reply::result(&rec)?]);
        let (fixture, client) = start_client(replies).await?;
        let observed = TransactionReader::get_transaction_status(&client, hash()).await?;
        match state {
            0 => assert_eq!(observed.value().state(), TransactionState::NotObserved),
            1 => assert_eq!(observed.value().state(), TransactionState::Pending),
            2 => assert!(matches!(
                observed.value().state(),
                TransactionState::Included {
                    execution: None,
                    ..
                }
            )),
            _ => assert!(matches!(
                observed.value().state(),
                TransactionState::Included {
                    execution: Some(ExecutionOutcome::Failed),
                    ..
                }
            )),
        }
        assert_eq!(fixture.requests()?.len(), 4);
        assert_eq!(observed.context().finality(), Finality::Unknown);
        assert_eq!(
            serde_json::from_str::<OperationObservation<regit_web3::domain::evm::TransactionStatus>>(
                &serde_json::to_string(&observed)?
            )?,
            observed
        );
    }
    Ok(())
}
#[tokio::test]
async fn status_rejects_changed_block_sender_recipient_type_and_impossible_gas() -> TestResult {
    for (key, bad) in [
        ("blockHash", json!(format!("0x{}", "cd".repeat(32)))),
        ("from", json!(CONTRACT)),
        ("to", json!(CONTRACT)),
        ("type", json!("0x2")),
        ("gasUsed", json!("0x5209")),
    ] {
        let mut value = receipt();
        value["logs"] = json!([]);
        value[key] = bad;
        let mut replies = prefix()?;
        replies.extend([Reply::result(&transaction())?, Reply::result(&value)?]);
        let (_fixture, client) = start_client(replies).await?;
        assert_eq!(
            client.get_transaction_status(hash()).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}
#[tokio::test]
async fn critical_duplicate_fields_and_rpc_ids_are_rejected_without_diagnostic_text() -> TestResult
{
    let tx = serde_json::to_string(&transaction())?;
    for body in [
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}}",
            tx.replacen(
                "\"nonce\":\"0x1\"",
                "\"nonce\":\"0x1\",\"nonce\":\"0x2\"",
                1
            )
        ),
        format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{tx}}}"),
    ] {
        let mut replies = prefix()?;
        replies.push(Reply::raw(200, body.into_bytes()));
        let (_fixture, client) = start_client(replies).await?;
        assert_eq!(
            client.get_transaction(hash()).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}
#[tokio::test]
async fn read_body_limits_apply_to_every_response_framing() -> TestResult {
    for framing in [
        Framing::ContentLength,
        Framing::CloseDelimited,
        Framing::Chunked,
    ] {
        let mut replies = prefix()?;
        let mut response = Reply::result(&json!("x".repeat(20_000)))?;
        response.framing = framing;
        replies.push(response);
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(&fixture, 2, 4096, Duration::from_secs(3))?).await?;
        assert_eq!(
            client.get_transaction(hash()).await.err(),
            Some(Error::Provider(ProviderError::ResponseTooLarge))
        );
    }
    Ok(())
}
