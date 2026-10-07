// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! LI.FI cross-family quotes, route alternatives, source preparation and status.
//!
//! Pure capabilities use explicit qualified identities and exact values.
//! `lifi-http` adds a replaceable outgoing backend with explicit endpoint,
//! credentials, chain catalogue and capacities. Preparation never submits.
//! Provider metadata and encoded payloads are source suggestions, not verified
//! signed content or independent consensus proof.

use crate::{
    domain::lifi::{PreparedStep, Request, Routes, Status, StatusQuery, StepView},
    error::Error,
};
use std::future::Future;

#[cfg(feature = "lifi-http")]
mod http;
#[cfg(feature = "lifi-http")]
mod wire;
#[cfg(feature = "lifi-http")]
pub use http::{LifiClient, LifiHttpConfig, LifiStepHandle};

/// Replaceable read/preparation capability with backend-specific continuations.
/// Handles preserve immutable typed selections while implementations retain any
/// private source continuation needed to prepare that exact selected step.
pub trait LifiReader {
    /// Immutable typed step and backend-specific continuation.
    type StepHandle: StepView;
    /// Returns one exact-input source quote, without selecting or signing it.
    fn get_quote(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<Self::StepHandle, Error>> + Send;
    /// Returns bounded alternatives without choosing one or implicitly paging.
    fn get_routes(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<Routes<Self::StepHandle>, Error>> + Send;
    /// Returns a fresh source preparation beside the immutable selected snapshot.
    /// The caller reviews the fresh estimate/payload; no transaction is submitted.
    fn prepare_step(
        &self,
        selected: &Self::StepHandle,
    ) -> impl Future<Output = Result<PreparedStep, Error>> + Send;
    /// Returns attributed progress with actual transaction hashes/units/times.
    fn get_status(&self, query: StatusQuery) -> impl Future<Output = Result<Status, Error>> + Send;
}
