// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_time::{SystemTime, UNIX_EPOCH};

use super::{LifiReader, wire};
use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        lifi::{
            ChainCatalogue, Limits, Origin, PreparedStep, Request, Route, Routes, Status,
            StatusQuery, StatusSelector, Step, StepView,
        },
    },
    error::{Error, ProviderError},
    transport::{HttpClient, OperationBudget},
};
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;
use std::fmt;

/// Explicit LI.FI API base (including `/v1`), qualified chains and capacities.
/// Optional credentials are caller-supplied endpoint headers; no key, URL,
/// environment or chain catalogue is loaded implicitly.
#[derive(Clone, Debug)]
pub struct LifiHttpConfig {
    http: HttpConfig,
    catalogue: ChainCatalogue,
    limits: Limits,
}
impl LifiHttpConfig {
    /// Checks the maximum two-MiB response/continuation ceiling.
    /// The optional `x-lifi-api-key` header uses normal sensitive endpoint headers.
    ///
    /// # Errors
    /// Rejects an HTTP body limit above the backend's bounded document ceiling.
    pub fn new(http: HttpConfig, catalogue: ChainCatalogue, limits: Limits) -> Result<Self, Error> {
        if http.limits().max_response_bytes() > wire::MAX_DOCUMENT_BYTES {
            return Err(Error::Configuration);
        }
        Ok(Self {
            http,
            catalogue,
            limits,
        })
    }
    /// Returns explicit endpoint/limits/provider labels with redacted Debug.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
    /// Returns every caller-qualified chain number and family.
    #[must_use]
    pub const fn catalogue(&self) -> &ChainCatalogue {
        &self.catalogue
    }
    /// Returns checked route/step/cost collection capacities.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }
}

// Binding prevents replaying a private continuation to another endpoint/key.
#[derive(Clone, Eq, PartialEq)]
struct EndpointBinding {
    url: String,
    headers: reqwest::header::HeaderMap,
}
impl EndpointBinding {
    fn new(config: &HttpConfig) -> Self {
        Self {
            url: config.endpoint().url.as_str().to_owned(),
            headers: config.endpoint().headers.clone(),
        }
    }
}

/// Immutable typed step and private exact provider continuation.
/// No Deserialize or arbitrary JSON export exists. Typed serialization omits the
/// continuation and cannot recreate a resumable HTTP handle. Debug is opaque.
#[derive(Clone)]
pub struct LifiStepHandle {
    step: Step,
    request: Request,
    origin: Origin,
    raw: Box<RawValue>,
    binding: EndpointBinding,
}
impl LifiStepHandle {
    /// Returns the selected step's immutable typed values.
    #[must_use]
    pub const fn step(&self) -> &Step {
        &self.step
    }
    /// Returns immutable original whole-transfer caller values.
    #[must_use]
    pub const fn request(&self) -> &Request {
        &self.request
    }
    /// Returns the source retrieval that created this selection.
    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }
}
impl StepView for LifiStepHandle {
    fn step(&self) -> &Step {
        self.step()
    }
    fn request(&self) -> &Request {
        self.request()
    }
    fn origin(&self) -> &Origin {
        self.origin()
    }
}
impl fmt::Debug for LifiStepHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LifiStepHandle")
            .field("step_id", &self.step.data().id)
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}
impl PartialEq for LifiStepHandle {
    fn eq(&self, other: &Self) -> bool {
        self.step == other.step
            && self.request == other.request
            && self.origin == other.origin
            && self.raw.get() == other.raw.get()
            && self.binding == other.binding
    }
}
impl Eq for LifiStepHandle {}
impl Serialize for LifiStepHandle {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct TypedView<'a> {
            step: &'a Step,
            request: &'a Request,
            origin: &'a Origin,
        }
        TypedView {
            step: &self.step,
            request: &self.request,
            origin: &self.origin,
        }
        .serialize(s)
    }
}

/// Outgoing bounded LI.FI quote/routes/preparation/status backend.
/// GET and documented no-submit POST operations retain identical encoded inputs
/// across safe read retries. One deadline covers retries, body reads and decoding.
/// On native targets, the caller supplies a Tokio runtime with networking/time enabled.
pub struct LifiClient {
    config: LifiHttpConfig,
    http: HttpClient,
    binding: EndpointBinding,
}
impl fmt::Debug for LifiClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LifiClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl LifiClient {
    /// Builds an explicitly configured outgoing client without contacting a server.
    ///
    /// # Errors
    /// Returns fixed configuration failures if the HTTP client cannot build.
    pub fn new(config: LifiHttpConfig) -> Result<Self, Error> {
        let http = HttpClient::new(&config.http)?;
        let binding = EndpointBinding::new(&config.http);
        Ok(Self {
            config,
            http,
            binding,
        })
    }
    /// Returns explicit redacted endpoint, source labels, chains and capacities.
    #[must_use]
    pub const fn config(&self) -> &LifiHttpConfig {
        &self.config
    }
    fn origin(&self, method: &str) -> Result<Origin, Error> {
        Ok(Origin {
            source: Source::new(
                self.config.http.provider_id(),
                method,
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
    fn check_request(&self, r: &Request) -> Result<(), Error> {
        for c in [r.data().from.chain(), r.data().to.chain()] {
            if self.config.catalogue.resolve(c.id())? != c {
                return Err(Error::Configuration);
            }
        }
        Ok(())
    }
    fn handle(
        &self,
        raw: Box<RawValue>,
        request: Request,
        origin: Origin,
    ) -> Result<LifiStepHandle, Error> {
        let step = wire::step(&raw, &self.config.catalogue, self.config.limits)?;
        Ok(LifiStepHandle {
            step,
            request,
            origin,
            raw,
            binding: self.binding.clone(),
        })
    }
    async fn read(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        body: Option<&[u8]>,
        budget: &OperationBudget,
    ) -> Result<Box<RawValue>, Error> {
        let response = self.http.read(path, query, body, budget).await?;
        if response.status == 404 {
            return Err(Error::UnavailableData);
        }
        wire::document(&response.into_success()?)
    }
    /// Returns a request-correlated exact-input quote with immutable continuation.
    /// The API is asked for transaction execution, with gasless/destination calls
    /// disabled. Missing preparation payload remains explicit on the typed step.
    ///
    /// # Errors
    /// Returns fixed bounded transport, provider, identity and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn get_quote(&self, request: Request) -> Result<LifiStepHandle, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.check_request(&request)?;
                let r = request.data();
                let mut fields = vec![
                    ("fromChain", r.from.chain().id().to_string()),
                    ("toChain", r.to.chain().id().to_string()),
                    ("fromToken", r.from.identifier().to_owned()),
                    ("toToken", r.to.identifier().to_owned()),
                    ("fromAmount", r.amount.raw().to_string()),
                    ("fromAddress", r.from_account.identifier().to_owned()),
                    ("slippage", r.slippage.value().canonical()),
                    ("executionType", "transaction".to_owned()),
                    ("gasless", "false".to_owned()),
                    ("allowDestinationCall", "false".to_owned()),
                    ("slippageScope", "step".to_owned()),
                ];
                if let Some(to) = &r.to_account {
                    fields.push(("toAddress", to.identifier().to_owned()));
                }
                let query: Vec<_> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
                let raw = self.read(&["quote"], &query, None, &budget).await?;
                let handle = self.handle(raw, request, self.origin("quote")?)?;
                if !handle.step.data().action.matches_request(&handle.request) {
                    return Err(Error::Provider(ProviderError::InvalidResponse));
                }
                if handle.step.data().estimate.is_none() {
                    return Err(Error::UnavailableData);
                }
                Ok(handle)
            })
            .await
    }
    /// Returns every bounded route alternative without selecting a candidate.
    /// Optional source unavailable metadata retains paths and machine error codes;
    /// free-form provider error messages are deliberately omitted.
    ///
    /// # Errors
    /// Returns fixed bounded transport, provider, identity and deadline failures.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn get_routes(&self, request: Request) -> Result<Routes<LifiStepHandle>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.check_request(&request)?;
                let body = routes_body(&request)?;
                let raw = self
                    .read(&["advanced", "routes"], &[], Some(&body), &budget)
                    .await?;
                let (alternatives, unavailable) = wire::routes(&raw, &self.config.catalogue)?;
                let origin = self.origin("advanced-routes")?;
                let mut routes = Vec::new();
                for (data, raw_steps) in alternatives {
                    let steps = raw_steps
                        .into_iter()
                        .map(|raw| self.handle(raw, request.clone(), origin.clone()))
                        .collect::<Result<Vec<_>, _>>()?;
                    routes.push(
                        Route::new(data, steps)
                            .map_err(|_| Error::Provider(ProviderError::InvalidResponse))?,
                    );
                }
                Routes::new(request, origin, routes, unavailable, self.config.limits)
                    .map_err(|_| Error::Provider(ProviderError::InvalidResponse))
            })
            .await
    }
    /// Replays the exact selected private step to the same configured authority.
    /// Fresh estimate and payload are returned beside the immutable old selection.
    /// This documented preparation POST signs/submits nothing.
    ///
    /// # Errors
    /// Rejects another endpoint/key/provider binding before any request, and
    /// rejects source rewrites of selected identities/units/settings.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn prepare_step(&self, selected: &LifiStepHandle) -> Result<PreparedStep, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                if selected.binding != self.binding
                    || selected.origin.source.provider_id() != self.config.http.provider_id()
                {
                    return Err(Error::Configuration);
                }
                self.check_request(&selected.request)?;
                // Recheck referenced chain qualification under this client's catalogue.
                let checked =
                    wire::step(&selected.raw, &self.config.catalogue, self.config.limits)?;
                if checked != selected.step {
                    return Err(Error::Configuration);
                }
                let raw = self
                    .read(
                        &["advanced", "stepTransaction"],
                        &[],
                        Some(selected.raw.get().as_bytes()),
                        &budget,
                    )
                    .await?;
                let fresh = wire::step(&raw, &self.config.catalogue, self.config.limits)?;
                PreparedStep::new(selected, fresh, self.origin("advanced-stepTransaction")?)
                    .map_err(|_| Error::Provider(ProviderError::InvalidResponse))
            })
            .await
    }
    /// Looks up a chain hash or provider transfer ID using one fixed `txHash` query.
    /// Quote/route step selection UUIDs cannot be queried independently.
    /// Optional provider chain hints are omitted: caller-qualified expected chains
    /// are validated against actual returned records, including historical caches.
    /// Source-chain refunds remain explicitly classified as refunds.
    ///
    /// # Errors
    /// Returns fixed bounded provider failures for mismatched hashes/chains.
    /// # Panics
    /// On native targets, a Tokio runtime with disabled networking/time drivers may panic.
    pub async fn get_status(&self, query: StatusQuery) -> Result<Status, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                for c in [query.from(), query.to()] {
                    if self.config.catalogue.resolve(c.id())? != c {
                        return Err(Error::Configuration);
                    }
                }
                let hash = match query.selector() {
                    StatusSelector::SendingTransaction(v)
                    | StatusSelector::ReceivingTransaction(v) => v.identifier(),
                    StatusSelector::ProviderTransfer(v) => v.as_str(),
                };
                let mut fields = vec![("txHash", hash)];
                if let Some(bridge) = query.bridge() {
                    fields.push(("bridge", bridge.as_str()));
                }
                let raw = self.read(&["status"], &fields, None, &budget).await?;
                wire::status(
                    &raw,
                    &self.config.catalogue,
                    query,
                    self.origin("status")?,
                    self.config.limits,
                )
            })
            .await
    }
}
impl LifiReader for LifiClient {
    type StepHandle = LifiStepHandle;
    async fn get_quote(&self, r: Request) -> Result<Self::StepHandle, Error> {
        Self::get_quote(self, r).await
    }
    async fn get_routes(&self, r: Request) -> Result<Routes<Self::StepHandle>, Error> {
        Self::get_routes(self, r).await
    }
    async fn prepare_step(&self, s: &Self::StepHandle) -> Result<PreparedStep, Error> {
        Self::prepare_step(self, s).await
    }
    async fn get_status(&self, q: StatusQuery) -> Result<Status, Error> {
        Self::get_status(self, q).await
    }
}
fn routes_body(request: &Request) -> Result<Vec<u8>, Error> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Body<'a> {
        from_chain_id: u64,
        to_chain_id: u64,
        from_token_address: &'a str,
        to_token_address: &'a str,
        from_amount: String,
        from_address: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        to_address: Option<&'a str>,
        options: Options,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Options {
        slippage: Box<RawValue>,
        slippage_scope: &'static str,
        allow_switch_chain: bool,
        execution_type: &'static str,
        allow_destination_call: bool,
        gasless: bool,
    }
    let r = request.data();
    let options = Options {
        slippage: RawValue::from_string(r.slippage.value().canonical())
            .map_err(|_| Error::Configuration)?,
        slippage_scope: "step",
        allow_switch_chain: r.allow_switch_chain,
        execution_type: "transaction",
        allow_destination_call: false,
        gasless: false,
    };
    serde_json::to_vec(&Body {
        from_chain_id: r.from.chain().id(),
        to_chain_id: r.to.chain().id(),
        from_token_address: r.from.identifier(),
        to_token_address: r.to.identifier(),
        from_amount: r.amount.raw().to_string(),
        from_address: r.from_account.identifier(),
        to_address: r
            .to_account
            .as_ref()
            .map(crate::domain::lifi::Account::identifier),
        options,
    })
    .map_err(|_| Error::Configuration)
}
