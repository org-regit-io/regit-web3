// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Deterministic public Solana HTTP contracts over loopback fixtures.

#![cfg(feature = "solana-http")]

use std::{
    future::Future,
    task::{Context, Poll, Waker},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use regit_web3::{
    chains::solana::{
        AccountReader, NativeBalanceReader, SolanaClient, SolanaHttpConfig, TokenBalanceReader,
    },
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        U256,
        solana::{
            Commitment, Hash, NativeBalance, Network, Observation, Pubkey, ReadOptions,
            TokenAccountState,
        },
    },
    error::{Error, ProviderError},
};
use serde_json::{Value, json};

#[path = "support/rpc_server.rs"]
mod server;
use server::{Fixture, Framing, Reply};

const GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

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
    config(endpoint, 0, Duration::from_secs(2), 1024 * 1024)
}

fn genesis() -> Result<Reply, serde_json::Error> {
    Reply::result(&json!(GENESIS))
}

fn response(value: &Value, slot: u64) -> Result<Reply, serde_json::Error> {
    Reply::result(&json!({"context":{"slot":slot,"apiVersion":"3.1.8"},"value":value}))
}

fn options() -> ReadOptions {
    ReadOptions::new(Commitment::Confirmed, Some(100))
}

fn binary_account(data: &str) -> Value {
    json!({"data":[data,"base64"],"lamports":9_007_199_254_740_993_u64,"owner":"11111111111111111111111111111111","executable":false,"rentEpoch":u64::MAX,"space":3})
}

fn token_account(program: &str, owner: &str, raw: &str, decimals: u8) -> Value {
    json!({"data":{"program":program,"space":165,"parsed":{"type":"account","info":{
        "mint":Pubkey::from_bytes([1;32]).to_string(),"owner":Pubkey::from_bytes([2;32]).to_string(),"state":"frozen",
        "tokenAmount":{"amount":raw,"decimals":decimals,"uiAmount":20.0,"uiAmountString":"20"},"extensions":[{"extension":"scaledUiAmount"}]
    }}},"lamports":2_039_280,"owner":owner,"executable":false,"rentEpoch":u64::MAX,"space":165})
}

fn unix_now() -> Result<u64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[tokio::test(flavor = "current_thread")]
async fn connects_only_after_full_genesis_verification() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![genesis()?]).await?;
    let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
    assert_eq!(client.genesis_hash(), Hash::parse(GENESIS)?);
    assert_eq!(client.config().network().alias(), "fixture");
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].body,
        json!({"jsonrpc":"2.0","id":1,"method":"getGenesisHash","params":[]})
    );
    assert!(
        requests[0]
            .headers
            .to_ascii_lowercase()
            .contains("content-type: application/json")
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_genesis_and_strict_envelopes_are_terminal()
-> Result<(), Box<dyn std::error::Error>> {
    let mut replies = vec![
        Reply::result(&json!(&GENESIS[..32]))?,
        Reply::result(&json!(null))?,
        Reply::result(&json!(7))?,
        Reply::result(&json!("not_a_hash"))?,
    ];
    for raw in [
        format!(r#"{{"jsonrpc":"1.0","id":1,"result":"{GENESIS}"}}"#),
        format!(r#"{{"jsonrpc":"2.0","id":2,"result":"{GENESIS}"}}"#),
        format!(r#"{{"jsonrpc":"2.0","id":"1","result":"{GENESIS}"}}"#),
        format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{GENESIS}","error":null}}"#),
        format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{GENESIS}","result":"{GENESIS}"}}"#),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601}}"#.to_owned(),
    ] {
        replies.push(Reply::raw(200, raw.into_bytes()));
    }
    for reply in replies {
        let fixture = Fixture::start(vec![reply]).await?;
        let error = SolanaClient::connect(config(
            &fixture.endpoint,
            2,
            Duration::from_secs(2),
            1024 * 1024,
        )?)
        .await
        .unwrap_err();
        assert_eq!(error, Error::Provider(ProviderError::InvalidResponse));
        assert_eq!(fixture.requests()?.len(), 1);
    }
    let fixture = Fixture::start(vec![Reply::result(&json!(
        Hash::from_bytes([9; 32]).to_string()
    ))?])
    .await?;
    assert_eq!(
        SolanaClient::connect(standard_config(&fixture.endpoint)?)
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::ChainMismatch)
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn native_reads_preserve_u64_precision_request_slot_and_source()
-> Result<(), Box<dyn std::error::Error>> {
    for (raw, commitment, minimum) in [
        (0, Commitment::Processed, None),
        (9_007_199_254_740_993, Commitment::Confirmed, Some(100)),
        (u64::MAX, Commitment::Finalized, Some(100)),
    ] {
        let fixture =
            Fixture::start(vec![genesis()?, genesis()?, response(&json!(raw), 120)?]).await?;
        let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
        let before = unix_now()?;
        let read_options = ReadOptions::new(commitment, minimum);
        let address = Pubkey::from_bytes([7; 32]);
        let observation =
            NativeBalanceReader::get_native_balance(&client, address, read_options).await?;
        let after = unix_now()?;
        assert_eq!(observation.value().address(), address);
        assert_eq!(observation.value().amount().raw(), U256::from(raw));
        assert_eq!(observation.value().amount().decimals(), Some(9));
        assert_eq!(observation.context().slot(), 120);
        assert_eq!(observation.context().requested_options(), read_options);
        assert_eq!(observation.context().source().method(), "getBalance");
        assert_eq!(observation.context().source().provider_id(), "local");
        assert!((before..=after).contains(&observation.context().retrieved_at().unix_seconds()));
        assert_eq!(
            serde_json::from_value::<Observation<NativeBalance>>(serde_json::to_value(
                &observation
            )?)?,
            observation
        );
        let requests = fixture.requests()?;
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[1].body["method"], "getGenesisHash");
        assert_eq!(requests[2].body["method"], "getBalance");
        assert_eq!(requests[2].body["params"][0], json!(address.to_string()));
        assert_eq!(
            requests[2].body["params"][1]["commitment"],
            serde_json::to_value(commitment)?
        );
        if let Some(minimum) = minimum {
            assert_eq!(
                requests[2].body["params"][1]["minContextSlot"],
                json!(minimum)
            );
        } else {
            assert!(
                requests[2].body["params"][1]
                    .get("minContextSlot")
                    .is_none()
            );
        }
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn changed_genesis_stops_every_read_before_requested_operation()
-> Result<(), Box<dyn std::error::Error>> {
    for operation in 0..3 {
        let fixture = Fixture::start(vec![
            genesis()?,
            Reply::result(&json!(Hash::from_bytes([9; 32]).to_string()))?,
        ])
        .await?;
        let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
        let address = Pubkey::from_bytes([7; 32]);
        let error = match operation {
            0 => client
                .get_native_balance(address, options())
                .await
                .unwrap_err(),
            1 => client.get_account(address, options()).await.unwrap_err(),
            _ => client
                .get_token_balance(address, options())
                .await
                .unwrap_err(),
        };
        assert_eq!(error, Error::Provider(ProviderError::ChainMismatch));
        assert_eq!(fixture.requests()?.len(), 2);
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_native_results_are_not_zero_or_retried() -> Result<(), Box<dyn std::error::Error>>
{
    let mut replies = Vec::new();
    for raw in [
        json!(null),
        json!(-1),
        json!(1.5),
        json!("1"),
        json!("18446744073709551616"),
    ] {
        replies.push(response(&raw, 120)?);
    }
    replies.push(Reply::raw(200,br#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":120},"value":18446744073709551616}}"#.to_vec()));
    replies.push(response(&json!(1), 99)?);
    replies.push(Reply::raw(
        200,
        br#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":120,"slot":121},"value":1}}"#
            .to_vec(),
    ));
    replies.push(Reply::result(&json!({"context":{"slot":120}}))?);
    for reply in replies {
        let fixture = Fixture::start(vec![genesis()?, genesis()?, reply]).await?;
        let client = SolanaClient::connect(config(
            &fixture.endpoint,
            2,
            Duration::from_secs(2),
            1024 * 1024,
        )?)
        .await?;
        assert_eq!(
            client
                .get_native_balance(Pubkey::from_bytes([7; 32]), options())
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn account_reads_decode_bytes_and_keep_null_distinct_from_empty_data()
-> Result<(), Box<dyn std::error::Error>> {
    for value in [binary_account("AAH/"), json!(null), {
        let mut empty = binary_account("");
        empty["space"] = json!(0);
        empty
    }] {
        let fixture = Fixture::start(vec![genesis()?, genesis()?, response(&value, 120)?]).await?;
        let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
        let address = Pubkey::from_bytes([7; 32]);
        let observation = AccountReader::get_account(&client, address, options()).await?;
        assert_eq!(observation.context().source().method(), "getAccountInfo");
        if value.is_null() {
            assert!(observation.value().account().is_none());
        } else {
            let account = observation
                .value()
                .account()
                .ok_or("expected present account")?;
            assert_eq!(account.address(), address);
            assert_eq!(
                account.amount().raw(),
                U256::from(9_007_199_254_740_993_u64)
            );
            assert_eq!(account.rent_epoch(), u64::MAX);
            if value["space"] == json!(0) {
                assert!(account.data().is_empty());
            } else {
                assert_eq!(account.data(), [0, 1, 255]);
            }
        }
        let requests = fixture.requests()?;
        assert_eq!(
            requests[2].body["params"],
            json!([address.to_string(),{"commitment":"confirmed","minContextSlot":100,"encoding":"base64"}])
        );
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_account_encoding_and_missing_value_fail()
-> Result<(), Box<dyn std::error::Error>> {
    let mut values = Vec::new();
    for data in [
        json!(["@@", "base64"]),
        json!(["AQ==", "base58"]),
        json!(["AB==", "base64"]),
        json!(["AQ=="]),
    ] {
        let mut value = binary_account("AAH/");
        value["data"] = data;
        values.push(value);
    }
    let mut mismatched = binary_account("AAH/");
    mismatched["space"] = json!(4);
    values.push(mismatched);
    let mut invalid_owner = binary_account("AAH/");
    invalid_owner["owner"] = json!("FAKE_SECRET_INVALID_KEY");
    values.push(invalid_owner);
    for value in values {
        let fixture = Fixture::start(vec![genesis()?, genesis()?, response(&value, 120)?]).await?;
        let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
        assert_eq!(
            client
                .get_account(Pubkey::from_bytes([7; 32]), options())
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
    }
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        Reply::result(&json!({"context":{"slot":120}}))?,
    ])
    .await?;
    let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
    assert_eq!(
        client
            .get_account(Pubkey::from_bytes([7; 32]), options())
            .await
            .unwrap_err(),
        Error::Provider(ProviderError::InvalidResponse)
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn spl_reads_preserve_mint_program_owner_state_and_raw_units_over_scaled_ui()
-> Result<(), Box<dyn std::error::Error>> {
    for (program, owner, decimals) in [
        ("spl-token", TOKEN, 0),
        ("spl-token-2022", TOKEN_2022, 6),
        ("spl-token-2022", TOKEN_2022, 255),
    ] {
        let fixture = Fixture::start(vec![
            genesis()?,
            genesis()?,
            response(
                &token_account(program, owner, "18446744073709551615", decimals),
                120,
            )?,
        ])
        .await?;
        let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
        let address = Pubkey::from_bytes([7; 32]);
        let observation =
            TokenBalanceReader::get_token_balance(&client, address, options()).await?;
        let balance = observation.value();
        assert_eq!(balance.token_account(), address);
        assert_eq!(balance.owner(), Pubkey::from_bytes([2; 32]));
        assert_eq!(balance.asset().mint(), Pubkey::from_bytes([1; 32]));
        assert_eq!(balance.asset().token_program(), Pubkey::parse(owner)?);
        assert_eq!(balance.amount().raw(), U256::from(u64::MAX));
        assert_eq!(balance.amount().decimals(), Some(decimals));
        assert_eq!(balance.state(), TokenAccountState::Frozen);
        assert_eq!(observation.context().source().method(), "getAccountInfo");
        assert_eq!(
            fixture.requests()?[2].body["params"][1]["encoding"],
            "jsonParsed"
        );
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_spl_records_are_terminal_without_binary_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let mut values = vec![
        binary_account("AAH/"),
        token_account("spl-token", TOKEN_2022, "1", 6),
        token_account("stake", TOKEN, "1", 6),
    ];
    for raw in ["-1", "1.0", "01", "18446744073709551616", "FAKE_SECRET"] {
        values.push(token_account("spl-token", TOKEN, raw, 6));
    }
    for (path, replacement) in [
        ("type", json!("mint")),
        ("state", json!("invalid")),
        ("decimals", json!(null)),
    ] {
        let mut value = token_account("spl-token", TOKEN, "1", 6);
        match path {
            "type" => value["data"]["parsed"]["type"] = replacement,
            "state" => value["data"]["parsed"]["info"]["state"] = replacement,
            _ => value["data"]["parsed"]["info"]["tokenAmount"]["decimals"] = replacement,
        }
        values.push(value);
    }
    for value in values {
        let fixture = Fixture::start(vec![genesis()?, genesis()?, response(&value, 120)?]).await?;
        let client = SolanaClient::connect(config(
            &fixture.endpoint,
            2,
            Duration::from_secs(2),
            1024 * 1024,
        )?)
        .await?;
        assert_eq!(
            client
                .get_token_balance(Pubkey::from_bytes([7; 32]), options())
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    let fixture =
        Fixture::start(vec![genesis()?, genesis()?, response(&json!(null), 120)?]).await?;
    let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
    assert_eq!(
        client
            .get_token_balance(Pubkey::from_bytes([7; 32]), options())
            .await
            .unwrap_err(),
        Error::UnavailableData
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn critical_account_and_token_duplicates_are_rejected_from_raw_json()
-> Result<(), Box<dyn std::error::Error>> {
    let account = serde_json::to_string(&binary_account("AAH/"))?.replace(
        "\"lamports\":9007199254740993",
        "\"lamports\":9007199254740993,\"lamports\":1",
    );
    let token = serde_json::to_string(&token_account("spl-token", TOKEN, "1", 6))?
        .replace("\"amount\":\"1\"", "\"amount\":\"1\",\"amount\":\"2\"");
    for (raw, is_token) in [(account, false), (token, true)] {
        let envelope = format!(
            r#"{{"jsonrpc":"2.0","id":1,"result":{{"context":{{"slot":120}},"value":{raw}}}}}"#
        );
        let fixture = Fixture::start(vec![
            genesis()?,
            genesis()?,
            Reply::raw(200, envelope.into_bytes()),
        ])
        .await?;
        let client = SolanaClient::connect(standard_config(&fixture.endpoint)?).await?;
        let error = if is_token {
            client
                .get_token_balance(Pubkey::from_bytes([7; 32]), options())
                .await
                .unwrap_err()
        } else {
            client
                .get_account(Pubkey::from_bytes([7; 32]), options())
                .await
                .unwrap_err()
        };
        assert_eq!(error, Error::Provider(ProviderError::InvalidResponse));
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn retry_attempts_keep_identical_read_options_and_do_not_repeat_genesis_stage()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        Reply::raw(503, b"FAKE_REMOTE_SECRET".to_vec()),
        response(&json!(42), 120)?,
    ])
    .await?;
    let client = SolanaClient::connect(config(
        &fixture.endpoint,
        1,
        Duration::from_secs(2),
        1024 * 1024,
    )?)
    .await?;
    assert_eq!(
        client
            .get_native_balance(Pubkey::from_bytes([7; 32]), options())
            .await?
            .value()
            .amount()
            .raw(),
        U256::from(42)
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].body, requests[3].body);
    assert_eq!(requests[2].body["method"], "getBalance");
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn one_deadline_covers_genesis_and_slow_chunked_read_body()
-> Result<(), Box<dyn std::error::Error>> {
    let mut delayed_genesis = genesis()?;
    delayed_genesis.delay = Duration::from_millis(90);
    let mut slow = response(&json!(42), 120)?;
    slow.framing = Framing::Chunked;
    slow.body_delay = Duration::from_millis(140);
    let fixture = Fixture::start(vec![genesis()?, delayed_genesis, slow]).await?;
    let client = SolanaClient::connect(config(
        &fixture.endpoint,
        0,
        Duration::from_millis(200),
        1024 * 1024,
    )?)
    .await?;
    assert_eq!(
        client
            .get_native_balance(Pubkey::from_bytes([7; 32]), options())
            .await
            .unwrap_err(),
        Error::Timeout
    );
    assert_eq!(fixture.requests()?.len(), 3);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn response_limits_and_error_diagnostics_hide_supplied_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    let errors = [
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"FAKE_REMOTE_SECRET","data":{"credential":"FAKE_BODY_SECRET"}}}),
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32016,"message":"FAKE_REMOTE_SECRET"}}),
    ];
    for (index, remote) in errors.into_iter().enumerate() {
        let fixture = Fixture::start(vec![genesis()?, genesis()?, Reply::json(&remote)?]).await?;
        let endpoint = RpcEndpoint::new(&format!("{}?key=FAKE_QUERY_SECRET", fixture.endpoint))?
            .with_header("x-api-key", "FAKE_HEADER_SECRET")?;
        let http = HttpConfig::new(
            endpoint,
            RpcLimits::new(
                Duration::from_secs(2),
                Duration::from_secs(2),
                1024 * 1024,
                0,
            )?,
            "local",
        )?;
        let client = SolanaClient::connect(SolanaHttpConfig::new(
            Network::new(Hash::parse(GENESIS)?, "fixture")?,
            http,
        ))
        .await?;
        let error = client
            .get_native_balance(Pubkey::from_bytes([7; 32]), options())
            .await
            .unwrap_err();
        assert_eq!(
            error,
            if index == 0 {
                Error::UnsupportedCapability
            } else {
                Error::Provider(ProviderError::Rpc)
            }
        );
        for diagnostic in [
            format!("{client:?}"),
            error.to_string(),
            format!("{error:?}"),
            serde_json::to_string(&error)?,
        ] {
            for sentinel in [
                "FAKE_QUERY_SECRET",
                "FAKE_HEADER_SECRET",
                "FAKE_REMOTE_SECRET",
                "FAKE_BODY_SECRET",
            ] {
                assert!(!diagnostic.contains(sentinel));
            }
        }
        let requests = fixture.requests()?;
        assert!(requests[2].target.contains("FAKE_QUERY_SECRET"));
        assert!(requests[2].headers.contains("FAKE_HEADER_SECRET"));
    }
    for framing in [
        Framing::ContentLength,
        Framing::CloseDelimited,
        Framing::Chunked,
    ] {
        let mut oversized = Reply::raw(200, vec![b'x'; 4096]);
        oversized.framing = framing;
        let fixture = Fixture::start(vec![genesis()?, genesis()?, oversized]).await?;
        let client =
            SolanaClient::connect(config(&fixture.endpoint, 2, Duration::from_secs(2), 512)?)
                .await?;
        assert_eq!(
            client
                .get_account(Pubkey::from_bytes([7; 32]), options())
                .await
                .unwrap_err(),
            Error::Provider(ProviderError::ResponseTooLarge)
        );
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}

#[test]
fn connection_requires_a_caller_runtime_without_starting_one()
-> Result<(), Box<dyn std::error::Error>> {
    let future = SolanaClient::connect(standard_config("http://127.0.0.1:1")?);
    let mut future = std::pin::pin!(future);
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(Error::Configuration))
    ));
    Ok(())
}
