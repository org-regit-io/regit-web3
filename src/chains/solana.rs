// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Runtime-independent Solana read capabilities with typed family contracts.
//!
//! Implementations supply network access and source attribution. A configured
//! network is caller-supplied expected identity; exposing it does not establish
//! that a remote source has been checked. Readers must honor the requested
//! commitment and minimum context slot without treating the minimum as an
//! exact historical anchor. No HTTP client or asynchronous runtime is selected.

use std::future::Future;

use crate::{
    domain::solana::{
        AccountLookup, NativeBalance, Network, Observation, Pubkey, ReadOptions, TokenBalance,
    },
    error::Error,
};

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
