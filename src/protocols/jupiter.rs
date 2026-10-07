// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Jupiter Swap API V2 quote-only selection and fresh Metis V0 unsigned preparation.
//!
//! `/order` source selection and `/build` fresh routes are separate operations.
//! Prepared source instructions and lookup tables are not independently verified
//! swap semantics. Callers own review, signed-content verification and signing.
//! This module never uses Jupiter managed execution or submits transactions.
use crate::{
    domain::jupiter::{BuildRequest, Observation, Quote, QuoteRequest, SwapBuild},
    error::Error,
};
use std::future::Future;
#[cfg(feature = "jupiter-http")]
mod http;
#[cfg(feature = "jupiter-http")]
mod wire;
#[cfg(feature = "jupiter-http")]
pub use http::{JupiterClient, JupiterHttpConfig};
/// Replaceable quote-only and fresh unsigned-instruction source capabilities.
/// Implementations retain exact choices, bounded results and honest source attribution.
pub trait JupiterReader {
    /// Reads a quote without providing a taker or requesting a transaction.
    /// # Errors
    /// Returns typed implementation or source-identity failures.
    fn quote(
        &self,
        request: QuoteRequest,
    ) -> impl Future<Output = Result<Observation<Quote>, Error>> + crate::future::MaybeSend;
    /// Reads fresh Metis V0 instructions and actual hash/height expiry.
    /// An earlier order's route is not implicitly preserved or approved.
    /// # Errors
    /// Returns typed implementation, source correlation or bounded-data failures.
    fn build(
        &self,
        request: BuildRequest,
    ) -> impl Future<Output = Result<Observation<SwapBuild>, Error>> + crate::future::MaybeSend;
}
