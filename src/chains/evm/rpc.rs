// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Private read-only RPC transport and strict response decoding.

use std::time::Duration;

use reqwest::{Client, Response, StatusCode, header::CONTENT_TYPE};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::time::sleep;

use crate::{
    config::EvmConfig,
    domain::U256,
    error::{Error, ProviderError},
};

// Every HTTP exchange owns its response. There is no shared RPC dispatcher or
// multiplexed response stream; matching this ID is local to that exchange.
const REQUEST_ID: u64 = 1;
const RETRY_DELAY: Duration = Duration::from_millis(25);

pub(super) enum ErrorPolicy {
    ChainIdentity,
    StateRead,
}

pub(super) async fn read<T, P>(
    http: &Client,
    config: &EvmConfig,
    method: &'static str,
    params: &P,
    policy: ErrorPolicy,
) -> Result<Option<T>, Error>
where
    T: DeserializeOwned,
    P: Serialize + ?Sized,
{
    let body = serde_json::to_vec(&RpcRequest {
        jsonrpc: "2.0",
        id: REQUEST_ID,
        method,
        params,
    })
    .map_err(|_| Error::Configuration)?;
    // Retain the exact serialized request across retries, including a captured
    // EIP-1898 hash when this is a balance request. Never re-resolve its anchor.
    for attempt in 0..=config.limits().max_retries() {
        match request(http, config, &body).await {
            Ok(bytes) => return decode_response(&bytes, &policy),
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
        Self {
            error: if error.is_timeout() {
                Error::Timeout
            } else {
                Error::Provider(ProviderError::Transport)
            },
            retry: true,
        }
    }
}

#[derive(Serialize)]
struct RpcRequest<'a, P: ?Sized> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: &'a P,
}

async fn request(
    http: &Client,
    config: &EvmConfig,
    body: &[u8],
) -> Result<Vec<u8>, AttemptFailure> {
    let response = http
        .post(config.endpoint().url.clone())
        .headers(config.endpoint().headers.clone())
        .header(CONTENT_TYPE, "application/json")
        .body(body.to_owned())
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
    bounded_body(response, config.limits().max_response_bytes()).await
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

// Direct typed deserialization rejects duplicate envelope and result fields
// before any intermediate Value can overwrite them. Explicit presence keeps
// a present null result distinct from an absent result without untagged enums.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
struct RpcEnvelope<T> {
    jsonrpc: String,
    id: u64,
    #[serde(default, deserialize_with = "result_presence")]
    result: ResultPresence<T>,
    #[serde(default, deserialize_with = "present")]
    error: Option<RpcFailure>,
}

#[derive(Default)]
enum ResultPresence<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

fn result_presence<'de, D, T>(deserializer: D) -> Result<ResultPresence<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|value| match value {
        Some(value) => ResultPresence::Value(value),
        None => ResultPresence::Null,
    })
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

fn decode_response<T: DeserializeOwned>(
    bytes: &[u8],
    policy: &ErrorPolicy,
) -> Result<Option<T>, Error> {
    let invalid = || Error::Provider(ProviderError::InvalidResponse);
    let envelope: RpcEnvelope<T> = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if envelope.jsonrpc != "2.0" || envelope.id != REQUEST_ID {
        return Err(invalid());
    }
    match (envelope.result, envelope.error) {
        (ResultPresence::Value(value), None) => Ok(Some(value)),
        (ResultPresence::Null, None) => Ok(None),
        (ResultPresence::Missing, Some(failure)) => {
            let RpcFailure {
                code,
                message: _message,
                data: _data,
            } = failure;
            // EIP-1474 assigns these method/resource meanings. EIP-1898's
            // recommended noncanonical code (-32000) is also invalid input:
            // retain a generic RPC failure rather than guessing from messages.
            Err(match (policy, code) {
                (ErrorPolicy::StateRead, -32601 | -32004) => Error::UnsupportedCapability,
                (ErrorPolicy::StateRead, -32001 | -32002) => Error::UnavailableData,
                _ => Error::Provider(ProviderError::Rpc),
            })
        }
        _ => Err(invalid()),
    }
}

pub(super) fn parse_quantity(quantity: &str) -> Result<U256, Error> {
    let invalid = || Error::Provider(ProviderError::InvalidResponse);
    let digits = quantity.strip_prefix("0x").ok_or_else(invalid)?;
    if digits.is_empty()
        || digits.len() > 64
        || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err(invalid());
    }
    U256::from_str_radix(digits, 16).map_err(|_| invalid())
}
