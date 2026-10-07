// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Opt-in read-only provider qualification through the public Rust API.

#![cfg(feature = "evm-http")]

#[path = "../examples/support/inputs.rs"]
mod inputs;

use std::time::{SystemTime, UNIX_EPOCH};

use regit_web3::{
    chains::evm::EvmClient,
    domain::{Balance, BlockSelector, Finality, Observation, Operation},
    error::Error,
};

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit read-only RPC configuration and a live provider"]
async fn native_balance_live() -> Result<(), Error> {
    let inputs = inputs::from_env()?;
    let before = unix_seconds()?;
    let client = EvmClient::connect(inputs.config).await?;
    let observation = client.get_native_balance(inputs.address, None).await?;
    let after = unix_seconds()?;
    let context = observation.context();
    let value = observation.value();

    assert_eq!(context.schema_version(), 1);
    assert_eq!(context.operation(), Operation::NativeBalance);
    assert_eq!(context.network(), client.config().network());
    assert_eq!(context.network().chain_id(), client.chain_id());
    assert_eq!(
        *context.requested_selector(),
        client.config().default_selector()
    );
    match context.requested_selector() {
        BlockSelector::Number(number) => assert_eq!(*number, context.block().number()),
        BlockSelector::Hash(hash) => assert_eq!(hash, context.block().hash()),
        BlockSelector::Latest | BlockSelector::Safe | BlockSelector::Finalized => {}
    }
    assert_eq!(value.address(), &inputs.address);
    assert_eq!(value.asset(), client.config().native_asset());
    assert_eq!(value.asset().identity().chain_id(), client.chain_id());
    assert_eq!(value.amount().decimals(), Some(value.asset().decimals()));
    assert_eq!(
        context.source().provider_id(),
        client.config().provider_id()
    );
    assert_eq!(context.source().method(), "eth_getBalance");
    assert_eq!(
        context.source().integration_version(),
        env!("CARGO_PKG_VERSION")
    );
    assert!((before..=after).contains(&context.retrieved_at().unix_seconds()));
    assert_eq!(context.finality(), Finality::Unknown);
    assert_eq!(context.confirmations(), None);

    let json = serde_json::to_string(&observation).map_err(|_| Error::Configuration)?;
    let decoded: Observation<Balance> =
        serde_json::from_str(&json).map_err(|_| Error::Configuration)?;
    assert_eq!(decoded, observation);
    println!("{json}");
    Ok(())
}

fn unix_seconds() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| Error::Configuration)
}
