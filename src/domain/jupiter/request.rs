// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::Router;
use crate::{
    domain::solana::{Hash, Network, Pubkey},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Explicit supported Solana mainnet identity with a caller-supplied display alias.
/// Construction declares identity without contacting Jupiter or an RPC source.
/// # Errors
/// Rejects malformed aliases or a compiled genesis constant.
pub fn mainnet(alias: &str) -> Result<Network, Error> {
    Network::new(
        Hash::parse("5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d")?,
        alias,
    )
}
pub(super) fn validate_network(network: &Network) -> Result<(), Error> {
    if network.genesis_hash() != mainnet("mainnet")?.genesis_hash() {
        return Err(invalid());
    }
    Ok(())
}
/// Bounded case-sensitive DEX label, without commas or control characters.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Label(String);
impl Label {
    /// Validates a 1–64-byte API label without echoing supplied text.
    /// # Errors
    /// Rejects empty/excessive text, commas or control characters.
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > 64 || value.chars().any(|c| c.is_control() || c == ',')
        {
            return Err(invalid());
        }
        Ok(Self(value.into()))
    }
    /// Returns the exact case-sensitive label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Label").finish_non_exhaustive()
    }
}
impl TryFrom<String> for Label {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(&v)
    }
}
impl From<Label> for String {
    fn from(v: Label) -> Self {
        v.0
    }
}
/// Explicit fixed slippage tolerance; no automatic RTSE or implicit default.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct SlippageBps(u16);
impl SlippageBps {
    /// Validates a tolerance from 0 through 10,000 basis points.
    /// # Errors
    /// Rejects excess tolerance.
    pub fn new(v: u16) -> Result<Self, Error> {
        if v > 10_000 {
            return Err(invalid());
        }
        Ok(Self(v))
    }
    /// Returns the exact caller choice.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
    pub(super) fn floor(self, out: u64) -> u64 {
        let retained = u64::from(10_000 - self.0);
        out / 10_000 * retained + (out % 10_000) * retained / 10_000
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
/// Complete quote-only source selection request. No taker or transaction is requested.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteRequestData {
    /// Explicit supported mainnet declaration and independent display alias.
    pub network: Network,
    /// Exact input token mint, including wrapped SOL when chosen explicitly.
    pub input_mint: Pubkey,
    /// Exact distinct output mint.
    pub output_mint: Pubkey,
    /// Exact positive smallest-unit input, with no token precision default.
    pub amount: u64,
    /// Explicit fixed output tolerance, never defaulted by the client.
    pub slippage: SlippageBps,
    /// Explicit excluded source engines, without duplicates or excluding all four.
    pub excluded_routers: Vec<Router>,
    /// Case-sensitive Metis-only exclusions, not promises about other engines.
    pub excluded_dexes: Vec<Label>,
    /// Caller response capacity, from 1 through 128 route steps.
    pub max_route_steps: u16,
}
/// Immutable exact-input Swap API V2 quote-only request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QuoteRequestData", into = "QuoteRequestData")]
pub struct QuoteRequest(QuoteRequestData);
impl QuoteRequest {
    /// Validates network, mint/amount identities and bounded source exclusions.
    /// # Errors
    /// Rejects unsupported network, zero input, identical mints or excessive/duplicate filters.
    pub fn new(data: QuoteRequestData) -> Result<Self, Error> {
        validate_network(&data.network)?;
        if data.amount == 0
            || data.input_mint == data.output_mint
            || !(1..=128).contains(&data.max_route_steps)
            || data.excluded_routers.len() > 3
            || data
                .excluded_routers
                .iter()
                .enumerate()
                .any(|(i, v)| data.excluded_routers[..i].contains(v))
        {
            return Err(invalid());
        }
        validate_labels(&data.excluded_dexes)?;
        Ok(Self(data))
    }
    /// Returns every immutable query choice.
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
/// Explicit mutually exclusive DEX source filter for the fresh Metis build.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "labels",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DexFilter {
    /// No client-imposed DEX restriction.
    All,
    /// Restrict Metis to the supplied case-sensitive labels.
    Include(Vec<Label>),
    /// Exclude supplied labels from Metis.
    Exclude(Vec<Label>),
}
/// Explicit output disposition, with no implicit native/token-account conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Destination {
    /// Source's taker output-account disposition.
    Taker,
    /// Exact existing token-account destination.
    TokenAccount {
        /// Explicit output token account, whose ownership/mint is caller-verified.
        address: Pubkey,
    },
    /// Exact native SOL recipient, valid only with wrapped-SOL output and wrapping enabled.
    Native {
        /// Explicit native recipient.
        address: Pubkey,
    },
}
/// Complete fresh Metis V0 build request; an earlier `/order` route is not preserved.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRequestData {
    /// Mainnet declaration, not an API-reported genesis proof.
    pub network: Network,
    /// Exact input mint.
    pub input_mint: Pubkey,
    /// Exact distinct output mint.
    pub output_mint: Pubkey,
    /// Exact positive smallest-unit input.
    pub amount: u64,
    /// Explicit taker expected to authorize the source instructions.
    pub taker: Pubkey,
    /// Explicit fee/rent payer; the client sends this even when equal to taker.
    pub payer: Pubkey,
    /// Explicit fixed slippage choice.
    pub slippage: SlippageBps,
    /// Caller absolute output floor; a weaker source threshold is rejected.
    pub minimum_output: u64,
    /// Explicit wrapping/unwrapping choice, never defaulted by the client.
    pub wrap_and_unwrap_sol: bool,
    /// Explicit output disposition.
    pub destination: Destination,
    /// Explicit fresh-route source filter.
    pub dex_filter: DexFilter,
    /// Explicit source routing capacity, from 1 through 64 accounts.
    pub max_accounts: u8,
    /// Explicit source blockhash age choice, from 1 through 300 slots; not a height or timestamp.
    pub blockhash_slots_to_expiry: u16,
    /// Explicit CU-price percentile in basis points, from 0 through 10,000.
    pub compute_unit_price_percentile: u16,
    /// Explicit response route-step capacity from 1 through 128.
    pub max_route_steps: u16,
}
/// Immutable fresh Swap API V2 Metis instruction request, explicitly V0.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BuildRequestData", into = "BuildRequestData")]
pub struct BuildRequest(BuildRequestData);
impl BuildRequest {
    /// Validates network and all exact identity/resource/expiry choices.
    /// # Errors
    /// Rejects unsupported network, amounts, destinations, filters or source limits.
    pub fn new(data: BuildRequestData) -> Result<Self, Error> {
        validate_network(&data.network)?;
        if data.amount == 0
            || data.input_mint == data.output_mint
            || !(1..=64).contains(&data.max_accounts)
            || !(1..=300).contains(&data.blockhash_slots_to_expiry)
            || data.compute_unit_price_percentile > 10_000
            || !(1..=128).contains(&data.max_route_steps)
        {
            return Err(invalid());
        }
        if let Destination::Native { .. } = data.destination
            && (!data.wrap_and_unwrap_sol
                || data.output_mint
                    != Pubkey::parse("So11111111111111111111111111111111111111112")?)
        {
            return Err(invalid());
        }
        if let DexFilter::Include(labels) | DexFilter::Exclude(labels) = &data.dex_filter {
            if labels.is_empty() {
                return Err(invalid());
            }
            validate_labels(labels)?;
        }
        Ok(Self(data))
    }
    /// Returns all immutable explicitly requested build choices.
    #[must_use]
    pub const fn data(&self) -> &BuildRequestData {
        &self.0
    }
}
impl TryFrom<BuildRequestData> for BuildRequest {
    type Error = Error;
    fn try_from(v: BuildRequestData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<BuildRequest> for BuildRequestData {
    fn from(v: BuildRequest) -> Self {
        v.0
    }
}
fn validate_labels(labels: &[Label]) -> Result<(), Error> {
    if labels.len() > 32
        || labels
            .iter()
            .enumerate()
            .any(|(i, v)| labels[..i].contains(v))
    {
        return Err(invalid());
    }
    Ok(())
}
fn invalid() -> Error {
    ValidationError::InvalidJupiterRequest.into()
}
