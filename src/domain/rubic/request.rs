// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Account, Asset, Identifier};
use crate::{domain::Amount, error::Error};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Explicit output tolerance in basis points; API encoding is a fraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct SlippageBps(u16);
impl SlippageBps {
    /// Checks `0..=5000`, matching the supported API maximum fraction 0.5.
    /// # Errors
    /// Rejects excess slippage without a default.
    pub fn new(v: u16) -> Result<Self, Error> {
        if v > 5000 {
            return Err(super::invalid_request());
        }
        Ok(Self(v))
    }
    /// Returns exact caller basis points.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
    /// Returns an exact normalized fraction for the API's numeric JSON field.
    #[must_use]
    pub fn fraction(self) -> String {
        if self.0 == 0 {
            return "0".into();
        }
        format!("0.{:04}", self.0).trim_end_matches('0').into()
    }
}
impl TryFrom<u16> for SlippageBps {
    type Error = Error;
    fn try_from(v: u16) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<SlippageBps> for u16 {
    fn from(v: SlippageBps) -> Self {
        v.0
    }
}

/// Explicit response capacities; all excess fails rather than truncating.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LimitFields")]
pub struct Limits {
    chains: u16,
    routes: u16,
    legs: u16,
    path_tokens: u16,
    warnings: u16,
}
impl Limits {
    /// Checks chains `1..=512`, routes/legs/path tokens/warnings `1..=128`.
    /// # Errors
    /// Rejects zero or excessive capacities.
    pub fn new(
        chains: u16,
        routes: u16,
        legs: u16,
        path_tokens: u16,
        warnings: u16,
    ) -> Result<Self, Error> {
        if chains == 0
            || chains > 512
            || [routes, legs, path_tokens, warnings]
                .iter()
                .any(|v| *v == 0 || *v > 128)
        {
            return Err(super::invalid_request());
        }
        Ok(Self {
            chains,
            routes,
            legs,
            path_tokens,
            warnings,
        })
    }
    /// Returns the complete source chain collection capacity.
    #[must_use]
    pub const fn chains(self) -> u16 {
        self.chains
    }
    /// Returns the route collection capacity.
    #[must_use]
    pub const fn routes(self) -> u16 {
        self.routes
    }
    /// Returns the maximum legs per route.
    #[must_use]
    pub const fn legs(self) -> u16 {
        self.legs
    }
    /// Returns the maximum tokens per leg.
    #[must_use]
    pub const fn path_tokens(self) -> u16 {
        self.path_tokens
    }
    /// Returns the maximum warnings per route.
    #[must_use]
    pub const fn warnings(self) -> u16 {
        self.warnings
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitFields {
    chains: u16,
    routes: u16,
    legs: u16,
    path_tokens: u16,
    warnings: u16,
}
impl TryFrom<LimitFields> for Limits {
    type Error = Error;
    fn try_from(v: LimitFields) -> Result<Self, Error> {
        Self::new(v.chains, v.routes, v.legs, v.path_tokens, v.warnings)
    }
}

/// Complete direct-only quote choices, including explicit capacities and filters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteRequestData {
    /// Exact source asset with mandatory precision.
    pub source: Asset,
    /// Exact destination asset with mandatory precision.
    pub destination: Asset,
    /// Positive gross caller input; source net and route amounts remain distinct.
    pub amount: Amount,
    /// Explicit fixed output tolerance.
    pub slippage: SlippageBps,
    /// Explicit calculation seconds in `2..=60`; outer deadline is separate.
    pub calculation_timeout: u8,
    /// Caller referrer, without an implied API credential.
    pub referrer: Identifier,
    /// Explicit excluded native providers, 0–128 distinct identifiers.
    pub excluded_providers: Vec<Identifier>,
    /// Optional exact source preference; not a promise of global optimality.
    pub preferred_provider: Option<Identifier>,
    /// Explicit whether dangerous source routes may be returned.
    pub allow_dangerous_routes: bool,
    /// Explicit whether providers charging source fees should be skipped.
    pub skip_fee_providers: bool,
    /// Optional exact source account; preparations cannot silently replace it.
    pub sender: Option<Account>,
    /// Optional exact destination account; preparations cannot silently replace it.
    pub receiver: Option<Account>,
    /// Optional explicit EVM integrator fee address; server defaults stay source facts.
    pub integrator: Option<crate::domain::Address>,
    /// Explicit per-operation response capacities.
    pub limits: Limits,
}
/// Immutable validated direct-route request; no wallet, default precision or policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QuoteRequestData", into = "QuoteRequestData")]
pub struct QuoteRequest(QuoteRequestData);
impl QuoteRequest {
    /// Checks amount/precision, family accounts, filters and explicit timeout.
    /// # Errors
    /// Rejects contradictions or excessive resource bounds.
    pub fn new(v: QuoteRequestData) -> Result<Self, Error> {
        let mut ids = BTreeSet::new();
        if v.amount.raw().is_zero()
            || v.amount.decimals() != Some(v.source.decimals())
            || !(2..=60).contains(&v.calculation_timeout)
            || v.source == v.destination
            || v.excluded_providers.len() > 128
            || v.excluded_providers.iter().any(|p| !ids.insert(p.clone()))
            || v.preferred_provider
                .as_ref()
                .is_some_and(|p| v.excluded_providers.contains(p))
        {
            return Err(super::invalid_request());
        }
        if let Some(a) = &v.sender {
            a.validate(v.source.chain())?;
        }
        if let Some(a) = &v.receiver {
            a.validate(v.destination.chain())?;
        }
        if v.integrator.is_some() && !matches!(v.source.chain().family(), super::Family::Evm { .. })
        {
            return Err(super::invalid_request());
        }
        Ok(Self(v))
    }
    /// Returns the complete immutable request.
    #[must_use]
    pub const fn data(&self) -> &QuoteRequestData {
        &self.0
    }
}
impl TryFrom<QuoteRequestData> for QuoteRequest {
    type Error = Error;
    fn try_from(v: QuoteRequestData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<QuoteRequest> for QuoteRequestData {
    fn from(v: QuoteRequest) -> Self {
        v.0
    }
}
