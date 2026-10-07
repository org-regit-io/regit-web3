// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Jupiter Swap API V2 source routes and fresh V0 instruction preparation.
//!
//! API network identity is an explicit mainnet declaration, not genesis
//! verification. Raw token units retain no inferred precision. Fresh `/build`
//! instructions and source lookup tables are structurally validated; they do
//! not prove the compiled transaction performs the declared trade.

mod build;
mod context;
mod estimate;
mod instruction;
mod preparation;
mod quote;
mod request;

pub use build::{FetchedAt, SwapBuild, SwapBuildData};
pub use context::{Context, Observation, Operation, OperationValue};
pub use estimate::SwapEstimate;
pub use instruction::{AccountMeta, Instruction, InstructionData, InstructionFields, LookupTable};
pub use preparation::{
    OtherInstructionPlacement, PreparationSettings, PreparedSwap, SwapIntent, SwapIntentData,
    UnsignedSwap,
};
pub use quote::{
    ExpiryLiteral, Quote, QuoteData, QuoteFees, RouteStep, RouteStepData, Router, SourceValidity,
};
pub use request::{
    BuildRequest, BuildRequestData, Destination, DexFilter, Label, QuoteRequest, QuoteRequestData,
    SlippageBps, mainnet,
};
