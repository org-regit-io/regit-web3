// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{V3Deployment, V3Path, encoding};
use crate::{
    domain::evm::{
        Address, ChainId, Data, FeeTerms, Quantity, TransactionCall, TransactionCallData, U256,
    },
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Complete caller-chosen local EVM call/envelope settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallSettingsData {
    /// Explicit simulated sender, or reviewed transaction sender during preparation.
    pub from: Address,
    /// Explicit nonce; no source lookup automatically replaces it.
    pub nonce: u64,
    /// Explicit nonzero gas cap/limit; no source quote gas becomes a policy margin.
    pub gas_limit: u64,
    /// Explicit complete fee terms, without defaults.
    pub fees: FeeTerms,
}
/// Immutable validated caller settings, with no signing or approval side effects.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CallSettingsData", into = "CallSettingsData")]
pub struct CallSettings(CallSettingsData);
impl CallSettings {
    /// Validates the complete EVM resource/fee choices without fetching state.
    /// # Errors
    /// Rejects an unusable nonce, zero gas cap or inconsistent/excessive fee terms.
    pub fn new(data: CallSettingsData) -> Result<Self, Error> {
        TransactionCall::new(TransactionCallData {
            chain_id: ChainId::from(0),
            from: data.from,
            to: None,
            nonce: data.nonce,
            gas_limit: data.gas_limit,
            value: Quantity::from(0),
            input: Data::new(vec![])?,
            fees: data.fees.clone(),
        })
        .map_err(|_| Error::from(ValidationError::InvalidUniswapPreparation))?;
        Ok(Self(data))
    }
    /// Returns every immutable explicit choice.
    #[must_use]
    pub const fn data(&self) -> &CallSettingsData {
        &self.0
    }
    pub(super) fn call(
        &self,
        chain_id: ChainId,
        to: Address,
        input: Data,
    ) -> Result<TransactionCall, Error> {
        TransactionCall::new(TransactionCallData {
            chain_id,
            from: self.0.from,
            to: Some(to),
            nonce: self.0.nonce,
            gas_limit: self.0.gas_limit,
            value: Quantity::from(0),
            input,
            fees: self.0.fees.clone(),
        })
    }
}
impl fmt::Debug for CallSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallSettings").finish_non_exhaustive()
    }
}
impl TryFrom<CallSettingsData> for CallSettings {
    type Error = Error;
    fn try_from(data: CallSettingsData) -> Result<Self, Error> {
        Self::new(data)
    }
}
impl From<CallSettings> for CallSettingsData {
    fn from(value: CallSettings) -> Self {
        value.0
    }
}

/// Complete exact-input V3 quotation request and declared deployment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V3QuoteRequestData {
    /// Explicit chain/`QuoterV2`/Universal Router 2.1.2 identities.
    pub deployment: V3Deployment,
    /// Exact forward pool sequence, without default fee tiers.
    pub path: V3Path,
    /// Exact positive raw input token units; precision remains unknown.
    pub amount_in: Quantity,
    /// Complete caller-chosen read-only simulation parameters.
    pub settings: CallSettings,
}
/// Immutable bounded `QuoterV2` exact-input request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "V3QuoteRequestData", into = "V3QuoteRequestData")]
pub struct V3QuoteRequest(V3QuoteRequestData);
impl V3QuoteRequest {
    /// Validates the positive int256-safe input used by the V3 `QuoterV2` implementation.
    /// # Errors
    /// Rejects zero or input above the V3 signed swap-amount range.
    pub fn new(data: V3QuoteRequestData) -> Result<Self, Error> {
        if data.amount_in.value() == U256::ZERO
            || data.amount_in.value() > U256::MAX / U256::from(2)
        {
            return Err(ValidationError::InvalidUniswapQuote.into());
        }
        Ok(Self(data))
    }
    /// Returns the complete immutable exact quotation request.
    #[must_use]
    pub const fn data(&self) -> &V3QuoteRequestData {
        &self.0
    }
    /// Derives exact `QuoterV2` calldata and explicit zero native value.
    /// No call, approval, signing or submission occurs.
    /// # Errors
    /// Reports local encoding/resource excess.
    pub fn call(&self) -> Result<TransactionCall, Error> {
        self.0.settings.call(
            self.0.deployment.chain_id(),
            self.0.deployment.data().quoter_v2,
            encoding::quote_call(&self.0.path, self.0.amount_in)?,
        )
    }
}
impl TryFrom<V3QuoteRequestData> for V3QuoteRequest {
    type Error = Error;
    fn try_from(data: V3QuoteRequestData) -> Result<Self, Error> {
        Self::new(data)
    }
}
impl From<V3QuoteRequest> for V3QuoteRequestData {
    fn from(value: V3QuoteRequest) -> Self {
        value.0
    }
}

/// Explicit bounded candidate routes sharing one input/output asset and raw amount.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V3RouteRequestData {
    /// Explicit expected V3/Universal Router 2.1.2 deployment.
    pub deployment: V3Deployment,
    /// Caller-supplied candidates in stable order, without duplicates or discovery.
    pub paths: Vec<V3Path>,
    /// Exact positive input raw units shared by every candidate.
    pub amount_in: Quantity,
    /// Explicit simulation settings, shared without mutation.
    pub settings: CallSettings,
}
/// Validated caller-supplied V3 path comparison, without global routing claims.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "V3RouteRequestData", into = "V3RouteRequestData")]
pub struct V3RouteRequest(V3RouteRequestData);
impl V3RouteRequest {
    /// Maximum number of supplied candidates per comparison.
    pub const MAX_CANDIDATES: usize = 16;
    /// Validates candidate/resource bounds and identical input/output identities.
    /// # Errors
    /// Rejects empty/excessive/duplicate candidates, mismatched assets or invalid input.
    pub fn new(data: V3RouteRequestData) -> Result<Self, Error> {
        if data.paths.is_empty() || data.paths.len() > Self::MAX_CANDIDATES {
            return Err(ValidationError::InvalidUniswapPath.into());
        }
        let first = &data.paths[0];
        for (index, path) in data.paths.iter().enumerate() {
            if path.token_in() != first.token_in()
                || path.token_out() != first.token_out()
                || data.paths[..index].contains(path)
            {
                return Err(ValidationError::InvalidUniswapPath.into());
            }
            V3QuoteRequest::new(V3QuoteRequestData {
                deployment: data.deployment.clone(),
                path: path.clone(),
                amount_in: data.amount_in,
                settings: data.settings.clone(),
            })?;
        }
        Ok(Self(data))
    }
    /// Returns all immutable declared inputs and candidates.
    #[must_use]
    pub const fn data(&self) -> &V3RouteRequestData {
        &self.0
    }
    /// Derives the exact request for a supplied candidate index.
    /// # Errors
    /// Rejects an index outside the bounded caller-supplied list.
    pub fn candidate(&self, index: usize) -> Result<V3QuoteRequest, Error> {
        V3QuoteRequest::new(V3QuoteRequestData {
            deployment: self.0.deployment.clone(),
            path: self
                .0
                .paths
                .get(index)
                .ok_or(ValidationError::InvalidUniswapPath)?
                .clone(),
            amount_in: self.0.amount_in,
            settings: self.0.settings.clone(),
        })
    }
}
impl TryFrom<V3RouteRequestData> for V3RouteRequest {
    type Error = Error;
    fn try_from(data: V3RouteRequestData) -> Result<Self, Error> {
        Self::new(data)
    }
}
impl From<V3RouteRequest> for V3RouteRequestData {
    fn from(value: V3RouteRequest) -> Self {
        value.0
    }
}
