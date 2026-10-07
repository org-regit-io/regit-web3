// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Shared bounded HTTP transport. Protocol decoding stays in each integration.

use std::{future::Future, time::Duration};

use reqwest::{Client, Response, StatusCode, header::CONTENT_TYPE};
use tokio::time::{Instant, sleep, timeout_at};
use url::Url;

use crate::{
    config::{HttpConfig, RpcLimits},
    error::{Error, ProviderError},
};

const RETRY_DELAY: Duration = Duration::from_millis(25);

pub(crate) struct OperationBudget {
    deadline: Instant,
}

impl OperationBudget {
    pub(crate) fn new(limits: RpcLimits) -> Result<Self, Error> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(Error::Configuration);
        }
        Ok(Self {
            deadline: Instant::now() + limits.request_timeout(),
        })
    }

    // The deadline covers all stages, retries and response reads. A second
    // check rejects success after synchronous decoding or construction ran late.
    pub(crate) async fn run<T, F>(&self, operation: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, Error>>,
    {
        let value = timeout_at(self.deadline, operation)
            .await
            .map_err(|_| Error::Timeout)??;
        if Instant::now() >= self.deadline {
            return Err(Error::Timeout);
        }
        Ok(value)
    }
}

pub(crate) struct HttpClient {
    config: HttpConfig,
    client: Client,
}

impl HttpClient {
    pub(crate) fn new(config: &HttpConfig) -> Result<Self, Error> {
        let client = Client::builder()
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
        Ok(Self {
            config: config.clone(),
            client,
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
                    sleep(RETRY_DELAY).await;
                }
                Ok(response) => return Ok(response),
                Err(failure) if failure.retry && attempt < self.config.limits().max_retries() => {
                    sleep(RETRY_DELAY).await;
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
        let request = match json_body {
            Some(body) => self
                .client
                .post(url.clone())
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_owned()),
            None => self.client.get(url.clone()),
        };
        let response = request
            .headers(self.config.endpoint().headers.clone())
            .send()
            .await
            .map_err(|error| AttemptFailure::transport(&error))?;
        let status = response.status();
        // Error bodies and remote messages are never retained. The status is
        // available privately for an integration's documented resource mapping.
        let body = if status.is_success() {
            bounded_body(response, self.config.limits().max_response_bytes()).await?
        } else {
            Vec::new()
        };
        Ok(HttpResponse { status, body })
    }
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

#[cfg(test)]
mod tests;
