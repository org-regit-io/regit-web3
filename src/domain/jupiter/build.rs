// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    BuildRequest, Context, Instruction, LookupTable, Operation, OperationValue, RouteStep,
    quote::validate_routes,
};
use crate::{
    domain::{
        ExactDecimal,
        solana::{Hash, Pubkey},
    },
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};
/// Exact source blockhash fetch time, independently of local API retrieval time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TimeFields", into = "TimeFields")]
pub struct FetchedAt {
    seconds: u64,
    nanoseconds: u32,
}
impl FetchedAt {
    /// Validates a source Unix second/nanosecond pair without rounding.
    /// # Errors
    /// Rejects nanoseconds outside one second.
    pub fn new(seconds: u64, nanoseconds: u32) -> Result<Self, Error> {
        if nanoseconds >= 1_000_000_000 {
            return Err(invalid());
        }
        Ok(Self {
            seconds,
            nanoseconds,
        })
    }
    /// Returns exact source Unix whole seconds.
    #[must_use]
    pub const fn seconds(self) -> u64 {
        self.seconds
    }
    /// Returns the exact subsecond component.
    #[must_use]
    pub const fn nanoseconds(self) -> u32 {
        self.nanoseconds
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeFields {
    seconds: u64,
    nanoseconds: u32,
}
impl TryFrom<TimeFields> for FetchedAt {
    type Error = Error;
    fn try_from(v: TimeFields) -> Result<Self, Error> {
        Self::new(v.seconds, v.nanoseconds)
    }
}
impl From<FetchedAt> for TimeFields {
    fn from(v: FetchedAt) -> Self {
        Self {
            seconds: v.seconds,
            nanoseconds: v.nanoseconds,
        }
    }
}
/// Complete fresh Metis V0 quote/instruction response, not an earlier order's transaction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwapBuildData {
    /// Complete exact build request; all defaults were explicit query choices.
    pub request: BuildRequest,
    /// Fresh source output raw units.
    pub out_amount: u64,
    /// Fresh source output floor, preserved without changing endpoint rounding.
    pub other_amount_threshold: u64,
    /// Source price-impact ratio, retained exactly.
    pub price_impact: ExactDecimal,
    /// Fresh actual Metis route plan.
    pub routes: Vec<RouteStep>,
    /// Exact source compute-budget price instructions, with no CU-limit assumption.
    pub compute_budget: Vec<Instruction>,
    /// Exact ordered source pre-swap instructions.
    pub setup: Vec<Instruction>,
    /// Exact Jupiter V6 program swap instruction; semantics remain unverified.
    pub swap: Instruction,
    /// Actual optional source cleanup instruction.
    pub cleanup: Option<Instruction>,
    /// Exact source additional instructions; the caller must select their insertion point.
    pub other: Vec<Instruction>,
    /// Full ordered source lookup-table entries, not authenticated account state.
    pub lookup_tables: Vec<LookupTable>,
    /// Actual source recent blockhash used for compilation.
    pub blockhash: Hash,
    /// Actual source final valid height; never a slot or an invented timestamp.
    pub last_valid_block_height: u64,
    /// Exact source fetch time for this blockhash.
    pub blockhash_fetched_at: FetchedAt,
}
/// Immutable bounded fresh V0 Metis source instructions and real height-based expiry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SwapBuildData", into = "SwapBuildData")]
pub struct SwapBuild(SwapBuildData);
impl SwapBuild {
    /// Maximum retained lookup tables per build; each table has at most 256 entries.
    pub const MAX_LOOKUP_TABLES: usize = 64;
    /// Maximum retained source instructions across every instruction group.
    pub const MAX_INSTRUCTIONS: usize = 64;
    /// Validates request bounds, actual output floor, current swap program and collection limits.
    /// Opaque instruction/lookup content is not proof of the quoted trade or account state.
    /// # Errors
    /// Rejects weaker output, disconnected/excessive routes, source collections or wrong swap program.
    pub fn new(data: SwapBuildData) -> Result<Self, Error> {
        let r = data.request.data();
        if data.other_amount_threshold < data.request.data().minimum_output
            || data.other_amount_threshold > data.out_amount
            || data.other_amount_threshold < r.slippage.floor(data.out_amount)
            || data.routes.len() > usize::from(r.max_route_steps)
            || data.lookup_tables.len() > Self::MAX_LOOKUP_TABLES
            || data.compute_budget.len()
                + data.setup.len()
                + data.other.len()
                + usize::from(data.cleanup.is_some())
                + 1
                > Self::MAX_INSTRUCTIONS
            || data.swap.fields().program_id
                != Pubkey::parse("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4")?
            || data.lookup_tables.iter().enumerate().any(|(i, t)| {
                data.lookup_tables[..i]
                    .iter()
                    .any(|p| p.address() == t.address())
            })
            || data.routes.iter().any(|leg| match &r.dex_filter {
                super::DexFilter::All => false,
                super::DexFilter::Include(labels) => !labels.contains(&leg.data().label),
                super::DexFilter::Exclude(labels) => labels.contains(&leg.data().label),
            })
        {
            return Err(invalid());
        }
        validate_routes(&data.routes, r.input_mint, r.output_mint, false)?;
        let result = Self(data);
        super::preparation::price(&result).map_err(|_| invalid())?;
        Ok(result)
    }
    /// Returns the complete exact fresh source result without mutable access.
    #[must_use]
    pub const fn data(&self) -> &SwapBuildData {
        &self.0
    }
    /// Returns the validated V0 source compute price in micro-lamports per CU.
    /// # Errors
    /// Reports a fixed failure if the supported price-instruction profile differs.
    pub fn compute_unit_price(&self) -> Result<u64, Error> {
        super::preparation::price(self)
    }
}
impl TryFrom<SwapBuildData> for SwapBuild {
    type Error = Error;
    fn try_from(v: SwapBuildData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<SwapBuild> for SwapBuildData {
    fn from(v: SwapBuild) -> Self {
        v.0
    }
}
impl OperationValue for SwapBuild {
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        if c.operation() != Operation::Build || c.network() != &self.0.request.data().network {
            return Err(invalid());
        }
        Ok(())
    }
}
fn invalid() -> Error {
    ValidationError::InvalidJupiterBuild.into()
}
