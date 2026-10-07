// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Certificate-verified TLS loopback proof for actual BCH public Rust operations.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "bitcoin-cash-electrum")]

use regit_web3::{
    chains::bitcoin_cash::{ElectrumClient, ElectrumConfig, ElectrumEndpoint, TlsTrustRoots},
    config::RpcLimits,
    domain::bitcoin_cash::*,
    error::{Error, ProviderError, ValidationError},
};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};
use tokio_rustls::TlsAcceptor;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ADDRESS: &str = "bitcoincash:qqvd9p0t8dxrs5twh4e7pznjq8s2vgdc4chzqrew27";
const TXID: &str = "f9540793088014f8f4a7f3ee29233c8e99932ab54e90a93d12811ed1a8445970";
const ROOT: &[u8] = include_bytes!("fixtures/bitcoin_cash/test-root.der");

#[derive(Clone)]
struct Reply {
    bytes: Vec<u8>,
    delay: Duration,
    drop_connection: bool,
}
impl Reply {
    fn result(value: Value) -> Result<Self, serde_json::Error> {
        let mut response = json!({"jsonrpc":"2.0","id":0});
        response["result"] = value;
        Ok(Self {
            bytes: serde_json::to_vec(&response)?,
            delay: Duration::ZERO,
            drop_connection: false,
        })
    }
    fn delayed(mut self) -> Self {
        self.delay = Duration::from_millis(1200);
        self
    }
}
struct Server {
    port: u16,
    requests: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start(mut replies: Vec<Reply>) -> Result<Self, Box<dyn std::error::Error>> {
        let tls = ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(
                include_bytes!("fixtures/bitcoin_cash/test-leaf.der").to_vec(),
            )],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                include_bytes!("fixtures/bitcoin_cash/test-leaf-key.der").to_vec(),
            )),
        )?;
        let acceptor = TlsAcceptor::from(Arc::new(tls));
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        replies.reverse();
        let task = tokio::spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    return;
                };
                let Ok(tls) = acceptor.accept(tcp).await else {
                    continue;
                };
                let mut stream = BufReader::new(tls);
                loop {
                    let mut line = String::new();
                    let Ok(n) = stream.read_line(&mut line).await else {
                        break;
                    };
                    if n == 0 {
                        break;
                    }
                    let Ok(request) = serde_json::from_str::<Value>(&line) else {
                        break;
                    };
                    if let Ok(mut records) = seen.lock() {
                        records.push(request.clone());
                    } else {
                        return;
                    }
                    let Some(reply) = replies.pop() else {
                        break;
                    };
                    if reply.drop_connection {
                        break;
                    }
                    tokio::time::sleep(reply.delay).await;
                    let mut bytes = reply.bytes;
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        bytes = text
                            .replacen("\"id\":0", &format!("\"id\":{}", request["id"]), 1)
                            .into_bytes();
                    }
                    bytes.push(b'\n');
                    if stream.get_mut().write_all(&bytes).await.is_err() {
                        break;
                    }
                    if stream.get_mut().flush().await.is_err() {
                        break;
                    }
                }
            }
        });
        Ok(Self {
            port,
            requests,
            task,
        })
    }
    fn config(&self, seconds: u64, body: usize, retries: u8) -> Result<ElectrumConfig, Error> {
        ElectrumConfig::new(
            NetworkIdentity::mainnet("mainnet")?,
            ElectrumEndpoint::new("127.0.0.1", self.port, "bitcoin-cash.fixture")?,
            TlsTrustRoots::new(vec![ROOT.to_vec()])?,
            RpcLimits::new(
                Duration::from_secs(seconds),
                Duration::from_secs(seconds),
                body,
                retries,
            )?,
            "fixture",
        )
    }
    fn requests(&self) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
        Ok(self
            .requests
            .lock()
            .map_err(|_| "fixture lock poisoned")?
            .clone())
    }
}
fn verification() -> Result<Vec<Reply>, serde_json::Error> {
    Ok(vec![
        Reply::result(json!(["Fulcrum 2.1.3", "1.6"]))?,
        Reply::result(
            json!({"genesis_hash":"000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f","hash_function":"sha256","cashtokens":true,"server_version":"Fulcrum 2.1.3","protocol_min":"1.4","protocol_max":"1.6"}),
        )?,
        Reply::result(json!(
            include_str!("fixtures/bitcoin_cash/genesis.hex").trim()
        ))?,
        Reply::result(json!(include_str!("fixtures/bitcoin_cash/fork.hex").trim()))?,
    ])
}
fn with_read(result: Value) -> Result<Vec<Reply>, serde_json::Error> {
    let mut r = verification()?;
    r.extend(verification()?);
    r.push(Reply::result(result)?);
    Ok(r)
}
fn address() -> Result<Address, Error> {
    Address::parse(ADDRESS, AddressNamespace::Mainnet)
}
fn txid() -> Result<Txid, Error> {
    Txid::parse(TXID)
}
fn range() -> Result<HistoryRange, Error> {
    HistoryRange::new(
        971_720,
        HistoryUpperBound::OpenTip,
        CollectionLimit::new(100)?,
    )
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn fixture(value: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(value)
}
async fn balance_failure(value: Value) -> Result<Error, Box<dyn std::error::Error>> {
    let server = Server::start(with_read(value)?).await?;
    let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
    client
        .get_address_balance(address()?, TokenFilter::IncludeTokens)
        .await
        .err()
        .ok_or_else(|| "expected balance failure".into())
}

#[tokio::test]
async fn balance_uses_local_scripthash_explicit_token_filter_and_exact_quantities() -> TestResult {
    let server = Server::start(with_read(
        json!({"confirmed":9_007_199_254_740_993_u64,"unconfirmed":-9_007_199_254_740_993_i64}),
    )?)
    .await?;
    let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
    let value = client
        .get_address_balance(address()?, TokenFilter::IncludeTokens)
        .await?;
    assert_eq!(value.value().confirmed.raw(), 9_007_199_254_740_993);
    assert_eq!(value.value().unconfirmed.raw(), -9_007_199_254_740_993);
    assert_eq!(
        value
            .context()
            .protocol_metadata()
            .ok_or(Error::UnavailableData)?
            .version
            .as_str(),
        "1.6"
    );
    assert_eq!(
        serde_json::from_value::<Observation<AddressBalance>>(serde_json::to_value(&value)?)?,
        value
    );
    let requests = server.requests()?;
    assert_eq!(requests.len(), 9);
    assert_eq!(requests[8]["method"], "blockchain.scripthash.get_balance");
    assert_eq!(
        requests[8]["params"],
        json!([address()?.script_hash().to_string(), "include_tokens"])
    );
    Ok(())
}
#[tokio::test]
async fn history_reads_complete_explicit_interval_and_rejects_truncation() -> TestResult {
    let source = fixture(include_str!("fixtures/bitcoin_cash/history.json"))?;
    let server = Server::start(with_read(source.clone())?).await?;
    let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
    let value = client.get_address_history(address()?, range()?).await?;
    assert_eq!(value.value().entries().len(), 15);
    assert_eq!(
        server.requests()?[8]["params"],
        json!([address()?.script_hash().to_string(), 971_720, -1])
    );
    assert_eq!(
        serde_json::from_value::<Observation<History>>(serde_json::to_value(&value)?)?,
        value
    );
    let server = Server::start(with_read(source)?).await?;
    let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
    assert_eq!(
        client
            .get_address_history(
                address()?,
                HistoryRange::new(
                    971_720,
                    HistoryUpperBound::OpenTip,
                    CollectionLimit::new(14)?
                )?
            )
            .await,
        Err(invalid())
    );
    Ok(())
}
#[tokio::test]
async fn exact_fee_and_unavailable_sentinel_retain_bch_per_thousand_bytes() -> TestResult {
    for (source, expected) in [("1e-5", Some("0.00001")), ("-1", None)] {
        let server = Server::start(with_read(fixture(source)?)?).await?;
        let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
        let value = client.get_fee_estimate(FeeTarget::new(6)?).await?;
        assert_eq!(
            value
                .value()
                .bch_per_kilobyte()
                .map(regit_web3::domain::ExactDecimal::canonical)
                .as_deref(),
            expected
        );
        assert_eq!(server.requests()?[8]["params"], json!([6]));
    }
    Ok(())
}
#[tokio::test]
async fn status_positive_header_zero_and_unknown_are_separate_source_facts() -> TestResult {
    for height in [json!(971_820), json!(0), Value::Null] {
        let mut replies = with_read(height.clone())?;
        if height == 971_820 {
            replies.push(Reply::result(fixture(include_str!(
                "fixtures/bitcoin_cash/inclusion.json"
            ))?)?);
        }
        let server = Server::start(replies).await?;
        let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
        let value = client.get_transaction_status(txid()?).await?;
        assert_eq!(
            value.value().source_height(),
            if height == 971_820 {
                SourceHeight::Positive { height: 971_820 }
            } else if height == 0 {
                SourceHeight::Zero
            } else {
                SourceHeight::Unknown
            }
        );
        assert_eq!(value.value().inclusion().is_some(), height == 971_820);
        assert_eq!(
            server.requests()?.len(),
            if height == 971_820 { 10 } else { 9 }
        );
        assert_eq!(
            serde_json::from_value::<Observation<TransactionStatus>>(serde_json::to_value(
                &value
            )?)?,
            value
        );
    }
    Ok(())
}
#[tokio::test]
async fn full_transaction_checks_raw_identity_and_preserves_complete_source_fields() -> TestResult {
    let mut replies = with_read(fixture(include_str!(
        "fixtures/bitcoin_cash/coinbase.json"
    ))?)?;
    replies.push(Reply::result(json!(
        include_str!("fixtures/bitcoin_cash/coinbase.hex").trim()
    ))?);
    replies.push(Reply::result(json!(971_820))?);
    replies.push(Reply::result(fixture(include_str!(
        "fixtures/bitcoin_cash/inclusion.json"
    ))?)?);
    let server = Server::start(replies).await?;
    let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
    let value = client
        .get_transaction(txid()?, CollectionLimit::new(10)?)
        .await?;
    let data = value.value().data();
    assert_eq!(data.raw.txid(), txid()?);
    assert_eq!(data.size, 123);
    assert_eq!(data.outputs[0].value.raw(), 312_508_205);
    assert_eq!(
        data.outputs[0]
            .addresses
            .as_ref()
            .ok_or(Error::UnavailableData)?[0],
        address()?
    );
    assert!(matches!(
        data.inputs[0],
        TransactionInput::Coinbase {
            sequence: u32::MAX,
            ..
        }
    ));
    assert_eq!(
        serde_json::from_value::<Observation<Transaction>>(serde_json::to_value(&value)?)?,
        value
    );
    let requests = server.requests()?;
    assert_eq!(requests.len(), 12);
    assert_eq!(requests[8]["params"], json!([TXID, true]));
    assert_eq!(requests[9]["params"], json!([TXID, false]));
    Ok(())
}
#[tokio::test]
async fn utxos_preserve_exact_cash_token_amount_and_nft_facts() -> TestResult {
    let source = json!([{"height":971_820,"tx_hash":TXID,"tx_pos":0,"value":1000,"token_data":{"category":"ab".repeat(32),"amount":"9007199254740993","nft":{"capability":"minting","commitment":"aabb"}}}]);
    let server = Server::start(with_read(source)?).await?;
    let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
    let value = client
        .get_unspent_outputs(
            address()?,
            TokenFilter::TokensOnly,
            CollectionLimit::new(1)?,
        )
        .await?;
    let token = value.value().outputs()[0]
        .token_data
        .as_ref()
        .ok_or(Error::UnavailableData)?;
    assert_eq!(token.amount(), 9_007_199_254_740_993);
    assert_eq!(
        token
            .nft()
            .ok_or(Error::UnavailableData)?
            .commitment()
            .hex(),
        "aabb"
    );
    assert_eq!(
        serde_json::from_value::<Observation<UnspentOutputs>>(serde_json::to_value(&value)?)?,
        value
    );
    assert_eq!(server.requests()?[8]["params"][1], "tokens_only");
    Ok(())
}
#[tokio::test]
async fn shared_bitcoin_genesis_cannot_replace_bch_fork_checkpoint() -> TestResult {
    let mut replies = verification()?;
    replies[3] = Reply::result(json!(
        include_str!("fixtures/bitcoin_cash/genesis.hex").trim()
    ))?;
    let server = Server::start(replies).await?;
    assert!(matches!(
        ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await,
        Err(Error::Validation(ValidationError::NetworkMismatch))
    ));
    assert_eq!(server.requests()?.len(), 4);
    Ok(())
}
#[tokio::test]
async fn tls_certificate_hostname_and_token_protocol_checks_remain_enabled() -> TestResult {
    let server = Server::start(verification()?).await?;
    let config = ElectrumConfig::new(
        NetworkIdentity::mainnet("mainnet")?,
        ElectrumEndpoint::new("127.0.0.1", server.port, "wrong.fixture")?,
        TlsTrustRoots::new(vec![ROOT.to_vec()])?,
        RpcLimits::new(
            Duration::from_secs(10),
            Duration::from_secs(10),
            1024 * 1024,
            0,
        )?,
        "fixture",
    )?;
    assert!(matches!(
        ElectrumClient::connect(config).await,
        Err(Error::Provider(ProviderError::Transport))
    ));
    assert!(server.requests()?.is_empty());
    let mut replies = verification()?;
    replies[1] = Reply::result(
        json!({"genesis_hash":"000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f","hash_function":"sha256","cashtokens":false,"server_version":"Fulcrum 2.1.3","protocol_min":"1.4","protocol_max":"1.6"}),
    )?;
    let server = Server::start(replies).await?;
    assert!(matches!(
        ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await,
        Err(Error::UnsupportedCapability)
    ));
    assert_eq!(server.requests()?.len(), 2);
    Ok(())
}
#[tokio::test]
async fn envelope_ids_null_missing_duplicates_and_diagnostics_are_checked() -> TestResult {
    for bytes in [br#"{"jsonrpc":"2.0","id":999,"result":{"confirmed":1,"unconfirmed":0}}"#.as_slice(),br#"{"jsonrpc":"2.0","id":0,"result":null}"#,br#"{"jsonrpc":"2.0","id":0}"#,br#"{"jsonrpc":"2.0","id":0,"result":{"confirmed":1,"confirmed":2,"unconfirmed":0}}"#,br#"{"jsonrpc":"2.0","id":0,"result":{},"error":{"code":-1,"message":"private-source-diagnostic"}}"#]{
        let mut replies=verification()?;replies.extend(verification()?);replies.push(Reply{bytes:bytes.to_vec(),delay:Duration::ZERO,drop_connection:false});
        let server=Server::start(replies).await?;let client=ElectrumClient::connect(server.config(10,1024*1024,0)?).await?;
        let failure=client.get_address_balance(address()?,TokenFilter::IncludeTokens).await.err().ok_or(Error::UnavailableData)?;assert_eq!(failure,invalid());assert!(!format!("{failure:?} {failure}").contains("private-source-diagnostic"));assert_eq!(server.requests()?.len(),9);
    }
    for value in [
        json!({"confirmed":null,"unconfirmed":0}),
        json!({"confirmed":1}),
        json!({"confirmed":1,"unconfirmed":0.5}),
    ] {
        assert_eq!(balance_failure(value).await?, invalid());
    }
    Ok(())
}
#[tokio::test]
async fn retries_reverify_same_explicit_source_without_changing_query() -> TestResult {
    let mut replies = verification()?;
    replies.extend(verification()?);
    replies.push(Reply {
        bytes: Vec::new(),
        delay: Duration::ZERO,
        drop_connection: true,
    });
    replies.extend(verification()?);
    replies.push(Reply::result(json!({"confirmed":2,"unconfirmed":0}))?);
    let server = Server::start(replies).await?;
    let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 1)?).await?;
    assert_eq!(
        client
            .get_address_balance(address()?, TokenFilter::IncludeTokens)
            .await?
            .value()
            .confirmed
            .raw(),
        2
    );
    let requests = server.requests()?;
    assert_eq!(requests.len(), 14);
    assert_eq!(requests[8]["method"], requests[13]["method"]);
    assert_eq!(requests[8]["params"], requests[13]["params"]);
    Ok(())
}
#[tokio::test]
async fn network_verification_and_final_balance_share_one_operation_deadline() -> TestResult {
    let mut replies = verification()?;
    let mut verification = verification()?;
    verification[3] = verification[3].clone().delayed();
    replies.extend(verification);
    replies.push(Reply::result(json!({"confirmed":2,"unconfirmed":0}))?.delayed());
    let server = Server::start(replies).await?;
    let client = ElectrumClient::connect(server.config(2, 1024 * 1024, 0)?).await?;
    assert_eq!(
        client
            .get_address_balance(address()?, TokenFilter::IncludeTokens)
            .await,
        Err(Error::Timeout)
    );
    let requests = server.requests()?;
    assert_eq!(requests.len(), 9);
    assert_eq!(requests[8]["method"], "blockchain.scripthash.get_balance");
    Ok(())
}
#[tokio::test]
async fn line_body_caps_and_namespace_failures_are_local_and_safe() -> TestResult {
    let mut replies = verification()?;
    replies.extend(verification()?);
    replies.push(Reply {
        bytes: vec![b'x'; 2048],
        delay: Duration::ZERO,
        drop_connection: false,
    });
    let server = Server::start(replies).await?;
    let client = ElectrumClient::connect(server.config(10, 1024, 0)?).await?;
    let failure = client
        .get_address_balance(address()?, TokenFilter::IncludeTokens)
        .await
        .err()
        .ok_or(Error::UnavailableData)?;
    assert_eq!(failure, Error::Provider(ProviderError::ResponseTooLarge));
    assert!(!format!("{client:?}").contains("127.0.0.1"));
    let wrong = Address::from_hash(
        AddressNamespace::Testnet,
        AddressKind::PubkeyHash,
        false,
        &[1; 20],
    )?;
    assert_eq!(
        client
            .get_address_balance(wrong, TokenFilter::IncludeTokens)
            .await,
        Err(ValidationError::NetworkMismatch.into())
    );
    assert_eq!(server.requests()?.len(), 9);
    Ok(())
}

async fn transaction_from(source: Value, raw: Value) -> Result<Observation<Transaction>, Error> {
    let run = async {
        let mut replies = with_read(source)?;
        replies.push(Reply::result(raw)?);
        replies.push(Reply::result(json!(971_820))?);
        replies.push(Reply::result(fixture(include_str!(
            "fixtures/bitcoin_cash/inclusion.json"
        ))?)?);
        let server = Server::start(replies).await?;
        let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
        Ok::<_, Box<dyn std::error::Error>>(
            client
                .get_transaction(txid()?, CollectionLimit::new(10)?)
                .await,
        )
    }
    .await;
    run.map_err(|_| Error::Configuration)?
}
#[tokio::test]
async fn transaction_rejects_malformed_identity_width_coinbase_and_money_facts() -> TestResult {
    let base = fixture(include_str!("fixtures/bitcoin_cash/coinbase.json"))?;
    for case in 0..9 {
        let mut source = base.clone();
        match case {
            0 => source["txid"] = json!("11".repeat(32)),
            1 => source["hex"] = json!("00"),
            2 => source["size"] = json!(124),
            3 => source["version"] = json!(2_147_483_648_u64),
            4 => source["vin"][0]["txid"] = json!(TXID),
            5 => source["vin"][0]["coinbase"] = Value::Null,
            6 => source["vout"][0]["value"] = fixture("21000000.00000001")?,
            7 => source["vout"][0]["value"] = fixture("0.000000001")?,
            _ => {
                source
                    .as_object_mut()
                    .ok_or(Error::UnavailableData)?
                    .remove("locktime");
            }
        }
        assert_eq!(
            transaction_from(
                source,
                json!(include_str!("fixtures/bitcoin_cash/coinbase.hex").trim())
            )
            .await,
            Err(invalid())
        );
    }
    assert_eq!(transaction_from(base, json!("00")).await, Err(invalid()));
    Ok(())
}
#[tokio::test]
async fn source_output_address_absence_and_cash_token_fields_are_retained() -> TestResult {
    let mut source = fixture(include_str!("fixtures/bitcoin_cash/coinbase.json"))?;
    source["vout"][0]["scriptPubKey"]
        .as_object_mut()
        .ok_or(Error::UnavailableData)?
        .remove("addresses");
    source["vout"][0]["tokenData"] = json!({"category":"aa".repeat(32),"amount":"9223372036854775807","nft":{"capability":"mutable","commitment":""}});
    let value = transaction_from(
        source.clone(),
        json!(include_str!("fixtures/bitcoin_cash/coinbase.hex").trim()),
    )
    .await?;
    assert!(value.value().data().outputs[0].addresses.is_none());
    assert_eq!(
        value.value().data().outputs[0]
            .token_data
            .as_ref()
            .ok_or(Error::UnavailableData)?
            .amount(),
        i64::MAX as u64
    );
    source["vout"][0]["scriptPubKey"]["addresses"] = Value::Null;
    assert_eq!(
        transaction_from(
            source,
            json!(include_str!("fixtures/bitcoin_cash/coinbase.hex").trim())
        )
        .await,
        Err(invalid())
    );
    Ok(())
}
#[tokio::test]
async fn utxo_null_tokens_filter_mismatch_duplicate_and_quantity_overflow_reject() -> TestResult {
    let good = json!({"height":971_820,"tx_hash":TXID,"tx_pos":0,"value":1000});
    for case in 0..5 {
        let mut output = good.clone();
        let filter = if case == 3 {
            TokenFilter::TokensOnly
        } else {
            TokenFilter::IncludeTokens
        };
        match case {
            0 => output["token_data"] = Value::Null,
            1 => {
                output["token_data"] =
                    json!({"category":"ab".repeat(32),"amount":"9223372036854775808"});
            }
            2 => output["height"] = json!(-1),
            3 | 4 => {}
            _ => return Err("invalid test case".into()),
        }
        let values = if case == 4 {
            json!([output.clone(), output])
        } else {
            json!([output])
        };
        let server = Server::start(with_read(values)?).await?;
        let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
        assert_eq!(
            client
                .get_unspent_outputs(address()?, filter, CollectionLimit::new(10)?)
                .await,
            Err(invalid())
        );
    }
    Ok(())
}
#[tokio::test]
async fn history_required_heights_pending_fees_order_and_rpc_errors_are_not_invented() -> TestResult
{
    for source in [
        json!([{"tx_hash":TXID}]),
        json!([{"tx_hash":TXID,"height":null}]),
        json!([{"tx_hash":TXID,"height":-2}]),
        json!([{"tx_hash":TXID,"height":0}]),
        json!([{"tx_hash":TXID,"height":971_719}]),
        json!([{"tx_hash":TXID,"height":971_820},{"tx_hash":TXID,"height":971_820}]),
    ] {
        let server = Server::start(with_read(source)?).await?;
        let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
        assert_eq!(
            client.get_address_history(address()?, range()?).await,
            Err(invalid())
        );
    }
    let mut replies = verification()?;
    replies.extend(verification()?);
    replies.push(Reply{bytes:serde_json::to_vec(&json!({"jsonrpc":"2.0","id":0,"error":{"code":-32_000,"message":"private-source-diagnostic"}}))?,delay:Duration::ZERO,drop_connection:false});
    let server = Server::start(replies).await?;
    let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 3)?).await?;
    let failure = client
        .get_address_history(address()?, range()?)
        .await
        .err()
        .ok_or(Error::UnavailableData)?;
    assert_eq!(failure, Error::Provider(ProviderError::Rpc));
    assert_eq!(server.requests()?.len(), 9);
    assert!(!format!("{failure:?}").contains("private-source-diagnostic"));
    Ok(())
}

#[tokio::test]
async fn ordinary_source_inputs_and_output_capacities_are_complete_and_checked() -> TestResult {
    let source = fixture(include_str!("fixtures/bitcoin_cash/ordinary.json"))?;
    let id = Txid::parse(source["txid"].as_str().ok_or(Error::UnavailableData)?)?;
    for capacity in [10, 1] {
        let mut replies = with_read(source.clone())?;
        replies.push(Reply::result(json!(
            include_str!("fixtures/bitcoin_cash/ordinary.hex").trim()
        ))?);
        replies.push(Reply::result(json!(971_820))?);
        replies.push(Reply::result(fixture(include_str!(
            "fixtures/bitcoin_cash/inclusion.json"
        ))?)?);
        let server = Server::start(replies).await?;
        let client = ElectrumClient::connect(server.config(10, 1024 * 1024, 0)?).await?;
        let result = client
            .get_transaction(id, CollectionLimit::new(capacity)?)
            .await;
        if capacity == 1 {
            assert_eq!(result, Err(invalid()));
        } else {
            let value = result?;
            assert_eq!(value.value().data().raw.txid(), id);
            assert_eq!(value.value().data().inputs.len(), 2);
            assert_eq!(value.value().data().outputs.len(), 2);
            assert!(matches!(
                value.value().data().inputs[0],
                TransactionInput::Ordinary {
                    output_index: 1,
                    sequence: u32::MAX,
                    ..
                }
            ));
            assert_eq!(value.value().data().outputs[0].value.raw(), 2_783_975);
            assert_eq!(
                serde_json::from_value::<Observation<Transaction>>(serde_json::to_value(&value)?)?,
                value
            );
        }
    }
    Ok(())
}
