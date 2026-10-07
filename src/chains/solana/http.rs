// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bounded Solana reads and explicitly invoked one-shot submission over HTTP.

mod execution;

use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        solana::{
            AccountLookup, Context, Hash, NativeBalance, Network, Observation, Operation, Pubkey,
            ReadOptions, TokenBalance,
        },
    },
    error::{Error, ProviderError},
    transport::{HttpClient, OperationBudget},
};

use super::wire::{
    BinaryAccount, ParsedAccount, RequestOptions, SlotValue, error_policy, invalid_response,
};

/// Explicit expected network and bounded HTTP configuration for Solana operations.
#[derive(Clone, Debug)]
pub struct SolanaHttpConfig {
    network: Network,
    http: HttpConfig,
}

impl SolanaHttpConfig {
    /// Records previously validated expected network and transport configuration.
    #[must_use]
    pub const fn new(network: Network, http: HttpConfig) -> Self {
        Self { network, http }
    }

    /// Returns the caller-supplied expected full genesis identity and alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }

    /// Returns transport limits and source labels with redacted endpoint diagnostics.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}

/// An optional HTTP backend verifying full Solana genesis identity at every operation.
///
/// The caller supplies a Tokio runtime with I/O and time drivers. The library
/// creates no runtime and loads no endpoints or credentials. Requested
/// commitment and minimum slot are retained; the minimum is not a historical
/// anchor. A per-operation genesis check does not make separate responses atomic.
/// HTTP redirects, proxy discovery, decompression and implicit retries are off.
pub struct SolanaClient {
    config: SolanaHttpConfig,
    http: HttpClient,
}

impl SolanaClient {
    /// Establishes a client after comparing `getGenesisHash` with expected identity.
    ///
    /// # Errors
    /// Returns fixed configuration, budget, transport, status, RPC, malformed
    /// response, or genesis-mismatch failures without supplied credentials.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn connect(config: SolanaHttpConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http_config().limits())?;
        let http = HttpClient::new(config.http_config())?;
        let client = Self { config, http };
        budget.run(client.verify_genesis(&budget)).await?;
        Ok(client)
    }

    /// Returns explicit configuration with redacted endpoint diagnostics.
    #[must_use]
    pub const fn config(&self) -> &SolanaHttpConfig {
        &self.config
    }

    /// Returns the full genesis identity verified at establishment.
    #[must_use]
    pub const fn genesis_hash(&self) -> Hash {
        self.config.network().genesis_hash()
    }

    /// Reads exact native lamports and actual reported slot through `getBalance`.
    ///
    /// A single deadline covers genesis verification, the read, all safe-read
    /// retries and body consumption. Source and retrieval time are attached
    /// after the response; no block identity or finality is invented.
    ///
    /// # Errors
    /// Returns typed failures for provider/identity errors or malformed data,
    /// including negative, fractional, oversized or null lamport balances and
    /// a slot below the supplied minimum. Null never means a zero balance.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_native_balance(
        &self,
        address: Pubkey,
        options: ReadOptions,
    ) -> Result<Observation<NativeBalance>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let result: SlotValue<u64> = self
                    .http
                    .read_rpc(
                        &budget,
                        "getBalance",
                        &(address, RequestOptions::new(options, None)),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                let context = self.context(
                    Operation::NativeBalance,
                    options,
                    result.context.slot,
                    "getBalance",
                )?;
                Observation::native_balance(
                    NativeBalance::new(self.config.network().clone(), address, result.value),
                    context,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }

    /// Reads decoded base64 account bytes or explicit source-reported absence.
    ///
    /// The account-data limit is checked before decoding oversized base64 and
    /// again against decoded bytes. Empty bytes remain a present account.
    ///
    /// # Errors
    /// Returns typed provider, budget, identity or malformed-response failures.
    /// A missing `value` field is malformed; an explicit null is valid absence.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_account(
        &self,
        address: Pubkey,
        options: ReadOptions,
    ) -> Result<Observation<AccountLookup>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let result: SlotValue<Option<BinaryAccount>> = self
                    .http
                    .read_rpc(
                        &budget,
                        "getAccountInfo",
                        &(address, RequestOptions::new(options, Some("base64"))),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                let context = self.context(
                    Operation::Account,
                    options,
                    result.context.slot,
                    "getAccountInfo",
                )?;
                let account = result
                    .value
                    .map(|value| value.into_account(self.config.network().clone(), address))
                    .transpose()?;
                let lookup = AccountLookup::new(self.config.network().clone(), address, account)
                    .map_err(|_| invalid_response())?;
                Observation::account(lookup, context).map_err(|_| invalid_response())
            })
            .await
    }

    /// Reads one SPL token account using typed `jsonParsed` account information.
    ///
    /// Retains the reported mint, token program, account owner/state, raw u64
    /// units, decimals and actual slot. Scaled UI amounts and Token-2022
    /// extension displays are ignored; raw units are never inferred from them.
    /// No fallback to a different encoding, source, or newer slot is attempted.
    ///
    /// # Errors
    /// Returns unavailable data for an explicit absent account, unsupported
    /// capability for RPC method-not-found, and fixed provider failures for
    /// malformed data, unexpected token programs or token-account parser kinds.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_token_balance(
        &self,
        token_account: Pubkey,
        options: ReadOptions,
    ) -> Result<Observation<TokenBalance>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let result: SlotValue<Option<ParsedAccount>> = self
                    .http
                    .read_rpc(
                        &budget,
                        "getAccountInfo",
                        &(
                            token_account,
                            RequestOptions::new(options, Some("jsonParsed")),
                        ),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                let context = self.context(
                    Operation::TokenBalance,
                    options,
                    result.context.slot,
                    "getAccountInfo",
                )?;
                let balance = result
                    .value
                    .ok_or(Error::UnavailableData)?
                    .into_balance(self.config.network().clone(), token_account)?;
                Observation::token_balance(balance, context).map_err(|_| invalid_response())
            })
            .await
    }

    async fn verify_genesis(&self, budget: &OperationBudget) -> Result<(), Error> {
        let value: Hash = self
            .http
            .read_rpc(budget, "getGenesisHash", &[] as &[(); 0], error_policy)
            .await?
            .ok_or_else(invalid_response)?;
        if value != self.config.network().genesis_hash() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(())
    }

    fn context(
        &self,
        operation: Operation,
        options: ReadOptions,
        slot: u64,
        method: &'static str,
    ) -> Result<Context, Error> {
        let retrieved_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| Timestamp::from_unix_seconds(duration.as_secs()))
            .map_err(|_| Error::Configuration)?;
        let source = Source::new(
            self.config.http_config().provider_id(),
            method,
            env!("CARGO_PKG_VERSION"),
        )?;
        Context::new(
            operation,
            self.config.network().clone(),
            options,
            slot,
            source,
            retrieved_at,
        )
        .map_err(|_| invalid_response())
    }
}

impl fmt::Debug for SolanaClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SolanaClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl super::NativeBalanceReader for SolanaClient {
    fn network(&self) -> &Network {
        self.config.network()
    }

    async fn get_native_balance(
        &self,
        address: Pubkey,
        options: ReadOptions,
    ) -> Result<Observation<NativeBalance>, Error> {
        Self::get_native_balance(self, address, options).await
    }
}

impl super::AccountReader for SolanaClient {
    fn network(&self) -> &Network {
        self.config.network()
    }

    async fn get_account(
        &self,
        address: Pubkey,
        options: ReadOptions,
    ) -> Result<Observation<AccountLookup>, Error> {
        Self::get_account(self, address, options).await
    }
}

impl super::TokenBalanceReader for SolanaClient {
    fn network(&self) -> &Network {
        self.config.network()
    }

    async fn get_token_balance(
        &self,
        address: Pubkey,
        options: ReadOptions,
    ) -> Result<Observation<TokenBalance>, Error> {
        Self::get_token_balance(self, address, options).await
    }
}
