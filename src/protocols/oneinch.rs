// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! 1inch Classic Swap v6.1 aggregated EVM quotes/routes and unsigned source review.
//!
//! Replaceable pure capabilities retain exact input units, source route graphs
//! and the original quote alongside fresh preparation. Optional outgoing HTTP
//! is explicit and read-only; it never approves, signs, relays or submits.
//! Source payload structural checks do not establish calldata ABI semantics.

use crate::{
    domain::oneinch::{LiquiditySources, PreparedSwap, Quote, QuoteRequest, Spender, SwapRequest},
    error::Error,
};
use std::future::Future;

/// Replaceable Classic Swap v6.1 read/unsigned-preparation capability.
pub trait ClassicSwapReader {
    /// Quotes exact raw input units with the actual bounded v6.1 route graph.
    /// # Errors
    /// Returns typed validation/provider/deadline failures; no unavailable amount becomes zero.
    fn quote_exact_input(
        &self,
        request: QuoteRequest,
    ) -> impl Future<Output = Result<Quote, Error>> + Send;
    /// Retrieves the complete bounded source liquidity catalogue for the configured chain.
    /// # Errors
    /// Rejects malformed/duplicate/excessive results rather than truncating them.
    fn get_liquidity_sources(&self)
    -> impl Future<Output = Result<LiquiditySources, Error>> + Send;
    /// Retrieves source-reported spender identity, without producing approvals.
    /// # Errors
    /// Returns malformed/provider/deadline failures without source diagnostic contents.
    fn get_spender(&self) -> impl Future<Output = Result<Spender, Error>> + Send;
    /// Generates an independent fresh source swap result for a new immutable review.
    /// # Errors
    /// Rejects caller/source identity, ordinary-envelope or floor conflicts before review.
    fn prepare_swap(
        &self,
        request: SwapRequest,
    ) -> impl Future<Output = Result<PreparedSwap, Error>> + Send;
}
#[cfg(feature = "oneinch-http")]
mod http;
#[cfg(feature = "oneinch-http")]
mod wire;
#[cfg(feature = "oneinch-http")]
pub use http::{OneinchClient, OneinchHttpConfig};
