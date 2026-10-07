// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit, validated HTTP(S) endpoint and request configuration.
//!
//! Endpoint URLs and headers are excluded from diagnostics. RPC configuration
//! and credentials are supplied by the caller. HTTPS uses verified standard
//! platform trust, including the platform's certificate-store discovery rules.
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

use std::{fmt, time::Duration};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use url::Url;

#[cfg(feature = "evm-http")]
use crate::domain::{Asset, BlockSelector, NetworkId};
use crate::{domain::Source, error::Error};

/// An HTTP(S) endpoint with caller-supplied authentication headers.
///
/// URLs may contain explicitly supplied user information or query parameters.
/// Both the entire URL and all headers are redacted from `Debug`. The endpoint
/// cannot be serialized through this API.
#[derive(Clone)]
pub struct RpcEndpoint {
    pub(crate) url: Url,
    pub(crate) headers: HeaderMap,
}

impl RpcEndpoint {
    /// Parses an explicit HTTP(S) endpoint without silently repairing whitespace.
    ///
    /// # Errors
    ///
    /// Rejects invalid URLs, unsupported schemes, absent hosts, fragments, and
    /// whitespace/control characters anywhere in the supplied URL. Diagnostics
    /// never include the supplied endpoint.
    pub fn new(endpoint: &str) -> Result<Self, Error> {
        let explicit_scheme = endpoint.split_once("://").is_some_and(|(scheme, _)| {
            scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
        });
        if !explicit_scheme
            || endpoint.contains('\\')
            || endpoint
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(Error::Configuration);
        }
        let url = Url::parse(endpoint).map_err(|_| Error::Configuration)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || url.fragment().is_some()
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            url,
            headers: HeaderMap::new(),
        })
    }

    /// Adds or replaces an explicit request header, keeping its value sensitive.
    ///
    /// # Errors
    ///
    /// Rejects malformed names/values and reserved `host`, `content-type`,
    /// `content-length`, and `transfer-encoding` headers. These headers are
    /// determined by the validated endpoint and request body. Explicit
    /// `authorization` cannot be combined with URL user information; this avoids
    /// ambiguous authentication precedence.
    pub fn with_header(mut self, name: &str, value: &str) -> Result<Self, Error> {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| Error::Configuration)?;
        if matches!(
            name.as_str(),
            "host" | "content-type" | "content-length" | "transfer-encoding"
        ) {
            return Err(Error::Configuration);
        }
        if name.as_str() == "authorization"
            && (!self.url.username().is_empty() || self.url.password().is_some())
        {
            return Err(Error::Configuration);
        }
        let mut value = HeaderValue::from_str(value).map_err(|_| Error::Configuration)?;
        value.set_sensitive(true);
        self.headers.insert(name, value);
        Ok(self)
    }
}

impl fmt::Debug for RpcEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RpcEndpoint { url: [REDACTED], headers: [REDACTED] }")
    }
}

/// Positive bounded transport settings and an explicit safe-read retry count.
///
/// The request timeout bounds the entire operation, including retries, their
/// delays, and response-body consumption. The connect timeout independently
/// bounds each connection attempt within that total budget. The retry count
/// applies separately to each HTTP stage; all stages of one operation share
/// one request timeout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RpcLimits {
    connect_timeout: Duration,
    request_timeout: Duration,
    max_response_bytes: usize,
    max_retries: u8,
}

impl RpcLimits {
    /// Validates explicit time, response-size, and additional-attempt limits.
    ///
    /// # Errors
    ///
    /// Timeouts must be positive and at most one day, with connect timeout no
    /// larger than request timeout. Response size must be 1–16 MiB inclusive.
    /// Additional safe-read attempts must be 0–8 inclusive.
    pub fn new(
        connect_timeout: Duration,
        request_timeout: Duration,
        max_response_bytes: usize,
        max_retries: u8,
    ) -> Result<Self, Error> {
        let maximum_timeout = Duration::from_hours(24);
        if connect_timeout.is_zero()
            || request_timeout.is_zero()
            || connect_timeout > request_timeout
            || request_timeout > maximum_timeout
            || !(1..=16 * 1024 * 1024).contains(&max_response_bytes)
            || max_retries > 8
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            connect_timeout,
            request_timeout,
            max_response_bytes,
            max_retries,
        })
    }

    /// Returns the per-connection-attempt timeout.
    #[must_use]
    pub const fn connect_timeout(self) -> Duration {
        self.connect_timeout
    }

    /// Returns the total operation budget, including all retries and body reads.
    #[must_use]
    pub const fn request_timeout(self) -> Duration {
        self.request_timeout
    }

    /// Returns the maximum accepted response-body size in bytes.
    #[must_use]
    pub const fn max_response_bytes(self) -> usize {
        self.max_response_bytes
    }

    /// Returns additional safe-read attempts after each HTTP stage's initial attempt.
    ///
    /// All stages and their retries share the operation's request timeout.
    #[must_use]
    pub const fn max_retries(self) -> u8 {
        self.max_retries
    }
}

/// Explicit HTTP(S) endpoint, request limits, and non-secret provider attribution.
///
/// Configuration is independent of chain identity and performs no requests.
/// Integrations validate their own network and protocol contracts.
#[derive(Clone, Debug)]
pub struct HttpConfig {
    endpoint: RpcEndpoint,
    limits: RpcLimits,
    provider_id: String,
}

impl HttpConfig {
    /// Constructs configuration from explicit validated endpoint and limits.
    ///
    /// # Errors
    ///
    /// Rejects invalid non-secret provider attribution labels. URLs and headers
    /// remain excluded from diagnostics.
    pub fn new(
        endpoint: RpcEndpoint,
        limits: RpcLimits,
        provider_id: impl Into<String>,
    ) -> Result<Self, Error> {
        let provider_id = provider_id.into();
        let _source = Source::new(&provider_id, "http", env!("CARGO_PKG_VERSION"))?;
        Ok(Self {
            endpoint,
            limits,
            provider_id,
        })
    }

    /// Returns the endpoint whose diagnostics are always redacted.
    #[must_use]
    pub const fn endpoint(&self) -> &RpcEndpoint {
        &self.endpoint
    }

    /// Returns validated explicit transport settings.
    #[must_use]
    pub const fn limits(&self) -> RpcLimits {
        self.limits
    }

    /// Returns the caller-supplied non-secret provider attribution label.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }
}

/// Explicit EVM network, endpoint, native metadata, selector, and request limits.
///
/// Native decimals and optional symbol are caller-configured metadata. The
/// provider label is non-secret attribution, not an endpoint or credential.
#[derive(Clone, Debug)]
#[cfg(feature = "evm-http")]
pub struct EvmConfig {
    network: NetworkId,
    http: HttpConfig,
    native_asset: Asset,
    default_selector: BlockSelector,
}

#[cfg(feature = "evm-http")]
impl EvmConfig {
    /// Constructs an explicit EVM configuration with validated native metadata.
    ///
    /// # Errors
    ///
    /// Rejects invalid optional native symbols or provider attribution labels.
    /// Endpoint and transport limits must already have passed their constructors.
    pub fn new(
        network: NetworkId,
        endpoint: RpcEndpoint,
        native_decimals: u8,
        native_symbol: Option<String>,
        default_selector: BlockSelector,
        limits: RpcLimits,
        provider_id: impl Into<String>,
    ) -> Result<Self, Error> {
        let http = HttpConfig::new(endpoint, limits, provider_id)?;
        let native_asset = Asset::native(network.clone(), native_decimals, native_symbol)?;
        Ok(Self {
            network,
            http,
            native_asset,
            default_selector,
        })
    }

    /// Returns the explicitly configured expected network.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }

    /// Returns the endpoint whose diagnostics are always redacted.
    #[must_use]
    pub const fn endpoint(&self) -> &RpcEndpoint {
        self.http.endpoint()
    }

    /// Returns native asset identity and caller-configured decimal metadata.
    #[must_use]
    pub const fn native_asset(&self) -> &Asset {
        &self.native_asset
    }

    /// Returns the explicitly configured default state-read selector.
    #[must_use]
    pub const fn default_selector(&self) -> BlockSelector {
        self.default_selector
    }

    /// Returns validated explicit transport settings.
    #[must_use]
    pub const fn limits(&self) -> RpcLimits {
        self.http.limits()
    }

    /// Returns the caller-supplied non-secret provider attribution label.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        self.http.provider_id()
    }

    pub(crate) const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}
