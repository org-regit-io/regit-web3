// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Public validation of caller-supplied EVM transport configuration.

#![cfg(feature = "evm")]

use std::time::Duration;

use regit_web3::{
    config::{EvmConfig, RpcEndpoint, RpcLimits},
    domain::{BlockSelector, ChainId, NetworkId},
    error::Error,
};

fn limits() -> Result<RpcLimits, Error> {
    RpcLimits::new(Duration::from_secs(1), Duration::from_secs(2), 4096, 0)
}

#[test]
fn endpoints_require_http_hosts_and_reject_ambiguous_input() {
    for endpoint in [
        "",
        "not a URL",
        "ftp://example.invalid",
        "file:///tmp/rpc",
        "https://",
        "http://127.0.0.1:9000/rpc#fragment",
        " https://example.invalid/rpc",
        "https://example.invalid/rpc ",
        "https://example.invalid/\nrpc",
    ] {
        assert!(
            RpcEndpoint::new(endpoint).is_err(),
            "accepted invalid endpoint"
        );
    }
    for endpoint in [
        "http://127.0.0.1:9000/rpc",
        "https://example.invalid/rpc",
        "https://fixture-user:fixture-password@example.invalid/rpc?key=fixture-key",
    ] {
        assert!(RpcEndpoint::new(endpoint).is_ok());
    }
}

#[test]
fn limits_enforce_positive_bounded_timeouts_sizes_and_additional_retries() {
    let day = Duration::from_hours(24);
    let valid = [
        (Duration::from_nanos(1), Duration::from_nanos(1), 1, 0),
        (day, day, 16_777_216, 8),
    ];
    for (connect, request, bytes, retries) in valid {
        assert!(RpcLimits::new(connect, request, bytes, retries).is_ok());
    }
    for (connect, request, bytes, retries) in [
        (Duration::ZERO, Duration::from_secs(1), 1, 0),
        (Duration::from_secs(1), Duration::ZERO, 1, 0),
        (Duration::from_secs(2), Duration::from_secs(1), 1, 0),
        (
            day + Duration::from_nanos(1),
            day + Duration::from_nanos(1),
            1,
            0,
        ),
        (Duration::from_secs(1), day + Duration::from_nanos(1), 1, 0),
        (Duration::from_secs(1), Duration::from_secs(1), 0, 0),
        (
            Duration::from_secs(1),
            Duration::from_secs(1),
            16_777_217,
            0,
        ),
        (Duration::from_secs(1), Duration::from_secs(1), 1, 9),
    ] {
        assert!(RpcLimits::new(connect, request, bytes, retries).is_err());
    }
}

#[test]
fn headers_allow_credentials_but_reject_protocol_overrides_and_injection() {
    for (name, value) in [
        ("host", "example.invalid"),
        ("Content-Type", "text/plain"),
        ("content-length", "0"),
        ("Transfer-Encoding", "chunked"),
        ("invalid header", "value"),
        ("x-api-key", "fixture-key\r\nInjected: value"),
    ] {
        assert!(
            RpcEndpoint::new("https://example.invalid/rpc")
                .unwrap()
                .with_header(name, value)
                .is_err()
        );
    }
    assert!(
        RpcEndpoint::new("https://example.invalid/rpc")
            .unwrap()
            .with_header("Authorization", "Bearer fixture-header-token")
            .is_ok()
    );
    assert!(
        RpcEndpoint::new("https://fixture-user:fixture-password@example.invalid/rpc")
            .unwrap()
            .with_header("Authorization", "Bearer fixture-header-token")
            .is_err()
    );
}

#[test]
fn native_configuration_preserves_explicit_metadata_and_default_selector() -> Result<(), Error> {
    let network = NetworkId::new(
        ChainId::from_decimal("9007199254740993")?,
        "fixture-network",
    )?;
    let config = EvmConfig::new(
        network.clone(),
        RpcEndpoint::new("https://example.invalid/rpc")?,
        0,
        None,
        BlockSelector::Finalized,
        limits()?,
        "fixture-provider",
    )?;
    assert_eq!(config.network(), &network);
    assert_eq!(config.native_asset().network(), &network);
    assert_eq!(config.native_asset().decimals(), 0);
    assert_eq!(config.native_asset().symbol(), None);
    assert_eq!(config.default_selector(), BlockSelector::Finalized);
    assert_eq!(config.provider_id(), "fixture-provider");
    Ok(())
}

#[test]
fn configuration_rejects_malformed_metadata_and_provider_labels() -> Result<(), Error> {
    for (symbol, provider) in [
        (Some(String::new()), "fixture"),
        (Some("symbol with spaces".to_owned()), "fixture"),
        (Some("https://example.invalid".to_owned()), "fixture"),
        (None, ""),
        (None, "https://user:password@example.invalid"),
        (None, "provider\nname"),
    ] {
        assert!(
            EvmConfig::new(
                NetworkId::new(ChainId::from(1), "fixture-network")?,
                RpcEndpoint::new("https://example.invalid/rpc")?,
                18,
                symbol,
                BlockSelector::Latest,
                limits()?,
                provider,
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn configuration_debug_and_failures_do_not_disclose_endpoint_or_headers() -> Result<(), Error> {
    let endpoint = RpcEndpoint::new(
        "https://fixture-user:fixture-password@example.invalid/fixture-path?key=fixture-query-token",
    )?
    .with_header("x-api-key", "fixture-header-token")?;
    let config = EvmConfig::new(
        NetworkId::new(ChainId::from(1), "fixture-network")?,
        endpoint,
        18,
        None,
        BlockSelector::Latest,
        limits()?,
        "fixture-provider",
    )?;
    let debug = format!("{config:?}");
    for sensitive in [
        "fixture-user",
        "fixture-password",
        "fixture-path",
        "fixture-query-token",
        "fixture-header-token",
    ] {
        assert!(!debug.contains(sensitive));
    }
    let error =
        RpcEndpoint::new("https://fixture-password@example.invalid/rpc#fixture-token").unwrap_err();
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        serde_json::to_string(&error).unwrap(),
    ] {
        assert!(!rendered.contains("fixture-password"));
        assert!(!rendered.contains("fixture-token"));
    }
    Ok(())
}
