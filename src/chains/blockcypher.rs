// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Compatible indexed read orchestration; family addresses, units and genesis stay explicit.

mod wire;

use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        indexed_utxo::{
            AddressBalance, AddressPolicy, Context, FeeEstimates, HistoryPage, HistoryRequest,
            NetworkId, NetworkPolicy, Observation, Operation, Transaction, TransactionStatus, Txid,
        },
    },
    error::{Error, ProviderError, ValidationError},
    transport::{HttpClient, OperationBudget},
};
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

/// Explicit expected family/genesis identity and replaceable bounded `BlockCypher` HTTP base.
/// The caller supplies the full chain base, such as a documented `/v1/ltc/main`.
/// This backend supports the documented mainnets; test/regression settings fail locally.
#[derive(Clone, Debug)]
pub struct BlockCypherConfig<A: AddressPolicy> {
    network: NetworkId<A::Network>,
    http: HttpConfig,
}
impl<A: AddressPolicy> BlockCypherConfig<A> {
    /// Records a documented supported expected network and caller-owned transport settings.
    /// # Errors
    /// Rejects a network without a documented `BlockCypher` family endpoint before dispatch.
    pub fn new(network: NetworkId<A::Network>, http: HttpConfig) -> Result<Self, Error> {
        if network.network().blockcypher_name().is_none() {
            return Err(Error::Configuration);
        }
        Ok(Self { network, http })
    }
    /// Returns complete expected family/genesis identity and caller display alias.
    #[must_use]
    pub const fn network(&self) -> &NetworkId<A::Network> {
        &self.network
    }
    /// Returns explicit transport settings with redacted URL/header diagnostics.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}

/// Optional read-only `BlockCypher` client retaining typed indexed source facts.
/// Connect and every read verify the exact `/blocks/0` hash, height and chain name.
/// Each operation shares one budget across verification, GET retries, bounded
/// bodies, decoding and construction. Separately queried facts are not atomic,
/// hash-pinned history or independent consensus/inclusion proof.
/// The caller supplies Tokio I/O/time drivers; the client reads no environment,
/// creates no runtime and chooses no credentials, proxies or fallback hosts.
pub struct BlockCypherClient<A: AddressPolicy> {
    config: BlockCypherConfig<A>,
    http: HttpClient,
}
impl<A: AddressPolicy> fmt::Debug for BlockCypherClient<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlockCypherClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl<A: AddressPolicy> BlockCypherClient<A> {
    /// Establishes the client after verifying the exact expected mainnet genesis.
    /// # Errors
    /// Returns fixed configuration, deadline, provider, malformed-source or chain-mismatch failures.
    /// # Panics
    /// Tokio may panic if the caller runtime lacks I/O or time drivers.
    pub async fn connect(config: BlockCypherConfig<A>) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http.limits())?;
        let http = HttpClient::new(&config.http)?;
        let client = Self { config, http };
        budget.run(client.verify_genesis(&budget)).await?;
        Ok(client)
    }
    /// Returns explicit redacted configuration.
    #[must_use]
    pub const fn config(&self) -> &BlockCypherConfig<A> {
        &self.config
    }
    fn check_address(&self, address: &A) -> Result<(), Error> {
        if address.network() != self.config.network.network() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(())
    }
    async fn read(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
        budget: &OperationBudget,
    ) -> Result<Vec<u8>, Error> {
        let response = self.http.read(path, query, None, budget).await?;
        if response.status.as_u16() == 404 {
            return Err(Error::UnavailableData);
        }
        response.into_success()
    }
    async fn verify_genesis(&self, budget: &OperationBudget) -> Result<(), Error> {
        let bytes = self
            .read(&["blocks", "0"], &[("limit", "1")], budget)
            .await?;
        let genesis: wire::Genesis = wire::decode(&bytes)?;
        if genesis.hash != *self.config.network.genesis_hash()
            || genesis.height != 0
            || Some(genesis.chain.as_str()) != self.config.network.network().blockcypher_name()
        {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(())
    }
    fn context(&self, operation: Operation<A>, suffix: &'static str) -> Result<Context<A>, Error> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        Context::new(
            self.config.network.clone(),
            operation,
            Source::new(
                self.config.http.provider_id(),
                suffix,
                env!("CARGO_PKG_VERSION"),
            )?,
            Timestamp::from_unix_seconds(timestamp),
        )
    }
    /// Reads exact confirmed funding/spending and separate signed unconfirmed balance facts.
    /// # Errors
    /// Rejects qualified-address/source mismatch, inconsistent sums, malformed data or bounded transport failures.
    /// # Panics
    /// Tokio may panic if the caller runtime lacks I/O or time drivers.
    pub async fn get_address_balance(
        &self,
        address: A,
    ) -> Result<Observation<AddressBalance<A>, A>, Error> {
        self.check_address(&address)?;
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let encoded = address.to_string();
                let response: wire::AddressFacts = wire::decode(
                    &self
                        .read(&["addrs", &encoded, "balance"], &[], &budget)
                        .await?,
                )?;
                let value = response
                    .into_balance(address.clone())
                    .map_err(|_| wire::invalid_response())?;
                Observation::address_balance(
                    value,
                    self.context(Operation::AddressBalance { address }, "address-balance")?,
                )
            })
            .await
    }
    /// Reads every supplied reference in one height page, including its complete boundary block.
    /// `minimum_entries` is a provider minimum; the separate hard capacity is never truncated.
    /// Missing `hasMore` does not prove exhaustion. Unconfirmed references stay separate.
    /// # Errors
    /// Rejects invalid identity/order/cursors, duplicate references, resource excess or transport failures.
    /// # Panics
    /// Tokio may panic if the caller runtime lacks I/O or time drivers.
    pub async fn get_address_history(
        &self,
        address: A,
        request: HistoryRequest,
    ) -> Result<Observation<HistoryPage<A>, A>, Error> {
        self.check_address(&address)?;
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let encoded = address.to_string();
                let minimum = request.minimum_entries().to_string();
                let before = request.before_height().map(|h| h.to_string());
                let mut query = vec![("limit", minimum.as_str()), ("includeScript", "true")];
                if let Some(before) = &before {
                    query.push(("before", before));
                }
                let response: wire::History =
                    wire::decode(&self.read(&["addrs", &encoded], &query, &budget).await?)?;
                let value = response
                    .into_history(address.clone(), request)
                    .map_err(|error| {
                        if error == Error::UnavailableData {
                            error
                        } else {
                            wire::invalid_response()
                        }
                    })?;
                Observation::address_history(
                    value,
                    self.context(
                        Operation::AddressHistory { address, request },
                        "address-history",
                    )?,
                )
            })
            .await
    }
    /// Reads exact native-unit fee preferences per 1000 bytes and actual source tip facts.
    /// Documented bucket horizons are source preferences, not confirmation guarantees.
    /// # Errors
    /// Rejects changed chain name/genesis, unavailable fields, malformed quantities or bounded transport failures.
    /// # Panics
    /// Tokio may panic if the caller runtime lacks I/O or time drivers.
    pub async fn get_fee_estimates(&self) -> Result<Observation<FeeEstimates<A>, A>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let response: wire::Chain = wire::decode(&self.read(&[], &[], &budget).await?)?;
                if Some(response.name.as_str()) != self.config.network.network().blockcypher_name()
                {
                    return Err(Error::Provider(ProviderError::ChainMismatch));
                }
                Observation::fee_estimates(
                    response.into_fees(),
                    self.context(Operation::FeeEstimates, "fee-estimates")?,
                )
            })
            .await
    }
    /// Reads actual indexed inclusion/count/conflict facts for an exact transaction identifier.
    /// A 404 is unavailable; no status or finality is fabricated from absence.
    /// # Errors
    /// Rejects response identity mismatch, inconsistent inclusion or bounded transport failures.
    /// # Panics
    /// Tokio may panic if the caller runtime lacks I/O or time drivers.
    pub async fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> Result<Observation<TransactionStatus, A>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let encoded = txid.to_string();
                let response: wire::Status = wire::decode(
                    &self
                        .read(&["txs", &encoded], &[("limit", "1")], &budget)
                        .await?,
                )?;
                let value = response
                    .into_status()
                    .map_err(|_| wire::invalid_response())?;
                Observation::transaction_status(
                    value,
                    self.context(Operation::TransactionStatus { txid }, "transaction-status")?,
                )
                .map_err(|_| wire::invalid_response())
            })
            .await
    }
    /// Reads complete typed indexed inputs/outputs and optional opaque raw bytes.
    /// `maximum_entries` is a local 1..=2000 per-array capacity sent as `limit`.
    /// Source-truncated arrays are unavailable; remote continuation URLs are never followed.
    /// Opaque bytes are not decoded or independently hashed, including `MWEB`/`AuxPoW`.
    /// # Errors
    /// Rejects invalid capacity before dispatch, wrong identity, impossible fields,
    /// unsupported/incomplete source records, resource excess or bounded transport failures.
    /// # Panics
    /// Tokio may panic if the caller runtime lacks I/O or time drivers.
    pub async fn get_transaction(
        &self,
        txid: Txid,
        maximum_entries: u32,
    ) -> Result<Observation<Transaction<A>, A>, Error> {
        if maximum_entries == 0 || maximum_entries > 2000 {
            return Err(crate::domain::indexed_utxo::invalid());
        }
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let encoded = txid.to_string();
                let capacity = maximum_entries.to_string();
                let response: wire::TransactionWire = wire::decode(
                    &self
                        .read(
                            &["txs", &encoded],
                            &[("limit", &capacity), ("includeHex", "true")],
                            &budget,
                        )
                        .await?,
                )?;
                let value = response
                    .into_transaction::<A>(self.config.network.network(), maximum_entries)?;
                Observation::transaction(
                    value,
                    self.context(
                        Operation::Transaction {
                            txid,
                            maximum_entries,
                        },
                        "transaction",
                    )?,
                )
                .map_err(|_| wire::invalid_response())
            })
            .await
    }
}
