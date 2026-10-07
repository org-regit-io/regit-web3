// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::{Asset, BlockSelector, NetworkId},
    error::Error,
};

use super::{HttpConfig, RpcEndpoint, RpcLimits};

/// Explicit EVM network, endpoint, native metadata, selector, and request limits.
///
/// Native decimals and optional symbol are caller-configured metadata. The
/// provider label is non-secret attribution, not an endpoint or credential.
#[derive(Clone, Debug)]
pub struct EvmConfig {
    network: NetworkId,
    http: HttpConfig,
    native_asset: Asset,
    default_selector: BlockSelector,
}

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
