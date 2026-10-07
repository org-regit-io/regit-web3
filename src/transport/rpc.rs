// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure typed JSON-RPC request encoding and strict response decoding.

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{DeserializeOwned, IgnoredAny},
};

use crate::error::{Error, ProviderError};

pub(super) fn encode_request<P>(request_id: u64, method: &str, params: &P) -> Result<Vec<u8>, Error>
where
    P: Serialize + ?Sized,
{
    serde_json::to_vec(&Request {
        jsonrpc: "2.0",
        id: request_id,
        method,
        params,
    })
    .map_err(|_| Error::Configuration)
}

#[derive(Serialize)]
struct Request<'a, P: ?Sized> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: &'a P,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
struct Envelope<T> {
    jsonrpc: String,
    id: u64,
    #[serde(default, deserialize_with = "result_presence")]
    result: ResultPresence<T>,
    #[serde(default, deserialize_with = "present")]
    error: Option<Failure>,
}

#[derive(Default)]
enum ResultPresence<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

fn result_presence<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<ResultPresence<T>, D::Error> {
    Option::<T>::deserialize(deserializer).map(|value| match value {
        Some(value) => ResultPresence::Value(value),
        None => ResultPresence::Null,
    })
}

fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    code: i64,
    #[serde(rename = "message")]
    _message: String,
    #[serde(default, rename = "data")]
    _data: Option<IgnoredAny>,
}

pub(super) fn decode_response<T: DeserializeOwned>(
    bytes: &[u8],
    expected_id: u64,
    error_policy: fn(i64) -> Error,
) -> Result<Option<T>, Error> {
    let invalid = || Error::Provider(ProviderError::InvalidResponse);
    let envelope: Envelope<T> = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if envelope.jsonrpc != "2.0" || envelope.id != expected_id {
        return Err(invalid());
    }
    match (envelope.result, envelope.error) {
        (ResultPresence::Value(value), None) => Ok(Some(value)),
        (ResultPresence::Null, None) => Ok(None),
        (ResultPresence::Missing, Some(failure)) => Err(error_policy(failure.code)),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{decode_response, encode_request};
    use crate::error::{Error, ProviderError};

    const fn policy(_code: i64) -> Error {
        Error::UnsupportedCapability
    }

    #[test]
    fn codec_associates_only_the_explicit_expected_request_id() {
        let request = encode_request(42, "read", &[7_u64]).unwrap();
        assert_eq!(
            request,
            br#"{"jsonrpc":"2.0","id":42,"method":"read","params":[7]}"#
        );
        let response = br#"{"jsonrpc":"2.0","id":42,"result":7}"#;
        assert_eq!(
            decode_response::<u64>(response, 42, policy).unwrap(),
            Some(7)
        );
        assert_eq!(
            decode_response::<u64>(response, 1, policy).unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
    }

    #[test]
    fn codec_preserves_typed_duplicate_detection_and_null_presence() {
        #[derive(Debug, Deserialize)]
        struct ResultValue {
            #[serde(rename = "amount")]
            _amount: u64,
        }

        let duplicate = br#"{"jsonrpc":"2.0","id":42,"result":{"amount":1,"amount":2}}"#;
        assert_eq!(
            decode_response::<ResultValue>(duplicate, 42, policy).unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        let null = br#"{"jsonrpc":"2.0","id":42,"result":null}"#;
        assert!(decode_response::<u64>(null, 42, policy).unwrap().is_none());
        let missing = br#"{"jsonrpc":"2.0","id":42}"#;
        assert_eq!(
            decode_response::<u64>(missing, 42, policy).unwrap_err(),
            Error::Provider(ProviderError::InvalidResponse)
        );
        let failure = br#"{"jsonrpc":"2.0","id":42,"error":{"code":-32601,"message":"FAKE_SECRET","data":{"key":"FAKE_KEY"}}}"#;
        assert_eq!(
            decode_response::<u64>(failure, 42, policy).unwrap_err(),
            Error::UnsupportedCapability
        );
    }
}
