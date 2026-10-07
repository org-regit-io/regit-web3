// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Typed market, analytics and indexed-data provider integrations.

#[cfg(any(feature = "coingecko-http", feature = "defillama-http"))]
mod market_wire;

#[cfg(feature = "coingecko")]
pub mod coingecko;

#[cfg(feature = "defillama")]
pub mod defillama;

#[cfg(feature = "helius")]
pub mod helius;

#[cfg(feature = "blockfrost")]
pub mod blockfrost;

#[cfg(feature = "mempool-space")]
pub mod mempool_space;
