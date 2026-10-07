// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Opt-in actual Cardano indexed reads and unsigned review, with explicit credentials.

#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "blockfrost-http")]

#[path = "support/live.rs"]
mod live;
use regit_web3::{
    config::HttpConfig,
    domain::cardano::{
        AssetId, EpochSelector, Hash, Network, NetworkId, Order, PageRequest, PaymentAddress,
        PaymentIntent, PaymentPreparation, StakeAddress, TransactionStatus,
    },
    error::Error,
    providers::blockfrost::{BlockfrostClient, BlockfrostHttpConfig},
    wallets::{Preparation, PreparedRequest},
};

const PREFIX: &str = "REGIT_WEB3_BLOCKFROST";
fn required(suffix: &str) -> Result<String, Error> {
    live::required(&format!("{PREFIX}_{suffix}"))
}

#[tokio::test]
#[ignore = "requires explicit hosted Blockfrost project credential, network and known indexed inputs; no writes"]
async fn all_indexed_reads_and_estimate_with_explicit_unsigned_payment_review()
-> Result<(), Box<dyn std::error::Error>> {
    let network = Network::new(
        NetworkId::new(
            required("NETWORK_TAG")?.parse()?,
            required("NETWORK_MAGIC")?.parse()?,
        )?,
        required("NETWORK_ALIAS")?,
    )?;
    let base = live::http_config(PREFIX)?;
    let endpoint = base
        .endpoint()
        .clone()
        .with_header("project_id", &required("PROJECT_ID")?)?;
    let http = HttpConfig::new(endpoint, base.limits(), base.provider_id())?;
    let c = BlockfrostClient::connect(BlockfrostHttpConfig::new(network.clone(), http)).await?;
    let address = PaymentAddress::parse(&required("ADDRESS")?)?;
    let stake = StakeAddress::parse(&required("STAKE_ADDRESS")?)?;
    let asset = AssetId::from_unit(network.identity(), &required("ASSET_UNIT")?)?;
    let transaction = Hash::parse(&required("TRANSACTION_ID")?)?;
    let epoch = EpochSelector::Number(required("EPOCH")?.parse()?);
    let intent_text = required("PAYMENT_INTENT_JSON")?;
    if intent_text.len() > 256 * 1024 {
        return Err(Error::Configuration.into());
    }
    let intent: PaymentIntent =
        serde_json::from_str(&intent_text).map_err(|_| Error::Configuration)?;
    if intent.network().identity() != network.identity() {
        return Err(Error::Configuration.into());
    }
    let page = PageRequest::new(1, 2, Order::Asc)?;
    let before = live::unix_seconds()?;
    let v = c.get_balance(address.clone()).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_utxos(address.clone(), page).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_address_details(address.clone()).await?;
    assert_eq!(v.value().data().balance.address(), &address);
    live::print_and_roundtrip(&v)?;
    let v = c.get_address_transactions(address, page).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_network_data().await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_epoch(epoch).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_protocol_parameters(epoch).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_stake_account(stake.clone()).await?;
    assert_eq!(&v.value().data().address, &stake);
    live::print_and_roundtrip(&v)?;
    let v = c.get_stake_rewards(stake, page).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_assets(page).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_asset(asset.clone()).await?;
    assert_eq!(&v.value().data().asset, &asset);
    live::print_and_roundtrip(&v)?;
    let v = c.get_asset_transactions(asset.clone(), page).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_asset_holders(asset, page).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_transaction(transaction).await?;
    assert_eq!(v.value().cbor().transaction_id(), transaction);
    live::print_and_roundtrip(&v)?;
    let v = c.get_transaction_utxos(transaction).await?;
    live::print_and_roundtrip(&v)?;
    let v = c.get_transaction_status(transaction).await?;
    assert!(matches!(v.value(), TransactionStatus::Indexed { .. }));
    live::print_and_roundtrip(&v)?;
    let v = c.estimate_payment_fee(intent.clone(), epoch).await?;
    assert_eq!(v.value().intent(), &intent);
    live::print_and_roundtrip(&v)?;
    let prep = PaymentPreparation::new(intent, v.value().parameters().clone())?;
    prep.validate()?;
    let review = PreparedRequest::new(prep)?;
    assert_eq!(review.review().network().identity(), network.identity());
    assert!(v.context().retrieved_at().unix_seconds() >= before);
    println!(
        "17 indexed read/estimate methods and explicit pure unsigned review passed; no submission"
    );
    Ok(())
}
