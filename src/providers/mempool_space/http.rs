// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_time::{SystemTime, UNIX_EPOCH};

use std::fmt;

use serde::de::DeserializeOwned;

use crate::{
    chains::bitcoin::{EsploraClient, EsploraConfig, TransactionReader},
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        bitcoin::{
            BlockHash, NetworkId, Observation as BitcoinObservation, Transaction,
            TransactionStatus, Txid,
        },
        mempool_space::{
            Context, MempoolSummary, Observation, Operation, RecentTransactions, RecommendedFees,
            TransactionIds, TransactionLimit,
        },
    },
    error::{Error, ProviderError},
    transport::{HttpClient, OperationBudget},
};

use super::{MempoolSpaceReader, wire};

/// Explicit expected Bitcoin genesis and bounded mempool.space-compatible API base.
/// The caller includes the API path prefix and supplies any optional credentials.
#[derive(Clone, Debug)]
pub struct MempoolSpaceHttpConfig {
    network: NetworkId,
    http: HttpConfig,
}
impl MempoolSpaceHttpConfig {
    /// Records caller-supplied identity and validated HTTP settings without discovery.
    #[must_use]
    pub const fn new(network: NetworkId, http: HttpConfig) -> Self {
        Self { network, http }
    }
    /// Returns the full expected Bitcoin genesis identity and independent display alias.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns explicit endpoint/header/limits configuration with redacted diagnostics.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}

/// Bounded outgoing mempool.space-compatible reads over an explicit API base.
///
/// Establishment and every read verify the full expected genesis. The caller
/// supplies a Tokio runtime with networking/time on native targets; no endpoint, key or proxy is
/// discovered. One operation budget covers genesis, safe-read retries, bodies
/// and decoding. Full transactions reuse the qualified Bitcoin Esplora backend
/// and preserve this configuration's provider label. No request mutates state.
pub struct MempoolSpaceClient {
    config: MempoolSpaceHttpConfig,
    http: HttpClient,
    bitcoin: EsploraClient,
}
impl fmt::Debug for MempoolSpaceClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MempoolSpaceClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl MempoolSpaceClient {
    /// Establishes a reader after verifying the configured full genesis identity.
    /// The total deadline begins before client setup and caps delegated genesis
    /// establishment; runtime validation also precedes client construction.
    /// # Errors
    /// Returns fixed configuration, budget, transport, identity or provider errors.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks networking or time drivers.
    pub async fn connect(config: MempoolSpaceHttpConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http_config().limits())?;
        budget
            .run(async {
                let http = HttpClient::new(config.http_config())?;
                let bitcoin = EsploraClient::connect(EsploraConfig::new(
                    config.network().clone(),
                    config.http_config().clone(),
                ))
                .await?;
                Ok(Self {
                    config,
                    http,
                    bitcoin,
                })
            })
            .await
    }
    /// Returns caller settings with redacted transport diagnostics.
    #[must_use]
    pub const fn config(&self) -> &MempoolSpaceHttpConfig {
        &self.config
    }
    /// Reads source backlog totals and strictly descending individual histogram bins.
    /// # Errors
    /// Rejects changed genesis, malformed/inconsistent records and transport limits.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks networking or time drivers.
    pub async fn get_mempool_summary(&self) -> Result<Observation<MempoolSummary>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let value: wire::Summary = self.read(&["mempool"], &budget).await?;
                Observation::summary(
                    value.into_domain()?,
                    self.context(Operation::Summary, "mempool")?,
                )
                .map_err(|_| wire::invalid())
            })
            .await
    }
    /// Reads the source's at most ten recent arrivals, without an exhaustive claim.
    /// # Errors
    /// Rejects malformed/duplicate/excess entries, changed genesis or provider failures.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks networking or time drivers.
    pub async fn get_recent_transactions(&self) -> Result<Observation<RecentTransactions>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let values = self.read(&["mempool", "recent"], &budget).await?;
                Observation::recent_transactions(
                    wire::recent(values)?,
                    self.context(Operation::RecentTransactions, "mempool-recent")?,
                )
                .map_err(|_| wire::invalid())
            })
            .await
    }
    /// Reads every ID from the unpaged source list, failing wholly at the local cap.
    ///
    /// Source order is arbitrary. This sends no invented page or limit parameter;
    /// the HTTP byte bound and 100000-ID hard bound also apply. The list can change
    /// independently of a separate summary or transaction request.
    /// # Errors
    /// Rejects exceeded bounds, duplicates, malformed IDs, changed genesis or failures.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks networking or time drivers.
    pub async fn get_mempool_txids(
        &self,
        limit: TransactionLimit,
    ) -> Result<Observation<TransactionIds>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let values = self.read(&["mempool", "txids"], &budget).await?;
                Observation::transaction_ids(
                    wire::txids(values, limit)?,
                    self.context(Operation::TransactionIds { limit }, "mempool-txids")?,
                )
                .map_err(|_| wire::invalid())
            })
            .await
    }
    /// Reads the five independent exact source suggestions in sat/vB.
    /// # Errors
    /// Rejects missing/negative/non-numeric suggestions, changed genesis or failures.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks networking or time drivers.
    pub async fn get_recommended_fees(&self) -> Result<Observation<RecommendedFees>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let values: wire::Fees = self.read(&["v1", "fees", "recommended"], &budget).await?;
                Observation::recommended_fees(
                    values.into_domain()?,
                    self.context(Operation::RecommendedFees, "fees-recommended")?,
                )
                .map_err(|_| wire::invalid())
            })
            .await
    }
    /// Reads canonical full bytes with separately retained fee/previous-output/inclusion facts.
    /// # Errors
    /// Rejects genesis, raw identity/structure or indexed inconsistencies; 404 is unavailable.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks networking or time drivers.
    pub async fn get_transaction(
        &self,
        txid: Txid,
    ) -> Result<BitcoinObservation<Transaction>, Error> {
        self.bitcoin.get_transaction(txid).await
    }
    /// Reads complete source inclusion or explicit unconfirmed state; 404 is unavailable.
    /// # Errors
    /// Returns fixed genesis, malformed status, transport or provider failures.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks networking or time drivers.
    pub async fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> Result<BitcoinObservation<TransactionStatus>, Error> {
        self.bitcoin.get_transaction_status(txid).await
    }
    async fn verify_genesis(&self, budget: &OperationBudget) -> Result<(), Error> {
        let bytes = self
            .http
            .read(&["block-height", "0"], &[], None, budget)
            .await?
            .into_success()?;
        let hash = std::str::from_utf8(&bytes)
            .map_err(|_| wire::invalid())
            .and_then(|value| BlockHash::parse(value).map_err(|_| wire::invalid()))?;
        if &hash != self.config.network().genesis_hash() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(())
    }
    async fn read<T: DeserializeOwned>(
        &self,
        path: &[&str],
        budget: &OperationBudget,
    ) -> Result<T, Error> {
        let bytes = self
            .http
            .read(path, &[], None, budget)
            .await?
            .into_success()?;
        wire::decode(&bytes)
    }
    fn context(&self, operation: Operation, method: &str) -> Result<Context, Error> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        Ok(Context::new(
            self.config.network().clone(),
            operation,
            Source::new(
                self.config.http_config().provider_id(),
                method,
                env!("CARGO_PKG_VERSION"),
            )?,
            Timestamp::from_unix_seconds(time),
        ))
    }
}
impl MempoolSpaceReader for MempoolSpaceClient {
    async fn get_mempool_summary(&self) -> Result<Observation<MempoolSummary>, Error> {
        Self::get_mempool_summary(self).await
    }
    async fn get_recent_transactions(&self) -> Result<Observation<RecentTransactions>, Error> {
        Self::get_recent_transactions(self).await
    }
    async fn get_mempool_txids(
        &self,
        limit: TransactionLimit,
    ) -> Result<Observation<TransactionIds>, Error> {
        Self::get_mempool_txids(self, limit).await
    }
    async fn get_recommended_fees(&self) -> Result<Observation<RecommendedFees>, Error> {
        Self::get_recommended_fees(self).await
    }
    async fn get_transaction(&self, txid: Txid) -> Result<BitcoinObservation<Transaction>, Error> {
        Self::get_transaction(self, txid).await
    }
    async fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> Result<BitcoinObservation<TransactionStatus>, Error> {
        Self::get_transaction_status(self, txid).await
    }
}
impl TransactionReader for MempoolSpaceClient {
    async fn get_transaction(&self, txid: Txid) -> Result<BitcoinObservation<Transaction>, Error> {
        Self::get_transaction(self, txid).await
    }
}
