// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit ignored keyless API-v2 direct quotes, unsigned data and progress reads.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "rubic-http")]
use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        Address, Amount,
        rubic::{
            Account, Catalogue, Limits, Observation, PreparationRequest, PreparationRequestData,
            QuoteRequest, StatusQuery, TransactionId,
        },
    },
    protocols::rubic::{RubicClient, RubicHttpConfig, RubicReader},
    wallets::{HandoffId, HandoffRequest, PreparedRequest},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    api_url: String,
    provider_id: String,
    user_agent: String,
    catalogue: Catalogue,
    limits: Limits,
    quote_request: QuoteRequest,
    sender: Account,
    receiver: Account,
    minimum_output: Amount,
    maximum_native_value: Amount,
    expected_evm_router: Address,
    source_transaction: TransactionId,
    handoff_id: HandoffId,
}
fn roundtrip<T: Serialize + DeserializeOwned + Eq + std::fmt::Debug>(v: &T) -> TestResult {
    let json = serde_json::to_string(v)?;
    assert_eq!(&serde_json::from_str::<T>(&json)?, v);
    println!("{json}");
    Ok(())
}
fn source<T>(observation: &Observation<T>, provider: &str, before: u64, method: &str) {
    assert_eq!(observation.source().provider_id(), provider);
    assert_eq!(observation.source().method(), method);
    assert_eq!(
        observation.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!(observation.retrieved_at().unix_seconds() >= before);
}
#[tokio::test]
#[ignore = "requires explicit REGIT_WEB3_RUBIC_INPUTS_JSON; public API10RPM; unsigned reads only"]
async fn direct_v2_chains_quotes_unsigned_handoff_and_status_live() -> TestResult {
    let text = std::env::var("REGIT_WEB3_RUBIC_INPUTS_JSON")?;
    let i: Inputs = serde_json::from_str(&text)?;
    let endpoint = RpcEndpoint::new(&i.api_url)?.with_header("user-agent", &i.user_agent)?;
    let http = HttpConfig::new(
        endpoint,
        RpcLimits::new(
            Duration::from_secs(10),
            Duration::from_secs(60),
            2 * 1024 * 1024,
            1,
        )?,
        &i.provider_id,
    )?;
    let c = RubicClient::new(RubicHttpConfig::new(http, i.catalogue.clone(), i.limits)?)?;
    let before = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let chains = RubicReader::chains(&c, false).await?;
    for chain in i.catalogue.chains() {
        chains
            .value()
            .entries()
            .iter()
            .find(|v| &v.alias == chain.alias())
            .ok_or("source alias unavailable")?
            .matches(chain)?;
    }
    source(&chains, &i.provider_id, before, "api_v2_chains");
    roundtrip(&chains)?;
    tokio::time::sleep(Duration::from_millis(6200)).await;
    let all = RubicReader::quote_all(&c, i.quote_request.clone()).await?;
    assert!(!all.value().routes().is_empty());
    source(&all, &i.provider_id, before, "api_v2_quote_all");
    roundtrip(&all)?;
    tokio::time::sleep(Duration::from_millis(6200)).await;
    let quote = RubicReader::quote_best(&c, i.quote_request.clone()).await?;
    source(&quote, &i.provider_id, before, "api_v2_quote_best");
    roundtrip(&quote)?;
    tokio::time::sleep(Duration::from_millis(6200)).await;
    let request = PreparationRequest::new(PreparationRequestData {
        quote: quote.value().clone(),
        sender: i.sender,
        receiver: i.receiver,
        minimum_output: i.minimum_output,
        maximum_native_value: i.maximum_native_value,
        expected_evm_router: Some(i.expected_evm_router),
        expected_approval: quote.value().data().approval_address,
        expected_permit2: quote.value().data().permit2_address,
        expected_solana_signers: Vec::new(),
    })?;
    let prepared = RubicReader::prepare(&c, request).await?;
    source(&prepared, &i.provider_id, before, "api_v2_swap_data");
    roundtrip(&prepared)?;
    let handoff = HandoffRequest::new(
        i.handoff_id,
        PreparedRequest::new(prepared.value().clone())?,
    );
    roundtrip(&handoff)?;
    tokio::time::sleep(Duration::from_millis(6200)).await;
    let query = StatusQuery::new(
        quote.value().data().id.clone(),
        i.quote_request.data().source.chain().clone(),
        i.source_transaction,
        i.quote_request.data().destination.chain().clone(),
    )?;
    let status = RubicReader::status(&c, query).await?;
    source(&status, &i.provider_id, before, "api_v2_status_extended");
    roundtrip(&status)?;
    println!(
        "rubic api-v2 chains={} routes={} provider={} route={} gross={} quote_input={} quoted_output={} original_min={} fresh_output={} fresh_min={} status={:?} retrieved={}",
        chains.value().entries().len(),
        all.value().routes().len(),
        quote.value().data().provider.as_str(),
        quote.value().data().id.as_str(),
        i.quote_request.data().amount.raw(),
        quote.value().data().input.amount().raw(),
        quote.value().data().estimate.data().output.raw(),
        quote.value().data().estimate.data().minimum_output.raw(),
        prepared
            .value()
            .data()
            .fresh_quote
            .data()
            .estimate
            .data()
            .output
            .raw(),
        prepared
            .value()
            .data()
            .fresh_quote
            .data()
            .estimate
            .data()
            .minimum_output
            .raw(),
        status.value().status(),
        status.retrieved_at().unix_seconds()
    );
    Ok(())
}
