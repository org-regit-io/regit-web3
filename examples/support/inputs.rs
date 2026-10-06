// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit environment inputs owned by the native-balance example.

use std::{env, time::Duration};

use regit_web3::{
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::{Address, BlockHash, BlockSelector, ChainId, NetworkId},
    error::Error,
};

pub(crate) struct Inputs {
    pub(crate) config: EvmConfig,
    pub(crate) address: Address,
}

pub(crate) fn from_env() -> Result<Inputs, Error> {
    let endpoint = RpcEndpoint::new(&required("REGIT_WEB3_RPC_URL")?)?;
    let chain_id = ChainId::from_decimal(&required("REGIT_WEB3_CHAIN_ID")?)?;
    let network = NetworkId::new(chain_id, required("REGIT_WEB3_NETWORK_ALIAS")?)?;
    let address = Address::parse(&required("REGIT_WEB3_ADDRESS")?)?;
    let decimals = canonical_decimal(&required("REGIT_WEB3_NATIVE_DECIMALS")?)?
        .parse::<u8>()
        .map_err(|_| Error::Configuration)?;
    let selector = selector(&required("REGIT_WEB3_BLOCK_SELECTOR")?)?;
    let symbol = match env::var("REGIT_WEB3_NATIVE_SYMBOL") {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => return Err(Error::Configuration),
    };
    // These are explicit example settings, never implicit library defaults.
    let limits = RpcLimits::new(
        Duration::from_secs(5),
        Duration::from_secs(15),
        1024 * 1024,
        2,
    )?;
    let config = EvmConfig::new(
        network,
        endpoint,
        decimals,
        symbol,
        selector,
        limits,
        required("REGIT_WEB3_PROVIDER_ID")?,
    )?;
    Ok(Inputs { config, address })
}

fn required(name: &str) -> Result<String, Error> {
    env::var(name).map_err(|_| Error::Configuration)
}

fn canonical_decimal(value: &str) -> Result<&str, Error> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(Error::Configuration);
    }
    Ok(value)
}

fn selector(value: &str) -> Result<BlockSelector, Error> {
    match value {
        "latest" => Ok(BlockSelector::Latest),
        "safe" => Ok(BlockSelector::Safe),
        "finalized" => Ok(BlockSelector::Finalized),
        _ => {
            if let Some(number) = value.strip_prefix("number:") {
                return canonical_decimal(number)?
                    .parse::<u64>()
                    .map(BlockSelector::Number)
                    .map_err(|_| Error::Configuration);
            }
            if let Some(hash) = value.strip_prefix("hash:") {
                return BlockHash::parse(hash).map(BlockSelector::Hash);
            }
            Err(Error::Configuration)
        }
    }
}
