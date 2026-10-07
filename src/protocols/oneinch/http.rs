// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{ClassicSwapReader, wire};
use crate::{
    config::HttpConfig,
    domain::{
        NetworkId, Source, Timestamp,
        oneinch::{
            Context, Limits, LiquiditySources, Method, PreparedSwap, Quote, QuoteRequest,
            ReturnPolicy, Spender, SwapRequest,
        },
    },
    error::Error,
    transport::{HttpClient, OperationBudget},
};
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

/// Explicit Classic Swap API base, caller-owned Bearer header, EVM network and bounds.
/// The base is the swap API prefix (normally `/swap/`); this backend appends literal
/// `v6.1`, the supplied chain ID and documented operation path. No ambient configuration exists.
#[derive(Clone, Debug)]
pub struct OneinchHttpConfig {
    http: HttpConfig,
    network: NetworkId,
    limits: Limits,
}
impl OneinchHttpConfig {
    /// Checks the two-MiB body ceiling and explicit Bearer API key/OAuth access token.
    /// Credentials are already-held caller header values; this library does not acquire or refresh them.
    /// # Errors
    /// Rejects absent/empty/invalid Bearer authorization, URL query parameters,
    /// excessive body bounds or zero chain ID. API parameters belong to the typed request.
    pub fn new(http: HttpConfig, network: NetworkId, limits: Limits) -> Result<Self, Error> {
        let auth = http
            .endpoint()
            .headers
            .get(reqwest::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());
        if http.limits().max_response_bytes() > wire::MAX_DOCUMENT_BYTES
            || http.endpoint().url.query().is_some()
            || network.chain_id().value().is_zero()
            || !auth.is_some_and(|v| {
                v.strip_prefix("Bearer ").is_some_and(|token| {
                    !token.is_empty() && token.bytes().all(|b| b.is_ascii_graphic())
                })
            })
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            http,
            network,
            limits,
        })
    }
    /// Returns caller endpoint, redacted headers and exact operation limits.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
    /// Returns caller-qualified EVM network; the API does not attest genesis or a block.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns explicit collection bounds; excess results fail whole.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }
}
/// Optional outgoing Classic Swap v6.1 backend. All operations use GET and never execute a swap.
/// One total deadline covers all source reads, safe retries, body transfer and typed decoding.
/// A Tokio runtime with time/network drivers is supplied by the caller.
pub struct OneinchClient {
    config: OneinchHttpConfig,
    http: HttpClient,
}
impl fmt::Debug for OneinchClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OneinchClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl OneinchClient {
    /// Builds the explicit HTTP client without contacting the source.
    /// # Errors
    /// Returns a fixed configuration failure if the HTTP client cannot build.
    pub fn new(config: OneinchHttpConfig) -> Result<Self, Error> {
        let http = HttpClient::new(&config.http)?;
        Ok(Self { config, http })
    }
    /// Returns explicit redacted configuration.
    #[must_use]
    pub const fn config(&self) -> &OneinchHttpConfig {
        &self.config
    }
    fn context(&self, method: Method) -> Result<Context, Error> {
        let tag = match method {
            Method::Quote => "classic-v6.1-quote",
            Method::Swap => "classic-v6.1-swap",
            Method::LiquiditySources => "classic-v6.1-liquidity-sources",
            Method::Spender => "classic-v6.1-spender",
        };
        Ok(Context {
            network: self.config.network.clone(),
            method,
            source: Source::new(
                self.config.http.provider_id(),
                tag,
                env!("CARGO_PKG_VERSION"),
            )?,
            retrieved_at: Timestamp::from_unix_seconds(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| Error::Configuration)?
                    .as_secs(),
            ),
        })
    }
    fn check(&self, r: &QuoteRequest) -> Result<(), Error> {
        if r.network() != &self.config.network {
            return Err(Error::Configuration);
        }
        Ok(())
    }
    async fn read(
        &self,
        operation: &[&str],
        query: &[(&str, &str)],
        budget: &OperationBudget,
    ) -> Result<Vec<u8>, Error> {
        let chain = self.config.network.chain_id().value().to_string();
        let mut path = vec!["v6.1", chain.as_str()];
        path.extend_from_slice(operation);
        self.http
            .read(&path, query, None, budget)
            .await?
            .into_success()
    }
    /// Retrieves the exact-input quote and actual bounded v6.1 graph.
    /// All token/graph/gas include flags are explicitly requested.
    /// # Errors
    /// Returns typed configuration, provider, body, identity or total-deadline failures.
    /// # Panics
    /// A Tokio runtime with disabled networking/time drivers may panic.
    pub async fn quote_exact_input(&self, request: QuoteRequest) -> Result<Quote, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.check(&request)?;
                let fields = quote_query(&request);
                let query = borrow(&fields);
                let bytes = self.read(&["quote"], &query, &budget).await?;
                wire::quote(
                    &bytes,
                    request,
                    self.context(Method::Quote)?,
                    self.config.limits,
                )
            })
            .await
    }
    /// Retrieves the complete source liquidity catalogue within explicit item/body bounds.
    /// # Errors
    /// Rejects duplicate IDs, malformed/oversize results and transport/deadline failures.
    /// # Panics
    /// A Tokio runtime with disabled networking/time drivers may panic.
    pub async fn get_liquidity_sources(&self) -> Result<LiquiditySources, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                let bytes = self.read(&["liquidity-sources"], &[], &budget).await?;
                wire::sources(
                    &bytes,
                    self.context(Method::LiquiditySources)?,
                    self.config.limits,
                )
            })
            .await
    }
    async fn spender(&self, budget: &OperationBudget) -> Result<Spender, Error> {
        let bytes = self.read(&["approve", "spender"], &[], budget).await?;
        wire::spender(&bytes, self.context(Method::Spender)?)
    }
    /// Reads the source spender identity without constructing or sending an approval.
    /// # Errors
    /// Returns fixed malformed/provider/body/deadline failures.
    /// # Panics
    /// A Tokio runtime with disabled networking/time drivers may panic.
    pub async fn get_spender(&self) -> Result<Spender, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget.run(self.spender(&budget)).await
    }
    /// Reads spender and fresh swap payload under one deadline, then creates a new review.
    /// The selected quote remains unchanged. GET swap may choose a different route.
    /// Source spender/transaction reads are separate observations, without atomicity.
    /// Opaque calldata does not prove recipient, origin, spender or output-floor semantics.
    /// # Errors
    /// Rejects source conflicts, unsupported fee-on-transfer/access-list profile and provider limits.
    /// # Panics
    /// A Tokio runtime with disabled networking/time drivers may panic.
    pub async fn prepare_swap(&self, request: SwapRequest) -> Result<PreparedSwap, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.check(request.selection().request())?;
                let spender = self.spender(&budget).await?;
                if spender.address() != request.settings().spender {
                    return Err(wire::invalid());
                }
                let mut fields = quote_query(request.selection().request());
                let s = request.settings();
                fields.extend([
                    ("from", s.from.to_string()),
                    ("origin", s.origin.to_string()),
                    ("receiver", s.recipient.to_string()),
                    ("disableEstimate", s.disable_estimate.to_string()),
                    ("allowPartialFill", "false".into()),
                    ("compatibility", "false".into()),
                    ("usePatching", "false".into()),
                    ("usePermit2", "false".into()),
                    ("forceApprove", "false".into()),
                    ("createAccessList", "false".into()),
                ]);
                match s.return_policy {
                    ReturnPolicy::Minimum(q) => fields.push(("minReturn", q.value().to_string())),
                    ReturnPolicy::Slippage(p) => fields.push(("slippage", p.percent())),
                }
                let query = borrow(&fields);
                let bytes = self.read(&["swap"], &query, &budget).await?;
                let (quote, tx) = wire::swap(
                    &bytes,
                    request.selection().request().clone(),
                    self.context(Method::Swap)?,
                    self.config.limits,
                )?;
                PreparedSwap::new(request, quote, tx, spender).map_err(|e| {
                    if e == Error::UnsupportedCapability {
                        e
                    } else {
                        wire::invalid()
                    }
                })
            })
            .await
    }
}
fn quote_query(r: &QuoteRequest) -> Vec<(&'static str, String)> {
    let mut fields = vec![
        ("src", r.source().provider_address().to_string()),
        ("dst", r.destination().provider_address().to_string()),
        ("amount", r.amount().value().to_string()),
        ("includeTokensInfo", "true".into()),
        ("includeProtocols", "true".into()),
        ("includeGas", "true".into()),
    ];
    let s = r.settings();
    if !s.protocols.is_empty() {
        fields.push((
            "protocols",
            s.protocols
                .iter()
                .map(crate::domain::oneinch::ProtocolId::as_str)
                .collect::<Vec<_>>()
                .join(","),
        ));
    }
    if !s.excluded_protocols.is_empty() {
        fields.push((
            "excludedProtocols",
            s.excluded_protocols
                .iter()
                .map(crate::domain::oneinch::ProtocolId::as_str)
                .collect::<Vec<_>>()
                .join(","),
        ));
    }
    if !s.connector_tokens.is_empty() {
        fields.push((
            "connectorTokens",
            s.connector_tokens
                .iter()
                .map(|a| a.provider_address().to_string())
                .collect::<Vec<_>>()
                .join(","),
        ));
    }
    if let Some(v) = s.gas_price {
        fields.push(("gasPrice", v.value().to_string()));
    }
    if let Some(v) = s.complexity_level {
        fields.push(("complexityLevel", v.to_string()));
    }
    if let Some(v) = s.parts {
        fields.push(("parts", v.to_string()));
    }
    if let Some(v) = s.main_route_parts {
        fields.push(("mainRouteParts", v.to_string()));
    }
    if let Some(v) = s.gas_limit {
        fields.push(("gasLimit", v.to_string()));
    }
    fields
}
fn borrow<'a>(fields: &'a [(&'static str, String)]) -> Vec<(&'a str, &'a str)> {
    fields.iter().map(|(k, v)| (*k, v.as_str())).collect()
}
impl ClassicSwapReader for OneinchClient {
    async fn quote_exact_input(&self, request: QuoteRequest) -> Result<Quote, Error> {
        Self::quote_exact_input(self, request).await
    }
    async fn get_liquidity_sources(&self) -> Result<LiquiditySources, Error> {
        Self::get_liquidity_sources(self).await
    }
    async fn get_spender(&self) -> Result<Spender, Error> {
        Self::get_spender(self).await
    }
    async fn prepare_swap(&self, request: SwapRequest) -> Result<PreparedSwap, Error> {
        Self::prepare_swap(self, request).await
    }
}
