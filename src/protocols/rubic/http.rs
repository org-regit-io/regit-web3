// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::wire::Switch;
use super::{RubicReader, wire};
use crate::{
    config::HttpConfig,
    domain::{
        Address, Source, Timestamp,
        rubic::{
            Account, Catalogue, Chains, Identifier, Limits, Observation, PreparationRequest,
            PreparedSwap, Quote, QuoteRequest, Routes, Status, StatusQuery,
        },
    },
    error::Error,
    transport::{HttpClient, OperationBudget},
};
use serde::Serialize;
use serde_json::value::RawValue;
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

/// Explicit API-v2 base, caller family catalogue and response capacities.
/// Optional `apikey` and descriptive `user-agent` headers are caller-supplied.
/// Public API-v2 is documented keyless at ten requests per minute; this backend
/// neither provisions access nor automatically paces independent operations.
#[derive(Clone, Debug)]
pub struct RubicHttpConfig {
    http: HttpConfig,
    catalogue: Catalogue,
    limits: Limits,
}
impl RubicHttpConfig {
    /// Checks the maximum2MiB document ceiling and fixed query namespace.
    /// The explicit endpoint includes `/api`; credentials stay in redacted headers.
    /// # Errors
    /// Rejects excessive bodies, endpoint queries or URL user information.
    pub fn new(http: HttpConfig, catalogue: Catalogue, limits: Limits) -> Result<Self, Error> {
        let e = http.endpoint();
        if http.limits().max_response_bytes() > wire::MAX_DOCUMENT
            || e.url.query().is_some()
            || !e.url.username().is_empty()
            || e.url.password().is_some()
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            http,
            catalogue,
            limits,
        })
    }
    /// Returns explicit redacted endpoint, provider and transport choices.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
    /// Returns every caller-qualified family/alias, without an API genesis proof.
    #[must_use]
    pub const fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }
    /// Returns bounded collection capacities.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }
}
/// Outgoing Rubic API-v2 direct-only read and unsigned-build backend.
/// Identical direct-only request bytes survive safe-read retries under one budget.
/// Deposit/private orders, authentication signatures, relay and execution are absent.
pub struct RubicClient {
    config: RubicHttpConfig,
    http: HttpClient,
}
impl RubicClient {
    /// Builds an explicitly configured client without contacting a provider.
    /// # Errors
    /// Returns fixed configuration failures.
    pub fn new(config: RubicHttpConfig) -> Result<Self, Error> {
        let http = HttpClient::new(&config.http)?;
        Ok(Self { config, http })
    }
    /// Returns all explicit configuration, with sensitive Debug redacted.
    #[must_use]
    pub const fn config(&self) -> &RubicHttpConfig {
        &self.config
    }
    fn observation<T>(&self, value: T, method: &str) -> Result<Observation<T>, Error> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        Ok(Observation::new(
            value,
            Source::new(
                self.config.http.provider_id(),
                method,
                env!("CARGO_PKG_VERSION"),
            )?,
            Timestamp::from_unix_seconds(time),
        ))
    }
    fn validate_request(&self, request: &QuoteRequest) -> Result<(), Error> {
        let r = request.data();
        if self
            .config
            .catalogue
            .resolve(r.source.chain().alias().as_str())?
            != r.source.chain()
            || self
                .config
                .catalogue
                .resolve(r.destination.chain().alias().as_str())?
                != r.destination.chain()
        {
            return Err(Error::Configuration);
        }
        let l = r.limits;
        if l.routes() > self.config.limits.routes()
            || l.legs() > self.config.limits.legs()
            || l.path_tokens() > self.config.limits.path_tokens()
            || l.warnings() > self.config.limits.warnings()
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
    /// Reads the actual source chain catalogue without automatically trusting it.
    /// # Errors
    /// Returns bounded provider or configuration failures.
    pub async fn chains(&self, include_testnets: bool) -> Result<Observation<Chains>, Error> {
        let b = OperationBudget::new(self.config.http.limits())?;
        b.run(async {
            let flag = if include_testnets { "true" } else { "false" };
            let bytes = self
                .http
                .read(&["info", "chains"], &[("includeTestnets", flag)], None, &b)
                .await?
                .into_success()?;
            self.observation(
                wire::chains(&bytes, include_testnets, self.config.limits.chains())?,
                "api_v2_chains",
            )
        })
        .await
    }
    /// Reads bounded source-selected routes; empty source routes remain empty.
    /// # Errors
    /// Returns provider, correlation or caller-capacity failures.
    pub async fn quote_all(&self, request: QuoteRequest) -> Result<Observation<Routes>, Error> {
        let b = OperationBudget::new(self.config.http.limits())?;
        b.run(async {
            self.validate_request(&request)?;
            let body = body(&request, None)?;
            let bytes = self
                .http
                .read(&["routes", "quoteAll"], &[], Some(&body), &b)
                .await?
                .into_success()?;
            self.observation(
                wire::quote_all(&bytes, &request, &self.config.catalogue)?,
                "api_v2_quote_all",
            )
        })
        .await
    }
    /// Reads the source's best selection, without claiming a global search proof.
    /// # Errors
    /// Returns provider, correlation or caller-capacity failures.
    pub async fn quote_best(&self, request: QuoteRequest) -> Result<Observation<Quote>, Error> {
        let b = OperationBudget::new(self.config.http.limits())?;
        b.run(async {
            self.validate_request(&request)?;
            let body = body(&request, None)?;
            let bytes = self
                .http
                .read(&["routes", "quoteBest"], &[], Some(&body), &b)
                .await?
                .into_success()?;
            self.observation(
                wire::quote_best(&bytes, &request, &self.config.catalogue)?,
                "api_v2_quote_best",
            )
        })
        .await
    }
    /// Builds unsigned data for an explicitly selected direct route and new review.
    /// Source recalculation is retained; no automatic approval or threshold policy.
    /// # Errors
    /// Rejects changed IDs, caller constraints, amounts, family bytes or source failures.
    pub async fn prepare(
        &self,
        request: PreparationRequest,
    ) -> Result<Observation<PreparedSwap>, Error> {
        let b = OperationBudget::new(self.config.http.limits())?;
        b.run(async {
            self.validate_request(&request.data().quote.data().request)?;
            let body = body(&request.data().quote.data().request, Some(&request))?;
            let bytes = self
                .http
                .read(&["routes", "swap"], &[], Some(&body), &b)
                .await?
                .into_success()?;
            self.observation(
                wire::prepared(&bytes, &request, &self.config.catalogue)?,
                "api_v2_swap_data",
            )
        })
        .await
    }
    /// Reads current statusExtended facts with the exact explicit ID and source hash.
    /// # Errors
    /// Returns provider, identity, source echo or bounded-data failures.
    pub async fn status(&self, query: StatusQuery) -> Result<Observation<Status>, Error> {
        let b = OperationBudget::new(self.config.http.limits())?;
        b.run(async {
            if self
                .config
                .catalogue
                .resolve(query.source().alias().as_str())?
                != query.source()
                || self
                    .config
                    .catalogue
                    .resolve(query.destination().alias().as_str())?
                    != query.destination()
            {
                return Err(Error::Configuration);
            }
            let tx = query.source_transaction().source_id();
            let bytes = self
                .http
                .read(
                    &["info", "statusExtended"],
                    &[("rubicId", query.id().as_str()), ("srcTxHash", &tx)],
                    None,
                    &b,
                )
                .await?
                .into_success()?;
            self.observation(wire::status(&bytes, query)?, "api_v2_status_extended")
        })
        .await
    }
}
impl fmt::Debug for RubicClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RubicClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl RubicReader for RubicClient {
    async fn chains(&self, v: bool) -> Result<Observation<Chains>, Error> {
        Self::chains(self, v).await
    }
    async fn quote_all(&self, v: QuoteRequest) -> Result<Observation<Routes>, Error> {
        Self::quote_all(self, v).await
    }
    async fn quote_best(&self, v: QuoteRequest) -> Result<Observation<Quote>, Error> {
        Self::quote_best(self, v).await
    }
    async fn prepare(&self, v: PreparationRequest) -> Result<Observation<PreparedSwap>, Error> {
        Self::prepare(self, v).await
    }
    async fn status(&self, v: StatusQuery) -> Result<Observation<Status>, Error> {
        Self::status(self, v).await
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Body<'a> {
    src_token_address: String,
    dst_token_address: String,
    src_token_blockchain: &'a str,
    dst_token_blockchain: &'a str,
    src_token_amount: String,
    slippage: Box<RawValue>,
    timeout: u8,
    referrer: &'a str,
    native_blacklist: Vec<&'a str>,
    provider_tags: [&'static str; 2],
    enable_testnets: Switch,
    enable_checks: Switch,
    skip_fee_providers: Switch,
    show_dangerous_routes: Switch,
    show_failed_routes: Switch,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferred_provider: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integrator_address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    receiver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
}
fn body<'a>(
    quote: &'a QuoteRequest,
    preparation: Option<&'a PreparationRequest>,
) -> Result<Vec<u8>, Error> {
    let r = quote.data();
    let (p_sender, p_receiver, id) =
        preparation.map_or((r.sender.as_ref(), r.receiver.as_ref(), None), |p| {
            (
                Some(&p.data().sender),
                Some(&p.data().receiver),
                Some(p.data().quote.data().id.as_str()),
            )
        });
    let b = Body {
        src_token_address: r.source.source_address(),
        dst_token_address: r.destination.source_address(),
        src_token_blockchain: r.source.chain().alias().as_str(),
        dst_token_blockchain: r.destination.chain().alias().as_str(),
        src_token_amount: r.amount.formatted().ok_or(Error::Configuration)?,
        slippage: RawValue::from_string(r.slippage.fraction()).map_err(|_| Error::Configuration)?,
        timeout: r.calculation_timeout,
        referrer: r.referrer.as_str(),
        native_blacklist: r
            .excluded_providers
            .iter()
            .map(Identifier::as_str)
            .collect(),
        provider_tags: ["notDeposits", "notPrivate"],
        enable_testnets: Switch(r.source.chain().testnet() || r.destination.chain().testnet()),
        enable_checks: Switch(false),
        skip_fee_providers: Switch(r.skip_fee_providers),
        show_dangerous_routes: Switch(r.allow_dangerous_routes),
        show_failed_routes: Switch(false),
        preferred_provider: r.preferred_provider.as_ref().map(Identifier::as_str),
        integrator_address: r.integrator,
        from_address: p_sender.map(Account::source_address),
        receiver: p_receiver.map(Account::source_address),
        id,
    };
    serde_json::to_vec(&b).map_err(|_| Error::Configuration)
}
