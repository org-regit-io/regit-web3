// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit
//! Opt-in actual TON read/fee/preparation proof with explicit public inputs and no submission.
#![cfg(feature = "ton-http")]

use regit_web3::{
    chains::ton::{TonClient, TonHttpConfig, TonReader},
    domain::ton::{
        Address, Boc, Cursor, FeeRequest, Hash, HistoryRequest, LogicalTime, Nanotons, Network,
        NetworkCategory, TransferIntent, TransferPreparation, ZeroState,
    },
    wallets::PreparedRequest,
};
use std::time::Duration;
#[path = "support/live.rs"]
mod live;

#[tokio::test]
#[ignore = "requires explicit public TON inputs and outgoing network; never submits"]
async fn current_ton_reads_fee_estimate_and_internal_review()
-> Result<(), Box<dyn std::error::Error>> {
    const PREFIX: &str = "REGIT_WEB3_TON";
    let network = Network::new(
        NetworkCategory::Mainnet,
        ZeroState::new(
            -1,
            Hash::parse(&live::required("REGIT_WEB3_TON_ZERO_ROOT")?)?,
            Hash::parse(&live::required("REGIT_WEB3_TON_ZERO_FILE")?)?,
        )?,
    )?;
    let address = Address::parse(&live::required("REGIT_WEB3_TON_ACCOUNT")?)?;
    let cursor = Cursor::new(
        LogicalTime::parse(&live::required("REGIT_WEB3_TON_TX_LT")?)?,
        Hash::parse(&live::required("REGIT_WEB3_TON_TX_HASH")?)?,
    )?;
    let message_hash = Hash::parse(&live::required("REGIT_WEB3_TON_MESSAGE_HASH")?)?;
    let outgoing_cursor = Cursor::new(
        LogicalTime::parse(&live::required("REGIT_WEB3_TON_OUTGOING_TX_LT")?)?,
        Hash::parse(&live::required("REGIT_WEB3_TON_OUTGOING_TX_HASH")?)?,
    )?;
    let page_limit = live::required("REGIT_WEB3_TON_PAGE_LIMIT")?.parse::<u8>()?;
    let body = Boc::parse(&live::required("REGIT_WEB3_TON_FEE_BODY_BOC")?)?;
    let fee_request = FeeRequest::new(network, address, body, None, None, true)?;
    // Test-owned caller pacing for the documented anonymous provider allowance.
    // This does not change library request, retry or operation-budget behavior.
    let interval = Duration::from_millis(
        live::required("REGIT_WEB3_TON_OPERATION_INTERVAL_MILLIS")?.parse::<u64>()?,
    );
    if interval > Duration::from_secs(60) {
        return Err(regit_web3::error::Error::Configuration.into());
    }
    let spacing = Duration::from_millis(
        live::required("REGIT_WEB3_TON_REQUEST_SPACING_MILLIS")?.parse::<u64>()?,
    );
    let config =
        TonHttpConfig::new(network, live::http_config(PREFIX)?)?.with_request_spacing(spacing)?;
    let client = TonClient::connect(config).await?;
    let started = live::unix_seconds()?;
    tokio::time::sleep(interval).await;
    let network = client.get_network_data().await?;
    assert!(network.context().retrieved_at().unix_seconds() >= started);
    live::print_and_roundtrip(&network)?;
    tokio::time::sleep(interval).await;
    let balance = client.get_account_balance(address).await?;
    assert!(balance.value().balance().native().raw() > 0);
    live::print_and_roundtrip(&balance)?;
    tokio::time::sleep(interval).await;
    let request = HistoryRequest::new(address, Some(cursor), page_limit, true)?;
    let history = client.get_account_history(request).await?;
    assert!(!history.value().transactions().is_empty());
    assert_eq!(history.value().transactions()[0].cursor(), cursor);
    live::print_and_roundtrip(&history)?;
    tokio::time::sleep(interval).await;
    let transaction = client.get_transaction(address, cursor).await?;
    assert_eq!(transaction.value().cursor(), cursor);
    live::print_and_roundtrip(&transaction)?;
    tokio::time::sleep(interval).await;
    let outgoing = client.get_transaction(address, outgoing_cursor).await?;
    assert_eq!(outgoing.value().cursor(), outgoing_cursor);
    assert!(!outgoing.value().outgoing().is_empty());
    let source = outgoing.value().provider_fees().ok_or("source fee facts")?;
    assert!(source.aggregate().raw() > outgoing.value().fees().native().raw());
    assert!(
        source
            .outgoing()
            .iter()
            .any(|fee| fee.forwarding_fee.raw() > 0)
    );
    live::print_and_roundtrip(&outgoing)?;
    tokio::time::sleep(interval).await;
    let status = client.get_transaction_status(address, cursor).await?;
    assert_eq!(status.value().transaction().cursor(), cursor);
    live::print_and_roundtrip(&status)?;
    tokio::time::sleep(interval).await;
    let message = client.get_message_status(message_hash, request).await?;
    assert!(!message.value().observed().is_empty());
    live::print_and_roundtrip(&message)?;
    tokio::time::sleep(interval).await;
    let fees = client.estimate_fee(fee_request.clone()).await?;
    assert_eq!(fees.value().request(), &fee_request);
    assert!(fees.value().source().gas_fee.raw() > 0);
    live::print_and_roundtrip(&fees)?;
    review_internal(client.config().network(), address)?;
    println!(
        "TON seven outgoing read/estimate methods and pure internal-message review qualified; no submission"
    );
    Ok(())
}

fn review_internal(network: Network, address: Address) -> Result<(), Box<dyn std::error::Error>> {
    let destination = Address::parse(&live::required("REGIT_WEB3_TON_PREPARE_DESTINATION")?)?;
    let amount = Nanotons::parse(&live::required("REGIT_WEB3_TON_PREPARE_NANOTONS")?)?;
    let expiry = live::required("REGIT_WEB3_TON_WALLET_VALID_UNTIL")?.parse::<u32>()?;
    let preparation = TransferPreparation::new(TransferIntent::new(
        network,
        address,
        destination,
        amount,
        false,
        None,
        Some(expiry),
    )?)?;
    let reviewed = PreparedRequest::new(preparation)?;
    assert_eq!(reviewed.review().intent().sender(), address);
    assert_eq!(
        reviewed.review().intent().wallet_valid_until(),
        Some(expiry)
    );
    live::print_and_roundtrip(&reviewed)?;
    Ok(())
}
