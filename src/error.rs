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
    /// The provider reports that a contract call reverted during execution.
    ExecutionReverted,
    /// A provider operation failed.
    Provider(ProviderError),
    /// An outgoing submission was attempted, but its outcome could not be established.
    /// The request may have reached the server; this is not evidence of rejection.
    SubmissionOutcomeUnknown(SubmissionFailure),
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
            Self::ExecutionReverted => "execution_reverted",
            Self::Provider(_) => "provider",
            Self::SubmissionOutcomeUnknown(_) => "submission_outcome_unknown",
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
            Self::ExecutionReverted => formatter.write_str("contract execution reverted"),
            Self::Provider(reason) => reason.fmt(formatter),
            Self::SubmissionOutcomeUnknown(reason) => {
                write!(formatter, "submission outcome unknown: {reason}")
            }
            Self::UnavailableData => formatter.write_str("data unavailable at source"),
        }
    }
}

impl std::error::Error for Error {}

/// A fixed, input-free cause of an attempted submission's unknown outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionFailure {
    /// The attempted submission exceeded its time limit.
    Timeout,
    /// Connection or response-body transfer failed after dispatch was attempted.
    Transport,
    /// The server returned HTTP 429 after dispatch was attempted.
    RateLimited,
    /// The server returned another non-success HTTP status after attempted dispatch.
    HttpStatus,
    /// The submission response exceeded its configured body limit.
    ResponseTooLarge,
    /// The submission response was malformed or mismatched after attempted dispatch.
    InvalidResponse,
    /// An RPC error was returned after dispatch was attempted.
    Rpc,
}
impl fmt::Display for SubmissionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Timeout => "operation timed out",
            Self::Transport => "transport failed",
            Self::RateLimited => "provider rate limited",
            Self::HttpStatus => "non-success HTTP response",
            Self::ResponseTooLarge => "response body too large",
            Self::InvalidResponse => "invalid provider response",
            Self::Rpc => "provider RPC error",
        })
    }
}

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
    /// An EVM transaction identifier has invalid hexadecimal encoding or width.
    InvalidEvmTransactionId,
    /// EVM bytes violate their bounded hexadecimal or binary encoding contract.
    InvalidEvmBytes,
    /// An EVM transaction, receipt or operation record violates its structural contract.
    InvalidEvmRecord,
    /// EVM token metadata violates its typed availability or value contract.
    InvalidEvmMetadata,
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
    /// A Bitcoin address has invalid encoding or checksum.
    InvalidBitcoinAddress,
    /// A Litecoin address violates its family encoding, checksum or network contract.
    InvalidLitecoinAddress,
    /// A Dogecoin address violates its family encoding, checksum or network contract.
    InvalidDogecoinAddress,
    /// An indexed UTxO-family source record violates its structural contract.
    InvalidUtxoRecord,
    /// Indexed UTxO-family bytes violate their bounded encoding contract.
    InvalidUtxoBytes,
    /// A Bitcoin address is incompatible with the explicitly declared network.
    BitcoinAddressNetworkMismatch,
    /// A Bitcoin block hash has an invalid hexadecimal encoding or length.
    InvalidBitcoinHash,
    /// A Bitcoin transaction identifier has invalid hexadecimal encoding or length.
    InvalidBitcoinTxid,
    /// A Bitcoin satoshi value exceeds the supported exact unsigned 64-bit range.
    BitcoinAmountOverflow,
    /// Bitcoin balance statistics imply a negative confirmed balance.
    BitcoinBalanceInconsistent,
    /// Bitcoin history entries violate pagination limits or identity uniqueness.
    InvalidBitcoinHistory,
    /// Bitcoin transaction confirmation and inclusion fields are inconsistent.
    InvalidBitcoinStatus,
    /// A fee estimate has an invalid horizon or negative exact rate.
    InvalidBitcoinFeeEstimate,
    /// A Bitcoin transaction has invalid or inconsistent indexed facts.
    InvalidBitcoinTransaction,
    /// Bitcoin bytes violate bounded canonical hexadecimal or script encoding.
    InvalidBitcoinBytes,
    /// A mempool summary, collection, fee rate or recent record is inconsistent.
    InvalidMempoolRecord,
    /// A `THORChain` network identity is invalid.
    InvalidThorchainNetwork,
    /// A `THORChain` account address is invalid.
    InvalidThorchainAddress,
    /// A `THORChain` asset identity is invalid.
    InvalidThorchainAsset,
    /// A `THORChain` record violates its typed structural contract.
    InvalidThorchainRecord,
    /// A caller-supplied wallet handoff identifier violates its lexical or length contract.
    InvalidWalletHandoffId,
    /// Returned handoff identity, network, intent or unsigned bytes differ from reviewed preparation.
    WalletBindingMismatch,
    /// A trusted verifier rejected the binding of actual signed contents to reviewed preparation.
    SignedPayloadRejected,
    /// A Bitcoin observation uses an unsupported schema version.
    UnsupportedBitcoinSchema,
    /// A Cardano address has invalid encoding or checksum.
    InvalidCardanoAddress,
    /// A Cardano network identity is invalid.
    InvalidCardanoNetwork,
    /// A Cardano hash has invalid encoding or length.
    InvalidCardanoHash,
    /// A Cardano native asset identity is invalid.
    InvalidCardanoAsset,
    /// A Cardano record violates its structural contract.
    InvalidCardanoRecord,
    /// A bounded page request is invalid.
    InvalidPageRequest,
    /// A Cardano output amount exceeds its supported width.
    CardanoAmountOverflow,
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
    /// An XRPL address has invalid encoding, length or checksum.
    InvalidXrplAddress,
    /// An XRPL hash has invalid hexadecimal encoding or width.
    InvalidXrplHash,
    /// An XRPL currency identifier violates its family contract.
    InvalidXrplCurrency,
    /// An XRPL native or issued amount violates its exact numeric contract.
    InvalidXrplAmount,
    /// An XRPL record violates its structural contract.
    InvalidXrplRecord,
    /// An XRPL network identity violates its explicit network contract.
    InvalidXrplNetwork,
    /// A market identifier or label violates its lexical or length contract.
    InvalidMarketIdentity,
    /// A market request or record violates its structural contract.
    InvalidMarketRecord,
    /// A LI.FI identity violates its qualified encoding contract.
    InvalidLifiIdentity,
    /// A LI.FI request or source record violates its typed contract.
    InvalidLifiRecord,
    /// A LI.FI prepared payload violates its bounded encoding contract.
    InvalidLifiPayload,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidAmount => "invalid unsigned decimal integer",
            Self::AmountOverflow => "unsigned integer exceeds 256 bits",
            Self::InvalidDecimal => "invalid finite decimal value",
            Self::DecimalOutOfBounds => "decimal value exceeds resource bounds",
            Self::InvalidAddress => "invalid EVM address",
            Self::InvalidEvmTransactionId => "invalid EVM transaction identifier",
            Self::InvalidEvmBytes => "invalid EVM byte encoding",
            Self::InvalidEvmRecord => "invalid EVM record",
            Self::InvalidEvmMetadata => "invalid EVM token metadata",
            Self::InvalidSolanaPubkey => "invalid Solana public key",
            Self::InvalidSolanaHash => "invalid Solana hash",
            Self::InvalidSolanaSignature => "invalid Solana signature",
            Self::SolanaAmountOverflow => "Solana amount exceeds 64 bits",
            Self::ObservationOperationMismatch => "observation operation and value differ",
            Self::ContextSlotBelowMinimum => "observation slot is below requested minimum",
            Self::InvalidSolanaAccount => "Solana account and requested identity differ",
            Self::SolanaAccountDataTooLarge => "Solana account data exceeds byte limit",
            Self::InvalidBitcoinAddress => "invalid Bitcoin address",
            Self::InvalidLitecoinAddress => "invalid Litecoin address",
            Self::InvalidDogecoinAddress => "invalid Dogecoin address",
            Self::InvalidUtxoRecord => "invalid indexed UTxO record",
            Self::InvalidUtxoBytes => "invalid indexed UTxO byte encoding",
            Self::BitcoinAddressNetworkMismatch => "Bitcoin address and declared network differ",
            Self::InvalidBitcoinHash => "invalid Bitcoin block hash",
            Self::InvalidBitcoinTxid => "invalid Bitcoin transaction identifier",
            Self::BitcoinAmountOverflow => "Bitcoin amount exceeds 64 bits",
            Self::BitcoinBalanceInconsistent => "inconsistent Bitcoin balance statistics",
            Self::InvalidBitcoinHistory => "invalid Bitcoin history page",
            Self::InvalidBitcoinStatus => "inconsistent Bitcoin transaction status",
            Self::InvalidBitcoinFeeEstimate => "invalid Bitcoin fee estimate",
            Self::InvalidBitcoinTransaction => "invalid Bitcoin transaction",
            Self::InvalidBitcoinBytes => "invalid Bitcoin byte encoding",
            Self::InvalidMempoolRecord => "invalid mempool record",
            Self::InvalidThorchainNetwork => "invalid THORChain network identity",
            Self::InvalidThorchainAddress => "invalid THORChain account address",
            Self::InvalidThorchainAsset => "invalid THORChain asset identity",
            Self::InvalidThorchainRecord => "invalid THORChain record",
            Self::InvalidWalletHandoffId => "invalid wallet handoff identifier",
            Self::WalletBindingMismatch => "wallet handoff does not match reviewed preparation",
            Self::SignedPayloadRejected => "signed payload rejected by verifier",
            Self::UnsupportedBitcoinSchema => "unsupported Bitcoin observation schema",
            Self::InvalidCardanoAddress => "invalid Cardano address",
            Self::InvalidCardanoNetwork => "invalid Cardano network identity",
            Self::InvalidCardanoHash => "invalid Cardano hash",
            Self::InvalidCardanoAsset => "invalid Cardano asset identity",
            Self::InvalidCardanoRecord => "invalid Cardano record",
            Self::InvalidPageRequest => "invalid page request",
            Self::CardanoAmountOverflow => "amount exceeds Cardano output width",
            Self::InvalidBlockHash => "invalid block hash",
            Self::InvalidNetworkAlias => "invalid network alias",
            Self::InvalidAssetSymbol => "invalid asset symbol",
            Self::InvalidSourceLabel => "invalid source label",
            Self::DecimalMismatch => "amount and asset decimals differ",
            Self::BlockMismatch => "selector and resolved block differ",
            Self::NetworkMismatch => "balance and observation chains differ",
            Self::UnsupportedSchemaVersion => "unsupported observation schema version",
            Self::InvalidFormattedAmount => "formatted amount differs from exact value",
            Self::InvalidXrplAddress => "invalid XRPL address",
            Self::InvalidXrplHash => "invalid XRPL hash",
            Self::InvalidXrplCurrency => "invalid XRPL currency",
            Self::InvalidXrplAmount => "invalid XRPL amount",
            Self::InvalidXrplRecord => "invalid XRPL record",
            Self::InvalidXrplNetwork => "invalid XRPL network identity",
            Self::InvalidMarketIdentity => "invalid market identity or label",
            Self::InvalidMarketRecord => "invalid market request or record",
            Self::InvalidLifiIdentity => "invalid LI.FI identity",
            Self::InvalidLifiRecord => "invalid LI.FI record",
            Self::InvalidLifiPayload => "invalid LI.FI prepared payload",
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
