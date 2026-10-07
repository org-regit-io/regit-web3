// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Rubic API-v2 direct-route quotes, unsigned data and source-reported progress.
//!
//! The profile excludes deposit and private routes. Provider-selected routes and
//! fresh unsigned preparations are separate snapshots. Preparation does not
//! authenticate program semantics, approve, sign, relay or submit transactions.
use crate::{
    domain::rubic::{
        Chains, Observation, PreparationRequest, PreparedSwap, Quote, QuoteRequest, Routes, Status,
        StatusQuery,
    },
    error::Error,
};
use std::future::Future;
#[cfg(feature = "rubic-http")]
mod http;
#[cfg(feature = "rubic-http")]
mod wire;
#[cfg(feature = "rubic-http")]
pub use http::{RubicClient, RubicHttpConfig};

/// Replaceable direct-route API source with no runtime requirement in its contract.
pub trait RubicReader {
    /// Reads the bounded source chain catalogue, without claiming genesis proof.
    /// # Errors
    /// Returns source, configuration or bounded-data failures.
    fn chains(
        &self,
        include_testnets: bool,
    ) -> impl Future<Output = Result<Observation<Chains>, Error>> + Send;
    /// Reads all source-selected direct routes within caller capacities.
    /// # Errors
    /// Returns source, correlation or bounded-data failures.
    fn quote_all(
        &self,
        request: QuoteRequest,
    ) -> impl Future<Output = Result<Observation<Routes>, Error>> + Send;
    /// Reads the source's selected best direct route; global optimality is unverified.
    /// # Errors
    /// Returns source, correlation or bounded-data failures.
    fn quote_best(
        &self,
        request: QuoteRequest,
    ) -> impl Future<Output = Result<Observation<Quote>, Error>> + Send;
    /// Requests unsigned data for a selected direct route, retaining fresh estimates.
    /// # Errors
    /// Returns correlation, source or unsupported-payload failures.
    fn prepare(
        &self,
        request: PreparationRequest,
    ) -> impl Future<Output = Result<Observation<PreparedSwap>, Error>> + Send;
    /// Reads current `statusExtended` provider facts for an explicit ID and source hash.
    /// # Errors
    /// Returns source, destination-identity or bounded-data failures.
    fn status(
        &self,
        query: StatusQuery,
    ) -> impl Future<Output = Result<Observation<Status>, Error>> + Send;
}
