// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Native EVM balance reads through deterministic loopback JSON-RPC fixtures.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "evm-http")]

#[path = "support/rpc_server.rs"]
mod rpc_server;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regit_web3::{
    chains::evm::{EvmClient, NativeBalanceReader},
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::{
        Address, Balance, BlockHash, BlockSelector, ChainId, Finality, NetworkId, Observation,
    },
    error::{Error, ProviderError},
};
use serde_json::{Value, json};

use rpc_server::{Fixture, Framing, Reply};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ADDRESS: &str = "0x0000000000000000000000000000000000000001";
const UINT256_MAX: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";

fn block_hash() -> String {
    format!("0x{}", "ab".repeat(32))
}

fn block() -> Value {
    json!({"number":"0x2a","hash":block_hash(),"timestamp":"0x6553f100",
        "transactions":[],"extra_provider_field":"fixture"})
}

fn config(
    fixture: &Fixture,
    decimals: u8,
    selector: BlockSelector,
    retries: u8,
    bytes: usize,
    timeout: Duration,
) -> Result<EvmConfig, Error> {
    EvmConfig::new(
        NetworkId::new(ChainId::from(1), "fixture-network")?,
        RpcEndpoint::new(&fixture.endpoint)?,
        decimals,
        Some("NATIVE".to_owned()),
        selector,
        RpcLimits::new(
            timeout.min(Duration::from_millis(200)),
            timeout,
            bytes,
            retries,
        )?,
        "fixture-provider",
    )
}

fn prefix() -> Result<Vec<Reply>, serde_json::Error> {
    Ok(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&json!("0x1"))?,
    ])
}

#[test]
fn native_read_without_an_entered_tokio_runtime_is_a_configuration_error() -> TestResult {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (fixture, client) = runtime.block_on(async {
        let fixture = Fixture::start(vec![Reply::result(&json!("0x1"))?]).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            0,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        Ok::<_, Box<dyn std::error::Error>>((fixture, client))
    })?;
    let address = Address::parse(ADDRESS)?;
    let mut read = std::pin::pin!(client.get_native_balance(address, None));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(
        std::future::Future::poll(read.as_mut(), &mut context),
        std::task::Poll::Ready(Err(Error::Configuration))
    ));
    assert_eq!(fixture.requests()?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn native_read_retains_exact_balance_source_anchor_and_separate_times() -> TestResult {
    let mut replies = prefix()?;
    replies.extend([
        Reply::result(&block())?,
        Reply::result(&json!("0x20000000000001"))?,
    ]);
    let fixture = Fixture::start(replies).await?;
    let client = EvmClient::connect(config(
        &fixture,
        18,
        BlockSelector::Latest,
        0,
        4096,
        Duration::from_secs(2),
    )?)
    .await?;
    let before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let observed =
        NativeBalanceReader::get_native_balance(&client, Address::parse(ADDRESS)?, None).await?;
    let after = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    assert_eq!(observed.value().address().to_string(), ADDRESS);
    assert_eq!(
        observed.value().amount().raw().to_string(),
        "9007199254740993"
    );
    assert_eq!(
        observed.value().amount().formatted().as_deref(),
        Some("0.009007199254740993")
    );
    assert_eq!(observed.value().asset(), client.config().native_asset());
    let captured = observed.context();
    assert_eq!(captured.schema_version(), 1);
    assert_eq!(captured.network(), client.config().network());
    assert_eq!(captured.requested_selector(), &BlockSelector::Latest);
    assert_eq!(captured.block().number(), 42);
    assert_eq!(captured.block().hash().to_string(), block_hash());
    assert_eq!(captured.block().timestamp().unix_seconds(), 1_700_000_000);
    assert!((before..=after).contains(&captured.retrieved_at().unix_seconds()));
    assert_ne!(captured.block().timestamp(), captured.retrieved_at());
    assert_eq!(captured.finality(), Finality::Unknown);
    assert_eq!(captured.confirmations(), None);
    assert_eq!(captured.source().provider_id(), "fixture-provider");
    assert_eq!(captured.source().method(), "eth_getBalance");
    assert_eq!(
        captured.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    let serialized = serde_json::to_value(observed)?;
    assert_eq!(serialized["value"]["amount"]["raw"], "9007199254740993");
    assert_eq!(serialized["finality"], "unknown");
    assert_eq!(serialized["confirmations"], Value::Null);
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].body["method"], "eth_chainId");
    assert_eq!(requests[1].body["method"], "eth_chainId");
    assert_eq!(requests[2].body["method"], "eth_getBlockByNumber");
    assert_eq!(requests[2].body["params"], json!(["latest", false]));
    assert_eq!(requests[3].body["method"], "eth_getBalance");
    assert_eq!(
        requests[3].body["params"],
        json!([ADDRESS,{"blockHash":block_hash(),"requireCanonical":true}])
    );
    assert_eq!(requests[3].target, "/rpc");
    assert!(
        requests[3]
            .headers
            .to_ascii_lowercase()
            .contains("content-type: application/json")
    );
    Ok(())
}

#[tokio::test]
async fn explicit_and_default_selectors_resolve_then_pin_the_actual_hash() -> TestResult {
    let hash = BlockHash::parse(&block_hash())?;
    for (requested, expected, method, parameters) in [
        (
            None,
            BlockSelector::Safe,
            "eth_getBlockByNumber",
            json!(["safe", false]),
        ),
        (
            Some(BlockSelector::Latest),
            BlockSelector::Latest,
            "eth_getBlockByNumber",
            json!(["latest", false]),
        ),
        (
            Some(BlockSelector::Safe),
            BlockSelector::Safe,
            "eth_getBlockByNumber",
            json!(["safe", false]),
        ),
        (
            Some(BlockSelector::Finalized),
            BlockSelector::Finalized,
            "eth_getBlockByNumber",
            json!(["finalized", false]),
        ),
        (
            Some(BlockSelector::Number(42)),
            BlockSelector::Number(42),
            "eth_getBlockByNumber",
            json!(["0x2a", false]),
        ),
        (
            Some(BlockSelector::Hash(hash)),
            BlockSelector::Hash(hash),
            "eth_getBlockByHash",
            json!([block_hash(), false]),
        ),
    ] {
        let mut replies = prefix()?;
        replies.extend([Reply::result(&block())?, Reply::result(&json!("0x0"))?]);
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Safe,
            0,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        let observed = client
            .get_native_balance(Address::parse(ADDRESS)?, requested)
            .await?;
        assert_eq!(observed.context().requested_selector(), &expected);
        assert_eq!(observed.context().finality(), Finality::Unknown);
        let requests = fixture.requests()?;
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[2].body["method"], method);
        assert_eq!(requests[2].body["params"], parameters);
        assert_eq!(
            requests[3].body["params"],
            json!([ADDRESS,{"blockHash":block_hash(),"requireCanonical":true}])
        );
    }
    Ok(())
}

#[tokio::test]
async fn native_values_preserve_zero_full_uint256_and_configured_decimals() -> TestResult {
    for (quantity, raw, decimals, expected_format) in [
        ("0x0".to_owned(), "0", 18, "0.000000000000000000".to_owned()),
        (
            format!("0x{}", "f".repeat(64)),
            UINT256_MAX,
            0,
            UINT256_MAX.to_owned(),
        ),
        (
            "0x1".to_owned(),
            "1",
            u8::MAX,
            format!("0.{}1", "0".repeat(254)),
        ),
    ] {
        let mut replies = prefix()?;
        replies.extend([Reply::result(&block())?, Reply::result(&json!(quantity))?]);
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            decimals,
            BlockSelector::Latest,
            0,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        let observed = client
            .get_native_balance(Address::parse(ADDRESS)?, None)
            .await?;
        assert_eq!(observed.value().amount().raw().to_string(), raw);
        assert_eq!(observed.value().amount().decimals(), Some(decimals));
        assert_eq!(observed.value().amount().formatted(), Some(expected_format));
        assert_eq!(observed.value().asset().symbol(), Some("NATIVE"));
    }
    Ok(())
}

#[tokio::test]
async fn the_chain_is_rechecked_before_any_state_lookup() -> TestResult {
    for chain in [json!("0x2"), json!("0x01")] {
        let fixture =
            Fixture::start(vec![Reply::result(&json!("0x1"))?, Reply::result(&chain)?]).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            2,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        let error = client
            .get_native_balance(Address::parse(ADDRESS)?, None)
            .await
            .unwrap_err();
        let expected = if chain == "0x2" {
            ProviderError::ChainMismatch
        } else {
            ProviderError::InvalidResponse
        };
        assert_eq!(error, Error::Provider(expected));
        let requests = fixture.requests()?;
        assert_eq!(requests.len(), 2);
        assert!(
            requests
                .iter()
                .all(|request| request.body["method"] == "eth_chainId")
        );
    }
    Ok(())
}

#[tokio::test]
async fn missing_block_and_missing_balance_remain_unavailable_not_zero() -> TestResult {
    for missing_block in [true, false] {
        let mut replies = prefix()?;
        if missing_block {
            replies.push(Reply::result(&Value::Null)?);
        } else {
            replies.extend([Reply::result(&block())?, Reply::result(&Value::Null)?]);
        }
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            2,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        assert_eq!(
            client
                .get_native_balance(Address::parse(ADDRESS)?, None)
                .await
                .unwrap_err(),
            Error::UnavailableData
        );
        assert_eq!(fixture.requests()?.len(), if missing_block { 3 } else { 4 });
    }
    Ok(())
}

#[tokio::test]
async fn blocks_require_valid_hash_number_and_timestamp_without_truncation() -> TestResult {
    for (field, bad) in [
        ("hash", Value::Null),
        ("hash", json!("0x01")),
        ("hash", json!(format!("0x{}", "g".repeat(64)))),
        ("number", Value::Null),
        ("number", json!(42)),
        ("number", json!("0x02a")),
        ("number", json!("0x10000000000000000")),
        ("timestamp", Value::Null),
        ("timestamp", json!("0x")),
        ("timestamp", json!("0x00")),
        ("timestamp", json!("0x10000000000000000")),
    ] {
        let mut bad_block = block();
        bad_block[field] = bad;
        let mut replies = prefix()?;
        replies.push(Reply::result(&bad_block)?);
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            2,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        assert_eq!(
            client
                .get_native_balance(Address::parse(ADDRESS)?, None)
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    for field in ["hash", "number", "timestamp"] {
        let mut missing = block();
        missing.as_object_mut().unwrap().remove(field);
        let mut replies = prefix()?;
        replies.push(Reply::result(&missing)?);
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            0,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        assert_eq!(
            client
                .get_native_balance(Address::parse(ADDRESS)?, None)
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
    }
    Ok(())
}

#[tokio::test]
async fn raw_json_blocks_reject_duplicate_critical_fields() -> TestResult {
    for duplicated in [
        format!("\"hash\":\"{}\",", block_hash()),
        "\"number\":\"0x2a\",".to_owned(),
        "\"timestamp\":\"0x6553f100\",".to_owned(),
    ] {
        let raw = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{{duplicated}\"hash\":\"{}\",\"number\":\"0x2a\",\"timestamp\":\"0x6553f100\"}}}}",
            block_hash()
        );
        let mut replies = prefix()?;
        replies.push(Reply::raw(200, raw.into_bytes()));
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            2,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        assert_eq!(
            client
                .get_native_balance(Address::parse(ADDRESS)?, None)
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}

#[tokio::test]
async fn explicit_selector_mismatches_fail_before_any_balance_read() -> TestResult {
    for selector in [
        BlockSelector::Number(43),
        BlockSelector::Hash(BlockHash::parse(&format!("0x{}", "cd".repeat(32)))?),
    ] {
        let mut replies = prefix()?;
        replies.push(Reply::result(&block())?);
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            2,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        assert_eq!(
            client
                .get_native_balance(Address::parse(ADDRESS)?, Some(selector))
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}

#[tokio::test]
async fn malformed_or_overflowing_balances_do_not_become_values() -> TestResult {
    for result in [
        json!(1),
        json!(""),
        json!("0x"),
        json!("0x00"),
        json!("0x01"),
        json!("0x-1"),
        json!("0x1.0"),
        json!("0xg"),
        json!(format!("0x1{}", "0".repeat(64))),
    ] {
        let mut replies = prefix()?;
        replies.extend([Reply::result(&block())?, Reply::result(&result)?]);
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            2,
            4096,
            Duration::from_secs(2),
        )?)
        .await?;
        assert_eq!(
            client
                .get_native_balance(Address::parse(ADDRESS)?, None)
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 4);
    }
    Ok(())
}

#[tokio::test]
async fn unsupported_unavailable_and_noncanonical_failures_have_no_fallback() -> TestResult {
    for balance_stage in [false, true] {
        for (code, expected) in [
            (-32_601, Error::UnsupportedCapability),
            (-32_004, Error::UnsupportedCapability),
            (-32_001, Error::UnavailableData),
            (-32_002, Error::UnavailableData),
            (-32_000, Error::Provider(ProviderError::Rpc)),
            (-32_602, Error::Provider(ProviderError::Rpc)),
        ] {
            let mut replies = prefix()?;
            if balance_stage {
                replies.push(Reply::result(&block())?);
            }
            replies.push(Reply::json(&json!({"jsonrpc":"2.0","id":1,"error":{"code":code,"message":"fixture provider message"}}))?);
            let fixture = Fixture::start(replies).await?;
            let client = EvmClient::connect(config(
                &fixture,
                18,
                BlockSelector::Finalized,
                2,
                4096,
                Duration::from_secs(2),
            )?)
            .await?;
            assert_eq!(
                client
                    .get_native_balance(Address::parse(ADDRESS)?, None)
                    .await
                    .unwrap_err(),
                expected
            );
            assert_eq!(fixture.requests()?.len(), if balance_stage { 4 } else { 3 });
        }
    }
    Ok(())
}

#[tokio::test]
async fn retries_reuse_the_original_hash_and_do_not_resolve_a_new_head() -> TestResult {
    let mut replies = prefix()?;
    replies.extend([
        Reply::result(&block())?,
        Reply::raw(503, b"fixture failure".to_vec()),
        Reply::result(&json!("0x5"))?,
    ]);
    let fixture = Fixture::start(replies).await?;
    let client = EvmClient::connect(config(
        &fixture,
        18,
        BlockSelector::Latest,
        1,
        4096,
        Duration::from_secs(2),
    )?)
    .await?;
    let observed = client
        .get_native_balance(Address::parse(ADDRESS)?, None)
        .await?;
    assert_eq!(observed.value().amount().raw().to_string(), "5");
    assert_eq!(observed.context().block().hash().to_string(), block_hash());
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 5);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.body["method"] == "eth_getBlockByNumber")
            .count(),
        1
    );
    assert_eq!(requests[3].body, requests[4].body);
    assert_eq!(
        requests[4].body["params"],
        json!([ADDRESS,{"blockHash":block_hash(),"requireCanonical":true}])
    );
    Ok(())
}

#[tokio::test]
async fn one_balance_deadline_covers_chain_resolution_and_body_consumption() -> TestResult {
    // Every delayed stage fits a fresh budget. Their combined delay exceeds
    // the total while leaving enough margin to reach the final body transfer.
    let mut chain = Reply::result(&json!("0x1"))?;
    chain.delay = Duration::from_millis(400);
    let mut resolved = Reply::result(&block())?;
    resolved.delay = Duration::from_millis(400);
    let mut balance = Reply::result(&json!("0x1"))?;
    balance.framing = Framing::Chunked;
    balance.body_delay = Duration::from_millis(1200);
    let fixture = Fixture::start(vec![
        Reply::result(&json!("0x1"))?,
        chain,
        resolved,
        balance,
    ])
    .await?;
    let client = EvmClient::connect(config(
        &fixture,
        18,
        BlockSelector::Latest,
        2,
        4096,
        Duration::from_millis(1600),
    )?)
    .await?;
    assert_eq!(
        client
            .get_native_balance(Address::parse(ADDRESS)?, None)
            .await
            .unwrap_err(),
        Error::Timeout
    );
    assert_eq!(fixture.requests()?.len(), 4);
    Ok(())
}

#[tokio::test]
async fn response_caps_apply_to_the_balance_body_with_each_http_framing() -> TestResult {
    for framing in [
        Framing::ContentLength,
        Framing::CloseDelimited,
        Framing::Chunked,
    ] {
        let mut oversized = Reply::raw(200, vec![b'x'; 8192]);
        oversized.framing = framing;
        let mut replies = prefix()?;
        replies.extend([Reply::result(&block())?, oversized]);
        let fixture = Fixture::start(replies).await?;
        let client = EvmClient::connect(config(
            &fixture,
            18,
            BlockSelector::Latest,
            2,
            512,
            Duration::from_secs(2),
        )?)
        .await?;
        assert_eq!(
            client
                .get_native_balance(Address::parse(ADDRESS)?, None)
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::ResponseTooLarge)
        );
        assert_eq!(fixture.requests()?.len(), 4);
    }
    Ok(())
}

#[tokio::test]
async fn balance_failures_do_not_disclose_query_headers_or_provider_payloads() -> TestResult {
    let mut replies = prefix()?;
    replies.extend([Reply::result(&block())?, Reply::json(&json!({"jsonrpc":"2.0","id":1,"error":{"code":-32_000,"message":"Bearer fixture-body-token","data":"fixture-data-token"}}))?]);
    let fixture = Fixture::start(replies).await?;
    let endpoint = RpcEndpoint::new(&format!("{}?key=fixture-query-token", fixture.endpoint))?
        .with_header("Authorization", "Bearer fixture-header-token")?;
    let configured = EvmConfig::new(
        NetworkId::new(ChainId::from(1), "fixture-network")?,
        endpoint,
        18,
        None,
        BlockSelector::Latest,
        RpcLimits::new(Duration::from_secs(1), Duration::from_secs(2), 4096, 2)?,
        "fixture-provider",
    )?;
    let client = EvmClient::connect(configured).await?;
    let error = client
        .get_native_balance(Address::parse(ADDRESS)?, None)
        .await
        .unwrap_err();
    assert_eq!(error, Error::Provider(ProviderError::Rpc));
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert!(requests[3].target.contains("fixture-query-token"));
    assert!(requests[3].headers.contains("fixture-header-token"));
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error)?,
    ] {
        for secret in [
            "fixture-query-token",
            "fixture-header-token",
            "fixture-body-token",
            "fixture-data-token",
        ] {
            assert!(!rendered.contains(secret));
        }
    }
    Ok(())
}

#[tokio::test]
async fn concurrent_reads_keep_address_block_and_balance_associations_with_the_same_rpc_id()
-> TestResult {
    let second_address = "0x0000000000000000000000000000000000000002";
    let second_hash = format!("0x{}", "cd".repeat(32));
    let routed_hash = second_hash.clone();
    let fixture = Fixture::start_routed(move |request| {
        let method = request.body["method"].as_str();
        let parameters = &request.body["params"];
        let result = match method {
            Some("eth_chainId") => json!("0x1"),
            Some("eth_getBlockByNumber") if parameters[0] == "0x2a" => block(),
            Some("eth_getBlockByNumber") if parameters[0] == "0x2b" => json!({
                "number":"0x2b","hash":routed_hash,"timestamp":"0x6553f101"
            }),
            Some("eth_getBalance")
                if parameters[0] == ADDRESS && parameters[1]["blockHash"] == block_hash() =>
            {
                json!("0x5")
            }
            Some("eth_getBalance")
                if parameters[0] == second_address && parameters[1]["blockHash"] == routed_hash =>
            {
                json!("0x7")
            }
            _ => return Err(std::io::Error::other("unexpected routed fixture request")),
        };
        let mut reply = Reply::result(&result)
            .map_err(|_| std::io::Error::other("routed fixture reply serialization failed"))?;
        if method == Some("eth_getBalance") && parameters[0] == ADDRESS {
            reply.delay = Duration::from_millis(50);
        }
        Ok(reply)
    })
    .await?;
    let client = EvmClient::connect(config(
        &fixture,
        18,
        BlockSelector::Latest,
        0,
        4096,
        Duration::from_secs(2),
    )?)
    .await?;
    let first_address = Address::parse(ADDRESS)?;
    let second_address = Address::parse(second_address)?;
    let (first, second) = tokio::join!(
        client.get_native_balance(first_address, Some(BlockSelector::Number(42))),
        client.get_native_balance(second_address, Some(BlockSelector::Number(43))),
    );
    let first = first?;
    let second = second?;
    assert_eq!(first.value().address(), &first_address);
    assert_eq!(second.value().address(), &second_address);
    assert_eq!(first.value().amount().raw().to_string(), "5");
    assert_eq!(second.value().amount().raw().to_string(), "7");
    assert_eq!(first.context().block().number(), 42);
    assert_eq!(second.context().block().number(), 43);
    assert_eq!(first.context().block().hash().to_string(), block_hash());
    assert_eq!(second.context().block().hash().to_string(), second_hash);
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 7);
    assert!(requests.iter().all(|request| request.body["id"] == 1));
    Ok(())
}

#[tokio::test]
async fn actual_native_observations_have_stable_wire_fields_and_roundtrip() -> TestResult {
    let mut replies = prefix()?;
    replies.extend([Reply::result(&block())?, Reply::result(&json!("0x5"))?]);
    let fixture = Fixture::start(replies).await?;
    let client = EvmClient::connect(config(
        &fixture,
        0,
        BlockSelector::Finalized,
        0,
        4096,
        Duration::from_secs(2),
    )?)
    .await?;
    let observed = client
        .get_native_balance(Address::parse(ADDRESS)?, None)
        .await?;
    let serialized = serde_json::to_value(&observed)?;
    let network = json!({"chain_id":"1","alias":"fixture-network"});
    let expected = json!({
        "schema_version":1,"operation":"native_balance","network":network,
        "requested_selector":{"kind":"finalized"},
        "block":{"number":42,"hash":block_hash(),"timestamp":1_700_000_000_u64},
        "source":{"provider_id":"fixture-provider","method":"eth_getBalance","integration_version":env!("CARGO_PKG_VERSION")},
        "retrieved_at":observed.context().retrieved_at().unix_seconds(),
        "finality":"unknown","confirmations":null,
        "value":{"address":ADDRESS,
            "asset":{"network":network,"kind":"native","decimals":0,"symbol":"NATIVE","metadata_origin":"caller_configured"},
            "amount":{"raw":"5","decimals":0,"formatted":"5"}}
    });
    assert_eq!(serialized, expected);
    assert_eq!(
        serde_json::from_value::<Observation<Balance>>(serialized)?,
        observed
    );
    Ok(())
}

#[tokio::test]
async fn block_and_balance_stages_enforce_the_same_strict_rpc_envelope() -> TestResult {
    for balance_stage in [false, true] {
        let result = if balance_stage { json!("0x1") } else { block() };
        let encoded_result = serde_json::to_string(&result)?;
        for raw in [
            format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{encoded_result}}}"),
            format!("{{\"jsonrpc\":\"1.0\",\"id\":1,\"result\":{encoded_result}}}"),
            format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{encoded_result},\"error\":null}}"),
            format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"id\":1,\"result\":{encoded_result}}}"),
        ] {
            let mut replies = prefix()?;
            if balance_stage {
                replies.push(Reply::result(&block())?);
            }
            replies.push(Reply::raw(200, raw.into_bytes()));
            let fixture = Fixture::start(replies).await?;
            let client = EvmClient::connect(config(
                &fixture,
                18,
                BlockSelector::Latest,
                2,
                4096,
                Duration::from_secs(2),
            )?)
            .await?;
            assert_eq!(
                client
                    .get_native_balance(Address::parse(ADDRESS)?, None)
                    .await
                    .unwrap_err(),
                Error::Provider(ProviderError::InvalidResponse)
            );
            assert_eq!(fixture.requests()?.len(), if balance_stage { 4 } else { 3 });
        }
    }
    Ok(())
}

#[tokio::test]
async fn transport_and_rate_limit_read_retries_keep_the_captured_hash() -> TestResult {
    let mut dropped = Reply::raw(200, Vec::new());
    dropped.close_connection = true;
    let mut replies = prefix()?;
    replies.extend([
        Reply::result(&block())?,
        dropped,
        Reply::raw(429, b"fixture-body-token".to_vec()),
        Reply::result(&json!("0x5"))?,
    ]);
    let fixture = Fixture::start(replies).await?;
    let client = EvmClient::connect(config(
        &fixture,
        18,
        BlockSelector::Latest,
        2,
        4096,
        Duration::from_secs(2),
    )?)
    .await?;
    assert_eq!(
        client
            .get_native_balance(Address::parse(ADDRESS)?, None)
            .await?
            .value()
            .amount()
            .raw()
            .to_string(),
        "5"
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 6);
    assert_eq!(requests[3].body, requests[4].body);
    assert_eq!(requests[4].body, requests[5].body);
    assert_eq!(
        requests[5].body["params"],
        json!([ADDRESS,{"blockHash":block_hash(),"requireCanonical":true}])
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.body["method"] == "eth_getBlockByNumber")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn malformed_balance_response_diagnostics_do_not_echo_raw_provider_text() -> TestResult {
    let mut replies = prefix()?;
    replies.extend([
        Reply::result(&block())?,
        Reply::raw(200, b"Bearer fixture-body-token not-json".to_vec()),
    ]);
    let fixture = Fixture::start(replies).await?;
    let client = EvmClient::connect(config(
        &fixture,
        18,
        BlockSelector::Latest,
        2,
        4096,
        Duration::from_secs(2),
    )?)
    .await?;
    let error = client
        .get_native_balance(Address::parse(ADDRESS)?, None)
        .await
        .unwrap_err();
    assert_eq!(error, Error::Provider(ProviderError::InvalidResponse));
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error)?,
    ] {
        assert!(!rendered.contains("fixture-body-token"));
        assert!(!rendered.contains("not-json"));
    }
    assert_eq!(fixture.requests()?.len(), 4);
    Ok(())
}
