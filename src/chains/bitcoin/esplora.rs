// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Esplora operation orchestration over the shared bounded HTTP backend.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_time::{SystemTime, UNIX_EPOCH};

use std::fmt;

use serde::de::DeserializeOwned;

use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        bitcoin::{
            Address, AddressBalance, BlockHash, Context, FeeEstimates, HistoryCursor, HistoryPage,
            NetworkId, Observation, Operation, Transaction, TransactionBody, TransactionStatus,
            Txid,
        },
    },
    error::{Error, ProviderError, ValidationError},
    transport::{HttpClient, OperationBudget},
};

use super::wire::{self, invalid_response};

/// Explicit expected Bitcoin identity and bounded Esplora HTTP configuration.
#[derive(Clone, Debug)]
pub struct EsploraConfig {
    network: NetworkId,
    http: HttpConfig,
}

impl EsploraConfig {
    /// Records validated network identity and caller-supplied transport settings.
    #[must_use]
    pub const fn new(network: NetworkId, http: HttpConfig) -> Self {
        Self { network, http }
    }
    /// Returns the expected full standard genesis identity and display alias.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns explicit transport configuration with redacted diagnostics.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}

/// Optional bounded Esplora reader over an explicitly configured Bitcoin index.
///
/// Establishment and every operation verify `GET /block-height/0` against the
/// expected full genesis hash. Separate requests are not an atomic snapshot;
/// address balances, mempool changes and history are source-reported index
/// facts. Inclusion fields are retained without inferred confirmations/finality.
///
/// On native targets, the caller supplies a Tokio runtime with I/O and time drivers. The client
/// creates no runtime and loads no RPC endpoints, credentials or proxies.
/// HTTPS uses the shared backend's verified platform trust. Redirects,
/// decompression and implicit transport retries are disabled.
pub struct EsploraClient {
    config: EsploraConfig,
    http: HttpClient,
}

impl EsploraClient {
    /// Establishes a client after verifying the source's full genesis identity.
    ///
    /// # Errors
    /// Returns fixed configuration, timeout, provider, malformed-response or
    /// network-mismatch errors. Remote bodies and credentials are never echoed.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn connect(config: EsploraConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http_config().limits())?;
        let http = HttpClient::new(config.http_config())?;
        let client = Self { config, http };
        budget.run(client.verify_genesis(&budget)).await?;
        Ok(client)
    }
    /// Returns explicit configuration with redacted endpoint/header diagnostics.
    #[must_use]
    pub const fn config(&self) -> &EsploraConfig {
        &self.config
    }

    /// Reads confirmed funding minus spending and a separate signed mempool delta.
    ///
    /// A single deadline covers genesis verification, all retries, response
    /// consumption, decoding and observation construction. It cannot preempt
    /// synchronous work, but late success is rejected and body bytes are bounded.
    ///
    /// # Errors
    /// Rejects an incompatible qualified address, changed genesis, malformed or
    /// inconsistent sums, and bounded transport failures. Null is never zero.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_address_balance(
        &self,
        address: Address,
    ) -> Result<Observation<AddressBalance>, Error> {
        self.check_address(&address)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let encoded = address.to_string();
                let result: wire::AddressStats =
                    self.read_json(&["address", &encoded], &budget).await?;
                let value = result
                    .into_balance(address.clone())
                    .map_err(|_| invalid_response())?;
                let context = self.context(Operation::AddressBalance { address }, "address")?;
                Observation::address_balance(value, context)
            })
            .await
    }

    /// Reads a recent or confirmed-only history chunk retaining the exact cursor.
    ///
    /// Recent results contain at most fifty mempool and twenty-five confirmed
    /// entries. Confirmed-only pages contain at most twenty-five entries. A
    /// count at a source limit means more entries may exist; it does not prove
    /// exhaustion or supply an exhaustive mempool/history view.
    ///
    /// # Errors
    /// Returns fixed failures for identity/transport errors, invalid inclusion,
    /// duplicate transactions, exceeded source limits or a nonadvancing cursor.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_address_history(
        &self,
        address: Address,
        cursor: HistoryCursor,
    ) -> Result<Observation<HistoryPage>, Error> {
        self.check_address(&address)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let encoded = address.to_string();
                let after = match cursor {
                    HistoryCursor::Confirmed { after: Some(txid) } => Some(txid.to_string()),
                    _ => None,
                };
                let mut path = vec!["address", &encoded, "txs"];
                if matches!(cursor, HistoryCursor::Confirmed { .. }) {
                    path.push("chain");
                }
                if let Some(after) = &after {
                    path.push(after);
                }
                let entries: Vec<wire::HistoryTransaction> = self.read_json(&path, &budget).await?;
                let entries = entries
                    .into_iter()
                    .map(wire::HistoryTransaction::into_entry)
                    .collect::<Result<Vec<_>, _>>()?;
                let value = HistoryPage::new(address.clone(), cursor, entries)
                    .map_err(|_| invalid_response())?;
                let context =
                    self.context(Operation::AddressHistory { address, cursor }, "address-txs")?;
                Observation::address_history(value, context)
            })
            .await
    }

    /// Reads exact nonnegative satoshi-per-vbyte fee estimates without floating point.
    ///
    /// Positive integer horizons and exact JSON numeric rates are retained;
    /// an empty estimate set remains empty. Duplicate horizons are rejected.
    ///
    /// # Errors
    /// Returns fixed identity/transport failures or malformed numeric/map data.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_fee_estimates(&self) -> Result<Observation<FeeEstimates>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let result: wire::FeeRates = self.read_json(&["fee-estimates"], &budget).await?;
                Observation::fee_estimates(
                    result.0,
                    self.context(Operation::FeeEstimates, "fee-estimates")?,
                )
            })
            .await
    }

    /// Reads complete source-reported confirmed inclusion or explicit unconfirmed status.
    ///
    /// HTTP 404 for this transaction resource means unavailable data, never an
    /// unconfirmed transaction. No block lookup, confirmation count or lasting
    /// finality is inferred from the indexer's inclusion fields.
    ///
    /// # Errors
    /// Returns unavailable data for HTTP 404 and fixed identity/transport or
    /// malformed-response failures for incomplete/inconsistent inclusion fields.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> Result<Observation<TransactionStatus>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let encoded = txid.to_string();
                let response = self
                    .http
                    .read(&["tx", &encoded, "status"], &[], None, &budget)
                    .await?;
                if response.status == reqwest::StatusCode::NOT_FOUND {
                    return Err(Error::UnavailableData);
                }
                let result: wire::Status = decode_json(&response.into_success()?)?;
                Observation::transaction_status(
                    result.into_status()?,
                    self.context(Operation::TransactionStatus { txid }, "tx-status")?,
                )
            })
            .await
    }

    /// Reads canonical full transaction bytes and separately attributed index facts.
    ///
    /// One deadline covers genesis verification, `/tx/:txid/hex`, `/tx/:txid`,
    /// retries, bounded response consumption and decoding. Computed txid and
    /// immutable fields must agree across both resources; previous outputs,
    /// nullable fee and status remain source-reported facts. Canonical decoding
    /// does not execute scripts, verify signatures or prove inclusion.
    ///
    /// # Errors
    /// HTTP 404 for either resource is unavailable data. Malformed/noncanonical
    /// bytes, immutable mismatches and invalid index facts return a fixed provider
    /// error. Missing previous output/fee data remain explicit nullable values.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_transaction(&self, txid: Txid) -> Result<Observation<Transaction>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let encoded = txid.to_string();
                let hex = self
                    .read_transaction_resource(&["tx", &encoded, "hex"], &budget)
                    .await?;
                let hex = std::str::from_utf8(&hex).map_err(|_| invalid_response())?;
                let body = TransactionBody::from_hex(hex).map_err(|_| invalid_response())?;
                if body.txid() != txid {
                    return Err(invalid_response());
                }
                let bytes = self
                    .read_transaction_resource(&["tx", &encoded], &budget)
                    .await?;
                let indexed: wire::IndexedTransaction = decode_json(&bytes)?;
                let value = indexed.into_transaction(body, self.config.network().network())?;
                Observation::transaction(
                    value,
                    self.context(Operation::Transaction { txid }, "tx-with-hex")?,
                )
            })
            .await
    }

    async fn read_transaction_resource(
        &self,
        path: &[&str],
        budget: &OperationBudget,
    ) -> Result<Vec<u8>, Error> {
        let response = self.http.read(path, &[], None, budget).await?;
        if response.status == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::UnavailableData);
        }
        response.into_success()
    }

    fn check_address(&self, address: &Address) -> Result<(), Error> {
        if address.network() != self.config.network().network() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(())
    }
    async fn read_json<T: DeserializeOwned>(
        &self,
        path: &[&str],
        budget: &OperationBudget,
    ) -> Result<T, Error> {
        let bytes = self
            .http
            .read(path, &[], None, budget)
            .await?
            .into_success()?;
        decode_json(&bytes)
    }
    async fn verify_genesis(&self, budget: &OperationBudget) -> Result<(), Error> {
        let bytes = self
            .http
            .read(&["block-height", "0"], &[], None, budget)
            .await?
            .into_success()?;
        let hash = std::str::from_utf8(&bytes)
            .map_err(|_| invalid_response())
            .and_then(|text| BlockHash::parse(text).map_err(|_| invalid_response()))?;
        if &hash != self.config.network().genesis_hash() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(())
    }
    fn context(&self, operation: Operation, method: &str) -> Result<Context, Error> {
        let retrieved_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        let source = Source::new(
            self.config.http_config().provider_id(),
            method,
            env!("CARGO_PKG_VERSION"),
        )?;
        Context::new(
            self.config.network().clone(),
            operation,
            source,
            Timestamp::from_unix_seconds(retrieved_at),
        )
    }
}

impl fmt::Debug for EsploraClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EsploraClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl super::BitcoinReader for EsploraClient {
    async fn get_address_balance(
        &self,
        address: Address,
    ) -> Result<Observation<AddressBalance>, Error> {
        self.get_address_balance(address).await
    }
    async fn get_address_history(
        &self,
        address: Address,
        cursor: HistoryCursor,
    ) -> Result<Observation<HistoryPage>, Error> {
        self.get_address_history(address, cursor).await
    }
    async fn get_fee_estimates(&self) -> Result<Observation<FeeEstimates>, Error> {
        self.get_fee_estimates().await
    }
    async fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> Result<Observation<TransactionStatus>, Error> {
        self.get_transaction_status(txid).await
    }
}

impl super::TransactionReader for EsploraClient {
    async fn get_transaction(&self, txid: Txid) -> Result<Observation<Transaction>, Error> {
        self.get_transaction(txid).await
    }
}

fn decode_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| invalid_response())
}
