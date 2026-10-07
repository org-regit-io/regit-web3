// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit read-only V2 quote/build, local handoff and exact Solana fee/simulation proof.
#![cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#![cfg(feature = "jupiter-http")]
use regit_web3::{
    chains::solana::{SolanaClient, SolanaHttpConfig},
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    domain::{
        jupiter::{
            BuildRequest, Context, Operation, PreparationSettings, PreparedSwap, QuoteRequest,
            SwapIntent, SwapIntentData,
        },
        solana::{Commitment, ReadOptions},
    },
    error::Error,
    protocols::jupiter::{JupiterClient, JupiterHttpConfig},
    wallets::{HandoffId, HandoffRequest, PreparedRequest},
};
use std::{
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn required(name: &str) -> Result<String, Error> {
    env::var(name).map_err(|_| Error::Configuration)
}
fn now() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| Error::Configuration)
}
fn http(prefix: &str) -> Result<HttpConfig, Error> {
    HttpConfig::new(
        RpcEndpoint::new(&required(&format!("{prefix}_URL"))?)?,
        RpcLimits::new(
            Duration::from_secs(5),
            Duration::from_secs(20),
            2 * 1024 * 1024,
            1,
        )?,
        required(&format!("{prefix}_PROVIDER_ID"))?,
    )
}
fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned + Eq + std::fmt::Debug>(
    value: &T,
) -> Result<(), Error> {
    let encoded = serde_json::to_string(value).map_err(|_| Error::Configuration)?;
    assert_eq!(
        &serde_json::from_str::<T>(&encoded).map_err(|_| Error::Configuration)?,
        value
    );
    println!("{encoded}");
    Ok(())
}
fn context(
    context: &Context,
    operation: Operation,
    client: &JupiterClient,
    before: u64,
    after: u64,
) {
    assert_eq!(context.schema_version(), 1);
    assert_eq!(context.operation(), operation);
    assert_eq!(
        context.network().genesis_hash(),
        client.config().network().genesis_hash()
    );
    assert_eq!(
        context.source().provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(
        context.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=after).contains(&context.retrieved_at().unix_seconds()));
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit Jupiter V2/mainnet RPC inputs; read-only, no signing or submission"]
async fn v2_quote_build_unsigned_handoff_fee_and_simulation_live() -> Result<(), Error> {
    let quote: QuoteRequest = serde_json::from_str(&required("REGIT_WEB3_JUPITER_QUOTE_JSON")?)
        .map_err(|_| Error::Configuration)?;
    let build: BuildRequest = serde_json::from_str(&required("REGIT_WEB3_JUPITER_BUILD_JSON")?)
        .map_err(|_| Error::Configuration)?;
    let settings: PreparationSettings =
        serde_json::from_str(&required("REGIT_WEB3_JUPITER_SETTINGS_JSON")?)
            .map_err(|_| Error::Configuration)?;
    assert_eq!(
        quote.data().network.genesis_hash(),
        build.data().network.genesis_hash()
    );
    assert_eq!(quote.data().input_mint, build.data().input_mint);
    assert_eq!(quote.data().output_mint, build.data().output_mint);
    assert_eq!(quote.data().amount, build.data().amount);
    let client = JupiterClient::new(JupiterHttpConfig::new(
        http("REGIT_WEB3_JUPITER")?,
        build.data().network.clone(),
        None,
    )?)?;
    let rpc = SolanaClient::connect(SolanaHttpConfig::new(
        build.data().network.clone(),
        http("REGIT_WEB3_JUPITER_RPC")?,
    ))
    .await?;
    let before = now()?;
    let quoted = client.quote(quote.clone()).await?;
    // Harness pacing is explicit, respecting current documented keyless0.5RPS.
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let response = client.build(build.clone()).await?;
    let after = now()?;
    assert_eq!(quoted.value().data().request, quote);
    assert_eq!(response.value().data().request, build);
    context(
        quoted.context(),
        Operation::OrderQuote,
        &client,
        before,
        after,
    );
    context(response.context(), Operation::Build, &client, before, after);
    assert_eq!(quoted.context().source().method(), "swap_v2_order");
    assert_eq!(response.context().source().method(), "swap_v2_build");
    let prepared = PreparedSwap::new(SwapIntent::new(SwapIntentData {
        build: response.clone(),
        settings,
    })?)?;
    assert_eq!(
        prepared.unsigned().lifetime().blockhash,
        response.value().data().blockhash
    );
    assert_eq!(
        prepared.unsigned().lifetime().last_valid_block_height,
        response.value().data().last_valid_block_height
    );
    let handoff = HandoffRequest::new(
        HandoffId::new(&required("REGIT_WEB3_JUPITER_HANDOFF_ID")?)?,
        PreparedRequest::new(prepared.clone())?,
    );
    assert_eq!(handoff.prepared().preparation(), &prepared);
    let estimate = client
        .estimate_swap(
            &rpc,
            prepared.clone(),
            ReadOptions::new(Commitment::Confirmed, None),
        )
        .await?;
    for c in [estimate.fee().context(), estimate.simulation().context()] {
        assert_eq!(
            c.network().genesis_hash(),
            client.config().network().genesis_hash()
        );
        assert_eq!(
            c.source().provider_id(),
            rpc.config().http_config().provider_id()
        );
        assert_eq!(c.source().integration_version(), env!("CARGO_PKG_VERSION"));
        assert!(c.evaluation_slot().is_some());
        assert!(c.retrieved_at().unix_seconds() >= before);
    }
    assert_eq!(estimate.prepared(), &prepared);
    roundtrip(&quoted)?;
    roundtrip(&response)?;
    roundtrip(&prepared)?;
    roundtrip(&estimate)?;
    Ok(())
}
