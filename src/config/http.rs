// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use url::Url;

#[cfg(feature = "evm-http")]
use crate::domain::{Asset, BlockSelector, NetworkId};
use crate::{domain::Source, error::Error};

use super::RpcLimits;

/// An HTTP(S) endpoint with caller-supplied authentication headers.
///
/// URLs may contain explicitly supplied user information or query parameters.
/// Both the entire URL and all headers are redacted from `Debug`. The endpoint
/// cannot be serialized through this API.
/// Browser backends reject URL user information because Fetch cannot send it.
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
    /// Browser backends additionally reject host-controlled or silently removed
    /// headers, including cookies, referrer, origin and user agent, before dispatch.
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

/// Explicit HTTP(S) endpoint, request limits, and non-secret provider attribution.
///
/// Configuration is independent of chain identity and performs no requests.
/// Integrations validate their own network and protocol contracts.
/// Native outgoing backends use the caller's Tokio runtime. Browser backends
/// use the JavaScript host's Fetch and timers without a Tokio runtime. Browser
/// requests omit ambient credentials, referrer and caches, reject redirects,
/// and require the endpoint's CORS policy to permit the explicit request.
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
