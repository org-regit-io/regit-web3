// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{HeliusReader, wire};
use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        helius::{
            Asset, AssetRequest, Context, Cursor, HistoryPage, HistoryRequest, Observation,
            ObservedData, OwnerPage, OwnerRequest, ParseRequest, ParsedBatch, Request,
        },
        solana::{Hash, Network},
    },
    error::{Error, ProviderError},
    transport::{HttpClient, OperationBudget, decode_response, encode_request},
};
use std::{
    fmt,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Explicit expected Solana genesis and one RPC/Parsed Events HTTP base.
/// Credentials belong to the caller-supplied endpoint/query/headers. No key,
/// cluster, host, runtime or environment variable is selected implicitly.
#[derive(Clone, Debug)]
pub struct HeliusHttpConfig {
    network: Network,
    http: HttpConfig,
}
impl HeliusHttpConfig {
    /// Records explicit network and transport settings without making a request.
    #[must_use]
    pub const fn new(network: Network, http: HttpConfig) -> Self {
        Self { network, http }
    }
    /// Returns expected full genesis identity and independent display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns transport limits/provider attribution with endpoint/auth redacted.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}
/// Opaque concrete-client-bound owner cursor, retaining every original query control.
/// The handle cannot be deserialized; source cursors remain available separately
/// for explicit caller-owned persistence whose configuration provenance is asserted.
#[derive(Clone)]
pub struct AssetContinuation {
    scope: Arc<()>,
    request: OwnerRequest,
    cursor: Cursor,
}
impl fmt::Debug for AssetContinuation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AssetContinuation { .. }")
    }
}
/// Opaque concrete-client-bound parsed-history continuation with immutable controls.
#[derive(Clone)]
pub struct HistoryContinuation {
    scope: Arc<()>,
    request: HistoryRequest,
    cursor: Cursor,
}
impl fmt::Debug for HistoryContinuation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HistoryContinuation { .. }")
    }
}
/// Bounded read-only Helius DAS/current Parsed Events implementation.
/// Safe-read retries freeze query bytes; no fallback, raw canonical fetch or
/// automatic pagination occurs. The caller supplies Tokio I/O/time drivers.
pub struct HeliusClient {
    config: HeliusHttpConfig,
    http: HttpClient,
    scope: Arc<()>,
}
impl fmt::Debug for HeliusClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeliusClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl HeliusClient {
    /// Verifies full expected genesis through the explicitly configured RPC base.
    /// # Errors
    /// Returns fixed network/provider/configuration/budget errors without credentials.
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks networking/time drivers.
    pub async fn connect(config: HeliusHttpConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http.limits())?;
        let http = HttpClient::new(&config.http)?;
        let client = Self {
            config,
            http,
            scope: Arc::new(()),
        };
        budget.run(client.verify_genesis(&budget)).await?;
        Ok(client)
    }
    /// Returns explicit configuration with redacted endpoint/auth diagnostics.
    #[must_use]
    pub const fn config(&self) -> &HeliusHttpConfig {
        &self.config
    }
    async fn rpc<T: serde::de::DeserializeOwned, P: serde::Serialize + ?Sized>(
        &self,
        budget: &OperationBudget,
        method: &str,
        params: &P,
    ) -> Result<T, Error> {
        let body = encode_request(1, method, params)?;
        let bytes = self
            .http
            .read(&[], &[], Some(&body), budget)
            .await?
            .into_success()?;
        decode_response(&bytes, 1, wire::rpc_error)?.ok_or_else(wire::invalid)
    }
    async fn verify_genesis(&self, budget: &OperationBudget) -> Result<(), Error> {
        let reported: Hash = self.rpc(budget, "getGenesisHash", &[] as &[(); 0]).await?;
        if reported != self.config.network.genesis_hash() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(())
    }
    fn observation<T: ObservedData>(
        &self,
        value: T,
        request: Request,
        index: Option<u64>,
        method: &str,
    ) -> Result<Observation<T>, Error> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        let context = Context::new(
            self.config.network.clone(),
            request,
            index,
            Source::new(
                self.config.http.provider_id(),
                method,
                env!("CARGO_PKG_VERSION"),
            )?,
            Timestamp::from_unix_seconds(time),
        )?;
        Observation::new(value, context).map_err(|_| wire::invalid())
    }
    /// Reads exact requested DAS asset, preserving supported source facts/index progress.
    /// # Errors
    /// Returns safe typed provider/network/bounds/identity failures; source not-found
    /// is unavailable data, while missing/null successful results are malformed.
    /// # Panics
    /// Tokio may panic without enabled runtime I/O/time drivers.
    pub async fn get_asset(&self, request: AssetRequest) -> Result<Observation<Asset>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let result: wire::AssetWire = self
                    .rpc(&budget, "getAsset", &wire::asset_params(&request))
                    .await?;
                let (asset, index) = result.into_asset()?;
                self.observation(asset, Request::Asset(request), index, "getAsset")
            })
            .await
    }
    /// Reads one exact owner page; no short-page/global-exhaustion inference is made.
    /// # Errors
    /// Returns safe typed errors for malformed quantities, capacity, query/owner
    /// mismatches, network changes, retries or the one shared deadline.
    /// # Panics
    /// Tokio may panic without enabled runtime I/O/time drivers.
    pub async fn get_assets_by_owner(
        &self,
        request: OwnerRequest,
    ) -> Result<Observation<OwnerPage>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let result: wire::OwnerWire = self
                    .rpc(&budget, "getAssetsByOwner", &wire::owner_params(&request))
                    .await?;
                let (page, index) = result.into_page(request.clone())?;
                self.observation(
                    page,
                    Request::OwnerAssets(request),
                    index,
                    "getAssetsByOwner",
                )
            })
            .await
    }
    /// Parses one bounded ordered batch through current Parsed Events v1.
    /// Parser failure is a per-item result; it does not imply pending status.
    /// # Errors
    /// Rejects malformed or reordered/missing/extra responses, network changes,
    /// body/decode limits and expired shared budgets with fixed diagnostics.
    /// # Panics
    /// Tokio may panic without enabled runtime I/O/time drivers.
    pub async fn parse_transactions(
        &self,
        request: ParseRequest,
    ) -> Result<Observation<ParsedBatch>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let body = serde_json::to_vec(&wire::parse_params(&request))
                    .map_err(|_| Error::Configuration)?;
                let bytes = self
                    .http
                    .read(
                        &["v1", "parsed-events", "transactions"],
                        &[],
                        Some(&body),
                        &budget,
                    )
                    .await?
                    .into_success()?;
                let raw: wire::List<wire::ResultWire> = wire::decode(&bytes)?;
                let results = raw
                    .0
                    .into_iter()
                    .map(wire::ResultWire::into_result)
                    .collect::<Result<_, _>>()?;
                let batch =
                    ParsedBatch::new(request.clone(), results).map_err(|_| wire::invalid())?;
                self.observation(
                    batch,
                    Request::ParseTransactions(request),
                    None,
                    "parsedEventsTransactions",
                )
            })
            .await
    }
    /// Reads one current Parsed Events address-history page and exact source token.
    /// Source range-end, parser failure and unknown inclusion stay distinct.
    /// # Errors
    /// Returns safe typed errors for malformed pages/identity/limits/network/deadlines.
    /// # Panics
    /// Tokio may panic without enabled runtime I/O/time drivers.
    pub async fn get_address_history(
        &self,
        request: HistoryRequest,
    ) -> Result<Observation<HistoryPage>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let body = serde_json::to_vec(&wire::history_params(&request))
                    .map_err(|_| Error::Configuration)?;
                let bytes = self
                    .http
                    .read(
                        &["v1", "parsed-events", "transaction-history"],
                        &[],
                        Some(&body),
                        &budget,
                    )
                    .await?
                    .into_success()?;
                let page = wire::decode::<wire::HistoryWire>(&bytes)?.into_page(request.clone())?;
                self.observation(
                    page,
                    Request::AddressHistory(request),
                    None,
                    "parsedEventsTransactionHistory",
                )
            })
            .await
    }
    fn verify_context(&self, c: &Context, method: &str) -> Result<(), Error> {
        if c.network().genesis_hash() != self.config.network.genesis_hash()
            || c.source().provider_id() != self.config.http.provider_id()
            || c.source().method() != method
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
    /// Binds an explicitly supplied attributed owner page to this client's config.
    /// Pure page/source provenance remains a caller assertion, not authentication.
    /// # Errors
    /// Rejects mismatched network/provider/method or non-cursor pagination.
    pub fn asset_continuation(
        &self,
        page: &Observation<OwnerPage>,
    ) -> Result<Option<AssetContinuation>, Error> {
        self.verify_context(page.context(), "getAssetsByOwner")?;
        page.value()
            .cursor()
            .map(|c| {
                page.value().request().with_cursor(c.clone())?;
                Ok(AssetContinuation {
                    scope: Arc::clone(&self.scope),
                    request: page.value().request().clone(),
                    cursor: c.clone(),
                })
            })
            .transpose()
    }
    /// Continues exactly the bound owner query, retaining every other source control.
    /// # Errors
    /// Rejects another client's handle before any request; otherwise returns read errors.
    /// # Panics
    /// Tokio may panic without enabled runtime I/O/time drivers.
    pub async fn continue_assets(
        &self,
        continuation: AssetContinuation,
    ) -> Result<Observation<OwnerPage>, Error> {
        if !Arc::ptr_eq(&self.scope, &continuation.scope) {
            return Err(Error::Configuration);
        }
        self.get_assets_by_owner(continuation.request.with_cursor(continuation.cursor)?)
            .await
    }
    /// Binds an attributed parsed-history page to this concrete client's config.
    /// # Errors
    /// Rejects mismatched network/provider/method without serializing credentials.
    pub fn history_continuation(
        &self,
        page: &Observation<HistoryPage>,
    ) -> Result<Option<HistoryContinuation>, Error> {
        self.verify_context(page.context(), "parsedEventsTransactionHistory")?;
        Ok(page
            .value()
            .pagination_token()
            .map(|c| HistoryContinuation {
                scope: Arc::clone(&self.scope),
                request: page.value().request().clone(),
                cursor: c.clone(),
            }))
    }
    /// Continues the bound source history range and original non-pagination controls.
    /// # Errors
    /// Rejects another client's handle before dispatch; otherwise returns read errors.
    /// # Panics
    /// Tokio may panic without enabled runtime I/O/time drivers.
    pub async fn continue_history(
        &self,
        continuation: HistoryContinuation,
    ) -> Result<Observation<HistoryPage>, Error> {
        if !Arc::ptr_eq(&self.scope, &continuation.scope) {
            return Err(Error::Configuration);
        }
        self.get_address_history(continuation.request.with_token(continuation.cursor))
            .await
    }
}
impl HeliusReader for HeliusClient {
    async fn get_asset(&self, q: AssetRequest) -> Result<Observation<Asset>, Error> {
        Self::get_asset(self, q).await
    }
    async fn get_assets_by_owner(&self, q: OwnerRequest) -> Result<Observation<OwnerPage>, Error> {
        Self::get_assets_by_owner(self, q).await
    }
    async fn parse_transactions(&self, q: ParseRequest) -> Result<Observation<ParsedBatch>, Error> {
        Self::parse_transactions(self, q).await
    }
    async fn get_address_history(
        &self,
        q: HistoryRequest,
    ) -> Result<Observation<HistoryPage>, Error> {
        Self::get_address_history(self, q).await
    }
}
