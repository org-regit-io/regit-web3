// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

mod indexed;

use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use super::wire;

use crate::{
    chains::cardano::{BalanceReader, UtxoReader},
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        cardano::{
            AddressBalance, Context, Network, Observation, Operation, Order, PageRequest,
            PaymentAddress, UtxoPage,
        },
    },
    error::{Error, ValidationError},
    transport::{HttpClient, OperationBudget},
};

/// Explicit endpoint, limits, provider label and expected network.
///
/// Supply the provider's project credential through the endpoint's explicit
/// header configuration. No credential or endpoint is loaded from environment.
/// The base URL includes the API version path; no provider endpoint is assumed.
#[derive(Clone, Debug)]
pub struct BlockfrostHttpConfig {
    http: HttpConfig,
    network: Network,
}
impl BlockfrostHttpConfig {
    /// Records validated expected network and explicit transport configuration.
    #[must_use]
    pub const fn new(network: Network, http: HttpConfig) -> Self {
        Self { http, network }
    }
    /// Returns explicit redacted transport configuration.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
    /// Returns configured expected identity and alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
}

/// A bounded Blockfrost backend for indexed reads, fee estimates and explicit submission.
///
/// Establishment and every read verify the provider's genesis network magic.
/// One budget covers verification, safe-read retries, body reads and validation.
/// Responses cannot exceed 2 MiB; original transaction CBOR has its own 64 KiB bound.
/// Submission uses one raw-CBOR dispatch, independent of safe-read retry settings.
/// These operations do not establish an exact evaluation block or finality.
/// The caller supplies a Tokio runtime with networking and time enabled.
pub struct BlockfrostClient {
    config: BlockfrostHttpConfig,
    http: HttpClient,
}

impl fmt::Debug for BlockfrostClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BlockfrostClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl BlockfrostClient {
    /// Establishes an explicitly configured reader after genesis verification.
    ///
    /// # Errors
    /// Returns fixed configuration, deadline, network and provider failures.
    ///
    /// # Panics
    /// A current Tokio runtime with disabled networking or time drivers may panic.
    pub async fn connect(config: BlockfrostHttpConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http_config().limits())?;
        if config.http_config().limits().max_response_bytes() > 2 * 1024 * 1024 {
            return Err(Error::Configuration);
        }
        let http = HttpClient::new(config.http_config())?;
        let value = Self { config, http };
        budget.run(value.verify_network(&budget)).await?;
        Ok(value)
    }
    /// Returns established configuration with redacted endpoint diagnostics.
    #[must_use]
    pub const fn config(&self) -> &BlockfrostHttpConfig {
        &self.config
    }
    /// Returns expected network identity, independently of any particular address.
    #[must_use]
    pub const fn network(&self) -> &Network {
        self.config.network()
    }

    async fn verify_network(&self, budget: &OperationBudget) -> Result<(), Error> {
        let bytes = self
            .http
            .read(&["genesis"], &[], None, budget)
            .await?
            .into_success()?;
        wire::genesis(&bytes, self.network().identity())
    }

    /// Reads every explicitly listed ADA/native-asset entry for one address.
    ///
    /// Token precision remains unknown. Provider 404 is unavailable data, never
    /// zero. An explicit empty successful collection stays empty.
    ///
    /// # Errors
    /// Rejects wrong-network addresses, changed provider networks, malformed
    /// identities/amounts, duplicate assets, provider failures and deadline expiry.
    ///
    /// # Panics
    /// A current Tokio runtime with disabled networking or time drivers may panic.
    pub async fn get_balance(
        &self,
        address: PaymentAddress,
    ) -> Result<Observation<AddressBalance>, Error> {
        if !address.is_compatible_with(self.network().identity()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let response = self
                    .http
                    .read(&["addresses", address.as_str()], &[], None, &budget)
                    .await?;
                if response.status.as_u16() == 404 {
                    return Err(Error::UnavailableData);
                }
                let bytes = response.into_success()?;
                let value = wire::balance(&bytes, self.network(), &address)?;
                let context = self.context(Operation::Balance)?;
                Observation::balance(value, context)
            })
            .await
    }

    /// Reads one bounded current indexed unspent-output page.
    ///
    /// A full page reports that more results may exist. Pages are not an atomic
    /// historical snapshot and this method never gathers additional pages.
    ///
    /// # Errors
    /// Rejects wrong-network addresses, mismatched/duplicate outputs or assets,
    /// invalid output widths/data, excessive pages, provider failures and deadlines.
    ///
    /// # Panics
    /// A current Tokio runtime with disabled networking or time drivers may panic.
    pub async fn get_utxos(
        &self,
        address: PaymentAddress,
        page: PageRequest,
    ) -> Result<Observation<UtxoPage>, Error> {
        if !address.is_compatible_with(self.network().identity()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let number = page.page().to_string();
                let count = page.count().to_string();
                let order = match page.order() {
                    Order::Asc => "asc",
                    Order::Desc => "desc",
                };
                let response = self
                    .http
                    .read(
                        &["addresses", address.as_str(), "utxos"],
                        &[("page", &number), ("count", &count), ("order", order)],
                        None,
                        &budget,
                    )
                    .await?;
                if response.status.as_u16() == 404 {
                    return Err(Error::UnavailableData);
                }
                let bytes = response.into_success()?;
                let value = wire::utxos(&bytes, self.network(), &address, page)?;
                Observation::utxos(value, self.context(Operation::Utxos)?)
            })
            .await
    }

    fn context(&self, operation: Operation) -> Result<Context, Error> {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        let method = match operation {
            Operation::Balance | Operation::AddressDetails => "addresses",
            Operation::Utxos => "addresses-utxos",
            Operation::AddressTransactions => "addresses-transactions",
            Operation::NetworkData => "network",
            Operation::Epoch => "epochs",
            Operation::ProtocolParameters => "epochs-parameters",
            Operation::StakeAccount => "accounts",
            Operation::Rewards => "accounts-rewards",
            Operation::Assets => "assets",
            Operation::AssetDetails => "assets-asset",
            Operation::AssetTransactions => "assets-transactions",
            Operation::AssetHolders => "assets-addresses",
            Operation::Transaction => "txs-and-cbor",
            Operation::TransactionUtxos => "txs-utxos",
            Operation::TransactionStatus => "txs-status-and-cbor",
            Operation::PaymentEstimate => "epochs-parameters-ordinary-estimate",
            Operation::Submission => "tx-submit",
        };
        let source = Source::new(
            self.config.http_config().provider_id(),
            method,
            env!("CARGO_PKG_VERSION"),
        )?;
        Ok(Context::new(
            operation,
            self.network().clone(),
            source,
            Timestamp::from_unix_seconds(seconds),
        ))
    }
}

impl BalanceReader for BlockfrostClient {
    fn network(&self) -> &Network {
        Self::network(self)
    }
    async fn get_balance(
        &self,
        address: PaymentAddress,
    ) -> Result<Observation<AddressBalance>, Error> {
        Self::get_balance(self, address).await
    }
}
impl UtxoReader for BlockfrostClient {
    fn network(&self) -> &Network {
        Self::network(self)
    }
    async fn get_utxos(
        &self,
        address: PaymentAddress,
        page: PageRequest,
    ) -> Result<Observation<UtxoPage>, Error> {
        Self::get_utxos(self, address, page).await
    }
}
