// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Opt-in read-only provider qualification through the public Rust API.

#![cfg(feature = "evm-http")]

#[path = "../examples/support/inputs.rs"]
mod inputs;

use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

use regit_web3::{
    chains::evm::EvmClient,
    domain::evm::{
        Address, Erc20Allowance, Erc20Balance, Erc20Metadata, OperationObservation, OperationValue,
        ReadState, ReceiptLookup, TransactionId, TransactionLookup, TransactionStatus,
    },
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

/// Exercises the new read methods with explicit token/spender/transaction inputs.
/// The harness owns environment reads; no signing or submission is performed.
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit EVM token/spender/transaction inputs and a read-only provider"]
async fn erc20_and_transactions_live() -> Result<(), Error> {
    let inputs = inputs::from_env()?;
    let contract = Address::parse(&required("REGIT_WEB3_ERC20_CONTRACT")?)?;
    let spender = Address::parse(&required("REGIT_WEB3_ERC20_SPENDER")?)?;
    let transaction_id = TransactionId::parse(&required("REGIT_WEB3_TRANSACTION_ID")?)?;
    let before = unix_seconds()?;
    let client = EvmClient::connect(inputs.config).await?;
    let native = client.get_native_balance(inputs.address, None).await?;
    let block = *native.context().block();
    let selector = Some(BlockSelector::Hash(*block.hash()));
    let balance = client
        .get_erc20_balance(contract, inputs.address, selector)
        .await?;
    let allowance = client
        .get_erc20_allowance(contract, inputs.address, spender, selector)
        .await?;
    let metadata = client.get_erc20_metadata(contract, selector).await?;
    let transaction = client.get_transaction(transaction_id).await?;
    let receipt = client.get_receipt(transaction_id).await?;
    let status = client.get_transaction_status(transaction_id).await?;
    assert_eq!(balance.value().contract(), contract);
    assert_eq!(balance.value().owner(), inputs.address);
    assert_eq!(balance.value().amount().decimals(), None);
    assert_eq!(allowance.value().contract(), contract);
    assert_eq!(allowance.value().owner(), inputs.address);
    assert_eq!(allowance.value().spender(), spender);
    assert_eq!(metadata.value().contract(), contract);
    for context in [balance.context(), allowance.context(), metadata.context()] {
        assert_eq!(
            context.state(),
            ReadState::CanonicalHash {
                requested_selector: BlockSelector::Hash(*block.hash()),
                block
            }
        );
    }
    assert_eq!(transaction.value().query(), transaction_id);
    assert_eq!(receipt.value().query(), transaction_id);
    assert_eq!(status.value().query(), transaction_id);
    assert!(transaction.value().transaction().is_some());
    assert!(receipt.value().receipt().is_some());
    assert_eq!(status.value().transaction(), transaction.value());
    assert_eq!(status.value().receipt(), receipt.value());
    let after = unix_seconds()?;
    for context in [
        balance.context(),
        allowance.context(),
        metadata.context(),
        transaction.context(),
        receipt.context(),
        status.context(),
    ] {
        assert_eq!(context.network().chain_id(), client.chain_id());
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
    roundtrip::<Erc20Balance>(&balance)?;
    roundtrip::<Erc20Allowance>(&allowance)?;
    roundtrip::<Erc20Metadata>(&metadata)?;
    roundtrip::<TransactionLookup>(&transaction)?;
    roundtrip::<ReceiptLookup>(&receipt)?;
    roundtrip::<TransactionStatus>(&status)?;
    Ok(())
}
fn required(name: &str) -> Result<String, Error> {
    env::var(name).map_err(|_| Error::Configuration)
}
fn roundtrip<T>(value: &OperationObservation<T>) -> Result<(), Error>
where
    T: OperationValue
        + serde::Serialize
        + serde::de::DeserializeOwned
        + PartialEq
        + std::fmt::Debug,
{
    let json = serde_json::to_string(value).map_err(|_| Error::Configuration)?;
    let decoded: OperationObservation<T> =
        serde_json::from_str(&json).map_err(|_| Error::Configuration)?;
    assert_eq!(&decoded, value);
    println!("{json}");
    Ok(())
}

fn unix_seconds() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| Error::Configuration)
}
