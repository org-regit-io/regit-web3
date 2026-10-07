// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Explicit Fetch policy with streamed limits and cancellation on every exit.

use js_sys::{Promise, Reflect, Uint8Array};
use reqwest::StatusCode;
use url::Url;
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AbortController, Headers, ReadableStreamDefaultReader, ReferrerPolicy, Request, RequestCache,
    RequestCredentials, RequestInit, RequestMode, RequestRedirect, Response,
};

use crate::{
    config::{HttpConfig, RpcEndpoint, RpcLimits},
    error::{Error, ProviderError},
    transport::clock::{Instant, timeout_at},
};

use super::{AttemptFailure, HttpResponse};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_name = fetch)]
    fn fetch(request: &Request) -> Result<Promise, JsValue>;
}

fn validate_endpoint(endpoint: &RpcEndpoint) -> Result<(), Error> {
    let fetch = Reflect::get(&js_sys::global(), &JsValue::from_str("fetch"))
        .map_err(|_| Error::Configuration)?;
    if !fetch.is_function() {
        return Err(Error::Configuration);
    }
    // Fetch rejects URL user information. Reject browser-owned headers rather
    // than silently ignoring an explicit caller authentication/policy setting.
    if !endpoint.url.username().is_empty() || endpoint.url.password().is_some() {
        return Err(Error::Configuration);
    }
    if endpoint
        .headers
        .keys()
        .any(|name| forbidden_header(name.as_str()))
    {
        return Err(Error::Configuration);
    }
    Ok(())
}

fn forbidden_header(name: &str) -> bool {
    name.starts_with("proxy-")
        || name.starts_with("sec-")
        || matches!(
            name,
            "accept-charset"
                | "accept-encoding"
                | "access-control-request-headers"
                | "access-control-request-method"
                | "connection"
                | "cookie"
                | "cookie2"
                | "date"
                | "dnt"
                | "expect"
                | "host"
                | "keep-alive"
                | "origin"
                | "permissions-policy"
                | "referer"
                | "te"
                | "trailer"
                | "transfer-encoding"
                | "upgrade"
                | "user-agent"
                | "via"
                | "x-http-method"
                | "x-http-method-override"
                | "x-method-override"
        )
}

pub(super) struct Backend {
    limits: RpcLimits,
}

impl Backend {
    pub(super) fn new(config: &HttpConfig) -> Result<Self, Error> {
        validate_endpoint(config.endpoint())?;
        Ok(Self {
            limits: config.limits(),
        })
    }

    pub(super) fn exchange(
        &self,
        config: &HttpConfig,
        url: &Url,
        body: Option<&[u8]>,
        content_type: &str,
    ) -> Result<Exchange, Error> {
        Exchange::build(config, self.limits, url, body, content_type)
    }
}

pub(super) struct Exchange {
    request: Request,
    abort: AbortController,
    limits: RpcLimits,
}

impl Exchange {
    fn build(
        config: &HttpConfig,
        limits: RpcLimits,
        url: &Url,
        body: Option<&[u8]>,
        content_type: &str,
    ) -> Result<Self, Error> {
        let headers = Headers::new().map_err(|_| Error::Configuration)?;
        for (name, value) in &config.endpoint().headers {
            headers
                .append(
                    name.as_str(),
                    value.to_str().map_err(|_| Error::Configuration)?,
                )
                .map_err(|_| Error::Configuration)?;
        }
        let abort = AbortController::new().map_err(|_| Error::Configuration)?;
        let init = RequestInit::new();
        init.set_method(if body.is_some() { "POST" } else { "GET" });
        init.set_mode(RequestMode::Cors);
        init.set_redirect(RequestRedirect::Error);
        init.set_credentials(RequestCredentials::Omit);
        init.set_cache(RequestCache::NoStore);
        init.set_referrer_policy(ReferrerPolicy::NoReferrer);
        init.set_referrer("");
        init.set_signal(Some(&abort.signal()));
        if let Some(body) = body {
            headers
                .set("content-type", content_type)
                .map_err(|_| Error::Configuration)?;
            init.set_body_opt_u8_array(Some(&Uint8Array::from(body)));
        }
        init.set_headers_headers(&headers);
        let request = Request::new_with_str_and_init(url.as_str(), &init)
            .map_err(|_| Error::Configuration)?;
        for (name, value) in &config.endpoint().headers {
            let actual = request
                .headers()
                .get(name.as_str())
                .map_err(|_| Error::Configuration)?;
            if actual.as_deref() != Some(value.to_str().map_err(|_| Error::Configuration)?) {
                return Err(Error::Configuration);
            }
        }
        Ok(Self {
            request,
            abort,
            limits,
        })
    }

    pub(super) async fn execute(self) -> Result<HttpResponse, AttemptFailure> {
        // Fetch exposes no separate socket-connection phase. Bound receipt of
        // the response headers by connect_timeout; body reads remain inside the
        // original total operation budget supplied by the parent transport.
        let deadline = Instant::now() + self.limits.connect_timeout();
        let response = timeout_at(deadline, async {
            let promise = fetch(&self.request).map_err(|_| transport_error())?;
            JsFuture::from(promise)
                .await
                .map_err(|_| transport_error())?
                .dyn_into::<Response>()
                .map_err(|_| invalid_response())
        })
        .await
        .map_err(|error| AttemptFailure {
            retry: matches!(
                error,
                Error::Timeout | Error::Provider(ProviderError::Transport)
            ),
            error,
        })?;
        let status = StatusCode::from_u16(response.status())
            .map_err(|_| AttemptFailure::terminal(invalid_response()))?;
        let body = if status.is_success() {
            bounded_body(&response, self.limits.max_response_bytes()).await?
        } else {
            // Remote error bodies and remote exception messages are discarded.
            Vec::new()
        };
        Ok(HttpResponse { status, body })
    }
}

impl Drop for Exchange {
    fn drop(&mut self) {
        // Also runs when the host future is cancelled, a shared deadline wins,
        // a stream exceeds its limit, or an error response is discarded.
        self.abort.abort();
    }
}

async fn bounded_body(response: &Response, maximum: usize) -> Result<Vec<u8>, AttemptFailure> {
    let content_length = response
        .headers()
        .get("content-length")
        .map_err(|_| AttemptFailure::terminal(invalid_response()))?
        .and_then(|length| length.parse::<u64>().ok());
    if content_length.is_some_and(|length| length > maximum as u64) {
        return Err(too_large());
    }
    let Some(stream) = response.body() else {
        return Ok(Vec::new());
    };
    let reader = ReadableStreamDefaultReader::new(&stream)
        .map_err(|_| AttemptFailure::terminal(invalid_response()))?;
    let mut body = Vec::new();
    loop {
        let result = JsFuture::from(reader.read())
            .await
            .map_err(|_| AttemptFailure {
                error: transport_error(),
                retry: true,
            })?;
        let done = Reflect::get(&result, &JsValue::from_str("done"))
            .map_err(|_| AttemptFailure::terminal(invalid_response()))?
            .as_bool()
            .ok_or_else(|| AttemptFailure::terminal(invalid_response()))?;
        if done {
            reader.release_lock();
            return Ok(body);
        }
        let chunk = Reflect::get(&result, &JsValue::from_str("value"))
            .map_err(|_| AttemptFailure::terminal(invalid_response()))?
            .dyn_into::<Uint8Array>()
            .map_err(|_| AttemptFailure::terminal(invalid_response()))?;
        let length = chunk.length() as usize;
        if length > maximum - body.len() {
            return Err(too_large());
        }
        let start = body.len();
        body.resize(start + length, 0);
        chunk.copy_to(&mut body[start..]);
    }
}

fn too_large() -> AttemptFailure {
    AttemptFailure::terminal(Error::Provider(ProviderError::ResponseTooLarge))
}

const fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

const fn transport_error() -> Error {
    Error::Provider(ProviderError::Transport)
}
