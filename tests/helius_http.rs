// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Deterministic Helius DAS and current Parsed Events HTTP contracts.
#![cfg(feature = "helius-http")]
use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        helius::*,
        solana::{Commitment, Hash, Network, Pubkey, Signature},
    },
    error::{Error, ProviderError},
    providers::helius::{HeliusClient, HeliusHttpConfig},
};
use serde_json::{Value, json};
use std::time::Duration;
#[path = "support/rpc_server.rs"]
mod server;
use server::{Fixture, Framing, Reply};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
const OWNER: &str = "86xCnPeV69n6t3DnyGvkKobf9FdN2H9oiVDdaMpo2MMY";
fn rest(value: &Value) -> Result<Reply, serde_json::Error> {
    Ok(Reply::raw(200, serde_json::to_vec(value)?))
}
fn genesis() -> Result<Reply, serde_json::Error> {
    Reply::result(&json!(GENESIS))
}
fn config(
    url: &str,
    timeout: Duration,
    cap: usize,
    retries: u8,
) -> Result<HeliusHttpConfig, Error> {
    Ok(HeliusHttpConfig::new(
        Network::new(Hash::parse(GENESIS)?, "fixture")?,
        HttpConfig::new(
            RpcEndpoint::new(url)?,
            RpcLimits::new(timeout, timeout, cap, retries)?,
            "fixture",
        )?,
    ))
}
fn standard(url: &str) -> Result<HeliusHttpConfig, Error> {
    config(url, Duration::from_secs(5), 1_048_576, 0)
}
fn options() -> AssetOptions {
    AssetOptions {
        show_unverified_collections: false,
        show_collection_metadata: false,
        show_fungible: true,
    }
}
fn asset() -> Result<Value, serde_json::Error> {
    serde_json::from_str(include_str!("fixtures/helius/asset.json"))
}
fn parsed() -> Result<Value, serde_json::Error> {
    serde_json::from_str(include_str!("fixtures/helius/parsed_events.json"))
}
fn asset_request() -> Result<AssetRequest, Box<dyn std::error::Error>> {
    Ok(AssetRequest {
        id: Pubkey::parse(asset()?["id"].as_str().ok_or("fixture id")?)?,
        options: options(),
    })
}
fn owner(cursor: Option<Cursor>) -> Result<OwnerRequest, Error> {
    OwnerRequest::new(
        Pubkey::parse(OWNER)?,
        2,
        AssetPosition::Cursor { cursor },
        AssetSort::Id,
        SortDirection::Asc,
        options(),
        true,
        true,
        false,
    )
}
fn signature() -> Result<Signature, Box<dyn std::error::Error>> {
    Ok(Signature::parse(
        parsed()?[0]["signature"]
            .as_str()
            .ok_or("fixture signature")?,
    )?)
}
fn history() -> Result<HistoryRequest, Error> {
    HistoryRequest::new(
        Pubkey::parse(OWNER)?,
        2,
        Commitment::Finalized,
        SortDirection::Desc,
        None,
        None,
        Some(Bounds::new(
            None,
            Some(433_950_000),
            None,
            Some(433_951_000),
        )?),
        Some(Bounds::new(None, Some(1_700_000_000), None, None)?),
        None,
    )
}
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn owner_response(items: &Value, cursor: Option<&str>) -> Value {
    json!({"last_indexed_slot":365_750_752_u64,"total":items.as_array().map_or(0,Vec::len),"grand_total":10,"limit":2,"page":null,"cursor":cursor,"items":items,"nativeBalance":{"lamports":9_007_199_254_740_993_u64,"price_per_sol":0,"total_price":0}})
}
#[tokio::test]
async fn asset_preserves_metadata_royalty_empty_hash_sentinels_and_query_identity() -> TestResult {
    let fixture = Fixture::start(vec![genesis()?, genesis()?, Reply::result(&asset()?)?]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let q = asset_request()?;
    let obs = client.get_asset(q.clone()).await?;
    assert_eq!(obs.value().id(), q.id);
    assert_eq!(obs.context().last_indexed_slot(), Some(365_750_752));
    assert_eq!(obs.value().data().compression.as_ref().unwrap().tree, None);
    assert_eq!(
        obs.value().data().compression.as_ref().unwrap().sequence,
        Some(0)
    );
    assert_eq!(
        obs.value()
            .data()
            .royalty
            .as_ref()
            .unwrap()
            .percent
            .canonical(),
        "0.042"
    );
    assert_eq!(
        serde_json::from_str::<Observation<Asset>>(&serde_json::to_string(&obs)?)?,
        obs
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].body["method"], "getAsset");
    assert!(requests[2].body["params"].is_object());
    assert_eq!(requests[2].body["params"]["id"], q.id.to_string());
    assert_eq!(requests[2].body["params"]["options"]["showFungible"], true);
    Ok(())
}
#[tokio::test]
async fn fungible_supply_balance_prices_and_metadata_numbers_are_lexically_exact() -> TestResult {
    let mut a = asset()?;
    a["interface"] = json!("FungibleToken");
    a["token_info"] = json!({"supply":u64::MAX,"balance":9_007_199_254_740_993_u64,"decimals":9,"token_program":"TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb","price_info":{"currency":"USD","total_price":null}});
    a["token_info"]["price_info"]["price_per_token"] =
        Value::Number("0.0000000000000000000000001".parse()?);
    a["content"]["metadata"]["large"] = Value::Number("9007199254740993.125".parse()?);
    a["mint_extensions"] = json!({"unmodeled":true});
    let fixture = Fixture::start(vec![genesis()?, genesis()?, Reply::result(&a)?]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let obs = client.get_asset(asset_request()?).await?;
    let token = obs.value().data().token_info.as_ref().unwrap();
    assert_eq!(token.supply, Some(u64::MAX));
    assert_eq!(token.balance, Some(9_007_199_254_740_993));
    assert_eq!(
        token
            .price
            .as_ref()
            .unwrap()
            .price_per_token
            .as_ref()
            .unwrap()
            .canonical(),
        "0.0000000000000000000000001"
    );
    assert_eq!(
        obs.value().data().unsupported_extensions[0].as_str(),
        "mint_extensions"
    );
    let MetadataNode::Object(fields) = obs
        .value()
        .data()
        .content
        .as_ref()
        .unwrap()
        .metadata
        .as_ref()
        .unwrap()
        .node()
    else {
        return Err("metadata object".into());
    };
    let MetadataNode::Number(n) = fields["large"].node() else {
        return Err("exact number".into());
    };
    assert_eq!(n.canonical(), "9007199254740993.125");
    Ok(())
}
#[tokio::test]
async fn owner_cursor_keeps_original_controls_source_totals_native_values_and_client_binding()
-> TestResult {
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        Reply::result(&owner_response(&json!([asset()?]), Some("opaque:cursor-1")))?,
        genesis()?,
        Reply::result(&owner_response(&json!([]), None))?,
        genesis()?,
    ])
    .await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let q = owner(None)?;
    let page = client.get_assets_by_owner(q.clone()).await?;
    assert_eq!(page.value().reported_total(), 1);
    assert_eq!(page.value().reported_grand_total(), Some(10));
    assert_eq!(
        page.value().native_balance().unwrap().lamports,
        9_007_199_254_740_993
    );
    let handle = client.asset_continuation(&page)?.ok_or("cursor")?;
    assert!(!format!("{handle:?}").contains("opaque"));
    let final_page = client.continue_assets(handle.clone()).await?;
    assert!(final_page.value().items().is_empty());
    assert!(client.asset_continuation(&final_page)?.is_none());
    let other = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let before = fixture.requests()?.len();
    assert_eq!(
        other.continue_assets(handle).await.unwrap_err(),
        Error::Configuration
    );
    assert_eq!(fixture.requests()?.len(), before);
    let requests = fixture.requests()?;
    assert_eq!(requests[4].body["params"]["cursor"], "opaque:cursor-1");
    let mut next = requests[4].body.clone();
    next["params"]
        .as_object_mut()
        .ok_or("params")?
        .remove("cursor");
    assert_eq!(requests[2].body, next);
    assert_eq!(
        serde_json::from_str::<Observation<OwnerPage>>(&serde_json::to_string(&page)?)?,
        page
    );
    Ok(())
}
#[tokio::test]
async fn full_official_parsed_batch_preserves_all_instructions_duplicates_and_item_errors()
-> TestResult {
    let p = parsed()?[0].clone();
    let sig = signature()?;
    let missing = Signature::from_bytes([8; 64]);
    let response = json!([p.clone(),p,{"signature":missing.to_string(),"parserStatus":"ERROR","parserError":{"code":"transaction_not_found","message":"private diagnostic secret"}}]);
    let fixture = Fixture::start(vec![genesis()?, genesis()?, rest(&response)?]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let q = ParseRequest::new(vec![sig, sig, missing], Commitment::Finalized)?;
    let obs = client.parse_transactions(q.clone()).await?;
    let ParseOutcome::Ok { parsed: p } = &obs.value().results()[0].outcome else {
        return Err("parsed success".into());
    };
    assert_eq!(p.data().instructions.len(), 30);
    assert_eq!(p.data().slot, 433_950_192);
    assert_eq!(p.data().fee, 2_005_000);
    assert_eq!(obs.context().last_indexed_slot(), None);
    assert!(matches!(
        obs.value().results()[2].outcome,
        ParseOutcome::Error { .. }
    ));
    assert!(!format!("{obs:?}").contains("private diagnostic"));
    assert_eq!(
        serde_json::from_str::<Observation<ParsedBatch>>(&serde_json::to_string(&obs)?)?,
        obs
    );
    let req = fixture.requests()?;
    assert!(req[2].target.contains("/v1/parsed-events/transactions"));
    assert_eq!(
        req[2].body["transactions"],
        serde_json::to_value(q.signatures())?
    );
    assert_eq!(req[2].body["commitment"], "finalized");
    assert_eq!(req[2].body["includeRawTransaction"], false);
    Ok(())
}
#[tokio::test]
async fn history_cursor_reuses_original_bounds_and_retains_source_available_range_end() -> TestResult
{
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        rest(&json!({"data":parsed()?,"paginationToken":"433950192:0"}))?,
        genesis()?,
        rest(&json!({"data":[],"paginationToken":null}))?,
        genesis()?,
    ])
    .await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let base = history()?;
    let before = Signature::from_bytes([7; 64]);
    let after = Signature::from_bytes([8; 64]);
    let q = HistoryRequest::new(
        base.address(),
        base.limit(),
        base.commitment(),
        base.direction(),
        Some(before),
        Some(after),
        base.slot(),
        base.time(),
        None,
    )?;
    let page = client.get_address_history(q.clone()).await?;
    let handle = client.history_continuation(&page)?.ok_or("token")?;
    let end = client.continue_history(handle.clone()).await?;
    assert!(end.value().results().is_empty());
    assert!(end.value().pagination_token().is_none());
    let other = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let count = fixture.requests()?.len();
    assert_eq!(
        other.continue_history(handle).await.unwrap_err(),
        Error::Configuration
    );
    assert_eq!(fixture.requests()?.len(), count);
    let requests = fixture.requests()?;
    assert_eq!(requests[2].body["beforeSignature"], before.to_string());
    assert_eq!(requests[2].body["afterSignature"], after.to_string());
    assert_eq!(
        requests[2].body["slot"],
        json!({"gte":433_950_000,"lte":433_951_000})
    );
    assert_eq!(requests[2].body["time"], json!({"gte":1_700_000_000}));
    let mut second = requests[4].body.clone();
    assert_eq!(second["paginationToken"], "433950192:0");
    second
        .as_object_mut()
        .ok_or("body")?
        .remove("paginationToken");
    assert_eq!(second, requests[2].body);
    assert_eq!(
        serde_json::from_str::<Observation<HistoryPage>>(&serde_json::to_string(&page)?)?,
        page
    );
    Ok(())
}
#[tokio::test]
async fn genesis_change_stops_before_asset_or_parsed_data_dispatch() -> TestResult {
    let fixture = Fixture::start(vec![
        genesis()?,
        Reply::result(&json!(Hash::from_bytes([9; 32]).to_string()))?,
    ])
    .await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.get_asset(asset_request()?).await.unwrap_err(),
        Error::Provider(ProviderError::ChainMismatch)
    );
    assert_eq!(fixture.requests()?.len(), 2);
    Ok(())
}
#[tokio::test]
async fn asset_identity_null_envelope_duplicates_and_safe_provider_errors_are_rejected()
-> TestResult {
    let mut wrong = asset()?;
    wrong["id"] = json!(Pubkey::from_bytes([1; 32]).to_string());
    for reply in [Reply::result(&wrong)?,Reply::result(&Value::Null)?,Reply::raw(200,br#"{"jsonrpc":"2.0","id":1,"id":1,"result":null}"#.to_vec()),Reply::raw(200,br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"api-key=secret-source"}}"#.to_vec())]{let fixture=Fixture::start(vec![genesis()?,genesis()?,reply]).await?;let client=HeliusClient::connect(standard(&fixture.endpoint)?).await?;let e=client.get_asset(asset_request()?).await.unwrap_err();assert!(matches!(e,Error::Provider(ProviderError::InvalidResponse|ProviderError::Rpc)));assert!(!format!("{e:?} {e}").contains("secret-source"));}
    let fixture=Fixture::start(vec![genesis()?,genesis()?,Reply::raw(200,br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32004,"message":"not found private detail"}}"#.to_vec())]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.get_asset(asset_request()?).await.unwrap_err(),
        Error::UnavailableData
    );
    Ok(())
}
#[tokio::test]
async fn parse_order_item_state_and_execution_contradictions_are_rejected() -> TestResult {
    let sig = signature()?;
    let other = Signature::from_bytes([7; 64]);
    let q = ParseRequest::new(vec![sig], Commitment::Confirmed)?;
    let mut wrong = parsed()?[0].clone();
    wrong["signature"] = json!(other.to_string());
    let mut contradictory = parsed()?[0].clone();
    contradictory["parserError"] = json!({"code":"failed","message":"secret"});
    let mut execution = parsed()?[0].clone();
    execution["parsed"]["error"] = json!({"InstructionError":[0,"InvalidArgument"]});
    let mut failed = parsed()?[0].clone();
    failed["parsed"]["transactionStatus"] = json!("ERROR");
    failed["parsed"]["error"] = json!({"InstructionError":[0,"InvalidArgument"]});
    for response in [
        json!([]),
        json!([wrong]),
        json!([contradictory]),
        json!([execution]),
    ] {
        let fixture = Fixture::start(vec![genesis()?, genesis()?, rest(&response)?]).await?;
        let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
        assert_eq!(
            client.parse_transactions(q.clone()).await.unwrap_err(),
            invalid()
        );
    }
    let fixture = Fixture::start(vec![genesis()?, genesis()?, rest(&json!([failed]))?]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let obs = client.parse_transactions(q).await?;
    let ParseOutcome::Ok { parsed } = &obs.value().results()[0].outcome else {
        return Err("parser should succeed despite execution failure".into());
    };
    assert_eq!(parsed.data().transaction_status, TransactionStatus::Failed);
    Ok(())
}
#[tokio::test]
async fn exact_integer_width_fraction_null_and_owner_capacity_fail_without_truncation() -> TestResult
{
    for amount in [
        json!(-1),
        Value::Number("1.5".parse()?),
        Value::Number("18446744073709551616".parse()?),
        Value::Null,
    ] {
        let mut a = asset()?;
        a["token_info"] = json!({"supply":amount,"decimals":9});
        let fixture = Fixture::start(vec![genesis()?, genesis()?, Reply::result(&a)?]).await?;
        let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
        let response = client.get_asset(asset_request()?).await;
        if amount.is_null() {
            assert_eq!(
                response?.value().data().token_info.as_ref().unwrap().supply,
                None
            );
        } else {
            assert_eq!(response.unwrap_err(), invalid());
        }
    }
    let mut a = asset()?;
    a["creators"][0]["share"] = json!(101);
    let fixture = Fixture::start(vec![genesis()?, genesis()?, Reply::result(&a)?]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.get_asset(asset_request()?).await.unwrap_err(),
        invalid()
    );
    let response = owner_response(&json!([asset()?, asset()?, asset()?]), None);
    let fixture = Fixture::start(vec![genesis()?, genesis()?, Reply::result(&response)?]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.get_assets_by_owner(owner(None)?).await.unwrap_err(),
        invalid()
    );
    Ok(())
}
#[tokio::test]
async fn duplicate_metadata_keys_and_excessive_nested_source_data_are_rejected() -> TestResult {
    let a = serde_json::to_string(&asset()?)?;
    let bad = a.replace(
        "\"metadata\":{",
        "\"metadata\":{\"duplicate\":1,\"duplicate\":2,",
    );
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        Reply::raw(
            200,
            format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{bad}}}").into_bytes(),
        ),
    ])
    .await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.get_asset(asset_request()?).await.unwrap_err(),
        invalid()
    );
    let mut p = parsed()?;
    p[0]["parsed"]["instructions"][0]["rawAccounts"] =
        json!(vec![Pubkey::from_bytes([1; 32]).to_string(); 257]);
    let fixture = Fixture::start(vec![genesis()?, genesis()?, rest(&p)?]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client
            .parse_transactions(ParseRequest::new(
                vec![signature()?],
                Commitment::Confirmed
            )?)
            .await
            .unwrap_err(),
        invalid()
    );
    Ok(())
}
#[tokio::test]
async fn retries_freeze_exact_asset_request_credentials_and_safe_diagnostics() -> TestResult {
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        Reply::raw(503, b"provider secret credential detail".to_vec()),
        Reply::result(&asset()?)?,
    ])
    .await?;
    let endpoint = format!("{}?api-key=mock-private-key", fixture.endpoint);
    let cfg = config(&endpoint, Duration::from_secs(5), 1_048_576, 1)?;
    assert!(!format!("{cfg:?}").contains("mock-private-key"));
    let client = HeliusClient::connect(cfg).await?;
    client.get_asset(asset_request()?).await?;
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].body, requests[3].body);
    assert_eq!(requests[2].target, requests[3].target);
    assert_eq!(requests[2].headers, requests[3].headers);
    assert!(requests[2].target.contains("api-key=mock-private-key"));
    Ok(())
}
#[tokio::test]
async fn genesis_and_final_body_share_one_deadline_and_body_cap_with_observed_stages() -> TestResult
{
    let mut network = genesis()?;
    network.delay = Duration::from_millis(650);
    let mut data = Reply::result(&asset()?)?;
    data.body_delay = Duration::from_millis(650);
    let fixture = Fixture::start(vec![genesis()?, network, data]).await?;
    let client = HeliusClient::connect(config(
        &fixture.endpoint,
        Duration::from_millis(1200),
        1_048_576,
        0,
    )?)
    .await?;
    assert_eq!(
        client.get_asset(asset_request()?).await.unwrap_err(),
        Error::Timeout
    );
    let requests = fixture.requests()?;
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1].body["method"], "getGenesisHash");
    assert_eq!(requests[2].body["method"], "getAsset");
    for framing in [Framing::Chunked, Framing::CloseDelimited] {
        let mut data = Reply::raw(200, vec![b'x'; 4097]);
        data.framing = framing;
        let fixture = Fixture::start(vec![genesis()?, genesis()?, data]).await?;
        let client =
            HeliusClient::connect(config(&fixture.endpoint, Duration::from_secs(5), 4096, 0)?)
                .await?;
        assert_eq!(
            client.get_asset(asset_request()?).await.unwrap_err(),
            Error::Provider(ProviderError::ResponseTooLarge)
        );
    }
    Ok(())
}

#[tokio::test]
async fn authentication_permissions_and_rate_limits_are_explicit_safe_failures_without_fallback()
-> TestResult {
    for status in [401, 403, 429] {
        let fixture = Fixture::start(vec![
            genesis()?,
            genesis()?,
            Reply::raw(status, b"secret authenticated provider detail".to_vec()),
        ])
        .await?;
        let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
        let error = client.get_asset(asset_request()?).await.unwrap_err();
        assert_eq!(
            error,
            Error::Provider(if status == 429 {
                ProviderError::RateLimited
            } else {
                ProviderError::HttpStatus
            })
        );
        assert!(!format!("{error:?} {error}").contains("secret"));
        assert_eq!(fixture.requests()?.len(), 3);
    }
    Ok(())
}
#[tokio::test]
async fn source_index_unknown_collections_and_history_inclusion_bounds_remain_honest() -> TestResult
{
    let a = json!({"id":asset_request()?.id.to_string(),"interface":"FutureInterface","token_info":{"supply":0,"decimals":0}});
    let fixture = Fixture::start(vec![genesis()?, genesis()?, Reply::result(&a)?]).await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    let observation = client.get_asset(asset_request()?).await?;
    assert!(observation.context().last_indexed_slot().is_none());
    assert!(observation.value().data().authorities.is_none());
    assert!(observation.value().data().ownership.is_none());
    assert_eq!(
        observation
            .value()
            .data()
            .token_info
            .as_ref()
            .unwrap()
            .supply,
        Some(0)
    );
    let mut out_of_range = parsed()?;
    out_of_range[0]["parsed"]["slot"] = json!(433_952_000);
    let fixture = Fixture::start(vec![
        genesis()?,
        genesis()?,
        rest(&json!({"data":out_of_range}))?,
    ])
    .await?;
    let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
    assert_eq!(
        client.get_address_history(history()?).await.unwrap_err(),
        invalid()
    );
    Ok(())
}
#[tokio::test]
async fn explicit_owner_page_and_binary_range_controls_function_without_implicit_pagination()
-> TestResult {
    for position in [
        AssetPosition::Page { page: 1 },
        AssetPosition::Range {
            before: Some(Pubkey::from_bytes([255; 32])),
            after: Some(Pubkey::from_bytes([0; 32])),
        },
    ] {
        let request = OwnerRequest::new(
            Pubkey::parse(OWNER)?,
            2,
            position.clone(),
            AssetSort::Id,
            SortDirection::Asc,
            options(),
            true,
            true,
            false,
        )?;
        let mut response = owner_response(&json!([asset()?]), None);
        if let AssetPosition::Page { page } = position {
            response["page"] = json!(page);
        }
        let fixture =
            Fixture::start(vec![genesis()?, genesis()?, Reply::result(&response)?]).await?;
        let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
        let observation = client.get_assets_by_owner(request).await?;
        assert_eq!(observation.value().items().len(), 1);
        let requests = fixture.requests()?;
        match position {
            AssetPosition::Page { page } => assert_eq!(requests[2].body["params"]["page"], page),
            AssetPosition::Range { before, after } => {
                assert_eq!(
                    requests[2].body["params"]["before"],
                    before.unwrap().to_string()
                );
                assert_eq!(
                    requests[2].body["params"]["after"],
                    after.unwrap().to_string()
                );
                assert!(requests[2].body["params"].get("page").is_none());
            }
            AssetPosition::Cursor { .. } => return Err("fixture mode".into()),
        }
        assert_eq!(requests.len(), 3);
    }
    Ok(())
}
#[tokio::test]
async fn source_raw_token_integer_strings_are_exact_but_scaled_or_noncanonical_values_fail()
-> TestResult {
    for value in [
        json!("9007199254740993"),
        json!("01"),
        json!("1.5"),
        json!("18446744073709551616"),
    ] {
        let mut response = parsed()?;
        response[0]["parsed"]["tokenTransfers"][0]["rawTokenAmount"] = value.clone();
        let fixture = Fixture::start(vec![genesis()?, genesis()?, rest(&response)?]).await?;
        let client = HeliusClient::connect(standard(&fixture.endpoint)?).await?;
        let read = client
            .parse_transactions(ParseRequest::new(
                vec![signature()?],
                Commitment::Confirmed,
            )?)
            .await;
        if value == json!("9007199254740993") {
            let read = read?;
            let ParseOutcome::Ok { parsed } = &read.value().results()[0].outcome else {
                return Err("parsed".into());
            };
            assert_eq!(
                parsed.data().token_transfers[0].raw_token_amount,
                9_007_199_254_740_993
            );
        } else {
            assert_eq!(read.unwrap_err(), invalid());
        }
    }
    Ok(())
}
