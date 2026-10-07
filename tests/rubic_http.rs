// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Real loopback API-v2 direct-route requests, exact wire facts and total bounds.
#![cfg(feature = "rubic-http")]
#[path = "support/rpc_server.rs"]
mod rpc_server;
use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        Amount,
        evm::TransactionId as EvmTransactionId,
        rubic::{
            Catalogue, Limits, Observation, PreparationRequest, PreparedSwap, ProviderStatus,
            Quote, QuoteRequest, Routes, StatusQuery, TransactionId,
        },
    },
    error::{Error, ProviderError},
    protocols::rubic::{RubicClient, RubicHttpConfig, RubicReader},
};
use rpc_server::{Fixture, Framing, Reply};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn rpc_envelope_is_not_a_rubic_quote_response() -> TestResult {
    let value: Value = serde_json::from_str(BEST)?;
    let fixture = Fixture::start(vec![Reply::result(&value)?]).await?;
    assert_eq!(
        client(&fixture)?.quote_best(request()?).await.err(),
        Some(Error::Provider(ProviderError::InvalidResponse))
    );
    assert_eq!(fixture.requests()?.len(), 1);
    Ok(())
}
const BEST: &str = include_str!("fixtures/rubic/quote_best_v2.json");
const ALL: &str = include_str!("fixtures/rubic/quote_all_v2.json");
const SWAP: &str = include_str!("fixtures/rubic/swap_v2.json");
const CHAINS: &str = include_str!("fixtures/rubic/chains_v2.json");
const STATUS: &str = include_str!("fixtures/rubic/status_v2.json");
fn quote() -> Result<Quote, serde_json::Error> {
    serde_json::from_str(include_str!("fixtures/rubic/quote_domain.json"))
}
fn request() -> Result<QuoteRequest, serde_json::Error> {
    Ok(quote()?.data().request.clone())
}
fn preparation() -> Result<PreparationRequest, serde_json::Error> {
    let p: PreparedSwap =
        serde_json::from_str(include_str!("fixtures/rubic/prepared_domain.json"))?;
    Ok(p.data().request.clone())
}
fn http(endpoint: &str, timeout: Duration, bytes: usize, retries: u8) -> Result<HttpConfig, Error> {
    HttpConfig::new(
        RpcEndpoint::new(endpoint)?
            .with_header("user-agent", "regit-web3/0.1.0")?
            .with_header("apikey", "SECRET")?,
        RpcLimits::new(Duration::from_secs(1).min(timeout), timeout, bytes, retries)?,
        "rubic-fixture",
    )
}
fn configured(
    endpoint: &str,
    timeout: Duration,
    bytes: usize,
    retries: u8,
) -> Result<RubicClient, Box<dyn std::error::Error>> {
    let q = request()?;
    let c = Catalogue::new(vec![
        q.data().source.chain().clone(),
        q.data().destination.chain().clone(),
    ])?;
    Ok(RubicClient::new(RubicHttpConfig::new(
        http(endpoint, timeout, bytes, retries)?,
        c,
        q.data().limits,
    )?)?)
}
fn client(f: &Fixture) -> Result<RubicClient, Box<dyn std::error::Error>> {
    configured(&f.endpoint, Duration::from_secs(10), 2 * 1024 * 1024, 2)
}
fn query() -> Result<StatusQuery, Box<dyn std::error::Error>> {
    let q = request()?;
    Ok(StatusQuery::new(
        regit_web3::domain::rubic::Identifier::new("00000000-0000-0000-0000-000000000000")?,
        q.data().source.chain().clone(),
        TransactionId::Evm(EvmTransactionId::parse(
            "0x6a851726e31e207dcea611424c194fb3ce132404598646b6903ee43fe8930e95",
        )?),
        q.data().destination.chain().clone(),
    )?)
}
fn roundtrip<T: Serialize + DeserializeOwned + Eq + std::fmt::Debug>(v: &T) -> TestResult {
    assert_eq!(&serde_json::from_str::<T>(&serde_json::to_string(v)?)?, v);
    Ok(())
}
fn assert_send<T: Send>(_: T) {}
async fn best_error(value: Value) -> TestResult {
    let f = Fixture::start(vec![Reply::json(&value)?]).await?;
    let error = client(&f)?
        .quote_best(request()?)
        .await
        .err()
        .ok_or("missing failure")?;
    assert_eq!(error, Error::Provider(ProviderError::InvalidResponse));
    assert_eq!(f.requests()?.len(), 1);
    assert!(!format!("{error:?} {error}").contains("SECRET"));
    Ok(())
}
async fn swap_error(value: Value) -> TestResult {
    let f = Fixture::start(vec![Reply::json(&value)?]).await?;
    assert_eq!(
        client(&f)?.prepare(preparation()?).await.err(),
        Some(Error::Provider(ProviderError::InvalidResponse))
    );
    assert_eq!(f.requests()?.len(), 1);
    Ok(())
}
#[tokio::test]
async fn all_five_public_methods_decode_actual_source_shapes_and_keep_context() -> TestResult {
    let f = Fixture::start(vec![
        Reply::raw(200, CHAINS.as_bytes().to_vec()),
        Reply::raw(200, BEST.as_bytes().to_vec()),
        Reply::raw(200, ALL.as_bytes().to_vec()),
        Reply::raw(200, SWAP.as_bytes().to_vec()),
        Reply::raw(200, STATUS.as_bytes().to_vec()),
    ])
    .await?;
    let c = client(&f)?;
    let before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let chains = RubicReader::chains(&c, false).await?;
    assert_eq!(chains.value().entries().len(), 101);
    for chain in c.config().catalogue().chains() {
        chains
            .value()
            .entries()
            .iter()
            .find(|v| v.alias == *chain.alias())
            .ok_or("missing chain")?
            .matches(chain)?;
    }
    let q = RubicReader::quote_best(&c, request()?).await?;
    assert_eq!(q.value(), &quote()?);
    let routes = RubicReader::quote_all(&c, request()?).await?;
    assert_eq!(routes.value().routes().len(), 6);
    let p = RubicReader::prepare(&c, preparation()?).await?;
    assert_ne!(
        p.value().data().fresh_quote.data().estimate,
        p.value().data().request.data().quote.data().estimate
    );
    let status = RubicReader::status(&c, query()?).await?;
    assert_eq!(status.value().status(), ProviderStatus::NotFound);
    assert!(status.value().destination().is_none());
    for (method, time) in [
        (chains.source().method(), chains.retrieved_at()),
        (q.source().method(), q.retrieved_at()),
        (routes.source().method(), routes.retrieved_at()),
        (p.source().method(), p.retrieved_at()),
        (status.source().method(), status.retrieved_at()),
    ] {
        assert!(method.starts_with("api_v2_"));
        assert!(time.unix_seconds() >= before);
    }
    assert_eq!(q.source().integration_version(), env!("CARGO_PKG_VERSION"));
    roundtrip(&chains)?;
    roundtrip(&q)?;
    roundtrip(&routes)?;
    roundtrip(&p)?;
    roundtrip(&status)?;
    let requests = f.requests()?;
    assert_eq!(requests.len(), 5);
    assert_eq!(
        requests[1].body["providerTags"],
        json!(["notDeposits", "notPrivate"])
    );
    assert_eq!(requests[1].body["srcTokenAmount"], "0.010000000000000000");
    assert_eq!(requests[1].body["slippage"], json!(0.01));
    assert_eq!(requests[1].body["enableChecks"], false);
    assert_eq!(requests[3].body["id"], quote()?.data().id.as_str());
    assert!(requests[4].target.contains("rubicId="));
    assert!(requests[4].target.contains("srcTxHash="));
    assert!(!requests[4].target.contains("/status?"));
    assert_send(RubicReader::quote_best(&c, request()?));
    Ok(())
}
#[tokio::test]
async fn direct_read_retries_freeze_complete_body_filters_and_credentials() -> TestResult {
    let f = Fixture::start(vec![
        Reply::raw(503, b"SECRET".to_vec()),
        Reply::raw(200, BEST.as_bytes().to_vec()),
    ])
    .await?;
    client(&f)?.quote_best(request()?).await?;
    let req = f.requests()?;
    assert_eq!(req.len(), 2);
    assert_eq!(req[0].body, req[1].body);
    assert_eq!(req[0].headers, req[1].headers);
    assert_eq!(req[0].target, req[1].target);
    assert!(!format!("{:?}", client(&f)?).contains("SECRET"));
    Ok(())
}
#[tokio::test]
async fn wrong_asset_chain_precision_and_raw_human_amounts_are_rejected() -> TestResult {
    for (field, value) in [
        ("blockchainId", json!(8453)),
        ("decimals", json!(6)),
        (
            "address",
            json!("0x1111111111111111111111111111111111111111"),
        ),
    ] {
        let mut body: Value = serde_json::from_str(BEST)?;
        body["tokens"]["to"][field] = value;
        best_error(body).await?;
    }
    for value in ["251507041920000000001", "-1", "01", "1e20"] {
        let mut body: Value = serde_json::from_str(BEST)?;
        body["estimate"]["destinationWeiAmount"] = json!(value);
        best_error(body).await?;
    }
    Ok(())
}
#[tokio::test]
async fn direct_profile_rejects_deposit_private_or_unknown_transaction_forms() -> TestResult {
    for field in [
        "depositAddress",
        "exchangeId",
        "signature",
        "psbt",
        "unknownForm",
    ] {
        let mut body: Value = serde_json::from_str(BEST)?;
        body["transaction"][field] = json!("SECRET");
        best_error(body).await?;
    }
    let mut body: Value = serde_json::from_str(BEST)?;
    body["private"] = json!(true);
    best_error(body).await?;
    let mut body: Value = serde_json::from_str(SWAP)?;
    body["transaction"]["depositAddress"] = json!("SECRET");
    swap_error(body).await?;
    Ok(())
}
#[tokio::test]
async fn duplicate_known_and_unknown_fields_are_rejected_before_mapping() -> TestResult {
    for body in [
        BEST.replacen("\"id\":", "\"id\":\"SECRET\",\"id\":", 1),
        BEST.replacen("\"tokens\":", "\"unknown\":1,\"unknown\":2,\"tokens\":", 1),
    ] {
        let f = Fixture::start(vec![Reply::raw(200, body.into_bytes())]).await?;
        assert_eq!(
            client(&f)?.quote_best(request()?).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}
#[tokio::test]
async fn explicit_collection_capacities_fail_instead_of_truncating() -> TestResult {
    let f = Fixture::start(vec![Reply::raw(200, ALL.as_bytes().to_vec())]).await?;
    let mut data = request()?.data().clone();
    data.limits = Limits::new(512, 1, 128, 128, 128)?;
    assert_eq!(
        client(&f)?.quote_all(QuoteRequest::new(data)?).await.err(),
        Some(Error::Provider(ProviderError::InvalidResponse))
    );
    let mut body: Value = serde_json::from_str(ALL)?;
    let first = body["routes"][0].clone();
    body["routes"] = json!([first, first]);
    let f = Fixture::start(vec![Reply::json(&body)?]).await?;
    assert_eq!(
        client(&f)?.quote_all(request()?).await.err(),
        Some(Error::Provider(ProviderError::InvalidResponse))
    );
    Ok(())
}
#[tokio::test]
async fn empty_routes_remain_empty_without_a_fabricated_best_selection() -> TestResult {
    let mut body: Value = serde_json::from_str(ALL)?;
    body["routes"] = json!([]);
    let f = Fixture::start(vec![Reply::json(&body)?]).await?;
    let routes: Observation<Routes> = client(&f)?.quote_all(request()?).await?;
    assert!(routes.value().routes().is_empty());
    Ok(())
}
#[tokio::test]
async fn preparation_rejects_changed_identity_sender_filters_and_payload_limits() -> TestResult {
    for path in ["id", "fromAddress", "receiver", "referrer"] {
        let mut body: Value = serde_json::from_str(SWAP)?;
        body["quote"][path] = json!(if matches!(path, "fromAddress" | "receiver") {
            "0x1111111111111111111111111111111111111111"
        } else {
            "SECRET"
        });
        swap_error(body).await?;
    }
    let mut body: Value = serde_json::from_str(SWAP)?;
    body["quote"]["providerTags"] = json!(["onlyDeposits"]);
    swap_error(body).await?;
    let mut body: Value = serde_json::from_str(SWAP)?;
    body["transaction"]["value"] = json!("20000000000000001");
    swap_error(body).await?;
    let mut body: Value = serde_json::from_str(SWAP)?;
    body["transaction"]["to"] = json!("0x1111111111111111111111111111111111111111");
    swap_error(body).await?;
    Ok(())
}
#[tokio::test]
async fn fresh_estimate_below_explicit_floor_is_rejected_without_signing() -> TestResult {
    let f = Fixture::start(vec![Reply::raw(200, SWAP.as_bytes().to_vec())]).await?;
    let mut r = preparation()?.data().clone();
    r.minimum_output = Amount::from_decimal("251503987850000000001", Some(18))?;
    assert_eq!(
        client(&f)?.prepare(PreparationRequest::new(r)?).await.err(),
        Some(Error::Provider(ProviderError::InvalidResponse))
    );
    assert_eq!(f.requests()?.len(), 1);
    Ok(())
}
#[tokio::test]
async fn bounded_provider_ids_and_arbitrary_diagnostics_stay_source_facts() -> TestResult {
    let mut body: Value = serde_json::from_str(SWAP)?;
    body["uniqueInfo"] = json!({"relayId":"SOURCE-SECRET"});
    body["warnings"] = json!([{"code":9001,"reason":"REMOTE-SECRET","message":"REMOTE-SECRET"}]);
    let f = Fixture::start(vec![Reply::json(&body)?]).await?;
    let result = client(&f)?.prepare(preparation()?).await?;
    assert_eq!(result.value().data().source_provider_ids.len(), 1);
    assert_eq!(
        result.value().data().fresh_quote.data().warnings[0].code,
        Some(9001)
    );
    assert!(!format!("{result:?}").contains("SECRET"));
    assert!(!serde_json::to_string(&result)?.contains("REMOTE-SECRET"));
    assert!(serde_json::to_string(&result)?.contains("SOURCE-SECRET"));
    Ok(())
}
#[tokio::test]
async fn status_preserves_literal_progress_and_requires_matching_echoes() -> TestResult {
    let mut body: Value = serde_json::from_str(STATUS)?;
    body["status"] = json!("SUCCESS");
    body["destinationTxHash"] =
        json!("0x5e8c67f32c11d5f5a44dd6e9a9f92bb5762ea0e41bda01c6a77228365b614e02");
    body["toAmount"] = json!("1.25");
    body["toAmountWei"] = json!("1250000");
    let f = Fixture::start(vec![Reply::json(&body)?]).await?;
    let s = client(&f)?.status(query()?).await?;
    assert_eq!(s.value().status(), ProviderStatus::Success);
    assert_eq!(
        s.value()
            .final_human_amount()
            .ok_or("missing amount")?
            .value()
            .canonical(),
        "1.25"
    );
    assert_eq!(
        s.value()
            .final_raw_amount()
            .ok_or("missing raw")?
            .decimals(),
        None
    );
    assert!(s.value().destination().is_some());
    roundtrip(&s)?;
    for (field, value) in [
        ("rubicId", json!("SECRET")),
        ("rubicId", json!("")),
        (
            "sourceTxHash",
            json!("0x1111111111111111111111111111111111111111111111111111111111111111"),
        ),
        ("sourceTxHash", json!("")),
        ("destinationNetworkChainId", json!(8453)),
        ("status", json!("UNRECOGNIZED")),
    ] {
        let mut b: Value = serde_json::from_str(STATUS)?;
        b[field] = value;
        let f = Fixture::start(vec![Reply::json(&b)?]).await?;
        assert_eq!(
            client(&f)?.status(query()?).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}
#[tokio::test]
async fn documented_success_without_echoes_retains_query_and_validates_each_available_echo()
-> TestResult {
    let captured: Value = serde_json::from_str(STATUS)?;
    let expected = query()?;
    for available in ["neither", "id", "hash", "null"] {
        // This is a contract vector, not a modified purported live capture. The
        // primary response schema does not require the two identity echoes.
        let mut body = json!({
            "status":"SUCCESS",
            "destinationTxHash":"0x5e8c67f32c11d5f5a44dd6e9a9f92bb5762ea0e41bda01c6a77228365b614e02",
            "destinationNetworkTitle":null,
            "destinationNetworkChainId":137
        });
        match available {
            "id" => body["rubicId"] = captured["rubicId"].clone(),
            "hash" => body["sourceTxHash"] = captured["sourceTxHash"].clone(),
            "null" => {
                body["rubicId"] = Value::Null;
                body["sourceTxHash"] = Value::Null;
            }
            _ => {}
        }
        let fixture = Fixture::start(vec![Reply::json(&body)?]).await?;
        let observation = client(&fixture)?.status(expected.clone()).await?;
        assert_eq!(observation.value().status(), ProviderStatus::Success);
        assert_eq!(observation.value().query(), &expected);
        assert!(observation.value().destination().is_some());
        assert!(observation.value().final_human_amount().is_none());
        assert!(observation.value().final_raw_amount().is_none());
        roundtrip(&observation)?;
        let requests = fixture.requests()?;
        assert_eq!(requests.len(), 1);
        assert!(requests[0].target.contains(expected.id().as_str()));
        assert!(
            requests[0]
                .target
                .contains(&expected.source_transaction().source_id())
        );
    }
    for (field, value) in [
        ("rubicId", json!("wrong-route")),
        ("rubicId", json!("")),
        (
            "sourceTxHash",
            json!("0x1111111111111111111111111111111111111111111111111111111111111111"),
        ),
        ("sourceTxHash", json!("")),
    ] {
        let mut body = json!({"status":"SUCCESS","destinationTxHash":null});
        body[field] = value;
        let fixture = Fixture::start(vec![Reply::json(&body)?]).await?;
        assert_eq!(
            client(&fixture)?.status(expected.clone()).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
        assert_eq!(fixture.requests()?.len(), 1);
    }
    Ok(())
}
#[tokio::test]
async fn malformed_duplicate_and_non_integer_fee_fields_fail_exactly() -> TestResult {
    let mut body: Value = serde_json::from_str(BEST)?;
    body["fees"]["gasTokenFees"]["gas"]["gasLimit"] = json!("1.5");
    best_error(body).await?;
    let body = BEST.replace("\"price\":2608.94", "\"price\":9007199254740993.125");
    let f = Fixture::start(vec![Reply::raw(200, body.into_bytes())]).await?;
    let q = client(&f)?.quote_best(request()?).await?;
    assert_eq!(
        q.value()
            .data()
            .input
            .token()
            .price_usd
            .as_ref()
            .ok_or("missing price")?
            .value()
            .canonical(),
        "9007199254740993.125"
    );
    Ok(())
}
#[tokio::test]
async fn complete_operation_deadline_covers_delayed_body_and_preparation_decode() -> TestResult {
    let mut reply = Reply::raw(200, SWAP.as_bytes().to_vec());
    reply.delay = Duration::from_millis(1200);
    reply.body_delay = Duration::from_millis(1200);
    let f = Fixture::start(vec![reply]).await?;
    assert_eq!(
        configured(&f.endpoint, Duration::from_secs(2), 2 * 1024 * 1024, 2)?
            .prepare(preparation()?)
            .await
            .err(),
        Some(Error::Timeout)
    );
    let r = f.requests()?;
    assert_eq!(r.len(), 1);
    assert!(r[0].target.ends_with("/routes/swap"));
    assert_eq!(r[0].body["id"], quote()?.data().id.as_str());
    Ok(())
}
#[tokio::test]
async fn body_limit_bounds_chunked_and_declared_documents() -> TestResult {
    for framing in [
        Framing::ContentLength,
        Framing::Chunked,
        Framing::CloseDelimited,
    ] {
        let mut r = Reply::raw(200, vec![b' '; 1025]);
        r.framing = framing;
        let f = Fixture::start(vec![r]).await?;
        assert_eq!(
            configured(&f.endpoint, Duration::from_secs(5), 1024, 2)?
                .quote_best(request()?)
                .await
                .err(),
            Some(Error::Provider(ProviderError::ResponseTooLarge))
        );
        assert_eq!(f.requests()?.len(), 1);
    }
    Ok(())
}
#[tokio::test]
async fn preflight_wrong_catalogue_sends_zero_requests() -> TestResult {
    let f = Fixture::start(vec![]).await?;
    let q = request()?;
    let catalogue = Catalogue::new(vec![q.data().source.chain().clone()])?;
    let c = RubicClient::new(RubicHttpConfig::new(
        http(&f.endpoint, Duration::from_secs(5), 2 * 1024 * 1024, 2)?,
        catalogue,
        q.data().limits,
    )?)?;
    assert_eq!(
        c.quote_best(q).await.err(),
        Some(Error::UnsupportedCapability)
    );
    assert!(f.requests()?.is_empty());
    Ok(())
}

#[tokio::test]
async fn captured_squid_additional_object_and_contract_optional_tokens_are_retained() -> TestResult
{
    const SQUID: &str = include_str!("fixtures/rubic/quote_best_squid_v2.json");
    for swap in [
        include_str!("fixtures/rubic/swap_squid_v2.json"),
        include_str!("fixtures/rubic/swap_without_tokens_v2.json"),
    ] {
        let fixture = Fixture::start(vec![
            Reply::raw(200, SQUID.as_bytes().to_vec()),
            Reply::raw(200, swap.as_bytes().to_vec()),
        ])
        .await?;
        let backend = client(&fixture)?;
        let mut choices = request()?.data().clone();
        choices.preferred_provider =
            Some(regit_web3::domain::rubic::Identifier::new("squidrouter")?);
        let quote = backend.quote_best(QuoteRequest::new(choices)?).await?;
        let mut choices = preparation()?.data().clone();
        choices.quote = quote.value().clone();
        choices.expected_approval = quote.value().data().approval_address;
        choices.expected_permit2 = quote.value().data().permit2_address;
        let prepared = backend.prepare(PreparationRequest::new(choices)?).await?;
        let data = prepared
            .value()
            .data()
            .source_additional_data
            .as_ref()
            .ok_or("missing source data")?;
        let object: Value = serde_json::from_str(data.as_json())?;
        assert!(object["squidrouterQuoteId"].is_string());
        assert_eq!(prepared.value().data().source_provider_ids.len(), 1);
        assert_eq!(
            prepared.value().data().fresh_quote.data().input.is_some(),
            swap.contains("\"tokens\"")
        );
        roundtrip(&prepared)?;
        assert_eq!(fixture.requests()?.len(), 2);
    }
    let mut body: Value = serde_json::from_str(SWAP)?;
    body["tokens"]["to"]["address"] = json!("0x1111111111111111111111111111111111111111");
    swap_error(body).await?;
    Ok(())
}

fn solana_request() -> Result<QuoteRequest, Box<dyn std::error::Error>> {
    use regit_web3::domain::{
        rubic::{Asset, AssetIdentifier, Chain, Family, Identifier},
        solana::{Hash, Network, Pubkey},
    };
    let mut r = request()?.data().clone();
    let chain = Chain::new(
        Identifier::new("SOLANA")?,
        Some(7_565_164),
        Family::Solana {
            network: Network::new(
                Hash::parse("5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d")?,
                "mainnet",
            )?,
        },
        false,
    )?;
    r.source = Asset::new(
        chain,
        AssetIdentifier::SolanaMint(Pubkey::parse(
            "So11111111111111111111111111111111111111112",
        )?),
        9,
    )?;
    r.amount = Amount::from_decimal("100000000", Some(9))?;
    Ok(QuoteRequest::new(r)?)
}
fn solana_client(f: &Fixture, r: &QuoteRequest) -> Result<RubicClient, Box<dyn std::error::Error>> {
    let catalogue = Catalogue::new(vec![
        r.data().source.chain().clone(),
        r.data().destination.chain().clone(),
    ])?;
    Ok(RubicClient::new(RubicHttpConfig::new(
        http(&f.endpoint, Duration::from_secs(10), 2 * 1024 * 1024, 2)?,
        catalogue,
        r.data().limits,
    )?)?)
}
fn unsigned_solana(
    payer: regit_web3::domain::solana::Pubkey,
) -> Result<regit_web3::domain::solana::UnsignedTransaction, Error> {
    use regit_web3::domain::solana::{
        BlockhashLifetime, Hash, Network, TransferIntent, TransferPreparation,
    };
    // Harmless fixture codec input only: these transfer instructions do not prove
    // the provider swap's semantics or intent.
    Ok(TransferPreparation::new(
        Network::new(Hash::from_bytes([1; 32]), "fixture")?,
        TransferIntent::Native {
            fee_payer: payer,
            sender: payer,
            recipient: payer,
            lamports: 1,
        },
        BlockhashLifetime {
            blockhash: Hash::from_bytes([2; 32]),
            last_valid_block_height: 100,
        },
    )?
    .unsigned_transaction()
    .clone())
}
fn solana_preparation(q: Quote) -> Result<PreparationRequest, Error> {
    use regit_web3::domain::{
        rubic::{Account, PreparationRequestData},
        solana::Pubkey,
    };
    PreparationRequest::new(PreparationRequestData {
        quote: q,
        sender: Account::Solana(Pubkey::from_bytes([3; 32])),
        receiver: Account::Evm(regit_web3::domain::Address::from_bytes([4; 20])),
        minimum_output: Amount::from_decimal("1", Some(18))?,
        maximum_native_value: Amount::from_decimal("100000000", Some(9))?,
        expected_evm_router: None,
        expected_approval: None,
        expected_permit2: None,
        expected_solana_signers: vec![Pubkey::from_bytes([3; 32])],
    })
}
fn solana_swap(
    q: &Quote,
    transaction: &regit_web3::domain::solana::UnsignedTransaction,
) -> Result<Value, Box<dyn std::error::Error>> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut body: Value =
        serde_json::from_str(include_str!("fixtures/rubic/quote_best_solana_v2.json"))?;
    let mut echo: Value = serde_json::from_str(SWAP)?;
    let echo = &mut echo["quote"];
    echo["srcTokenAddress"] = json!(q.data().request.data().source.source_address());
    echo["srcTokenBlockchain"] = json!("SOLANA");
    echo["srcTokenAmount"] = json!("0.1");
    echo["id"] = json!(q.data().id.as_str());
    echo["fromAddress"] =
        json!(regit_web3::domain::solana::Pubkey::from_bytes([3; 32]).to_string());
    echo["receiver"] = json!(regit_web3::domain::Address::from_bytes([4; 20]).to_string());
    body["quote"] = echo.clone();
    body.as_object_mut().ok_or("missing object")?.remove("id");
    body["transaction"] = json!({"data":STANDARD.encode(transaction.bytes())});
    Ok(body)
}

#[tokio::test]
async fn actual_solana_quote_keeps_native_fee_sentinel_and_wrapped_input_distinct() -> TestResult {
    use regit_web3::domain::rubic::{Asset, AssetIdentifier, SOLANA_NATIVE_ASSET_ADDRESS};
    const SOL: &str = include_str!("fixtures/rubic/quote_best_solana_v2.json");
    let r = solana_request()?;
    let f = Fixture::start(vec![Reply::raw(200, SOL.as_bytes().to_vec())]).await?;
    let q = solana_client(&f, &r)?.quote_best(r.clone()).await?;
    assert!(matches!(
        q.value().data().input.token().asset.identifier(),
        AssetIdentifier::SolanaMint(_)
    ));
    assert_eq!(
        q.value().data().fees.native_token.asset.source_address(),
        SOLANA_NATIVE_ASSET_ADDRESS
    );
    assert_eq!(
        f.requests()?[0].body["srcTokenAddress"],
        "So11111111111111111111111111111111111111112"
    );
    roundtrip(&q)?;
    let mut native = r.data().clone();
    native.source = Asset::new(native.source.chain().clone(), AssetIdentifier::Native, 9)?;
    let native = QuoteRequest::new(native)?;
    let f = Fixture::start(vec![Reply::raw(200, SOL.as_bytes().to_vec())]).await?;
    assert_eq!(
        solana_client(&f, &native)?.quote_best(native).await.err(),
        Some(Error::Provider(ProviderError::InvalidResponse))
    );
    assert_eq!(
        f.requests()?[0].body["srcTokenAddress"],
        SOLANA_NATIVE_ASSET_ADDRESS
    );
    for bad in [
        "0x0000000000000000000000000000000000000000",
        "So11111111111111111111111111111111111111112",
    ] {
        let mut body: Value = serde_json::from_str(SOL)?;
        body["fees"]["gasTokenFees"]["nativeToken"]["address"] = json!(bad);
        let f = Fixture::start(vec![Reply::json(&body)?]).await?;
        assert_eq!(
            solana_client(&f, &r)?.quote_best(r.clone()).await.err(),
            Some(Error::Provider(ProviderError::InvalidResponse))
        );
    }
    Ok(())
}

#[tokio::test]
async fn canonical_unsigned_solana_payload_is_reviewed_without_signature_or_expiry_claim()
-> TestResult {
    use regit_web3::domain::{rubic::Payload, solana::Pubkey};
    const SOL: &str = include_str!("fixtures/rubic/quote_best_solana_v2.json");
    let r = solana_request()?;
    let unsigned = unsigned_solana(Pubkey::from_bytes([3; 32]))?;
    let initial = Fixture::start(vec![Reply::raw(200, SOL.as_bytes().to_vec())]).await?;
    let q = solana_client(&initial, &r)?.quote_best(r.clone()).await?;
    let body = solana_swap(q.value(), &unsigned)?;
    let f = Fixture::start(vec![Reply::json(&body)?]).await?;
    let p = solana_client(&f, &r)?
        .prepare(solana_preparation(q.value().clone())?)
        .await?;
    assert!(matches!(&p.value().data().payload,Payload::Solana(tx) if tx==&unsigned));
    let handoff = regit_web3::wallets::HandoffRequest::new(
        regit_web3::wallets::HandoffId::new("solana-direct-fixture")?,
        regit_web3::wallets::PreparedRequest::new(p.value().clone())?,
    );
    roundtrip(&handoff)?;
    assert_eq!(f.requests()?.len(), 1);
    assert!(f.requests()?[0].target.ends_with("/routes/swap"));
    Ok(())
}

#[tokio::test]
async fn signed_bad_base64_wrong_family_or_changed_required_signers_fail_closed() -> TestResult {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use regit_web3::domain::solana::Pubkey;
    const SOL: &str = include_str!("fixtures/rubic/quote_best_solana_v2.json");
    let r = solana_request()?;
    let initial = Fixture::start(vec![Reply::raw(200, SOL.as_bytes().to_vec())]).await?;
    let q = solana_client(&initial, &r)?.quote_best(r.clone()).await?;
    let unsigned = unsigned_solana(Pubkey::from_bytes([3; 32]))?;
    let good = solana_swap(q.value(), &unsigned)?;
    let mut signed = unsigned.bytes().to_vec();
    signed[1] = 1;
    for change in ["signed", "base64", "signers", "family", "echo", "sentinel"] {
        let mut body = good.clone();
        match change {
            "signed" => body["transaction"]["data"] = json!(STANDARD.encode(&signed)),
            "base64" => body["transaction"]["data"] = json!("not-canonical-base64!"),
            "signers" => {
                body["transaction"]["data"] =
                    json!(STANDARD.encode(unsigned_solana(Pubkey::from_bytes([9; 32]))?.bytes()));
            }
            "family" => {
                body["transaction"]["to"] = json!("0x1111111111111111111111111111111111111111");
            }
            "echo" => body["quote"]["srcTokenBlockchain"] = json!("ETH"),
            _ => {
                body["quote"]["srcTokenAddress"] =
                    json!(regit_web3::domain::rubic::SOLANA_NATIVE_ASSET_ADDRESS);
            }
        }
        let f = Fixture::start(vec![Reply::json(&body)?]).await?;
        assert_eq!(
            solana_client(&f, &r)?
                .prepare(solana_preparation(q.value().clone())?)
                .await
                .err(),
            Some(Error::Provider(ProviderError::InvalidResponse)),
            "{change}"
        );
        assert_eq!(f.requests()?.len(), 1);
    }
    Ok(())
}
