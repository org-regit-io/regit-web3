// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Helius DAS assets and current Parsed Events transactions/address history.
//!
//! Pure capabilities require no network, runtime or credentials. `helius-http`
//! adds an explicit replaceable outgoing backend. Provider interpretation and
//! index progress remain distinct from canonical Solana bytes/finality proof.

use crate::{
    domain::helius::{
        Asset, AssetRequest, HistoryPage, HistoryRequest, Observation, OwnerPage, OwnerRequest,
        ParseRequest, ParsedBatch,
    },
    error::Error,
};
use std::future::Future;
#[cfg(feature = "helius-http")]
mod http;
#[cfg(feature = "helius-http")]
mod wire;
#[cfg(feature = "helius-http")]
pub use http::{AssetContinuation, HeliusClient, HeliusHttpConfig, HistoryContinuation};

/// Replaceable read-only Helius data capability. Callers own credentials and runtime.
pub trait HeliusReader {
    /// Reads exact asset identity, ownership, compression and supported token metadata.
    fn get_asset(
        &self,
        request: AssetRequest,
    ) -> impl Future<Output = Result<Observation<Asset>, Error>> + crate::future::MaybeSend;
    /// Reads one explicitly selected owner page, without automatic pagination.
    fn get_assets_by_owner(
        &self,
        request: OwnerRequest,
    ) -> impl Future<Output = Result<Observation<OwnerPage>, Error>> + crate::future::MaybeSend;
    /// Parses an ordered batch, retaining duplicate input positions and parser errors.
    fn parse_transactions(
        &self,
        request: ParseRequest,
    ) -> impl Future<Output = Result<Observation<ParsedBatch>, Error>> + crate::future::MaybeSend;
    /// Reads one parsed address-history page with exact source continuation metadata.
    fn get_address_history(
        &self,
        request: HistoryRequest,
    ) -> impl Future<Output = Result<Observation<HistoryPage>, Error>> + crate::future::MaybeSend;
}
