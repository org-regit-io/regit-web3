// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Direct Uniswap V3 `QuoterV2` quotes, supplied-path comparison and unsigned
//! Universal Router 2.1.2 swap preparation.
//!
//! The pure capability has no HTTP/runtime requirements. The optional
//! `uniswap-http` backend composes the EVM canonical-hash call capability.
//! Comparisons cover only bounded caller-supplied paths, without global routing
//! or V4 support. Source simulations do not promise future swap execution.

use crate::{
    domain::{
        BlockSelector,
        uniswap::{RouteComparison, V3QuoteObservation, V3QuoteRequest, V3RouteRequest},
    },
    error::Error,
};
use std::future::Future;

#[cfg(feature = "uniswap-http")]
mod http;
#[cfg(feature = "uniswap-http")]
mod wire;
#[cfg(feature = "uniswap-http")]
pub use http::UniswapV3Client;

/// Replaceable pure capability for explicitly deployed V3 `QuoterV2` operations.
/// Implementations bind complete request identity and source provenance to one
/// captured canonical EVM hash. Signing, approvals and submission are separate.
pub trait V3QuoteReader {
    /// Quotes an exact forward V3 path with complete explicit simulation choices.
    /// # Errors
    /// Reports source reverts, unavailable/malformed state and backend failures.
    fn quote_exact_input(
        &self,
        request: V3QuoteRequest,
        selector: Option<BlockSelector>,
    ) -> impl Future<Output = Result<V3QuoteObservation, Error>> + crate::future::MaybeSend;
    /// Compares supplied paths at one captured hash, retaining each source revert.
    /// # Errors
    /// Reports mismatched identities/state, unknown provider errors or resource excess.
    fn compare_routes(
        &self,
        request: V3RouteRequest,
        selector: Option<BlockSelector>,
    ) -> impl Future<Output = Result<RouteComparison, Error>> + crate::future::MaybeSend;
}
