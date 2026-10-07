// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Native Reqwest exchange policy and incrementally bounded response reads.

use reqwest::{Client, Request, Response, header::CONTENT_TYPE};
use url::Url;

use crate::{
    config::HttpConfig,
    error::{Error, ProviderError},
};

use super::{AttemptFailure, HttpResponse};

pub(super) struct Backend {
    client: Client,
}

impl Backend {
    pub(super) fn new(config: &HttpConfig) -> Result<Self, Error> {
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
        Ok(Self { client })
    }

    pub(super) fn exchange(
        &self,
        config: &HttpConfig,
        url: &Url,
        body: Option<&[u8]>,
        content_type: &str,
    ) -> Result<Exchange, Error> {
        let request = match body {
            Some(body) => self
                .client
                .post(url.clone())
                .header(CONTENT_TYPE, content_type)
                .body(body.to_owned()),
            None => self.client.get(url.clone()),
        }
        .headers(config.endpoint().headers.clone())
        .build()
        .map_err(|_| Error::Configuration)?;
        Ok(Exchange {
            client: self.client.clone(),
            request,
            maximum: config.limits().max_response_bytes(),
        })
    }
}

pub(super) struct Exchange {
    client: Client,
    request: Request,
    maximum: usize,
}

impl Exchange {
    pub(super) async fn execute(self) -> Result<HttpResponse, AttemptFailure> {
        let response = self
            .client
            .execute(self.request)
            .await
            .map_err(|error| transport_failure(&error))?;
        let status = response.status();
        let body = if status.is_success() {
            bounded_body(response, self.maximum).await?
        } else {
            // Remote error bodies and remote exception messages are discarded.
            Vec::new()
        };
        Ok(HttpResponse { status, body })
    }
}

fn transport_failure(error: &reqwest::Error) -> AttemptFailure {
    AttemptFailure {
        error: if error.is_timeout() {
            Error::Timeout
        } else {
            Error::Provider(ProviderError::Transport)
        },
        retry: true,
    }
}

async fn bounded_body(mut response: Response, maximum: usize) -> Result<Vec<u8>, AttemptFailure> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| transport_failure(&error))?
    {
        if chunk.len() > maximum - body.len() {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn too_large() -> AttemptFailure {
    AttemptFailure::terminal(Error::Provider(ProviderError::ResponseTooLarge))
}
