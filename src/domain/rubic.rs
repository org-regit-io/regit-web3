// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact Rubic API-v2 direct-route selection, review and progress primitives.
//!
//! Provider aliases and numeric catalogue IDs are separate from caller-qualified
//! EVM chain IDs and Solana genesis identities. API responses do not attest
//! genesis, finality, executable success or the semantics of unsigned bytes.
mod identity;
pub(crate) mod json;
mod preparation;
mod records;
mod request;
mod status;
pub use crate::domain::market::Observation;
use crate::error::{Error, ValidationError};
pub use identity::{
    Account, Asset, AssetIdentifier, Catalogue, Chain, Family, Identifier,
    SOLANA_NATIVE_ASSET_ADDRESS, Text, TransactionId,
};
pub use preparation::{
    EvmPayload, ForeignFilters, Payload, PreparationRequest, PreparationRequestData, PreparedSwap,
    PreparedSwapData, SourceAdditionalData, SourceId,
};
pub use records::{
    ChainInfo, Chains, Estimate, EstimateData, Fees, FixedFee, FreshQuote, FreshQuoteData, GasFees,
    Leg, LegKind, Quote, QuoteData, Routes, SwapKind, Token, TokenAmount, Warning,
};
pub use request::{Limits, QuoteRequest, QuoteRequestData, SlippageBps};
pub use status::{DestinationTransaction, ProviderStatus, Status, StatusQuery};
fn invalid_identity() -> Error {
    ValidationError::InvalidRubicIdentity.into()
}
fn invalid_request() -> Error {
    ValidationError::InvalidRubicRequest.into()
}
fn invalid_quote() -> Error {
    ValidationError::InvalidRubicQuote.into()
}
fn invalid_preparation() -> Error {
    ValidationError::InvalidRubicPreparation.into()
}
fn invalid_status() -> Error {
    ValidationError::InvalidRubicStatus.into()
}
