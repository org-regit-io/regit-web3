// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit environment inputs and output checks owned by ignored live tests.

use std::{
    env,
    fmt::Debug,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use regit_web3::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    error::Error,
};
use serde::{Serialize, de::DeserializeOwned};

pub(super) fn required(name: &str) -> Result<String, Error> {
    env::var(name).map_err(|_| Error::Configuration)
}

pub(super) fn http_config(prefix: &str) -> Result<HttpConfig, Error> {
    HttpConfig::new(
        RpcEndpoint::new(&required(&format!("{prefix}_URL"))?)?,
        // These are test-owned limits, never implicit library configuration.
        RpcLimits::new(
            Duration::from_secs(5),
            Duration::from_secs(20),
            2 * 1024 * 1024,
            2,
        )?,
        required(&format!("{prefix}_PROVIDER_ID"))?,
    )
}

pub(super) fn unix_seconds() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| Error::Configuration)
}

pub(super) fn print_and_roundtrip<T>(value: &T) -> Result<(), serde_json::Error>
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let json = serde_json::to_string(value)?;
    let decoded: T = serde_json::from_str(&json)?;
    assert_eq!(&decoded, value);
    println!("{json}");
    Ok(())
}
