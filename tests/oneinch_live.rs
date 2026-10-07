// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Opt-in authenticated Classic Swap v6.1 Rust quote/catalogue/spender/preparation.
//! Caller-owned harness credentials and public inputs never enter library defaults.
//! All operations are GET; no approval, signed transaction or execution is performed.
#![cfg(feature = "oneinch-http")]

#[path = "support/live.rs"]
mod live;
use regit_web3::{
    config::HttpConfig,
    domain::{
        Address, ChainId, NetworkId,
        evm::Quantity,
        oneinch::{
            Asset, AssetKind, Context, Expiry, Limits, Method, PreparedSwap, QuoteRequest,
            QuoteSettings, ReturnPolicy, SlippageBps, SwapRequest, SwapSettings,
        },
    },
    error::Error,
    protocols::oneinch::{OneinchClient, OneinchHttpConfig},
    wallets::{HandoffId, HandoffRequest, PreparedRequest},
};
fn input(name: &str) -> Result<String, Error> {
    live::required(&format!("REGIT_WEB3_ONEINCH_{name}"))
}
fn account(name: &str) -> Result<Address, Error> {
    Address::parse(&input(name)?)
}
fn quantity(name: &str) -> Result<Quantity, Error> {
    Quantity::from_decimal(&input(name)?)
}
fn asset(chain: ChainId, name: &str) -> Result<Asset, Error> {
    let text = input(name)?;
    Asset::new(
        chain,
        if text == "native" {
            AssetKind::Native
        } else {
            AssetKind::Erc20(Address::parse(&text)?)
        },
    )
}
fn check(
    context: &Context,
    client: &OneinchClient,
    method: Method,
    before: u64,
) -> Result<(), Error> {
    assert_eq!(&context.network, client.config().network());
    assert_eq!(context.method, method);
    assert_eq!(
        context.source.provider_id(),
        client.config().http_config().provider_id()
    );
    assert_eq!(
        context.source.integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=live::unix_seconds()?).contains(&context.retrieved_at.unix_seconds()));
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
#[ignore = "requires explicit caller-owned Bearer token/API base/network/assets/amount/actor/router/spender/value/floor/slippage inputs; GET only"]
async fn classic_swap_four_methods_live_without_execution() -> Result<(), Box<dyn std::error::Error>>
{
    let http = live::http_config("REGIT_WEB3_ONEINCH")?;
    let http = HttpConfig::new(
        http.endpoint().clone().with_header(
            "Authorization",
            &format!("Bearer {}", input("BEARER_TOKEN")?),
        )?,
        http.limits(),
        http.provider_id(),
    )?;
    let chain = ChainId::from_decimal(&input("CHAIN_ID")?)?;
    let network = NetworkId::new(chain, input("NETWORK_ALIAS")?)?;
    let request = QuoteRequest::new(
        network.clone(),
        asset(chain, "SOURCE_ASSET")?,
        asset(chain, "DESTINATION_ASSET")?,
        quantity("AMOUNT")?,
        QuoteSettings::provider_defaults(),
    )?;
    let client = OneinchClient::new(OneinchHttpConfig::new(
        http,
        network,
        Limits::new(4096, 512)?,
    )?)?;
    let before = live::unix_seconds()?;
    let selected_quote = client.quote_exact_input(request.clone()).await?;
    check(selected_quote.context(), &client, Method::Quote, before)?;
    assert_eq!(selected_quote.request(), &request);
    assert!(!selected_quote.data().destination_amount.value().is_zero());
    assert_eq!(selected_quote.data().expiry, Expiry::Unreported);
    roundtrip(&selected_quote)?;
    live::print_and_roundtrip(selected_quote.context())?;
    println!(
        "quote raw_input={} raw_output={} route_items={} gas_suggestion={:?}",
        selected_quote.request().amount().value(),
        selected_quote.data().destination_amount.value(),
        selected_quote.data().routes.item_count(),
        selected_quote.data().estimated_gas
    );
    let before = live::unix_seconds()?;
    let sources = client.get_liquidity_sources().await?;
    check(sources.context(), &client, Method::LiquiditySources, before)?;
    assert!(!sources.items().is_empty());
    roundtrip(&sources)?;
    println!("liquidity_sources={}", sources.items().len());
    let before = live::unix_seconds()?;
    let spender = client.get_spender().await?;
    check(spender.context(), &client, Method::Spender, before)?;
    assert_eq!(spender.address(), account("SPENDER")?);
    roundtrip(&spender)?;
    live::print_and_roundtrip(&spender)?;
    let settings = SwapSettings {
        from: account("FROM")?,
        origin: account("ORIGIN")?,
        recipient: account("RECIPIENT")?,
        output_floor: quantity("OUTPUT_FLOOR")?,
        return_policy: ReturnPolicy::Slippage(SlippageBps::new(
            input("SLIPPAGE_BPS")?
                .parse()
                .map_err(|_| Error::Configuration)?,
        )?),
        router: account("ROUTER")?,
        spender: account("SPENDER")?,
        native_value: quantity("NATIVE_VALUE")?,
        disable_estimate: input("DISABLE_ESTIMATE")?
            .parse()
            .map_err(|_| Error::Configuration)?,
    };
    let swap_request = SwapRequest::new(selected_quote.clone(), settings)?;
    let before = live::unix_seconds()?;
    let preparation = client.prepare_swap(swap_request.clone()).await?;
    check(
        preparation.fresh_quote().context(),
        &client,
        Method::Swap,
        before,
    )?;
    assert_eq!(preparation.request(), &swap_request);
    assert_eq!(preparation.request().selection(), &selected_quote);
    assert!(
        preparation.fresh_quote().data().destination_amount.value()
            >= swap_request.settings().output_floor.value()
    );
    roundtrip(&preparation)?;
    let handoff = HandoffRequest::new(
        HandoffId::new("live-read-only-review")?,
        PreparedRequest::new(preparation.clone())?,
    );
    roundtrip::<HandoffRequest<PreparedSwap>>(&handoff)?;
    println!(
        "fresh_output={} route_items={} payload_bytes={} source_gas={} source_gas_price_wei={}",
        preparation.fresh_quote().data().destination_amount.value(),
        preparation.fresh_quote().data().routes.item_count(),
        preparation.transaction().data().data.bytes().len(),
        preparation.transaction().data().gas.value(),
        preparation.transaction().data().gas_price.value()
    );
    live::print_and_roundtrip(preparation.fresh_quote().context())?;
    Ok(())
}
