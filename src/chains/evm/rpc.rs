// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Private read-only RPC transport and strict response decoding.

use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    domain::U256,
    error::{Error, ProviderError},
    transport::{HttpClient, OperationBudget},
};

// Every HTTP exchange owns its response. There is no shared RPC dispatcher or
// multiplexed response stream; matching this ID is local to that exchange.
const REQUEST_ID: u64 = 1;

pub(super) enum ErrorPolicy {
    ChainIdentity,
    StateRead,
}

pub(super) async fn read<T, P>(
    http: &HttpClient,
    budget: &OperationBudget,
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
    let bytes = http
        .read(&[], &[], Some(&body), budget)
        .await?
        .into_success()?;
    decode_response(&bytes, &policy)
}

#[derive(Serialize)]
struct RpcRequest<'a, P: ?Sized> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: &'a P,
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
