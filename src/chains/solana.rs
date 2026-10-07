// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Runtime-independent Solana read, execution and separate submission capabilities.
//!
//! Implementations supply network access and source attribution. A configured
//! network is caller-supplied expected identity; exposing it does not establish
//! that a remote source has been checked. Readers must honor the requested
//! commitment and minimum context slot without treating the minimum as an
//! exact historical anchor. The pure `solana` feature selects no HTTP client
//! or runtime. The optional `solana-http` feature supplies a bounded concrete
//! implementation that verifies the source's full genesis hash. Its separate
//! one-shot submission operation requires caller-supplied canonical signed bytes;
//! signing, cryptographic verification, secrets and approval remain external.

use std::future::Future;

use crate::{
    domain::solana::{
        AccountLookup, BlockHeight, BlockhashValidity, Commitment, ExecutionObservation, Hash,
        LatestBlockhash, MessageFee, NativeBalance, Network, Observation, Pubkey, ReadOptions,
        Signature, SignedTransaction, Simulation, StatusLookup, StatusOptions, Submission,
        SubmitOptions, TokenBalance, TransactionLookup, TransactionReadOptions, UnsignedMessage,
        UnsignedTransaction,
    },
    error::Error,
};

#[cfg(feature = "solana-http")]
mod http;
#[cfg(feature = "solana-http")]
mod wire;
#[cfg(feature = "solana-http")]
pub use http::{SolanaClient, SolanaHttpConfig};

/// An externally implemented reader of exact SOL balances and reported slots.
pub trait NativeBalanceReader {
    /// Returns the configured expected network identity and display alias.
    fn network(&self) -> &Network;

    /// Reads native lamports with explicit options and required source context.
    ///
    /// # Errors
    /// Returns typed failures from the implementation or observation validation.
    fn get_native_balance(
        &self,
        address: Pubkey,
        options: ReadOptions,
    ) -> impl Future<Output = Result<Observation<NativeBalance>, Error>> + Send;
}

/// An externally implemented reader of one SPL token account's exact raw units.
pub trait TokenBalanceReader {
    /// Returns the configured expected network identity and display alias.
    fn network(&self) -> &Network;

    /// Reads a token account, preserving mint/program/owner identity and precision.
    ///
    /// # Errors
    /// Returns typed failures; an unavailable token account is never a zero balance.
    fn get_token_balance(
        &self,
        token_account: Pubkey,
        options: ReadOptions,
    ) -> impl Future<Output = Result<Observation<TokenBalance>, Error>> + Send;
}

/// An externally implemented reader retaining present and absent account results.
pub trait AccountReader {
    /// Returns the configured expected network identity and display alias.
    fn network(&self) -> &Network;

    /// Reads decoded account bytes or explicit source-reported absence.
    ///
    /// # Errors
    /// Returns typed failures from the implementation or observation validation.
    fn get_account(
        &self,
        address: Pubkey,
        options: ReadOptions,
    ) -> impl Future<Output = Result<Observation<AccountLookup>, Error>> + Send;
}

/// Replaceable transaction retrieval and source status capabilities.
pub trait TransactionReader {
    /// Returns caller-supplied expected network identity.
    fn network(&self) -> &Network;
    /// Reads canonical bytes and exact metadata, or explicit source lookup absence.
    /// # Errors
    /// Returns typed implementation or observation-correlation failures.
    fn get_transaction(
        &self,
        signature: Signature,
        options: TransactionReadOptions,
    ) -> impl Future<Output = Result<ExecutionObservation<TransactionLookup>, Error>> + Send;
    /// Reads recent-cache or explicitly requested history status without invented controls.
    /// # Errors
    /// Returns typed implementation or observation-correlation failures.
    fn get_transaction_status(
        &self,
        signature: Signature,
        options: StatusOptions,
    ) -> impl Future<Output = Result<ExecutionObservation<StatusLookup>, Error>> + Send;
}
/// Replaceable recent-blockhash, validity and actual block-height readers.
pub trait BlockhashReader {
    /// Returns caller-supplied expected network identity.
    fn network(&self) -> &Network;
    /// Reads recent hash with separately reported last-valid block height.
    /// # Errors
    /// Returns typed source, budget or context failures.
    fn get_latest_blockhash(
        &self,
        options: ReadOptions,
    ) -> impl Future<Output = Result<ExecutionObservation<LatestBlockhash>, Error>> + Send;
    /// Checks the exact caller-supplied hash without implicit replacement.
    /// # Errors
    /// Returns typed source, budget or context failures.
    fn is_blockhash_valid(
        &self,
        blockhash: Hash,
        options: ReadOptions,
    ) -> impl Future<Output = Result<ExecutionObservation<BlockhashValidity>, Error>> + Send;
    /// Reads exact actual block height without an invented evaluation slot.
    /// # Errors
    /// Returns typed source, budget or context failures.
    fn get_block_height(
        &self,
        commitment: Commitment,
    ) -> impl Future<Output = Result<ExecutionObservation<BlockHeight>, Error>> + Send;
}
/// Replaceable exact fee estimation and unsigned simulation capabilities.
pub trait ExecutionReader {
    /// Returns caller-supplied expected network identity.
    fn network(&self) -> &Network;
    /// Estimates lamport fees for the exact canonical message bytes.
    /// # Errors
    /// Returns typed source failures; null remains explicit unavailable fee data.
    fn get_fee_for_message(
        &self,
        message: UnsignedMessage,
        options: ReadOptions,
    ) -> impl Future<Output = Result<ExecutionObservation<MessageFee>, Error>> + Send;
    /// Simulates exact zero-placeholder bytes without signing or submission.
    /// # Errors
    /// Returns typed source failures; execution errors remain actual result values.
    fn simulate_transaction(
        &self,
        transaction: UnsignedTransaction,
        options: ReadOptions,
    ) -> impl Future<Output = Result<ExecutionObservation<Simulation>, Error>> + Send;
}
/// Separate explicitly invoked one-shot caller-signed submission capability.
/// Signature and reviewed-intent verification, secrets and approval remain external.
pub trait SolanaSubmitter {
    /// Returns caller-supplied expected network identity, absent from encoded message bytes.
    fn network(&self) -> &Network;
    /// Sends canonical caller-signed bytes once, without automatic leader retries.
    /// # Errors
    /// Unresolved post-dispatch outcomes must retain possible submission.
    fn submit_signed(
        &self,
        transaction: SignedTransaction,
        options: SubmitOptions,
    ) -> impl Future<Output = Result<ExecutionObservation<Submission>, Error>> + Send;
}
