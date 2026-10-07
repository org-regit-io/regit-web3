// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Actual native-balance example processes against deterministic HTTP fixtures.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "evm-http")]

#[path = "support/rpc_server.rs"]
mod rpc_server;

use std::{
    io,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

use regit_web3::domain::{Balance, Observation};
use serde_json::json;

use rpc_server::{Fixture, Framing, Reply};

type TestResult = Result<(), Box<dyn std::error::Error>>;
type Inputs = Vec<(&'static str, String)>;

const ADDRESS: &str = "0x0000000000000000000000000000000000000001";
const INPUT_NAMES: [&str; 8] = [
    "REGIT_WEB3_RPC_URL",
    "REGIT_WEB3_CHAIN_ID",
    "REGIT_WEB3_NETWORK_ALIAS",
    "REGIT_WEB3_ADDRESS",
    "REGIT_WEB3_NATIVE_DECIMALS",
    "REGIT_WEB3_PROVIDER_ID",
    "REGIT_WEB3_BLOCK_SELECTOR",
    "REGIT_WEB3_NATIVE_SYMBOL",
];

fn inputs(endpoint: &str) -> Inputs {
    vec![
        ("REGIT_WEB3_RPC_URL", endpoint.to_owned()),
        ("REGIT_WEB3_CHAIN_ID", "1".to_owned()),
        ("REGIT_WEB3_NETWORK_ALIAS", "fixture-network".to_owned()),
        ("REGIT_WEB3_ADDRESS", ADDRESS.to_owned()),
        ("REGIT_WEB3_NATIVE_DECIMALS", "0".to_owned()),
        ("REGIT_WEB3_PROVIDER_ID", "fixture-provider".to_owned()),
        ("REGIT_WEB3_BLOCK_SELECTOR", "number:42".to_owned()),
    ]
}

fn block_hash() -> String {
    format!("0x{}", "ab".repeat(32))
}

fn before_balance() -> Result<Vec<Reply>, serde_json::Error> {
    let mut block = Reply::result(&json!({
        "number":"0x2a", "hash":block_hash(), "timestamp":"0x6553f100"
    }))?;
    block.framing = Framing::Chunked;
    Ok(vec![
        Reply::result(&json!("0x1"))?,
        Reply::result(&json!("0x1"))?,
        block,
    ])
}

async fn run_example(inputs: Inputs) -> io::Result<Output> {
    tokio::task::spawn_blocking(move || {
        let mut command = Command::new(env!("CARGO"));
        command
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args([
                "run",
                "--quiet",
                "--frozen",
                "--no-default-features",
                "--features",
                "evm-http",
                "--example",
                "native_balance",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in INPUT_NAMES {
            command.env_remove(name);
        }
        command.envs(inputs);
        let mut child = command.spawn()?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if child.try_wait()?.is_some() {
                return child.wait_with_output();
            }
            if Instant::now() >= deadline {
                child.kill()?;
                child.wait()?;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "fixture example process timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })
    .await
    .map_err(|_| io::Error::other("fixture example process task failed"))?
}

#[tokio::test]
async fn actual_example_returns_one_exact_observation_with_explicit_configuration() -> TestResult {
    let mut replies = before_balance()?;
    replies.push(Reply::result(&json!("0x20000000000001"))?);
    let fixture = Fixture::start(replies).await?;
    let output = run_example(inputs(&fixture.endpoint)).await?;
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout)?;
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.ends_with('\n'));
    let observed: Observation<Balance> = serde_json::from_str(&stdout)?;
    assert_eq!(observed.value().address().to_string(), ADDRESS);
    assert_eq!(
        observed.value().amount().raw().to_string(),
        "9007199254740993"
    );
    assert_eq!(
        observed.value().amount().formatted().as_deref(),
        Some("9007199254740993")
    );
    assert_eq!(observed.value().asset().decimals(), 0);
    assert_eq!(observed.value().asset().symbol(), None);
    assert_eq!(
        observed.context().source().provider_id(),
        "fixture-provider"
    );
    assert_eq!(observed.context().block().hash().to_string(), block_hash());
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].body["params"], json!(["0x2a", false]));
    assert_eq!(
        requests[3].body["params"],
        json!([ADDRESS,{"blockHash":block_hash(),"requireCanonical":true}])
    );
    Ok(())
}

#[tokio::test]
async fn actual_example_remote_failure_has_a_fixed_exit_and_no_credential_output() -> TestResult {
    let mut failure = Reply::raw(
        200,
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"error":{
            "code":-32_000,"message":"Bearer fixture-body-token","data":"fixture-data-token"
        }}))?,
    );
    failure.framing = Framing::CloseDelimited;
    let mut replies = before_balance()?;
    replies.push(failure);
    let fixture = Fixture::start(replies).await?;
    let endpoint = format!("{}?key=fixture-query-token", fixture.endpoint).replacen(
        "http://",
        "http://fixture-user:fixture-password@",
        1,
    );
    let output = run_example(inputs(&endpoint)).await?;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr)?;
    assert_eq!(stderr, "provider RPC failure\n");
    for sensitive in [
        "fixture-user",
        "fixture-password",
        "fixture-query-token",
        "fixture-body-token",
        "fixture-data-token",
    ] {
        assert!(!stderr.contains(sensitive));
    }
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert!(requests[3].target.contains("fixture-query-token"));
    assert!(
        requests[3]
            .headers
            .to_ascii_lowercase()
            .contains("authorization:")
    );
    Ok(())
}

#[tokio::test]
async fn actual_example_rejects_invalid_or_missing_input_before_network_access() -> TestResult {
    let fixture = Fixture::start(Vec::new()).await?;
    let mut invalid = inputs(&fixture.endpoint);
    for (name, value) in &mut invalid {
        if *name == "REGIT_WEB3_NATIVE_DECIMALS" {
            *value = "018".to_owned();
        }
    }
    let mut missing = inputs(&fixture.endpoint);
    missing.retain(|(name, _)| *name != "REGIT_WEB3_ADDRESS");
    for configured in [invalid, missing] {
        let output = run_example(configured).await?;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"invalid configuration\n");
    }
    assert!(fixture.requests()?.is_empty());
    Ok(())
}
