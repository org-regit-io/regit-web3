// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_time::{SystemTime, UNIX_EPOCH};

use super::{JupiterReader, wire};
use crate::{
    chains::solana::ExecutionReader,
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        jupiter::{
            BuildRequest, Context, Destination, DexFilter, Observation, Operation, PreparedSwap,
            Quote, QuoteRequest, SwapBuild, SwapEstimate, mainnet,
        },
        solana::{Network, ReadOptions},
    },
    error::{Error, ProviderError},
    transport::{HttpClient, OperationBudget},
};
use std::fmt;
/// Explicit Swap API V2 base (including `/swap/v2`), mainnet declaration and auth.
/// Keyless access is selected explicitly; rate/plan access remains source-dependent.
/// Endpoint headers and credentials are redacted and never loaded from the environment.
#[derive(Clone, Debug)]
pub struct JupiterHttpConfig {
    http: HttpConfig,
    network: Network,
    authenticated: bool,
}
impl JupiterHttpConfig {
    /// Adds the optional caller-supplied sensitive `x-api-key` header.
    /// `None` selects documented keyless access with no credential fallback.
    /// # Errors
    /// Rejects non-mainnet identity, hidden source-query/auth overrides, empty keys,
    /// or response bounds exceeding two MiB.
    pub fn new(http: HttpConfig, network: Network, api_key: Option<&str>) -> Result<Self, Error> {
        if network.genesis_hash() != mainnet("mainnet")?.genesis_hash()
            || http.limits().max_response_bytes() > wire::MAX_DOCUMENT
            || http.endpoint().url.query().is_some()
            || http.endpoint().headers.contains_key("x-api-key")
        {
            return Err(Error::Configuration);
        }
        let authenticated = api_key.is_some();
        let http = if let Some(key) = api_key {
            if key.is_empty() {
                return Err(Error::Configuration);
            }
            HttpConfig::new(
                http.endpoint().clone().with_header("x-api-key", key)?,
                http.limits(),
                http.provider_id(),
            )?
        } else {
            http
        };
        Ok(Self {
            http,
            network,
            authenticated,
        })
    }
    /// Returns exact redacted endpoint, bounded transport and provider attribution.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
    /// Returns explicit mainnet declaration, not an API-reported genesis proof.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Reports whether the caller explicitly supplied the Jupiter key header.
    #[must_use]
    pub const fn authenticated(&self) -> bool {
        self.authenticated
    }
}
/// Bounded outgoing Jupiter Swap API V2 quote/build backend.
/// No managed execution, signing, account funding or automatic submission exists.
/// Native callers supply a Tokio runtime with I/O/time drivers and explicit access limits.
pub struct JupiterClient {
    config: JupiterHttpConfig,
    http: HttpClient,
}
impl fmt::Debug for JupiterClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JupiterClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl JupiterClient {
    /// Constructs the explicit backend without contacting an API.
    /// # Errors
    /// Reports fixed HTTP-client construction failures.
    pub fn new(config: JupiterHttpConfig) -> Result<Self, Error> {
        let http = HttpClient::new(&config.http)?;
        Ok(Self { config, http })
    }
    /// Returns explicit limits, auth selection and network declaration.
    #[must_use]
    pub const fn config(&self) -> &JupiterHttpConfig {
        &self.config
    }
    /// Reads an exact-input quote without requesting or retaining a transaction.
    /// One total budget covers retries, body consumption and strict bounded decoding.
    /// Source selection is not global-route optimality or an execution guarantee.
    /// # Errors
    /// Reports configuration, fixed provider, identity, response-bound or deadline failures.
    /// # Panics
    /// On native targets, Tokio may panic if the caller disables I/O or time drivers.
    pub async fn quote(&self, request: QuoteRequest) -> Result<Observation<Quote>, Error> {
        self.network(&request.data().network)?;
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                let r = request.data();
                let mut query = base(r.input_mint, r.output_mint, r.amount, r.slippage.value());
                if !r.excluded_routers.is_empty() {
                    query.push((
                        "excludeRouters",
                        r.excluded_routers
                            .iter()
                            .map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(","),
                    ));
                }
                if !r.excluded_dexes.is_empty() {
                    query.push(("excludeDexes", labels(&r.excluded_dexes)));
                }
                let bytes = self.read("order", &query, &budget).await?;
                let network = r.network.clone();
                Observation::new(
                    wire::quote(&bytes, request)?,
                    self.context(Operation::OrderQuote, network, "swap_v2_order")?,
                )
            })
            .await
    }
    /// Reads a fresh Metis V0 route and source instructions with actual height expiry.
    /// Every build setting is explicit; no earlier `/order` route is preserved.
    /// Unknown provider query errors and missing liquidity remain fixed HTTP failures.
    /// # Errors
    /// Reports configuration, fixed source, identity, bound or deadline failures.
    /// # Panics
    /// On native targets, Tokio may panic if the caller disables I/O or time drivers.
    pub async fn build(&self, request: BuildRequest) -> Result<Observation<SwapBuild>, Error> {
        self.network(&request.data().network)?;
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                let r = request.data();
                let mut query = base(r.input_mint, r.output_mint, r.amount, r.slippage.value());
                query.extend([
                    ("taker", r.taker.to_string()),
                    ("payer", r.payer.to_string()),
                    ("wrapAndUnwrapSol", r.wrap_and_unwrap_sol.to_string()),
                    ("maxAccounts", r.max_accounts.to_string()),
                    (
                        "blockhashSlotsToExpiry",
                        r.blockhash_slots_to_expiry.to_string(),
                    ),
                    (
                        "computeUnitPricePercentile",
                        r.compute_unit_price_percentile.to_string(),
                    ),
                    ("transactionVersion", "0".into()),
                    ("forJitoBundle", "false".into()),
                    ("platformFeeBps", "0".into()),
                ]);
                match r.destination {
                    Destination::Taker => {}
                    Destination::TokenAccount { address } => {
                        query.push(("destinationTokenAccount", address.to_string()));
                    }
                    Destination::Native { address } => {
                        query.push(("nativeDestinationAccount", address.to_string()));
                    }
                }
                match &r.dex_filter {
                    DexFilter::All => {}
                    DexFilter::Include(v) => query.push(("dexes", labels(v))),
                    DexFilter::Exclude(v) => query.push(("excludeDexes", labels(v))),
                }
                let bytes = self.read("build", &query, &budget).await?;
                let network = r.network.clone();
                Observation::new(
                    wire::build(&bytes, request)?,
                    self.context(Operation::Build, network, "swap_v2_build")?,
                )
            })
            .await
    }
    /// Evaluates the exact preparation through a caller-supplied Solana reader.
    /// One Jupiter-configured total budget covers both sequential RPC operations.
    /// The concrete Solana backend verifies full genesis on each operation; generic
    /// implementations must honor that source-identity contract. Slots stay separate.
    /// No hash refresh, signature verification, signing or submission is performed.
    /// # Errors
    /// Reports network/request mismatch, fixed source failures or the shared deadline.
    /// # Panics
    /// On native targets, Tokio may panic if the caller disables I/O or time drivers.
    pub async fn estimate_swap<R: ExecutionReader + crate::future::MaybeSync>(
        &self,
        reader: &R,
        prepared: PreparedSwap,
        options: ReadOptions,
    ) -> Result<SwapEstimate, Error> {
        self.network(
            &prepared
                .intent()
                .data()
                .build
                .value()
                .data()
                .request
                .data()
                .network,
        )?;
        self.network(reader.network())?;
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                let fee = reader
                    .get_fee_for_message(
                        prepared.unsigned().transaction().message().clone(),
                        options,
                    )
                    .await?;
                let simulation = reader
                    .simulate_transaction(prepared.unsigned().transaction().clone(), options)
                    .await?;
                SwapEstimate::new(prepared, options, fee, simulation)
            })
            .await
    }
    fn network(&self, n: &Network) -> Result<(), Error> {
        if n.genesis_hash() != self.config.network.genesis_hash() {
            return Err(Error::Configuration);
        }
        Ok(())
    }
    async fn read(
        &self,
        path: &str,
        query: &[(&str, String)],
        budget: &OperationBudget,
    ) -> Result<Vec<u8>, Error> {
        let borrowed = query
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        let response = self.http.read(&[path], &borrowed, None, budget).await?;
        if response.status.as_u16() == 404 {
            return Err(Error::UnavailableData);
        }
        response.into_success()
    }
    fn context(
        &self,
        operation: Operation,
        network: Network,
        method: &str,
    ) -> Result<Context, Error> {
        let retrieved = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        Context::new(
            operation,
            network,
            Source::new(
                self.config.http.provider_id(),
                method,
                env!("CARGO_PKG_VERSION"),
            )?,
            Timestamp::from_unix_seconds(retrieved),
        )
        .map_err(|_| Error::Provider(ProviderError::InvalidResponse))
    }
}
fn base(
    input: crate::domain::solana::Pubkey,
    output: crate::domain::solana::Pubkey,
    amount: u64,
    slippage: u16,
) -> Vec<(&'static str, String)> {
    vec![
        ("inputMint", input.to_string()),
        ("outputMint", output.to_string()),
        ("amount", amount.to_string()),
        ("slippageBps", slippage.to_string()),
    ]
}
fn labels(v: &[crate::domain::jupiter::Label]) -> String {
    v.iter()
        .map(crate::domain::jupiter::Label::as_str)
        .collect::<Vec<_>>()
        .join(",")
}
impl JupiterReader for JupiterClient {
    async fn quote(&self, r: QuoteRequest) -> Result<Observation<Quote>, Error> {
        Self::quote(self, r).await
    }
    async fn build(&self, r: BuildRequest) -> Result<Observation<SwapBuild>, Error> {
        Self::build(self, r).await
    }
}
