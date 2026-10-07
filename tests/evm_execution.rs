// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Canonical-state execution/nonce reads and independently sourced fee suggestions.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "evm-http")]
#[path = "support/rpc_server.rs"]
mod rpc_server;

use regit_web3::{
    chains::evm::{EvmClient, ExecutionReader, FeeReader},
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::evm::{
        AccessListEntry, Data, FeeTerms, OptionalFeeSuggestion, Quantity, ReadState,
        TransactionCall, TransactionCallData, Word,
    },
    domain::{Address, BlockSelector, ChainId, Finality, NetworkId, U256},
    error::{Error, ProviderError},
};
use rpc_server::{Fixture, Framing, Reply};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn block_hash() -> String {
    format!("0x{}", "ab".repeat(32))
}
fn block() -> Value {
    json!({"number":"0x2a","hash":block_hash(),"timestamp":"0x6553f100"})
}
fn fee_block() -> Value {
    let mut b = block();
    b["baseFeePerGas"] = json!(format!("0x{:x}", U256::MAX));
    b
}
fn config(f: &Fixture, retries: u8, bytes: usize, timeout: Duration) -> Result<EvmConfig, Error> {
    EvmConfig::new(
        NetworkId::new(ChainId::from(1), "fixture")?,
        RpcEndpoint::new(&f.endpoint)?,
        18,
        None,
        BlockSelector::Safe,
        RpcLimits::new(Duration::from_secs(1).min(timeout), timeout, bytes, retries)?,
        "evm-fixture",
    )
}
async fn client(replies: Vec<Reply>) -> Result<(Fixture, EvmClient), Box<dyn std::error::Error>> {
    let f = Fixture::start(replies).await?;
    let c = EvmClient::connect(config(&f, 3, 2 * 1024 * 1024, Duration::from_secs(3))?).await?;
    Ok((f, c))
}
fn prefix() -> Result<Vec<Reply>, serde_json::Error> {
    Ok(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&json!("0x1"))?,
        Reply::result(&block())?,
    ])
}
fn call(fees: FeeTerms) -> Result<TransactionCall, Error> {
    TransactionCall::new(TransactionCallData {
        chain_id: ChainId::from(1),
        from: Address::from_bytes([1; 20]),
        to: Some(Address::from_bytes([2; 20])),
        nonce: 9,
        gas_limit: 100_000,
        value: Quantity::new(U256::MAX),
        input: Data::parse("0xdeadbeef")?,
        fees,
    })
}
fn legacy() -> FeeTerms {
    FeeTerms::Legacy {
        gas_price: Quantity::from(20),
    }
}
fn rpc_error(code: i64) -> Result<Reply, serde_json::Error> {
    Reply::json(
        &json!({"jsonrpc":"2.0","id":1,"error":{"code":code,"message":"SECRET_REMOTE_TEXT"}}),
    )
}

#[tokio::test]
async fn nonce_call_and_estimate_pin_hash_and_preserve_explicit_wire_choices() -> TestResult {
    for operation in [0, 1, 2] {
        let mut replies = prefix()?;
        replies.push(Reply::result(&if operation == 1 {
            json!("0x")
        } else {
            json!("0x5208")
        })?);
        let (f, c) = client(replies).await?;
        let input = call(legacy())?;
        let state = match operation {
            0 => {
                let o = ExecutionReader::get_account_nonce(&c, Address::from_bytes([1; 20]), None)
                    .await?;
                assert_eq!(o.value().nonce, 21_000);
                o.context().clone()
            }
            1 => {
                let o = ExecutionReader::call(&c, input.clone(), None).await?;
                assert_eq!(o.value().call, input);
                assert!(o.value().output.bytes().is_empty());
                o.context().clone()
            }
            _ => {
                let o = ExecutionReader::estimate_gas(&c, input.clone(), None).await?;
                assert_eq!(o.value().gas(), 21_000);
                assert_eq!(o.value().call(), &input);
                o.context().clone()
            }
        };
        assert!(matches!(
            state.state(),
            ReadState::CanonicalHash {
                requested_selector: BlockSelector::Safe,
                ..
            }
        ));
        assert_eq!(state.finality(), Finality::Unknown);
        assert_eq!(state.confirmations(), None);
        assert_eq!(
            state.source().integration_version(),
            env!("CARGO_PKG_VERSION")
        );
        assert!(
            state.retrieved_at().unix_seconds()
                <= SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
        );
        let requests = f.requests()?;
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[3].target, "/rpc");
        assert!(requests[3].headers.contains("application/json"));
        assert_eq!(
            requests[3].body["params"][1],
            json!({"blockHash":block_hash(),"requireCanonical":true})
        );
        if operation != 0 {
            assert_eq!(
                requests[3].body["params"][0],
                json!({
                    "chainId":"0x1","from":Address::from_bytes([1;20]).to_string(),"to":Address::from_bytes([2;20]).to_string(),
                    "nonce":"0x9","gas":"0x186a0","value":format!("0x{:x}",U256::MAX),"data":"0xdeadbeef","type":"0x0","gasPrice":"0x14"
                })
            );
        }
        assert!(
            requests
                .iter()
                .all(|r| r.body["method"] != "eth_sendRawTransaction")
        );
    }
    Ok(())
}

#[tokio::test]
async fn typed_call_wire_retains_access_duplicates_fee_caps_and_contract_creation() -> TestResult {
    let entry = AccessListEntry {
        address: Address::from_bytes([3; 20]),
        storage_keys: vec![Word::from_bytes([4; 32]); 2],
    };
    for dynamic in [false, true] {
        let fees = if dynamic {
            FeeTerms::DynamicFee {
                max_fee_per_gas: Quantity::from(3),
                max_priority_fee_per_gas: Quantity::from(2),
                access_list: vec![entry.clone(); 2],
            }
        } else {
            FeeTerms::AccessList {
                gas_price: Quantity::from(2),
                access_list: vec![entry.clone(); 2],
            }
        };
        let mut data = call(fees)?.data().clone();
        data.to = None;
        let input = TransactionCall::new(data)?;
        let mut replies = prefix()?;
        replies.push(Reply::result(&json!("0x00"))?);
        let (f, c) = client(replies).await?;
        assert_eq!(c.call(input, None).await?.value().output.to_hex(), "0x00");
        let request = &f.requests()?[3].body["params"][0];
        assert!(request.get("to").is_none());
        assert_eq!(
            request["accessList"],
            json!([{"address":entry.address.to_string(),"storageKeys":[entry.storage_keys[0].to_string(),entry.storage_keys[0].to_string()]},{"address":entry.address.to_string(),"storageKeys":[entry.storage_keys[0].to_string(),entry.storage_keys[0].to_string()]}])
        );
        if dynamic {
            assert!(request.get("gasPrice").is_none());
            assert_eq!(request["maxFeePerGas"], "0x3");
            assert_eq!(request["maxPriorityFeePerGas"], "0x2");
        } else {
            assert!(request.get("maxFeePerGas").is_none());
            assert_eq!(request["gasPrice"], "0x2");
        }
    }
    Ok(())
}

#[tokio::test]
async fn fee_facts_remain_independent_exact_and_optional_without_defaults() -> TestResult {
    for optional in [0, 1, 2, 3, 4] {
        let mut replies = prefix()?;
        let header = if optional == 4 { block() } else { fee_block() };
        replies.push(Reply::result(&header)?);
        replies.push(Reply::result(&json!(format!("0x{:x}", U256::MAX)))?);
        replies.push(match optional {
            0 => Reply::result(&json!("0x0"))?,
            1 => Reply::result(&Value::Null)?,
            2 => rpc_error(-32601)?,
            3 => rpc_error(-32001)?,
            _ => Reply::result(&json!("0x2"))?,
        });
        let (f, c) = client(replies).await?;
        let o = FeeReader::get_fee_suggestions(&c).await?;
        assert_eq!(o.value().gas_price.value(), U256::MAX);
        assert_eq!(o.context().state(), ReadState::Unanchored);
        assert_eq!(
            o.value().block_fee.base_fee_per_gas,
            if optional == 4 {
                None
            } else {
                Some(Quantity::new(U256::MAX))
            }
        );
        assert_eq!(
            o.value().max_priority_fee_per_gas,
            match optional {
                0 => OptionalFeeSuggestion::Available(Quantity::from(0)),
                1 => OptionalFeeSuggestion::NullResult,
                2 => OptionalFeeSuggestion::Unsupported,
                3 => OptionalFeeSuggestion::Unavailable,
                _ => OptionalFeeSuggestion::Available(Quantity::from(2)),
            }
        );
        let requests = f.requests()?;
        assert_eq!(requests.len(), 6);
        assert_eq!(requests[3].body["params"], json!([block_hash(), false]));
        assert_eq!(requests[4].body["method"], "eth_gasPrice");
        assert_eq!(requests[5].body["method"], "eth_maxPriorityFeePerGas");
    }
    Ok(())
}

#[tokio::test]
async fn fee_headers_prices_and_diagnostics_fail_closed() -> TestResult {
    for bad in [0, 1, 2, 3, 4, 5, 6] {
        let mut replies = prefix()?;
        let mut header = fee_block();
        if bad == 0 {
            header["hash"] = json!(format!("0x{}", "cd".repeat(32)));
        }
        if bad == 1 {
            header["number"] = json!("0x2b");
        }
        if bad == 2 {
            header["timestamp"] = json!("0x2");
        }
        if bad == 3 {
            header["baseFeePerGas"] = Value::Null;
        }
        replies.push(if bad==4{Reply::raw(200,format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"hash\":\"{}\",\"number\":\"0x2a\",\"timestamp\":\"0x6553f100\",\"baseFeePerGas\":\"0x1\",\"baseFeePerGas\":\"0x2\"}}}}",block_hash()).into_bytes())}else{Reply::result(&header)?});
        replies.push(Reply::result(&if bad == 5 {
            json!("0x00")
        } else {
            json!("0x1")
        })?);
        replies.push(rpc_error(-32602)?);
        let (f, c) = client(replies).await?;
        let error = c.get_fee_suggestions().await.expect_err("malformed fee");
        assert_eq!(
            error,
            Error::Provider(if bad == 6 {
                ProviderError::Rpc
            } else {
                ProviderError::InvalidResponse
            })
        );
        assert!(!format!("{error:?} {error}").contains("SECRET_REMOTE_TEXT"));
        assert!(
            f.requests()?
                .iter()
                .all(|r| r.body["method"] != "eth_sendRawTransaction")
        );
    }
    Ok(())
}

#[tokio::test]
async fn call_estimate_errors_never_change_hash_or_fall_back_to_height() -> TestResult {
    for estimate in [false, true] {
        for code in [3, -32601, -32004, -32602, -32000, -32001] {
            let mut replies = prefix()?;
            replies.push(rpc_error(code)?);
            let (f, c) = client(replies).await?;
            let error = if estimate {
                c.estimate_gas(call(legacy())?, None)
                    .await
                    .expect_err("source error")
            } else {
                c.call(call(legacy())?, None)
                    .await
                    .expect_err("source error")
            };
            assert_eq!(
                error,
                match code {
                    3 => Error::ExecutionReverted,
                    -32601 | -32004 => Error::UnsupportedCapability,
                    -32001 => Error::UnavailableData,
                    _ => Error::Provider(ProviderError::Rpc),
                }
            );
            assert_eq!(f.requests()?.len(), 4);
            assert_eq!(
                f.requests()?[3].body["params"][1],
                json!({"blockHash":block_hash(),"requireCanonical":true})
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn exact_execution_retry_inputs_and_local_or_remote_chain_rejection() -> TestResult {
    let mut replies = prefix()?;
    replies.push(Reply::raw(429, Vec::new()));
    replies.push(Reply::result(&json!("0x5208"))?);
    let (f, c) = client(replies).await?;
    assert_eq!(
        c.estimate_gas(call(legacy())?, None).await?.value().gas(),
        21_000
    );
    let requests = f.requests()?;
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[3].body, requests[4].body);
    let (f, c) = client(vec![Reply::result(&json!("0x1"))?]).await?;
    let mut data = call(legacy())?.data().clone();
    data.chain_id = ChainId::from(2);
    assert_eq!(
        c.call(TransactionCall::new(data)?, None).await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    assert_eq!(f.requests()?.len(), 1);
    let (f, c) = client(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&json!("0x2"))?,
    ])
    .await?;
    assert_eq!(
        c.estimate_gas(call(legacy())?, None).await,
        Err(Error::Provider(ProviderError::ChainMismatch))
    );
    assert_eq!(f.requests()?.len(), 2);
    Ok(())
}

#[tokio::test]
async fn execution_rejects_null_bad_quantities_overflow_and_out_of_cap_results() -> TestResult {
    for operation in [0, 1, 2] {
        for invalid in [
            Value::Null,
            json!("0x00"),
            json!("0x10000000000000000"),
            json!("0x0"),
            json!("0x186a1"),
        ] {
            if operation == 0 && (invalid == json!("0x0") || invalid == json!("0x186a1")) {
                continue;
            }
            if operation == 1 && invalid == json!("0x00") {
                continue;
            }
            let mut replies = prefix()?;
            replies.push(Reply::result(&invalid)?);
            let (_, c) = client(replies).await?;
            let failed = match operation {
                0 => c
                    .get_account_nonce(Address::from_bytes([1; 20]), None)
                    .await
                    .is_err(),
                1 => c.call(call(legacy())?, None).await.is_err(),
                _ => c.estimate_gas(call(legacy())?, None).await.is_err(),
            };
            assert!(failed, "operation{operation} value{invalid}");
        }
    }
    Ok(())
}

#[tokio::test]
async fn fee_budget_covers_header_gas_and_priority_stages() -> TestResult {
    let mut replies = prefix()?;
    for result in [fee_block(), json!("0x1"), json!("0x2")] {
        let mut r = Reply::result(&result)?;
        r.delay = Duration::from_millis(1500);
        replies.push(r);
    }
    let f = Fixture::start(replies).await?;
    let c = EvmClient::connect(config(&f, 0, 1024, Duration::from_secs(4))?).await?;
    assert_eq!(c.get_fee_suggestions().await, Err(Error::Timeout));
    let requests = f.requests()?;
    assert_eq!(requests.len(), 6);
    assert_eq!(requests[5].body["method"], "eth_maxPriorityFeePerGas");
    Ok(())
}

#[tokio::test]
async fn call_response_bytes_are_bounded_for_each_http_framing() -> TestResult {
    for framing in [
        Framing::ContentLength,
        Framing::Chunked,
        Framing::CloseDelimited,
    ] {
        let mut replies = prefix()?;
        let mut oversized = Reply::raw(200, vec![b' '; 1025]);
        oversized.framing = framing;
        replies.push(oversized);
        let f = Fixture::start(replies).await?;
        let c = EvmClient::connect(config(&f, 3, 1024, Duration::from_secs(3))?).await?;
        assert_eq!(
            c.call(call(legacy())?, None).await,
            Err(Error::Provider(ProviderError::ResponseTooLarge))
        );
        assert_eq!(f.requests()?.len(), 4);
    }
    Ok(())
}
