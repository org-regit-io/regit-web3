// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact Uniswap V3 quotes, caller-supplied routes and Universal Router 2.1.2 preparation.
//!
//! Raw token units retain no assumed precision. Quotes are source simulations;
//! unsigned preparation never approves, signs or submits a transaction.

mod deployment;
pub(crate) mod encoding;
mod path;
mod preparation;
mod quote;
mod request;

pub use deployment::{V3Deployment, V3DeploymentData};
pub use path::{V3Path, V3PathData};
pub use preparation::{
    AllowanceRequirements, PreparedSwap, SlippageBps, SwapIntent, SwapIntentData, UnsignedSwap,
};
pub use quote::{
    RouteComparison, RouteComparisonData, RouteOutcome, V3Quote, V3QuoteData, V3QuoteObservation,
};
pub use request::{
    CallSettings, CallSettingsData, V3QuoteRequest, V3QuoteRequestData, V3RouteRequest,
    V3RouteRequestData,
};
