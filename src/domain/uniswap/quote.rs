// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{V3QuoteRequest, V3RouteRequest};
use crate::{
    domain::evm::{
        AccountNonce, BlockContext, OperationContext, OperationObservation, OperationValue,
        Quantity, ReadOperation, ReadState,
    },
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// Exact source-returned `QuoterV2` values bound to a complete request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V3QuoteData {
    /// The exact path, amount, deployment and simulated parameters.
    pub request: V3QuoteRequest,
    /// Exact output raw units, with no precision default or execution promise.
    pub amount_out: Quantity,
    /// Source uint160 post-quote square-root prices, one per supplied pool.
    pub sqrt_price_x96_after: Vec<Quantity>,
    /// Source uint32 initialized-tick counts, one per supplied pool.
    pub initialized_ticks_crossed: Vec<u32>,
    /// `QuoterV2`'s source gas estimate; not the prepared router transaction's gas limit.
    pub gas_estimate: Quantity,
}
/// Immutable bounded Uniswap V3 simulation result; no future execution is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "V3QuoteData", into = "V3QuoteData")]
pub struct V3Quote(V3QuoteData);
impl V3Quote {
    /// Validates exact path/array cardinality and actual ABI widths.
    /// # Errors
    /// Rejects excess/missing per-hop facts or an overflowing uint160 price.
    pub fn new(data: V3QuoteData) -> Result<Self, Error> {
        let hops = data.request.data().path.hops();
        if data.sqrt_price_x96_after.len() != hops
            || data.initialized_ticks_crossed.len() != hops
            || data
                .sqrt_price_x96_after
                .iter()
                .any(|p| p.value().bit_len() > 160)
        {
            return Err(invalid());
        }
        Ok(Self(data))
    }
    /// Returns all exact source facts and their immutable request.
    #[must_use]
    pub const fn data(&self) -> &V3QuoteData {
        &self.0
    }
}
impl TryFrom<V3QuoteData> for V3Quote {
    type Error = Error;
    fn try_from(data: V3QuoteData) -> Result<Self, Error> {
        Self::new(data)
    }
}
impl From<V3Quote> for V3QuoteData {
    fn from(value: V3Quote) -> Self {
        value.0
    }
}
impl OperationValue for V3Quote {
    fn validate_context(&self, context: &OperationContext) -> Result<(), Error> {
        if context.operation() != ReadOperation::Call
            || context.network().chain_id() != self.0.request.data().deployment.chain_id()
            || !matches!(context.state(), ReadState::CanonicalHash { .. })
        {
            return Err(invalid());
        }
        Ok(())
    }
}
/// Attributed V3 quote with captured canonical state and honest unknown finality.
pub type V3QuoteObservation = OperationObservation<V3Quote>;

/// One supplied route's actual quotation outcome, without inferred pool existence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RouteOutcome {
    /// Successful `QuoterV2` source simulation at the shared captured hash.
    Quoted {
        /// Exact attributed source quote.
        quote: Box<V3QuoteObservation>,
    },
    /// Source code 3 reported a revert; no success/canonicality proof is supplied.
    Reverted {
        /// Exact candidate parameters used for the attempted frozen-hash call.
        request: Box<V3QuoteRequest>,
    },
}
impl RouteOutcome {
    /// Returns the exact supplied candidate regardless of its source outcome.
    #[must_use]
    pub fn request(&self) -> &V3QuoteRequest {
        match self {
            Self::Quoted { quote } => &quote.value().data().request,
            Self::Reverted { request } => request,
        }
    }
}
/// Complete immutable bounded comparison record, retaining its actual anchor read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteComparisonData {
    /// Original supplied paths and exact shared simulation choices.
    pub request: V3RouteRequest,
    /// Actual canonical nonce observation used only to capture state, not choose a nonce.
    pub anchor: OperationObservation<AccountNonce>,
    /// One result per supplied path in the original order.
    pub outcomes: Vec<RouteOutcome>,
}
/// Best raw output among supplied candidates at one captured hash.
/// Ties retain the first candidate. No global route discovery, gas-adjusted
/// optimization, future execution guarantee or inferred lasting finality exists.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RouteComparisonData", into = "RouteComparisonData")]
pub struct RouteComparison {
    data: RouteComparisonData,
    best_index: Option<usize>,
}
impl RouteComparison {
    /// Matches every candidate, source/network identity and captured block before comparison.
    /// # Errors
    /// Rejects missing/reordered outcomes, changed requests or inconsistent source state.
    pub fn new(data: RouteComparisonData) -> Result<Self, Error> {
        let anchor = data.anchor.context();
        let block = canonical_block(anchor)?;
        if data.anchor.value().address != data.request.data().settings.data().from
            || anchor.network().chain_id() != data.request.data().deployment.chain_id()
            || data.outcomes.len() != data.request.data().paths.len()
        {
            return Err(invalid());
        }
        let mut best_index = None;
        let mut best_amount = None;
        for (index, outcome) in data.outcomes.iter().enumerate() {
            if *outcome.request() != data.request.candidate(index)? {
                return Err(invalid());
            }
            if let RouteOutcome::Quoted { quote } = outcome {
                let context = quote.context();
                if canonical_block(context)? != block
                    || context.network() != anchor.network()
                    || context.source().provider_id() != anchor.source().provider_id()
                    || context.source().integration_version()
                        != anchor.source().integration_version()
                {
                    return Err(invalid());
                }
                let amount = quote.value().data().amount_out;
                if best_amount.is_none_or(|best| amount > best) {
                    best_index = Some(index);
                    best_amount = Some(amount);
                }
            }
        }
        Ok(Self { data, best_index })
    }
    /// Returns the original inputs, observed state and ordered outcomes.
    #[must_use]
    pub const fn data(&self) -> &RouteComparisonData {
        &self.data
    }
    /// Returns the highest raw output candidate index, or none if every call reverted.
    #[must_use]
    pub const fn best_index(&self) -> Option<usize> {
        self.best_index
    }
    /// Returns the best actual supplied quote without inferring other routes.
    #[must_use]
    pub fn best_quote(&self) -> Option<&V3QuoteObservation> {
        self.best_index
            .and_then(|index| match &self.data.outcomes[index] {
                RouteOutcome::Quoted { quote } => Some(quote.as_ref()),
                RouteOutcome::Reverted { .. } => None,
            })
    }
}
impl TryFrom<RouteComparisonData> for RouteComparison {
    type Error = Error;
    fn try_from(data: RouteComparisonData) -> Result<Self, Error> {
        Self::new(data)
    }
}
impl From<RouteComparison> for RouteComparisonData {
    fn from(value: RouteComparison) -> Self {
        value.data
    }
}
pub(super) fn canonical_block(context: &OperationContext) -> Result<BlockContext, Error> {
    match context.state() {
        ReadState::CanonicalHash { block, .. } => Ok(block),
        _ => Err(invalid()),
    }
}
fn invalid() -> Error {
    ValidationError::InvalidUniswapQuote.into()
}
