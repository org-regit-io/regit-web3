// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bounded, read-only EVM operations with explicit chain and block context.
//!
//! Establishment and each balance operation check the provider's `eth_chainId`.
//! Balance reads use EIP-1898 with a captured hash and `requireCanonical: true`.
//! These checks describe the source's responses; they do not prove atomic
//! behavior across a routed provider or establish lasting block finality.
//! The caller owns a Tokio runtime with I/O and time drivers enabled. HTTPS
//! uses verified standard platform trust. Proxy discovery, redirects,
//! automatic transport retries, cookies, and decompression are disabled.

use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    config::EvmConfig,
    domain::{
        Address, Amount, Balance, BlockContext, BlockSelector, ChainId, Observation,
        ObservationContext, Source, Timestamp,
    },
    error::{Error, ProviderError},
    transport::{HttpClient, OperationBudget},
};

use super::wire::{
    CanonicalBlock, RpcBlock, chain_error, invalid_response, parse_quantity, state_error,
};

mod reads;

/// A read-only EVM client with explicit configuration and verified chain identity.
///
/// Construct with [`Self::connect`] inside an existing Tokio runtime with its
/// I/O and time drivers enabled. The library creates no runtime, loads no RPC
/// configuration or credentials, and discovers no proxy configuration.
/// Native balances are read only at a captured canonical block hash.
pub struct EvmClient {
    config: EvmConfig,
    chain_id: ChainId,
    http: HttpClient,
}

impl EvmClient {
    /// Establishes a client after verifying the provider's exact EVM chain ID.
    ///
    /// The single configured request budget includes connection, every retry
    /// and delay, response transfer, and decoding. Only `eth_chainId` is called.
    /// Redirects and implicit Reqwest retries are disabled. Additional attempts
    /// are permitted only after transport failure, HTTP 429, or HTTP 5xx.
    /// The async deadline cannot preempt synchronous client construction or
    /// JSON decoding; response bytes are bounded and late successes rejected.
    ///
    /// # Errors
    ///
    /// Returns fixed typed failures for absent runtime or invalid configuration,
    /// elapsed budgets, transfer/status failures, oversized responses, invalid
    /// JSON-RPC envelopes or quantities, RPC errors, and a chain ID differing
    /// from the configured identity. Errors retain no endpoint URLs, headers,
    /// or remote response messages.
    ///
    /// # Panics
    ///
    /// Tokio and its networking stack may panic when an existing runtime lacks
    /// enabled time or I/O drivers. Both drivers must be enabled by the caller.
    pub async fn connect(config: EvmConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.limits())?;
        let http = HttpClient::new(config.http_config())?;
        let mut client = Self {
            chain_id: config.network().chain_id(),
            config,
            http,
        };
        let chain_id = budget.run(client.verify_chain(&budget)).await?;
        client.chain_id = chain_id;
        Ok(client)
    }

    /// Reads an exact native balance at a resolved canonical block hash.
    ///
    /// An omitted selector uses the explicitly configured default. Each read
    /// rechecks `eth_chainId`, resolves the selector through a block lookup,
    /// then calls `eth_getBalance` with that hash and `requireCanonical: true`.
    /// Balance retries retain the same address and captured hash; there is no
    /// fallback to a height or a newer head. The returned observation preserves
    /// the requested selector, block, configured asset precision, and source.
    /// Retrieval time is captured from the system clock in whole Unix seconds
    /// after the responses. Finality remains unknown and confirmations absent.
    ///
    /// One configured request budget covers all stages, retries, delays and
    /// body consumption. Synchronous decoding and observation construction
    /// cannot be preempted; bounded bodies and a final deadline check prevent
    /// accepting late success. Each independent HTTP response must match its
    /// request's JSON-RPC ID. Calls may execute concurrently on this client.
    ///
    /// The per-read chain check cannot make multiple provider responses atomic.
    /// Canonicality is the provider's assertion when it processes the pinned
    /// read and does not establish lasting finality. A safe/finalized selector
    /// alone does not change the observation's finality metadata.
    ///
    /// # Errors
    ///
    /// Returns typed failures for chain mismatch, malformed or inconsistent
    /// responses, elapsed budgets, transport/status failures and body limits.
    /// A null block or balance is unavailable data, never a zero balance.
    /// For block/balance RPC errors, codes `-32601`/`-32004` indicate unsupported
    /// methods and `-32001`/`-32002` unavailable data. Other codes, including the
    /// ambiguous `-32000` canonicality rejection, remain provider RPC failures;
    /// remote message text is never interpreted. Missing runtime or a system
    /// clock before the Unix epoch returns a configuration failure.
    ///
    /// # Panics
    ///
    /// Tokio and its networking stack may panic if the caller's existing
    /// runtime lacks enabled time or I/O drivers.
    pub async fn get_native_balance(
        &self,
        address: Address,
        selector: Option<BlockSelector>,
    ) -> Result<Observation<Balance>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        let selector = selector.unwrap_or_else(|| self.config.default_selector());
        budget
            .run(self.native_balance(address, selector, &budget))
            .await
    }

    /// Returns the explicit configuration, with redacted endpoint diagnostics.
    #[must_use]
    pub const fn config(&self) -> &EvmConfig {
        &self.config
    }

    /// Returns the exact chain ID verified at establishment.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    async fn verify_chain(&self, budget: &OperationBudget) -> Result<ChainId, Error> {
        let quantity: String = self
            .http
            .read_rpc(budget, "eth_chainId", &[] as &[(); 0], chain_error)
            .await?
            .ok_or_else(invalid_response)?;
        let chain_id = ChainId::new(parse_quantity(&quantity)?);
        if chain_id != self.config.network().chain_id() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(chain_id)
    }

    async fn resolve_block(
        &self,
        selector: BlockSelector,
        budget: &OperationBudget,
    ) -> Result<BlockContext, Error> {
        let (method, identifier) = match selector {
            BlockSelector::Latest => ("eth_getBlockByNumber", "latest".to_owned()),
            BlockSelector::Safe => ("eth_getBlockByNumber", "safe".to_owned()),
            BlockSelector::Finalized => ("eth_getBlockByNumber", "finalized".to_owned()),
            BlockSelector::Number(number) => ("eth_getBlockByNumber", format!("0x{number:x}")),
            BlockSelector::Hash(hash) => ("eth_getBlockByHash", hash.to_string()),
        };
        let block: RpcBlock = self
            .http
            .read_rpc(budget, method, &(identifier, false), state_error)
            .await?
            .ok_or(Error::UnavailableData)?;
        let block = block.context()?;
        let mismatch = match selector {
            BlockSelector::Number(number) => number != block.number(),
            BlockSelector::Hash(hash) => hash != *block.hash(),
            BlockSelector::Latest | BlockSelector::Safe | BlockSelector::Finalized => false,
        };
        if mismatch {
            return Err(invalid_response());
        }
        Ok(block)
    }

    async fn native_balance(
        &self,
        address: Address,
        selector: BlockSelector,
        budget: &OperationBudget,
    ) -> Result<Observation<Balance>, Error> {
        self.verify_chain(budget).await?;
        let block = self.resolve_block(selector, budget).await?;
        let quantity: String = self
            .http
            .read_rpc(
                budget,
                "eth_getBalance",
                &(address, CanonicalBlock::new(*block.hash())),
                state_error,
            )
            .await?
            .ok_or(Error::UnavailableData)?;
        let amount = Amount::new(
            parse_quantity(&quantity)?,
            Some(self.config.native_asset().decimals()),
        );
        let retrieved_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| Timestamp::from_unix_seconds(duration.as_secs()))
            .map_err(|_| Error::Configuration)?;
        let source = Source::new(
            self.config.provider_id(),
            "eth_getBalance",
            env!("CARGO_PKG_VERSION"),
        )?;
        let context = ObservationContext::new(
            self.config.network().clone(),
            selector,
            block,
            source,
            retrieved_at,
        )?;
        let balance = Balance::new(address, self.config.native_asset().clone(), amount)?;
        Observation::native_balance(balance, context)
    }
}

impl fmt::Debug for EvmClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvmClient")
            .field("config", &self.config)
            .field("chain_id", &self.chain_id)
            .finish_non_exhaustive()
    }
}

impl super::NativeBalanceReader for EvmClient {
    fn get_native_balance(
        &self,
        address: Address,
        selector: Option<BlockSelector>,
    ) -> impl std::future::Future<Output = Result<Observation<Balance>, Error>> + Send {
        Self::get_native_balance(self, address, selector)
    }
}
