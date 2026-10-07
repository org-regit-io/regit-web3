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

// Keep each public variant and its fixed diagnostic together, so additions
// cannot silently omit their Display mapping or expose arbitrary source text.
macro_rules! validation_errors {
    ($( $(#[$documentation:meta])* $variant:ident => $message:literal, )*) => {
        /// The contract violated by an input value.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum ValidationError {
            $( $(#[$documentation])* $variant, )*
        }

        impl fmt::Display for ValidationError {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(match self {
                    $( Self::$variant => $message, )*
                })
            }
        }
    };
}

validation_errors! {
    /// An integer is not in canonical unsigned decimal notation.
    InvalidAmount => "invalid unsigned decimal integer",
    /// An integer exceeds the 256-bit unsigned range.
    AmountOverflow => "unsigned integer exceeds 256 bits",
    /// A signed decimal is not in supported finite decimal notation.
    InvalidDecimal => "invalid finite decimal value",
    /// A signed decimal exceeds bounded input, exponent, scale, or output size.
    DecimalOutOfBounds => "decimal value exceeds resource bounds",
    /// An EVM address has an invalid length, encoding, or mixed-case checksum.
    InvalidAddress => "invalid EVM address",
    /// An EVM transaction identifier has invalid hexadecimal encoding or width.
    InvalidEvmTransactionId => "invalid EVM transaction identifier",
    /// EVM bytes violate their bounded hexadecimal or binary encoding contract.
    InvalidEvmBytes => "invalid EVM byte encoding",
    /// An EVM transaction, receipt or operation record violates its structural contract.
    InvalidEvmRecord => "invalid EVM record",
    /// EVM token metadata violates its typed availability or value contract.
    InvalidEvmMetadata => "invalid EVM token metadata",
    /// An unsigned EVM preparation violates its explicit transaction intent.
    InvalidEvmPreparation => "invalid EVM transaction preparation",
    /// A signed EVM transaction violates its supported envelope contract.
    InvalidEvmSignedTransaction => "invalid EVM signed transaction envelope",
    /// A Solana public key has an invalid base58 encoding or byte length.
    InvalidSolanaPubkey => "invalid Solana public key",
    /// A Solana hash has an invalid base58 encoding or byte length.
    InvalidSolanaHash => "invalid Solana hash",
    /// A Solana signature has an invalid base58 encoding or byte length.
    InvalidSolanaSignature => "invalid Solana signature",
    /// A Solana transaction violates its bounded supported encoding contract.
    InvalidSolanaTransaction => "invalid Solana transaction",
    /// A Solana preparation violates its explicit unsigned transfer contract.
    InvalidSolanaPreparation => "invalid Solana transaction preparation",
    /// A Solana execution record violates its method-specific source contract.
    InvalidSolanaExecution => "invalid Solana execution record",
    /// A Solana native or token base-unit amount exceeds the unsigned 64-bit range.
    SolanaAmountOverflow => "Solana amount exceeds 64 bits",
    /// An observation context declares an operation differing from its value.
    ObservationOperationMismatch => "observation operation and value differ",
    /// A Solana observation slot is below the explicitly requested minimum.
    ContextSlotBelowMinimum => "observation slot is below requested minimum",
    /// A Solana account record differs from the requested account identity.
    InvalidSolanaAccount => "Solana account and requested identity differ",
    /// Solana account data exceeds the protocol's maximum byte length.
    SolanaAccountDataTooLarge => "Solana account data exceeds byte limit",
    /// A Bitcoin address has invalid encoding or checksum.
    InvalidBitcoinAddress => "invalid Bitcoin address",
    /// A Litecoin address violates its family encoding, checksum or network contract.
    InvalidLitecoinAddress => "invalid Litecoin address",
    /// A Dogecoin address violates its family encoding, checksum or network contract.
    InvalidDogecoinAddress => "invalid Dogecoin address",
    /// A Bitcoin Cash address violates its qualified encoding contract.
    InvalidBitcoinCashAddress => "invalid Bitcoin Cash address",
    /// A Bitcoin Cash source record violates its typed contract.
    InvalidBitcoinCashRecord => "invalid Bitcoin Cash record",
    /// Bitcoin Cash bytes violate their bounded encoding contract.
    InvalidBitcoinCashBytes => "invalid Bitcoin Cash byte encoding",
    /// A TON address violates its encoding, flags or checksum contract.
    InvalidTonAddress => "invalid TON address",
    /// A TON network identity violates its zero-state or alias contract.
    InvalidTonNetwork => "invalid TON network",
    /// A TON hash violates its exact-width encoding contract.
    InvalidTonHash => "invalid TON hash",
    /// A TON bag of cells violates its bounded container contract.
    InvalidTonBoc => "invalid TON bag of cells",
    /// A TON source record violates its typed structural contract.
    InvalidTonRecord => "invalid TON record",
    /// A TON transfer violates its explicit unsigned intent contract.
    InvalidTonTransfer => "invalid TON transfer preparation",
    /// A Jupiter request violates its bounded typed contract.
    InvalidJupiterRequest => "invalid Jupiter request",
    /// A Jupiter quote violates its bounded typed contract.
    InvalidJupiterQuote => "invalid Jupiter quote",
    /// A Jupiter build violates its bounded typed contract.
    InvalidJupiterBuild => "invalid Jupiter build",
    /// A Jupiter preparation violates its bounded typed contract.
    InvalidJupiterPreparation => "invalid Jupiter swap preparation",
    /// A Jupiter estimate violates its bounded typed contract.
    InvalidJupiterEstimate => "invalid Jupiter estimate",
    /// A Uniswap deployment violates its explicit supported-version contract.
    InvalidUniswapDeployment => "invalid Uniswap deployment",
    /// A Uniswap V3 path violates its bounded token/fee contract.
    InvalidUniswapPath => "invalid Uniswap V3 path",
    /// A Uniswap V3 quote violates its exact request/source contract.
    InvalidUniswapQuote => "invalid Uniswap V3 quote",
    /// A Uniswap preparation violates its exact reviewed swap intent.
    InvalidUniswapPreparation => "invalid Uniswap swap preparation",
    /// An indexed UTxO-family source record violates its structural contract.
    InvalidUtxoRecord => "invalid indexed UTxO record",
    /// Indexed UTxO-family bytes violate their bounded encoding contract.
    InvalidUtxoBytes => "invalid indexed UTxO byte encoding",
    /// A Bitcoin address is incompatible with the explicitly declared network.
    BitcoinAddressNetworkMismatch => "Bitcoin address and declared network differ",
    /// A Bitcoin block hash has an invalid hexadecimal encoding or length.
    InvalidBitcoinHash => "invalid Bitcoin block hash",
    /// A Bitcoin transaction identifier has invalid hexadecimal encoding or length.
    InvalidBitcoinTxid => "invalid Bitcoin transaction identifier",
    /// A Bitcoin satoshi value exceeds the supported exact unsigned 64-bit range.
    BitcoinAmountOverflow => "Bitcoin amount exceeds 64 bits",
    /// Bitcoin balance statistics imply a negative confirmed balance.
    BitcoinBalanceInconsistent => "inconsistent Bitcoin balance statistics",
    /// Bitcoin history entries violate pagination limits or identity uniqueness.
    InvalidBitcoinHistory => "invalid Bitcoin history page",
    /// Bitcoin transaction confirmation and inclusion fields are inconsistent.
    InvalidBitcoinStatus => "inconsistent Bitcoin transaction status",
    /// A fee estimate has an invalid horizon or negative exact rate.
    InvalidBitcoinFeeEstimate => "invalid Bitcoin fee estimate",
    /// A Bitcoin transaction has invalid or inconsistent indexed facts.
    InvalidBitcoinTransaction => "invalid Bitcoin transaction",
    /// Bitcoin bytes violate bounded canonical hexadecimal or script encoding.
    InvalidBitcoinBytes => "invalid Bitcoin byte encoding",
    /// A mempool summary, collection, fee rate or recent record is inconsistent.
    InvalidMempoolRecord => "invalid mempool record",
    /// A `THORChain` network identity is invalid.
    InvalidThorchainNetwork => "invalid THORChain network identity",
    /// A `THORChain` account address is invalid.
    InvalidThorchainAddress => "invalid THORChain account address",
    /// A `THORChain` asset identity is invalid.
    InvalidThorchainAsset => "invalid THORChain asset identity",
    /// A `THORChain` record violates its typed structural contract.
    InvalidThorchainRecord => "invalid THORChain record",
    /// A caller-supplied wallet handoff identifier violates its lexical or length contract.
    InvalidWalletHandoffId => "invalid wallet handoff identifier",
    /// Returned handoff identity, network, intent or unsigned bytes differ from reviewed preparation.
    WalletBindingMismatch => "wallet handoff does not match reviewed preparation",
    /// A trusted verifier rejected the binding of actual signed contents to reviewed preparation.
    SignedPayloadRejected => "signed payload rejected by verifier",
    /// A Bitcoin observation uses an unsupported schema version.
    UnsupportedBitcoinSchema => "unsupported Bitcoin observation schema",
    /// A Cardano address has invalid encoding or checksum.
    InvalidCardanoAddress => "invalid Cardano address",
    /// A Cardano network identity is invalid.
    InvalidCardanoNetwork => "invalid Cardano network identity",
    /// A Cardano hash has invalid encoding or length.
    InvalidCardanoHash => "invalid Cardano hash",
    /// A Cardano native asset identity is invalid.
    InvalidCardanoAsset => "invalid Cardano asset identity",
    /// A Cardano record violates its structural contract.
    InvalidCardanoRecord => "invalid Cardano record",
    /// A Cardano transaction violates its bounded era-aware source contract.
    InvalidCardanoTransaction => "invalid Cardano transaction",
    /// A Cardano preparation violates its explicit unsigned payment contract.
    InvalidCardanoPreparation => "invalid Cardano preparation",
    /// A bounded page request is invalid.
    InvalidPageRequest => "invalid page request",
    /// A Cardano output amount exceeds its supported width.
    CardanoAmountOverflow => "amount exceeds Cardano output width",
    /// A block hash is not a 32-byte hexadecimal value.
    InvalidBlockHash => "invalid block hash",
    /// A network alias is not a bounded label.
    InvalidNetworkAlias => "invalid network alias",
    /// An asset symbol is not a bounded display label.
    InvalidAssetSymbol => "invalid asset symbol",
    /// A source identifier, method, or version is not a bounded label.
    InvalidSourceLabel => "invalid source label",
    /// A balance's amount and native asset declare different decimals.
    DecimalMismatch => "amount and asset decimals differ",
    /// An explicit selector does not match the resolved block.
    BlockMismatch => "selector and resolved block differ",
    /// A balance's chain identity does not match its observation context.
    NetworkMismatch => "balance and observation chains differ",
    /// An observation uses an unsupported schema version.
    UnsupportedSchemaVersion => "unsupported observation schema version",
    /// A serialized formatted amount does not match its exact raw value.
    InvalidFormattedAmount => "formatted amount differs from exact value",
    /// An XRPL address has invalid encoding, length or checksum.
    InvalidXrplAddress => "invalid XRPL address",
    /// An XRPL hash has invalid hexadecimal encoding or width.
    InvalidXrplHash => "invalid XRPL hash",
    /// An XRPL currency identifier violates its family contract.
    InvalidXrplCurrency => "invalid XRPL currency",
    /// An XRPL native or issued amount violates its exact numeric contract.
    InvalidXrplAmount => "invalid XRPL amount",
    /// An XRPL record violates its structural contract.
    InvalidXrplRecord => "invalid XRPL record",
    /// An XRPL network identity violates its explicit network contract.
    InvalidXrplNetwork => "invalid XRPL network identity",
    /// A market identifier or label violates its lexical or length contract.
    InvalidMarketIdentity => "invalid market identity or label",
    /// A market request or record violates its structural contract.
    InvalidMarketRecord => "invalid market request or record",
    /// A LI.FI identity violates its qualified encoding contract.
    InvalidLifiIdentity => "invalid LI.FI identity",
    /// A LI.FI request or source record violates its typed contract.
    InvalidLifiRecord => "invalid LI.FI record",
    /// A LI.FI prepared payload violates its bounded encoding contract.
    InvalidLifiPayload => "invalid LI.FI prepared payload",
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
