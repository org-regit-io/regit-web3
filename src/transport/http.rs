// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bounded HTTP sending, URL policy, safe-read retries and one-shot writes.

use std::time::Duration;

use reqwest::StatusCode;
use url::Url;

#[cfg(any(feature = "evm-http", feature = "solana-http"))]
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    config::HttpConfig,
    error::{Error, ProviderError},
};

use super::{OperationBudget, clock::sleep};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod browser;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use browser::Backend;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use native::Backend;

#[cfg(any(
    feature = "xrpl-http",
    feature = "evm-http",
    feature = "ton-http",
    feature = "solana-http",
    feature = "blockfrost-http"
))]
use crate::error::SubmissionFailure;

const RETRY_DELAY: Duration = Duration::from_millis(25);

pub(crate) struct HttpClient {
    config: HttpConfig,
    backend: Backend,
}

impl HttpClient {
    // HTTP responses are associated with the request by their owning exchange.
    // Fixed ID 1 is local to that exchange; no global dispatcher is involved.
    #[cfg(any(feature = "evm-http", feature = "solana-http"))]
    pub(crate) async fn read_rpc<T, P>(
        &self,
        budget: &OperationBudget,
        method: &'static str,
        params: &P,
        error_policy: fn(i64) -> Error,
    ) -> Result<Option<T>, Error>
    where
        T: DeserializeOwned,
        P: Serialize + ?Sized,
    {
        const REQUEST_ID: u64 = 1;
        let body = super::rpc::encode_request(REQUEST_ID, method, params)?;
        let response = self
            .read(&[], &[], Some(&body), budget)
            .await?
            .into_success()?;
        super::rpc::decode_response(&response, REQUEST_ID, error_policy)
    }

    pub(crate) fn new(config: &HttpConfig) -> Result<Self, Error> {
        Ok(Self {
            config: config.clone(),
            backend: Backend::new(config)?,
        })
    }

    // Only integrations call this read-only entry point. A JSON body selects
    // POST, otherwise GET; mutation/submission must have a separate one-shot
    // path rather than reuse safe-read retries. Serialized requests and URLs
    // remain identical across attempts.
    pub(crate) async fn read(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        json_body: Option<&[u8]>,
        budget: &OperationBudget,
    ) -> Result<HttpResponse, Error> {
        let url = self.request_url(path, query)?;
        budget.run(self.read_attempts(&url, json_body)).await
    }

    // This boundary executes exactly one POST, regardless of safe-read retry
    // settings. Building and checking the deadline happen before execute. Once
    // execution is attempted, unresolved failures retain possible dispatch;
    // dropping the future also cannot establish that no submission occurred.
    #[cfg(any(
        feature = "xrpl-http",
        feature = "evm-http",
        feature = "ton-http",
        feature = "solana-http",
        all(test, feature = "blockfrost-http")
    ))]
    pub(crate) async fn write_once(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        json_body: &[u8],
        budget: &OperationBudget,
    ) -> Result<HttpResponse, Error> {
        self.write_once_with_content_type(path, query, json_body, "application/json", budget)
            .await
    }

    // Blockfrost accepts exact serialized transaction bytes, never JSON or safe-read retries.
    #[cfg(feature = "blockfrost-http")]
    pub(crate) async fn write_once_cbor(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        body: &[u8],
        budget: &OperationBudget,
    ) -> Result<HttpResponse, Error> {
        self.write_once_with_content_type(path, query, body, "application/cbor", budget)
            .await
    }

    #[cfg(any(
        feature = "xrpl-http",
        feature = "evm-http",
        feature = "ton-http",
        feature = "solana-http",
        feature = "blockfrost-http"
    ))]
    async fn write_once_with_content_type(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        body: &[u8],
        content_type: &'static str,
        budget: &OperationBudget,
    ) -> Result<HttpResponse, Error> {
        let url = self.request_url(path, query)?;
        let exchange = self
            .backend
            .exchange(&self.config, &url, Some(body), content_type)?;
        budget.check_remaining()?;
        budget
            .run(async {
                Box::pin(exchange.execute())
                    .await
                    .map_err(|failure| failure.error)
            })
            .await
            .map_err(submission_unknown)
    }

    fn request_url(&self, path: &[&str], query: &[(&str, &str)]) -> Result<Url, Error> {
        if path.iter().any(|segment| {
            segment.is_empty()
                || matches!(*segment, "." | "..")
                || segment.chars().any(char::is_control)
        }) {
            return Err(Error::Configuration);
        }
        let mut url = self.config.endpoint().url.clone();
        if !path.is_empty() {
            url.path_segments_mut()
                .map_err(|()| Error::Configuration)?
                .pop_if_empty()
                .extend(path);
        }
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query.iter().copied());
        }
        Ok(url)
    }

    async fn read_attempts(
        &self,
        url: &Url,
        json_body: Option<&[u8]>,
    ) -> Result<HttpResponse, Error> {
        for attempt in 0..=self.config.limits().max_retries() {
            match self.request(url, json_body).await {
                Ok(response)
                    if retry_status(response.status)
                        && attempt < self.config.limits().max_retries() =>
                {
                    sleep(RETRY_DELAY).await?;
                }
                Ok(response) => return Ok(response),
                Err(failure) if failure.retry && attempt < self.config.limits().max_retries() => {
                    sleep(RETRY_DELAY).await?;
                }
                Err(failure) => return Err(failure.error),
            }
        }
        // The inclusive range always makes at least one attempt.
        Err(Error::Provider(ProviderError::Transport))
    }

    async fn request(
        &self,
        url: &Url,
        json_body: Option<&[u8]>,
    ) -> Result<HttpResponse, AttemptFailure> {
        let exchange = self
            .backend
            .exchange(&self.config, url, json_body, "application/json")
            .map_err(AttemptFailure::terminal)?;
        // Keep transport state from multiplying the stack size of composed
        // protocol and provider operation futures.
        Box::pin(exchange.execute()).await
    }
}

#[cfg(any(
    feature = "xrpl-http",
    feature = "evm-http",
    feature = "ton-http",
    feature = "solana-http",
    feature = "blockfrost-http"
))]
pub(crate) const fn submission_unknown(error: Error) -> Error {
    let reason = match error {
        Error::SubmissionOutcomeUnknown(reason) => return Error::SubmissionOutcomeUnknown(reason),
        Error::Timeout => SubmissionFailure::Timeout,
        Error::Provider(ProviderError::Transport) => SubmissionFailure::Transport,
        Error::Provider(ProviderError::RateLimited) => SubmissionFailure::RateLimited,
        Error::Provider(ProviderError::HttpStatus) => SubmissionFailure::HttpStatus,
        Error::Provider(ProviderError::ResponseTooLarge) => SubmissionFailure::ResponseTooLarge,
        Error::Provider(ProviderError::Rpc) | Error::ExecutionReverted => SubmissionFailure::Rpc,
        Error::Configuration
        | Error::Validation(_)
        | Error::UnsupportedCapability
        | Error::UnavailableData
        | Error::Provider(ProviderError::InvalidResponse | ProviderError::ChainMismatch) => {
            SubmissionFailure::InvalidResponse
        }
    };
    Error::SubmissionOutcomeUnknown(reason)
}

pub(crate) struct HttpResponse {
    pub(crate) status: StatusCode,
    body: Vec<u8>,
}

impl HttpResponse {
    pub(crate) fn into_success(self) -> Result<Vec<u8>, Error> {
        if self.status.is_success() {
            Ok(self.body)
        } else if self.status == StatusCode::TOO_MANY_REQUESTS {
            Err(Error::Provider(ProviderError::RateLimited))
        } else {
            Err(Error::Provider(ProviderError::HttpStatus))
        }
    }
}

fn retry_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
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
}

#[cfg(all(test, not(all(target_arch = "wasm32", target_os = "unknown"))))]
#[path = "tests.rs"]
mod tests;

#[cfg(all(
    test,
    not(all(target_arch = "wasm32", target_os = "unknown")),
    any(
        feature = "xrpl-http",
        feature = "evm-http",
        feature = "ton-http",
        feature = "solana-http",
        feature = "blockfrost-http"
    )
))]
#[path = "write_tests.rs"]
mod write_tests;
