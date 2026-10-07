// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Opt-in actual LI.FI Rust quote/routes/preparation and transaction/transfer status.
//! Inputs belong to this harness. No wallet, signed transaction or submission is used.
#![cfg(feature = "lifi-http")]

#[path = "support/live.rs"]
mod live;
use regit_web3::{
    domain::{Amount, ExactDecimal, lifi::*},
    error::Error,
    protocols::lifi::{LifiClient, LifiHttpConfig},
};

fn number(name: &str) -> Result<u64, Error> {
    live::required(name)?
        .parse()
        .map_err(|_| Error::Configuration)
}
fn request(catalogue: &ChainCatalogue) -> Result<Request, Error> {
    let from = catalogue.resolve(number("REGIT_WEB3_LIFI_FROM_CHAIN_ID")?)?;
    let to = catalogue.resolve(number("REGIT_WEB3_LIFI_TO_CHAIN_ID")?)?;
    Request::new(RequestData {
        from: Asset::new(from, &live::required("REGIT_WEB3_LIFI_FROM_TOKEN")?)?,
        to: Asset::new(to, &live::required("REGIT_WEB3_LIFI_TO_TOKEN")?)?,
        amount: Amount::from_decimal(&live::required("REGIT_WEB3_LIFI_AMOUNT")?, None)?,
        from_account: Account::new(from, &live::required("REGIT_WEB3_LIFI_FROM_ACCOUNT")?)?,
        to_account: Some(Account::new(
            to,
            &live::required("REGIT_WEB3_LIFI_TO_ACCOUNT")?,
        )?),
        slippage: Slippage::new(ExactDecimal::parse(&live::required(
            "REGIT_WEB3_LIFI_SLIPPAGE",
        )?)?)?,
        allow_switch_chain: false,
    })
}
fn check(origin: &Origin, client: &LifiClient, method: &str, before: u64) -> Result<(), Error> {
    assert_eq!(
        origin.source.provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(origin.source.method(), method);
    assert_eq!(
        origin.source.integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=live::unix_seconds()?).contains(&origin.retrieved_at.unix_seconds()));
    Ok(())
}
fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(
    value: &T,
) -> Result<(), serde_json::Error> {
    assert_eq!(
        &serde_json::from_slice::<T>(&serde_json::to_vec(value)?)?,
        value
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit LI.FI URL/provider/catalogue/token/account/amount/slippage/status inputs; quote/preparation/read only"]
async fn lifi_four_operations_and_both_status_selectors_live()
-> Result<(), Box<dyn std::error::Error>> {
    let catalogue: ChainCatalogue =
        serde_json::from_str(&live::required("REGIT_WEB3_LIFI_CHAINS")?)?;
    let request = request(&catalogue)?;
    let client = LifiClient::new(LifiHttpConfig::new(
        live::http_config("REGIT_WEB3_LIFI")?,
        catalogue.clone(),
        Limits::new(64, 256, 128)?,
    )?)?;
    let before = live::unix_seconds()?;
    let quote = client.get_quote(request.clone()).await?;
    check(quote.origin(), &client, "quote", before)?;
    assert!(quote.step().data().action.matches_request(&request));
    let estimate = quote
        .step()
        .data()
        .estimate
        .as_ref()
        .ok_or(Error::UnavailableData)?;
    assert!(!estimate.data().to_amount.raw().is_zero());
    assert_eq!(estimate.data().expiry, Expiry::Unreported);
    roundtrip(quote.step())?;
    live::print_and_roundtrip(quote.origin())?;
    println!(
        "quote id={} tool={} from_units={} to_units={} minimum_units={} included_steps={}",
        quote.step().data().id.as_str(),
        quote.step().data().tool.as_str(),
        estimate.data().from_amount.raw(),
        estimate.data().to_amount.raw(),
        estimate.data().to_amount_min.raw(),
        quote.step().data().included_steps.len()
    );
    let before = live::unix_seconds()?;
    let routes = client.get_routes(request.clone()).await?;
    check(routes.origin(), &client, "advanced-routes", before)?;
    assert_eq!(routes.request(), &request);
    assert!(!routes.routes().is_empty());
    let selected = routes
        .routes()
        .first()
        .and_then(|r| r.steps().first())
        .ok_or(Error::UnavailableData)?;
    println!(
        "routes={} filtered_paths={} failed_paths={} selected_id={} selected_tool={}",
        routes.routes().len(),
        routes.unavailable().map_or(0, |u| u.filtered_paths.len()),
        routes.unavailable().map_or(0, |u| u.failed_paths.len()),
        selected.step().data().id.as_str(),
        selected.step().data().tool.as_str()
    );
    live::print_and_roundtrip(routes.origin())?;
    let before = live::unix_seconds()?;
    let prepared = client.prepare_step(selected).await?;
    check(
        prepared.origin(),
        &client,
        "advanced-stepTransaction",
        before,
    )?;
    assert_eq!(prepared.original_request(), &request);
    assert_eq!(prepared.selected_step(), selected.step());
    assert_eq!(prepared.selected_origin(), selected.origin());
    assert_eq!(
        prepared.payload().chain(),
        selected
            .step()
            .data()
            .action
            .data()
            .from_token
            .data()
            .asset
            .chain()
    );
    roundtrip(&prepared)?;
    println!(
        "prepared id={} payload_chain={} encoding={}",
        prepared.refreshed_step().data().id.as_str(),
        prepared.payload().chain().id(),
        match prepared.payload() {
            Payload::Evm(_) => "evm_fields_and_opaque_calldata",
            Payload::Encoded(_) => "family_encoded_source_payload",
        }
    );
    live::print_and_roundtrip(prepared.origin())?;
    qualify_statuses(&client, &catalogue).await
}
async fn qualify_statuses(
    client: &LifiClient,
    catalogue: &ChainCatalogue,
) -> Result<(), Box<dyn std::error::Error>> {
    let from = catalogue.resolve(number("REGIT_WEB3_LIFI_STATUS_FROM_CHAIN_ID")?)?;
    let to = catalogue.resolve(number("REGIT_WEB3_LIFI_STATUS_TO_CHAIN_ID")?)?;
    let hash = TransactionId::new(from, &live::required("REGIT_WEB3_LIFI_STATUS_TX_HASH")?)?;
    let query = StatusQuery::new(
        from,
        to,
        StatusSelector::SendingTransaction(hash.clone()),
        None,
    )?;
    let before = live::unix_seconds()?;
    let status = client.get_status(query.clone()).await?;
    check(status.origin(), client, "status", before)?;
    assert_eq!(status.query(), &query);
    assert_eq!(status.data().progress, Progress::Done);
    assert_eq!(status.receiving_role(), ReceivingRole::Destination);
    let sending = status
        .data()
        .sending
        .as_ref()
        .ok_or(Error::UnavailableData)?
        .data();
    let receiving = status
        .data()
        .receiving
        .as_ref()
        .ok_or(Error::UnavailableData)?
        .data();
    assert_eq!(sending.id.as_ref(), Some(&hash));
    assert_eq!(receiving.chain, to);
    assert!(receiving.id.is_some());
    assert!(sending.timestamp.is_some());
    assert!(receiving.timestamp.is_some());
    roundtrip(&status)?;
    println!(
        "transaction status={:?} substatus={:?} sending_chain={} receiving_chain={} transfer_id={:?} sending_data_time={:?} receiving_data_time={:?}",
        status.data().progress,
        status.data().substatus,
        from.id(),
        receiving.chain.id(),
        status.data().transfer_id,
        sending.timestamp,
        receiving.timestamp
    );
    live::print_and_roundtrip(status.origin())?;
    let transfer_id = TransferId::new(&live::required("REGIT_WEB3_LIFI_STATUS_TRANSFER_ID")?)?;
    let transfer_query = StatusQuery::new(
        from,
        to,
        StatusSelector::ProviderTransfer(transfer_id.clone()),
        None,
    )?;
    let before = live::unix_seconds()?;
    let transfer_status = client.get_status(transfer_query.clone()).await?;
    check(transfer_status.origin(), client, "status", before)?;
    assert_eq!(transfer_status.query(), &transfer_query);
    roundtrip(&transfer_status)?;
    assert_eq!(transfer_status.data().progress, Progress::Done);
    assert_eq!(
        transfer_status.data().transfer_id.as_ref(),
        Some(&transfer_id)
    );
    assert_eq!(transfer_status.data().sending, status.data().sending);
    assert_eq!(transfer_status.data().receiving, status.data().receiving);
    println!(
        "provider transfer={} progress={:?} actual sending_chain={} receiving_chain={}",
        transfer_id.as_str(),
        transfer_status.data().progress,
        from.id(),
        to.id()
    );
    live::print_and_roundtrip(transfer_status.origin())?;
    Ok(())
}
