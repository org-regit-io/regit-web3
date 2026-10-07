// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Maps the frozen EVM call observation into the bounded `QuoterV2` ABI.

use crate::{
    domain::{
        BlockContext,
        evm::{
            AccountNonce, CallResult, OperationContext, OperationObservation, ReadOperation,
            ReadState,
        },
        uniswap::{V3QuoteObservation, V3QuoteRequest, encoding::decode_quote},
    },
    error::{Error, ValidationError},
};

pub(super) fn anchor_block(
    anchor: &OperationObservation<AccountNonce>,
) -> Result<BlockContext, Error> {
    match anchor.context().state() {
        ReadState::CanonicalHash { block, .. } => Ok(block),
        _ => Err(invalid()),
    }
}
pub(super) fn quote(
    request: V3QuoteRequest,
    anchor: &OperationObservation<AccountNonce>,
    call: &OperationObservation<CallResult>,
) -> Result<V3QuoteObservation, Error> {
    let ReadState::CanonicalHash {
        requested_selector,
        block,
    } = anchor.context().state()
    else {
        return Err(invalid());
    };
    let context = call.context();
    if call.value().call != request.call()?
        || context.network() != anchor.context().network()
        || context.source().provider_id() != anchor.context().source().provider_id()
        || context.source().integration_version() != anchor.context().source().integration_version()
        || !matches!(context.state(), ReadState::CanonicalHash { block: actual, .. } if actual == block)
    {
        return Err(invalid());
    }
    let value = decode_quote(request, call.value().output.bytes())?;
    // Retain the original selection separately from the actual captured hash;
    // every call/retry used that hash, regardless of the original moving tag.
    V3QuoteObservation::new(
        value,
        OperationContext::new(
            ReadOperation::Call,
            context.network().clone(),
            ReadState::CanonicalHash {
                requested_selector,
                block,
            },
            context.source().clone(),
            context.retrieved_at(),
        )?,
    )
}
fn invalid() -> Error {
    ValidationError::InvalidUniswapQuote.into()
}
