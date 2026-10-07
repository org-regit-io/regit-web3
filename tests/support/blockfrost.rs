// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Integration-local recorder for JSON GETs and exact non-JSON CBOR submission.

use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    error::Error,
    providers::blockfrost::BlockfrostHttpConfig,
};
use serde_json::{Value, json};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

pub(crate) const CREDENTIAL: &str = "explicit-project-fixture-credential";
#[derive(Clone)]
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) body: String,
    pub(crate) delay: Duration,
}
impl Reply {
    pub(crate) fn json(value: &Value) -> Self {
        Self::raw(200, value.to_string())
    }
    pub(crate) fn raw(status: u16, body: String) -> Self {
        Self {
            status,
            body,
            delay: Duration::ZERO,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub(crate) headers: String,
    pub(crate) body: Vec<u8>,
}
pub(crate) struct Fixture {
    pub(crate) endpoint: String,
    requests: Arc<Mutex<Vec<Request>>>,
    task: JoinHandle<io::Result<()>>,
}
impl Fixture {
    pub(crate) async fn start(replies: Vec<Reply>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/api/v0/", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            for reply in replies {
                let (mut stream, _) = listener.accept().await?;
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    if bytes.len() > 16_384 {
                        return Err(io::Error::other("fixture header limit"));
                    }
                    let n = stream.read(&mut buffer).await?;
                    if n == 0 {
                        return Err(io::ErrorKind::UnexpectedEof.into());
                    }
                    bytes.extend(&buffer[..n]);
                }
                let end = bytes
                    .windows(4)
                    .position(|part| part == b"\r\n\r\n")
                    .ok_or_else(|| io::Error::other("fixture header framing"))?
                    + 4;
                let headers = String::from_utf8(bytes[..end].to_vec())
                    .map_err(|_| io::Error::other("fixture header encoding"))?;
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (k, v) = line.split_once(':')?;
                        k.eq_ignore_ascii_case("content-length")
                            .then(|| v.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if length > 65_536 {
                    return Err(io::Error::other("fixture body limit"));
                }
                while bytes.len() - end < length {
                    let n = stream.read(&mut buffer).await?;
                    if n == 0 {
                        return Err(io::ErrorKind::UnexpectedEof.into());
                    }
                    bytes.extend(&buffer[..n]);
                }
                captured
                    .lock()
                    .map_err(|_| io::Error::other("fixture lock"))?
                    .push(Request {
                        headers,
                        body: bytes[end..end + length].to_vec(),
                    });
                let header = format!(
                    "HTTP/1.1 {} Fixture\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    reply.status,
                    reply.body.len()
                );
                stream.write_all(header.as_bytes()).await?;
                tokio::time::sleep(reply.delay).await;
                stream.write_all(reply.body.as_bytes()).await?;
                stream.shutdown().await?;
            }
            Ok(())
        });
        Ok(Self {
            endpoint,
            requests,
            task,
        })
    }
    pub(crate) fn requests(&self) -> io::Result<Vec<Request>> {
        self.requests
            .lock()
            .map(|items| items.clone())
            .map_err(|_| io::Error::other("fixture lock poisoned"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub(crate) fn genesis() -> Reply {
    Reply::json(&json!({"network_magic":764_824_073,"system_start":1_506_203_091}))
}
pub(crate) fn config(
    endpoint: &str,
    retries: u8,
    total: Duration,
    cap: usize,
) -> Result<BlockfrostHttpConfig, Error> {
    Ok(BlockfrostHttpConfig::new(
        super::cardano::network()?,
        HttpConfig::new(
            RpcEndpoint::new(endpoint)?.with_header("project_id", CREDENTIAL)?,
            RpcLimits::new(Duration::from_secs(5), total, cap, retries)?,
            "fixture-blockfrost",
        )?,
    ))
}
pub(crate) fn standard(endpoint: &str) -> Result<BlockfrostHttpConfig, Error> {
    config(endpoint, 0, Duration::from_secs(10), 2 * 1024 * 1024)
}
pub(crate) fn params() -> Value {
    json!({"epoch":600,"min_fee_a":44,"min_fee_b":155_381,"max_tx_size":16_384,"protocol_major_ver":10,"protocol_minor_ver":0,"key_deposit":"2000000","pool_deposit":"500000000","max_val_size":"5000","coins_per_utxo_size":"4310","price_mem":0.0577,"price_step":0.000_072_1})
}
pub(crate) fn units(coin: u64) -> Value {
    json!([{"unit":"lovelace","quantity":coin.to_string()},{"unit":format!("{}00ff","03".repeat(28)),"quantity":"7"}])
}
pub(crate) fn tx_summary(cbor: &regit_web3::domain::cardano::TransactionCbor) -> Value {
    json!({"hash":cbor.transaction_id(),"block":"05".repeat(32),"block_height":12_000_000,"block_time":1_700_000_000,"slot":199_000_000,"index":0,"output_amount":units(9_500_000),"fees":"500000","deposit":"0","size":cbor.bytes().len(),"invalid_before":"199000000","invalid_hereafter":"200000000","utxo_count":2,"valid_contract":true,"treasury_donation":"0"})
}
