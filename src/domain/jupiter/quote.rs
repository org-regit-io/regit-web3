// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Context, Label, Operation, OperationValue, QuoteRequest};
use crate::{
    domain::{ExactDecimal, solana::Pubkey},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};
use std::fmt;
/// Current Swap API V2 selection engine, independently of individual DEX legs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Router {
    /// Jupiter's on-chain router.
    Metis,
    /// `JupiterZ` market-maker selection.
    JupiterZ,
    /// Source-selected Dflow engine.
    Dflow,
    /// Source-selected OKX engine.
    Okx,
}
impl Router {
    /// Returns the exact case-sensitive API query value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metis => "metis",
            Self::JupiterZ => "jupiterz",
            Self::Dflow => "dflow",
            Self::Okx => "okx",
        }
    }
}
/// Exact source route leg, without independently verified liquidity or conservation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteStepData {
    /// Exact source AMM account identity; repeated accounts can be legitimate.
    pub amm: Pubkey,
    /// Bounded source label, independent of account identity.
    pub label: Label,
    /// Exact source input mint.
    pub input_mint: Pubkey,
    /// Exact source output mint.
    pub output_mint: Pubkey,
    /// Exact source input raw units.
    pub in_amount: u64,
    /// Exact source output raw units.
    pub out_amount: u64,
    /// Source percentage retained exactly without rounding to an integer.
    pub percent: ExactDecimal,
    /// Separately supplied source basis-point share; not summed across serial hops.
    pub bps: u16,
}
/// Immutable source route leg preserving exact percentages and raw token units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RouteStepData", into = "RouteStepData")]
pub struct RouteStep(RouteStepData);
impl RouteStep {
    /// Validates mint distinctness and bounded source percentage fields.
    /// # Errors
    /// Rejects identical mints, negative/excessive percentage or excess basis points.
    pub fn new(data: RouteStepData) -> Result<Self, Error> {
        if data.input_mint == data.output_mint
            || data.percent.is_negative()
            || data.percent > ExactDecimal::parse("100")?
            || data.bps > 10_000
        {
            return Err(invalid());
        }
        Ok(Self(data))
    }
    /// Returns all immutable source facts without treating them as chain proof.
    #[must_use]
    pub const fn data(&self) -> &RouteStepData {
        &self.0
    }
}
impl TryFrom<RouteStepData> for RouteStep {
    type Error = Error;
    fn try_from(v: RouteStepData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<RouteStep> for RouteStepData {
    fn from(v: RouteStep) -> Self {
        v.0
    }
}
/// Optional source quote fee facts, kept absent rather than defaulted to zero.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteFees {
    /// Source total swap fee in basis points, including any source recoup component.
    pub total_fee_bps: Option<u16>,
    /// Source platform fee in basis points, if reported.
    pub platform_fee_bps: Option<u16>,
    /// Source total swap fee mint, if reported.
    pub fee_mint: Option<Pubkey>,
    /// Separately reported platform fee mint, if supplied.
    pub platform_fee_mint: Option<Pubkey>,
    /// Source platform fee amount, with enclosing mint units.
    pub platform_fee_amount: Option<u64>,
    /// Source-estimated signature fee in lamports, if reported.
    pub signature_fee_lamports: Option<u64>,
    /// Source-estimated priority fee in lamports, if reported.
    pub priority_fee_lamports: Option<u64>,
    /// Source-estimated rent fee in lamports, if reported.
    pub rent_fee_lamports: Option<u64>,
}
/// Source-reported quote validity, distinct from a compiled transaction lifetime.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceValidity {
    /// Optional provider `expireAt` literal. The API schema declares no format;
    /// this bounded opaque value is not normalized or used as a local TTL.
    pub expires_at_literal: Option<ExpiryLiteral>,
    /// Optional source height, without an accompanying transaction/hash binding.
    pub last_valid_block_height: Option<u64>,
}
/// Bounded opaque provider expiry literal without an assumed timestamp grammar.
/// The API declares a string but does not define its format. This value is
/// retained exactly, not parsed into a local TTL or used to establish validity.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExpiryLiteral(String);
impl ExpiryLiteral {
    /// Records a nonempty expiry literal of at most 128 bytes.
    /// # Errors
    /// Rejects empty/excessive text or control characters without echoing text.
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(invalid());
        }
        Ok(Self(value.into()))
    }
    /// Returns the exact supplied source literal without interpreting its format.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for ExpiryLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpiryLiteral").finish_non_exhaustive()
    }
}
impl TryFrom<String> for ExpiryLiteral {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::new(&value)
    }
}
impl From<ExpiryLiteral> for String {
    fn from(value: ExpiryLiteral) -> Self {
        value.0
    }
}
/// Full quote-only response, bound to exact local source-selection parameters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteData {
    /// Exact immutable quote-only request.
    pub request: QuoteRequest,
    /// Actual source-selected engine, not a promised execution venue.
    pub router: Router,
    /// Exact source output raw units, without token precision assumptions.
    pub out_amount: u64,
    /// Exact source minimum output; endpoint rounding is preserved.
    pub other_amount_threshold: u64,
    /// Source price-impact ratio retained exactly, including negative improvement.
    pub price_impact: ExactDecimal,
    /// Actual source route plan, preserving supplied order and legitimate repetition.
    pub routes: Vec<RouteStep>,
    /// Separate source fee estimates, not a chain fee quote.
    pub fees: QuoteFees,
    /// Actual optional source validity; no missing expiry is invented.
    pub validity: SourceValidity,
}
/// Immutable source-selected quote-only result, with no transaction or execution guarantee.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QuoteData", into = "QuoteData")]
pub struct Quote(QuoteData);
impl Quote {
    /// Validates exclusions, requested capacity, source threshold and route asset connectivity.
    /// Percentages are not summed across serial hops; amount conservation and
    /// individual pool execution are not independently proved.
    /// # Errors
    /// Rejects excluded engines, weaker tolerance, excess/disconnected route facts or fee width.
    pub fn new(data: QuoteData) -> Result<Self, Error> {
        let r = data.request.data();
        if r.excluded_routers.contains(&data.router)
            || data.routes.len() > usize::from(r.max_route_steps)
            || data.other_amount_threshold > data.out_amount
            || data.other_amount_threshold < r.slippage.floor(data.out_amount)
            || data.fees.platform_fee_bps.is_some_and(|v| v > 10_000)
            || data.fees.total_fee_bps.is_some_and(|v| v > 10_000)
            || data.router == Router::Metis
                && data
                    .routes
                    .iter()
                    .any(|leg| r.excluded_dexes.contains(&leg.data().label))
        {
            return Err(invalid());
        }
        validate_routes(
            &data.routes,
            r.input_mint,
            r.output_mint,
            data.router == Router::JupiterZ,
        )?;
        Ok(Self(data))
    }
    /// Returns the exact source-selected quote and immutable request.
    #[must_use]
    pub const fn data(&self) -> &QuoteData {
        &self.0
    }
}
impl TryFrom<QuoteData> for Quote {
    type Error = Error;
    fn try_from(v: QuoteData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Quote> for QuoteData {
    fn from(v: Quote) -> Self {
        v.0
    }
}
impl OperationValue for Quote {
    fn validate_context(&self, c: &Context) -> Result<(), Error> {
        if c.operation() != Operation::OrderQuote || c.network() != &self.0.request.data().network {
            return Err(invalid());
        }
        Ok(())
    }
}
pub(super) fn validate_routes(
    routes: &[RouteStep],
    input: Pubkey,
    output: Pubkey,
    empty_allowed: bool,
) -> Result<(), Error> {
    if routes.is_empty() {
        return if empty_allowed {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let mut forward = vec![input];
    let mut backward = vec![output];
    for _ in 0..routes.len() {
        for leg in routes {
            let d = leg.data();
            if forward.contains(&d.input_mint) && !forward.contains(&d.output_mint) {
                forward.push(d.output_mint);
            }
            if backward.contains(&d.output_mint) && !backward.contains(&d.input_mint) {
                backward.push(d.input_mint);
            }
        }
    }
    if !forward.contains(&output)
        || routes.iter().any(|leg| {
            !forward.contains(&leg.data().input_mint) || !backward.contains(&leg.data().output_mint)
        })
    {
        return Err(invalid());
    }
    Ok(())
}
fn invalid() -> Error {
    ValidationError::InvalidJupiterQuote.into()
}
