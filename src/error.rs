// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Typed failures with fixed diagnostics that contain no supplied input.
//!
//! Errors intentionally carry no endpoint, credential, response body, or
//! arbitrary provider message. Their serialization preserves the category and
//! typed reason. Parsing a serialization format may independently produce
//! errors from that format's parser.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A typed failure with a stable category and secret-safe diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "category", content = "reason", rename_all = "snake_case")]
pub enum Error {
    /// Caller-supplied configuration is invalid.
    Configuration,
    /// A value violates a domain contract.
    Validation(ValidationError),
    /// The requested capability is unsupported.
    UnsupportedCapability,
    /// The operation exceeded its time limit.
    Timeout,
    /// A provider operation failed.
    Provider(ProviderError),
    /// The requested data is unavailable at the selected source.
    UnavailableData,
}

impl Error {
    /// Returns the stable serialized category name.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::Validation(_) => "validation",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::Timeout => "timeout",
            Self::Provider(_) => "provider",
            Self::UnavailableData => "unavailable_data",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration => formatter.write_str("invalid configuration"),
            Self::Validation(reason) => reason.fmt(formatter),
            Self::UnsupportedCapability => formatter.write_str("unsupported capability"),
            Self::Timeout => formatter.write_str("operation timed out"),
            Self::Provider(reason) => reason.fmt(formatter),
            Self::UnavailableData => formatter.write_str("data unavailable at source"),
        }
    }
}

impl std::error::Error for Error {}

/// The contract violated by an input value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationError {
    /// An integer is not in canonical unsigned decimal notation.
    InvalidAmount,
    /// An integer exceeds the 256-bit unsigned range.
    AmountOverflow,
    /// A signed decimal is not in supported finite decimal notation.
    InvalidDecimal,
    /// A signed decimal exceeds bounded input, exponent, scale, or output size.
    DecimalOutOfBounds,
    /// An EVM address has an invalid length, encoding, or mixed-case checksum.
    InvalidAddress,
    /// A Solana public key has an invalid base58 encoding or byte length.
    InvalidSolanaPubkey,
    /// A Solana hash has an invalid base58 encoding or byte length.
    InvalidSolanaHash,
    /// A Solana signature has an invalid base58 encoding or byte length.
    InvalidSolanaSignature,
    /// A Solana native or token base-unit amount exceeds the unsigned 64-bit range.
    SolanaAmountOverflow,
    /// An observation context declares an operation differing from its value.
    ObservationOperationMismatch,
    /// A Solana observation slot is below the explicitly requested minimum.
    ContextSlotBelowMinimum,
    /// A Solana account record differs from the requested account identity.
    InvalidSolanaAccount,
    /// Solana account data exceeds the protocol's maximum byte length.
    SolanaAccountDataTooLarge,
    /// A block hash is not a 32-byte hexadecimal value.
    InvalidBlockHash,
    /// A network alias is not a bounded label.
    InvalidNetworkAlias,
    /// An asset symbol is not a bounded display label.
    InvalidAssetSymbol,
    /// A source identifier, method, or version is not a bounded label.
    InvalidSourceLabel,
    /// A balance's amount and native asset declare different decimals.
    DecimalMismatch,
    /// An explicit selector does not match the resolved block.
    BlockMismatch,
    /// A balance's chain identity does not match its observation context.
    NetworkMismatch,
    /// An observation uses an unsupported schema version.
    UnsupportedSchemaVersion,
    /// A serialized formatted amount does not match its exact raw value.
    InvalidFormattedAmount,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidAmount => "invalid unsigned decimal integer",
            Self::AmountOverflow => "unsigned integer exceeds 256 bits",
            Self::InvalidDecimal => "invalid finite decimal value",
            Self::DecimalOutOfBounds => "decimal value exceeds resource bounds",
            Self::InvalidAddress => "invalid EVM address",
            Self::InvalidSolanaPubkey => "invalid Solana public key",
            Self::InvalidSolanaHash => "invalid Solana hash",
            Self::InvalidSolanaSignature => "invalid Solana signature",
            Self::SolanaAmountOverflow => "Solana amount exceeds 64 bits",
            Self::ObservationOperationMismatch => "observation operation and value differ",
            Self::ContextSlotBelowMinimum => "observation slot is below requested minimum",
            Self::InvalidSolanaAccount => "Solana account and requested identity differ",
            Self::SolanaAccountDataTooLarge => "Solana account data exceeds byte limit",
            Self::InvalidBlockHash => "invalid block hash",
            Self::InvalidNetworkAlias => "invalid network alias",
            Self::InvalidAssetSymbol => "invalid asset symbol",
            Self::InvalidSourceLabel => "invalid source label",
            Self::DecimalMismatch => "amount and asset decimals differ",
            Self::BlockMismatch => "selector and resolved block differ",
            Self::NetworkMismatch => "balance and observation chains differ",
            Self::UnsupportedSchemaVersion => "unsupported observation schema version",
            Self::InvalidFormattedAmount => "formatted amount differs from exact value",
        })
    }
}

impl From<ValidationError> for Error {
    fn from(reason: ValidationError) -> Self {
        Self::Validation(reason)
    }
}

/// A provider failure category without an arbitrary remote message.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderError {
    /// The provider returned an RPC error.
    Rpc,
    /// The provider returned a malformed or inconsistent response.
    InvalidResponse,
    /// The provider rate-limited the request.
    RateLimited,
    /// A connection or response-body transfer failed.
    Transport,
    /// The provider returned an unsuccessful HTTP status.
    HttpStatus,
    /// The response exceeded the explicitly configured byte limit.
    ResponseTooLarge,
    /// The provider's chain identity differs from the configured identity.
    ChainMismatch,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Rpc => "provider RPC failure",
            Self::InvalidResponse => "invalid provider response",
            Self::RateLimited => "provider rate limit reached",
            Self::Transport => "provider transport failure",
            Self::HttpStatus => "unsuccessful provider HTTP status",
            Self::ResponseTooLarge => "provider response exceeds byte limit",
            Self::ChainMismatch => "provider chain identity differs from configuration",
        })
    }
}
