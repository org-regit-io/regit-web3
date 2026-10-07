// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure `DefiLlama` TVL, percentage yields, stablecoin and USD analytics values.
//!
//! Provider IDs and reporting breakdowns are retained as supplied; chain labels
//! are not treated as verified blockchain identities. USD valuations, peg-unit
//! circulating amounts and percentage yields stay distinct.
mod request;
mod value;
pub use request::{AnalyticsMetric, AnalyticsRequest, StablecoinId, StablecoinScope, TvlScope};
pub use value::{
    Analytics, AnalyticsTotals, BreakdownValue, ProtocolHistory, ProtocolTvl, Stablecoin,
    StablecoinHistory, StablecoinPoint, Stablecoins, TvlHistory, UsdPoint, YieldHistory,
    YieldPoint, YieldPool, YieldPools, YieldRates, YieldSymbol,
};
/// A validated provider protocol slug or pool identifier, not an on-chain identity.
pub type ProviderId = super::market::Identifier;
