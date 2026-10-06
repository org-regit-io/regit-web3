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

mod rpc;

use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, timeout_at};

use crate::{
    config::EvmConfig,
    domain::{
        Address, Amount, Balance, BlockContext, BlockHash, BlockSelector, ChainId, Observation,
        ObservationContext, Source, Timestamp,
    },
    error::{Error, ProviderError},
};

use self::rpc::{ErrorPolicy, parse_quantity, read};

/// A read-only EVM client with explicit configuration and verified chain identity.
///
/// Construct with [`Self::connect`] inside an existing Tokio runtime with its
/// I/O and time drivers enabled. The library creates no runtime, loads no RPC
/// configuration or credentials, and discovers no proxy configuration.
/// Native balances are read only at a captured canonical block hash.
pub struct EvmClient {
    config: EvmConfig,
    chain_id: ChainId,
    http: Client,
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
        require_runtime()?;
        let deadline = Instant::now() + config.limits().request_timeout();
        let http = Client::builder()
            .tls_backend_rustls()
            .tls_sslkeylogfile(false)
            .connect_timeout(config.limits().connect_timeout())
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .referer(false)
            .no_gzip()
            .no_brotli()
            .no_zstd()
            .no_deflate()
            .build()
            .map_err(|_| Error::Configuration)?;
        let mut client = Self {
            chain_id: config.network().chain_id(),
            config,
            http,
        };
        let chain_id = timeout_at(deadline, client.verify_chain())
            .await
            .map_err(|_| Error::Timeout)??;
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
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
        require_runtime()?;
        let deadline = Instant::now() + self.config.limits().request_timeout();
        let selector = selector.unwrap_or_else(|| self.config.default_selector());
        let observation = timeout_at(deadline, self.native_balance(address, selector))
            .await
            .map_err(|_| Error::Timeout)??;
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        Ok(observation)
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

    async fn verify_chain(&self) -> Result<ChainId, Error> {
        let quantity: String = read(
            &self.http,
            &self.config,
            "eth_chainId",
            &[] as &[(); 0],
            ErrorPolicy::ChainIdentity,
        )
        .await?
        .ok_or_else(invalid_response)?;
        let chain_id = ChainId::new(parse_quantity(&quantity)?);
        if chain_id != self.config.network().chain_id() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(chain_id)
    }

    async fn resolve_block(&self, selector: BlockSelector) -> Result<BlockContext, Error> {
        let (method, identifier) = match selector {
            BlockSelector::Latest => ("eth_getBlockByNumber", "latest".to_owned()),
            BlockSelector::Safe => ("eth_getBlockByNumber", "safe".to_owned()),
            BlockSelector::Finalized => ("eth_getBlockByNumber", "finalized".to_owned()),
            BlockSelector::Number(number) => ("eth_getBlockByNumber", format!("0x{number:x}")),
            BlockSelector::Hash(hash) => ("eth_getBlockByHash", hash.to_string()),
        };
        let block: RpcBlock = read(
            &self.http,
            &self.config,
            method,
            &(identifier, false),
            ErrorPolicy::StateRead,
        )
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
    ) -> Result<Observation<Balance>, Error> {
        self.verify_chain().await?;
        let block = self.resolve_block(selector).await?;
        let quantity: String = read(
            &self.http,
            &self.config,
            "eth_getBalance",
            &(
                address,
                CanonicalBlock {
                    block_hash: *block.hash(),
                    require_canonical: true,
                },
            ),
            ErrorPolicy::StateRead,
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

fn require_runtime() -> Result<(), Error> {
    if tokio::runtime::Handle::try_current().is_err() {
        Err(Error::Configuration)
    } else {
        Ok(())
    }
}

const fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

// Block responses include many protocol fields, including future extensions.
// Deserialize the required anchor directly so duplicate critical fields cannot
// be silently overwritten in an intermediate generic JSON object.
#[derive(Deserialize)]
struct RpcBlock {
    hash: String,
    number: String,
    timestamp: String,
}

impl RpcBlock {
    fn context(self) -> Result<BlockContext, Error> {
        let hash = BlockHash::parse(&self.hash).map_err(|_| invalid_response())?;
        let number =
            u64::try_from(parse_quantity(&self.number)?).map_err(|_| invalid_response())?;
        let timestamp =
            u64::try_from(parse_quantity(&self.timestamp)?).map_err(|_| invalid_response())?;
        Ok(BlockContext::new(
            number,
            hash,
            Timestamp::from_unix_seconds(timestamp),
        ))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalBlock {
    block_hash: BlockHash,
    require_canonical: bool,
}
