// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit EVM client establishment with bounded, read-only chain verification.
//!
//! Establishment checks the selected provider's `eth_chainId`. The caller owns
//! the Tokio runtime, with I/O and time drivers enabled. HTTPS uses verified
//! standard platform certificate trust. Proxy discovery, redirects, automatic
//! transport retries, cookies, and response decompression are disabled.

use std::{fmt, time::Duration};

use reqwest::{Client, Response, StatusCode, header::CONTENT_TYPE};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use tokio::time::{Instant, sleep, timeout_at};

use crate::{
    config::EvmConfig,
    domain::{ChainId, U256},
    error::{Error, ProviderError},
};

const REQUEST_ID: u64 = 1;
const RETRY_DELAY: Duration = Duration::from_millis(25);

/// A read-only EVM client whose expected chain was verified at establishment.
///
/// Construct with [`Self::connect`] inside an existing Tokio runtime with its
/// I/O and time drivers enabled. The library creates no runtime, loads no RPC
/// configuration or credentials, and discovers no proxy configuration.
///
/// The chain verification describes the provider at establishment time; it
/// does not establish future endpoint behavior. This client currently exposes
/// configuration and verified identity only, without a balance operation.
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
    /// Returns fixed typed failures for absent runtime or invalid client configuration,
    /// elapsed budgets, transfer/status failures, oversized responses, invalid
    /// JSON-RPC envelopes or quantities, RPC errors, and a
    /// chain ID differing from the explicitly configured identity. No failure
    /// retains endpoint URLs, headers, or remote response messages.
    ///
    /// # Panics
    ///
    /// Tokio and its networking stack may panic when an existing runtime lacks
    /// enabled time or I/O drivers. Both drivers must be enabled by the caller.
    pub async fn connect(config: EvmConfig) -> Result<Self, Error> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(Error::Configuration);
        }
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
        let chain_id = timeout_at(deadline, verify_chain(&client.http, &client.config))
            .await
            .map_err(|_| Error::Timeout)??;
        // An async timeout cannot preempt synchronous JSON decoding. The body
        // is bounded; additionally reject any result accepted after its budget.
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        client.chain_id = chain_id;
        Ok(client)
    }

    /// Returns the explicit configuration, with redacted endpoint diagnostics.
    #[must_use]
    pub const fn config(&self) -> &EvmConfig {
        &self.config
    }

    /// Returns the exact chain ID verified against the configured expectation.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
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

async fn verify_chain(http: &Client, config: &EvmConfig) -> Result<ChainId, Error> {
    for attempt in 0..=config.limits().max_retries() {
        match request_chain_id(http, config).await {
            Ok(chain_id) => {
                if chain_id != config.network().chain_id() {
                    return Err(Error::Provider(ProviderError::ChainMismatch));
                }
                return Ok(chain_id);
            }
            Err(failure) if failure.retry && attempt < config.limits().max_retries() => {
                sleep(RETRY_DELAY).await;
            }
            Err(failure) => return Err(failure.error),
        }
    }
    // The inclusive range always makes at least one attempt.
    Err(Error::Provider(ProviderError::Transport))
}

struct AttemptFailure {
    error: Error,
    retry: bool,
}

impl AttemptFailure {
    fn terminal(error: Error) -> Self {
        Self {
            error,
            retry: false,
        }
    }

    fn transport(error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            Self {
                error: Error::Timeout,
                retry: true,
            }
        } else {
            Self {
                error: Error::Provider(ProviderError::Transport),
                retry: true,
            }
        }
    }
}

#[derive(Serialize)]
struct ChainRequest {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: [(); 0],
}

async fn request_chain_id(http: &Client, config: &EvmConfig) -> Result<ChainId, AttemptFailure> {
    let body = serde_json::to_vec(&ChainRequest {
        jsonrpc: "2.0",
        id: REQUEST_ID,
        method: "eth_chainId",
        params: [],
    })
    .map_err(|_| AttemptFailure::terminal(Error::Configuration))?;
    let response = http
        .post(config.endpoint().url.clone())
        .headers(config.endpoint().headers.clone())
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await
        .map_err(|error| AttemptFailure::transport(&error))?;
    let status = response.status();
    if !status.is_success() {
        let error = if status == StatusCode::TOO_MANY_REQUESTS {
            Error::Provider(ProviderError::RateLimited)
        } else {
            Error::Provider(ProviderError::HttpStatus)
        };
        return Err(AttemptFailure {
            error,
            retry: status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error(),
        });
    }
    let bytes = bounded_body(response, config.limits().max_response_bytes()).await?;
    decode_chain_id(&bytes).map_err(AttemptFailure::terminal)
}

async fn bounded_body(mut response: Response, maximum: usize) -> Result<Vec<u8>, AttemptFailure> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(AttemptFailure::terminal(Error::Provider(
            ProviderError::ResponseTooLarge,
        )));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| AttemptFailure::transport(&error))?
    {
        if chunk.len() > maximum - body.len() {
            return Err(AttemptFailure::terminal(Error::Provider(
                ProviderError::ResponseTooLarge,
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

// Direct typed deserialization rejects duplicate envelope fields before any
// intermediate Value could overwrite them. The custom presence decoder keeps
// an explicit null result distinct from an absent result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcEnvelope {
    jsonrpc: String,
    id: u64,
    #[serde(default, deserialize_with = "present")]
    result: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    error: Option<RpcFailure>,
}

fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcFailure {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

fn decode_chain_id(bytes: &[u8]) -> Result<ChainId, Error> {
    let invalid = || Error::Provider(ProviderError::InvalidResponse);
    let envelope: RpcEnvelope = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if envelope.jsonrpc != "2.0" || envelope.id != REQUEST_ID {
        return Err(invalid());
    }
    match (envelope.result, envelope.error) {
        (Some(Value::String(quantity)), None) => parse_quantity(&quantity),
        (None, Some(failure)) => {
            // Type-check all fields, then discard the remote message and data.
            let RpcFailure {
                code: _code,
                message: _message,
                data: _data,
            } = failure;
            Err(Error::Provider(ProviderError::Rpc))
        }
        _ => Err(invalid()),
    }
}

fn parse_quantity(quantity: &str) -> Result<ChainId, Error> {
    let invalid = || Error::Provider(ProviderError::InvalidResponse);
    let digits = quantity.strip_prefix("0x").ok_or_else(invalid)?;
    if digits.is_empty()
        || digits.len() > 64
        || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err(invalid());
    }
    U256::from_str_radix(digits, 16)
        .map(ChainId::new)
        .map_err(|_| invalid())
}
