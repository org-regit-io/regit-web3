// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit read-only Uniswap V3 public RPC qualification and local unsigned preparation.
#![cfg(feature = "uniswap-http")]
#[path = "../examples/support/inputs.rs"]
mod inputs;

use regit_web3::{
    domain::{
        Address, BlockSelector, Finality, Timestamp,
        evm::ReadState,
        uniswap::{
            PreparedSwap, RouteComparison, SlippageBps, SwapIntent, SwapIntentData,
            V3QuoteObservation, V3QuoteRequest, V3RouteRequest,
        },
    },
    error::Error,
    protocols::uniswap::UniswapV3Client,
    wallets::{HandoffId, HandoffRequest, PreparedRequest},
};
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};
fn required(name: &str) -> Result<String, Error> {
    env::var(name).map_err(|_| Error::Configuration)
}
fn now() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| Error::Configuration)
}
fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned + Eq + std::fmt::Debug>(
    value: &T,
) -> Result<(), Error> {
    let json = serde_json::to_string(value).map_err(|_| Error::Configuration)?;
    let decoded: T = serde_json::from_str(&json).map_err(|_| Error::Configuration)?;
    assert_eq!(&decoded, value);
    println!("{json}");
    Ok(())
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit V3 QuoterV2 request/routes, supported deployment and read-only RPC"]
async fn v3_quotes_supplied_routes_and_router_2_1_2_unsigned_preparation_live() -> Result<(), Error>
{
    let inputs = inputs::from_env()?;
    let request: V3QuoteRequest = serde_json::from_str(&required("REGIT_WEB3_UNISWAP_QUOTE_JSON")?)
        .map_err(|_| Error::Configuration)?;
    let routes: V3RouteRequest = serde_json::from_str(&required("REGIT_WEB3_UNISWAP_ROUTES_JSON")?)
        .map_err(|_| Error::Configuration)?;
    let recipient = Address::parse(&required("REGIT_WEB3_UNISWAP_RECIPIENT")?)?;
    let slippage = SlippageBps::new(
        required("REGIT_WEB3_UNISWAP_SLIPPAGE_BPS")?
            .parse()
            .map_err(|_| Error::Configuration)?,
    )?;
    let deadline = Timestamp::from_unix_seconds(
        required("REGIT_WEB3_UNISWAP_DEADLINE")?
            .parse()
            .map_err(|_| Error::Configuration)?,
    );
    assert_eq!(request.data().settings.data().from, inputs.address);
    assert_eq!(request.data().deployment, routes.data().deployment);
    assert_eq!(request.data().amount_in, routes.data().amount_in);
    let before = now()?;
    let client = UniswapV3Client::connect(inputs.config, request.data().deployment.clone()).await?;
    let quote = client.quote_exact_input(request.clone(), None).await?;
    let ReadState::CanonicalHash { block, .. } = quote.context().state() else {
        return Err(Error::Configuration);
    };
    let comparison = client
        .compare_routes(routes.clone(), Some(BlockSelector::Hash(*block.hash())))
        .await?;
    let after = now()?;
    assert_eq!(quote.value().data().request, request);
    assert_eq!(comparison.data().request, routes);
    let best = comparison
        .best_quote()
        .ok_or(Error::UnavailableData)?
        .clone();
    assert_eq!(
        best.context().state(),
        ReadState::CanonicalHash {
            requested_selector: BlockSelector::Hash(*block.hash()),
            block
        }
    );
    for context in [
        quote.context(),
        best.context(),
        comparison.data().anchor.context(),
    ] {
        assert_eq!(context.network(), client.config().network());
        assert_eq!(
            context.source().provider_id(),
            client.config().provider_id()
        );
        assert_eq!(
            context.source().integration_version(),
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(context.finality(), Finality::Unknown);
        assert_eq!(context.confirmations(), None);
        assert!((before..=after).contains(&context.retrieved_at().unix_seconds()));
    }
    let prepared = PreparedSwap::new(SwapIntent::new(SwapIntentData {
        quote: best,
        recipient,
        slippage,
        deadline,
        settings: request.data().settings.clone(),
    })?)?;
    let reviewed = PreparedRequest::new(prepared.clone())?;
    let handoff = HandoffRequest::new(
        HandoffId::new(&required("REGIT_WEB3_UNISWAP_HANDOFF_ID")?)?,
        reviewed,
    );
    assert_eq!(handoff.prepared().preparation(), &prepared);
    roundtrip::<V3QuoteObservation>(&quote)?;
    roundtrip::<RouteComparison>(&comparison)?;
    roundtrip::<PreparedSwap>(&prepared)?;
    // No signer, verifier or submission is invoked by this read-only harness.
    Ok(())
}
