// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit authenticated read-only Helius data qualification; ordinary tests stay offline.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "helius-http")]
use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        helius::*,
        solana::{Commitment, Hash, Network, Pubkey, Signature},
    },
    providers::helius::{HeliusClient, HeliusHttpConfig},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{env, time::Duration};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn input(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    env::var(name).map_err(|_| format!("explicit {name} input required").into())
}
fn roundtrip<T: ObservedData + Serialize + DeserializeOwned + Eq + std::fmt::Debug>(
    value: &Observation<T>,
) -> TestResult {
    assert_eq!(
        &serde_json::from_str::<Observation<T>>(&serde_json::to_string(value)?)?,
        value
    );
    Ok(())
}
fn configuration() -> Result<HeliusHttpConfig, Box<dyn std::error::Error>> {
    let endpoint = input("REGIT_WEB3_HELIUS_URL")?;
    let genesis = Hash::parse(&input("REGIT_WEB3_HELIUS_GENESIS_HASH")?)?;
    let alias = input("REGIT_WEB3_HELIUS_NETWORK_ALIAS")?;
    let provider = input("REGIT_WEB3_HELIUS_PROVIDER_ID")?;
    let config = HeliusHttpConfig::new(
        Network::new(genesis, alias)?,
        HttpConfig::new(
            RpcEndpoint::new(&endpoint)?,
            RpcLimits::new(
                Duration::from_secs(10),
                Duration::from_secs(60),
                16 * 1024 * 1024,
                0,
            )?,
            &provider,
        )?,
    );
    Ok(config)
}
#[tokio::test]
#[ignore = "requires explicit caller-owned authenticated Helius endpoint and public data vectors"]
async fn helius_assets_parsed_transactions_and_history_live() -> TestResult {
    let asset_id = Pubkey::parse(&input("REGIT_WEB3_HELIUS_ASSET_ID")?)?;
    let owner = Pubkey::parse(&input("REGIT_WEB3_HELIUS_OWNER")?)?;
    let signature = Signature::parse(&input("REGIT_WEB3_HELIUS_SIGNATURE")?)?;
    let commitment = match input("REGIT_WEB3_HELIUS_COMMITMENT")?.as_str() {
        "confirmed" => Commitment::Confirmed,
        "finalized" => Commitment::Finalized,
        _ => return Err("explicit supported Helius commitment required".into()),
    };
    let client = HeliusClient::connect(configuration()?).await?;
    let options = AssetOptions {
        show_unverified_collections: false,
        show_collection_metadata: false,
        show_fungible: true,
    };
    let asset = client
        .get_asset(AssetRequest {
            id: asset_id,
            options,
        })
        .await?;
    roundtrip(&asset)?;
    let owner_page = client
        .get_assets_by_owner(OwnerRequest::new(
            owner,
            2,
            AssetPosition::Cursor { cursor: None },
            AssetSort::Id,
            SortDirection::Asc,
            options,
            true,
            true,
            false,
        )?)
        .await?;
    roundtrip(&owner_page)?;
    let batch = client
        .parse_transactions(ParseRequest::new(vec![signature, signature], commitment)?)
        .await?;
    roundtrip(&batch)?;
    let ParseOutcome::Ok { parsed } = &batch.value().results()[0].outcome else {
        return Err(
            "public signature did not produce parsed data; not live data qualification".into(),
        );
    };
    let history = client
        .get_address_history(HistoryRequest::new(
            owner,
            2,
            commitment,
            SortDirection::Desc,
            None,
            None,
            None,
            None,
            None,
        )?)
        .await?;
    roundtrip(&history)?;
    assert_eq!(asset.value().id(), asset_id);
    assert_eq!(batch.value().results()[1].signature, signature);
    assert!(
        history
            .value()
            .results()
            .iter()
            .any(|r| matches!(r.outcome, ParseOutcome::Ok { .. })),
        "history must contain actual parsed data for this qualifier"
    );
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"operation":"getAsset","context":asset.context(),"id":asset.value().id(),"supply":asset.value().data().token_info.as_ref().and_then(|t|t.supply)})
        )?
    );
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"operation":"getAssetsByOwner","context":owner_page.context(),"items":owner_page.value().items().len(),"total":owner_page.value().reported_total(),"grand_total":owner_page.value().reported_grand_total(),"lamports":owner_page.value().native_balance().map(|b|b.lamports)})
        )?
    );
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"operation":"parsedEventsTransactions","context":batch.context(),"signature":signature,"slot":parsed.data().slot,"fee":parsed.data().fee,"execution":parsed.data().transaction_status,"instructions":parsed.data().instructions.len()})
        )?
    );
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"operation":"parsedEventsTransactionHistory","context":history.context(),"items":history.value().results().len(),"has_source_token":history.value().pagination_token().is_some()})
        )?
    );
    Ok(())
}
