// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit, validated transport limits and optional HTTP(S) configuration.
//!
//! Endpoint URLs and headers are excluded from diagnostics. RPC configuration
//! and credentials are supplied by the caller. Native HTTPS uses verified
//! standard platform trust, including certificate-store discovery. On
//! `wasm32-unknown-unknown`, the JavaScript host owns TLS and CORS policy; outgoing
//! backends require Fetch, streams, abort signals, performance clocks and timers.
//! Redirects and ambient cookies are disabled; explicit headers and endpoint
//! credentials remain caller-owned. A browser backend rejects URL user information
//! and browser-controlled headers instead of silently dropping them.
//!
//! ```
//! # #[cfg(feature = "http")]
//! # fn main() -> Result<(), regit_web3::error::Error> {
//! use std::time::Duration;
//! use regit_web3::config::{HttpConfig, RpcEndpoint, RpcLimits};
//!
//! let endpoint = RpcEndpoint::new("https://api.example.invalid/v1")?;
//! let limits = RpcLimits::new(
//!     Duration::from_secs(3), Duration::from_secs(10), 1024 * 1024, 1,
//! )?;
//! let config = HttpConfig::new(endpoint, limits, "example-provider")?;
//! assert_eq!(config.provider_id(), "example-provider");
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "http"))]
//! # fn main() {}
//! ```
//!
//! ```
//! # #[cfg(feature = "evm-http")]
//! # fn main() -> Result<(), regit_web3::error::Error> {
//! use std::time::Duration;
//! use regit_web3::{
//!     config::{EvmConfig, RpcEndpoint, RpcLimits},
//!     domain::{BlockSelector, ChainId, NetworkId},
//! };
//!
//! let network = NetworkId::new(ChainId::from(1), "ethereum")?;
//! let endpoint = RpcEndpoint::new("https://rpc.example.invalid")?;
//! let limits = RpcLimits::new(
//!     Duration::from_secs(3), Duration::from_secs(10), 1024 * 1024, 1,
//! )?;
//! let config = EvmConfig::new(
//!     network, endpoint, 18, Some("ETH".to_owned()),
//!     BlockSelector::Latest, limits, "example-provider",
//! )?;
//! assert_eq!(config.native_asset().decimals(), 18);
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "evm-http"))]
//! # fn main() {}
//! ```

mod limits;

#[cfg(feature = "http")]
mod http;

#[cfg(feature = "evm-http")]
pub use http::EvmConfig;
#[cfg(feature = "http")]
pub use http::{HttpConfig, RpcEndpoint};
pub use limits::RpcLimits;
