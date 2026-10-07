// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Network-qualified `THORNode` and enabled Cosmos bank reads over bounded HTTP.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_time::{SystemTime, UNIX_EPOCH};

use std::fmt;

use super::{ThorchainReader, wire};
use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        thorchain::{
            Address, Asset, AssetKind, CollectionLimit, Context, InboundAddresses, LastBlocks,
            Network, NetworkData, Observation, Operation, Pool, Pools, RuneBalance, SwapQuote,
            SwapRequest, TransactionStatus, Txid,
        },
    },
    error::{Error, ProviderError, ValidationError},
    transport::{HttpClient, OperationBudget},
};

/// Explicit expected Cosmos identity, account prefix and bounded `THORNode` HTTP base.
///
/// The base must expose the enabled Cosmos bank/node-info endpoints and `THORNode`
/// routes. Network IDs, endpoints, credentials and any path prefix are supplied
/// by the caller. No host fallback or chain-ID/account-prefix pairing is inferred.
#[derive(Clone, Debug)]
pub struct ThorchainHttpConfig {
    network: Network,
    http: HttpConfig,
}
impl ThorchainHttpConfig {
    /// Records separately validated family identity and outgoing transport limits.
    #[must_use]
    pub const fn new(network: Network, http: HttpConfig) -> Self {
        Self { network, http }
    }
    /// Returns expected exact Cosmos chain ID and independently supplied account prefix.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns caller-owned outgoing settings with redacted endpoint/header diagnostics.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}

/// Optional network-qualified `THORNode`/Cosmos reader of exact source observations.
///
/// Establishment and every operation verify Cosmos node-info's exact network ID.
/// One deadline covers verification, safe GET retries, response consumption,
/// decoding and construction. Separately queried source state is not a common
/// block-hash snapshot or independent consensus proof. Source height/finality
/// fields remain family-specific. Expired quotes are unavailable without refresh.
///
/// On native targets, the caller supplies a Tokio runtime with I/O/time drivers. The client creates
/// no runtime and reads no environment variables, endpoints or credentials.
pub struct ThorchainClient {
    config: ThorchainHttpConfig,
    http: HttpClient,
}
impl fmt::Debug for ThorchainClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThorchainClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl ThorchainClient {
    /// Establishes the client after verifying its explicit Cosmos network identity.
    ///
    /// # Errors
    /// Returns fixed configuration, deadline, provider or network-mismatch failures.
    /// Remote diagnostic bodies and endpoint/credential values are not retained.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime has disabled I/O or time drivers.
    pub async fn connect(config: ThorchainHttpConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http_config().limits())?;
        let http = HttpClient::new(config.http_config())?;
        let client = Self { config, http };
        budget.run(client.verify_network(&budget)).await?;
        Ok(client)
    }
    /// Returns explicit configurations with redacted endpoint/header diagnostics.
    #[must_use]
    pub const fn config(&self) -> &ThorchainHttpConfig {
        &self.config
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
    async fn verify_network(&self, budget: &OperationBudget) -> Result<(), Error> {
        let bytes = self
            .read(
                &["cosmos", "base", "tendermint", "v1beta1", "node_info"],
                &[],
                budget,
            )
            .await?;
        if wire::chain_id(&bytes)? != self.config.network.chain_id() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(())
    }
    fn context(
        &self,
        operation: Operation,
        method: &str,
        timestamp: Timestamp,
    ) -> Result<Context, Error> {
        Context::new(
            self.config.network.clone(),
            operation,
            Source::new(
                self.config.http.provider_id(),
                method,
                env!("CARGO_PKG_VERSION"),
            )?,
            timestamp,
        )
    }
    fn check_address(&self, address: &Address) -> Result<(), Error> {
        if address.prefix() != self.config.network.account_prefix() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(())
    }
    /// Reads one explicitly queried native RUNE denomination in protocol 1e8 units.
    /// Missing/null bank data remains unavailable; only an explicit zero is zero.
    ///
    /// # Errors
    /// Rejects wrong account prefix, network changes, malformed/absent source facts
    /// and bounded transport failures. No implicit denomination fallback is used.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_rune_balance(
        &self,
        address: Address,
    ) -> Result<Observation<RuneBalance>, Error> {
        self.check_address(&address)?;
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .read(
                        &[
                            "cosmos",
                            "bank",
                            "v1beta1",
                            "balances",
                            address.as_str(),
                            "by_denom",
                        ],
                        &[("denom", "rune")],
                        &budget,
                    )
                    .await?;
                let value = wire::balance(&bytes, address.clone())?;
                Observation::rune_balance(
                    value,
                    self.context(Operation::RuneBalance { address }, "bank-balance", now()?)?,
                )
            })
            .await
    }
    /// Reads the exact full layer-one pool and checks returned asset/unit identities.
    ///
    /// # Errors
    /// Rejects wrapped asset requests, differing returned pool identity, malformed
    /// source fields, network changes and bounded transport failures.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_pool(&self, asset: Asset) -> Result<Observation<Pool>, Error> {
        if asset.kind() != AssetKind::LayerOne {
            return Err(ValidationError::InvalidThorchainAsset.into());
        }
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .read(&["thorchain", "pool", &asset.to_string()], &[], &budget)
                    .await?;
                let value = wire::pool(&bytes, &asset)?;
                Observation::pool(
                    value,
                    self.context(Operation::Pool { asset }, "pool", now()?)?,
                )
            })
            .await
    }
    /// Reads the complete source pool catalogue or fails at the explicit item ceiling.
    ///
    /// # Errors
    /// Rejects duplicate/inconsistent pools, excess items, malformed facts,
    /// network changes and bounded transport failures; records are never truncated.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_pools(&self, limit: CollectionLimit) -> Result<Observation<Pools>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.read(&["thorchain", "pools"], &[], &budget).await?;
                Observation::pools(
                    wire::pools(&bytes, limit)?,
                    self.context(Operation::Pools { limit }, "pools", now()?)?,
                )
            })
            .await
    }
    /// Reads exact source bond/reserve/price/fee facts without selecting caller policy.
    ///
    /// # Errors
    /// Returns fixed malformed-source, network-mismatch and bounded transport failures.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_network(&self) -> Result<Observation<NetworkData>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.read(&["thorchain", "network"], &[], &budget).await?;
                Observation::network(
                    wire::network(&bytes)?,
                    self.context(Operation::Network, "network", now()?)?,
                )
            })
            .await
    }
    /// Reads an exact request-bound swap quote, fee denominations, native dust/gas
    /// units and source expiry. Completion after expiry returns unavailable data
    /// without implicitly requesting a refreshed quote. The current source does not
    /// echo the resolved input asset: metadata is explicitly unreported, independently
    /// of the retained caller request. No additional pool reads or fallback infer it.
    ///
    /// # Errors
    /// Rejects wrong THOR address prefixes, expired quotes, incompatible fees,
    /// malformed data, network changes and bounded transport failures.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_swap_quote(
        &self,
        request: SwapRequest,
    ) -> Result<Observation<SwapQuote>, Error> {
        let operation = Operation::SwapQuote {
            request: Box::new(request.clone()),
        };
        self.context(operation.clone(), "quote-swap", now()?)?;
        let query = quote_query(&request);
        let query_refs: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .read(&["thorchain", "quote", "swap"], &query_refs, &budget)
                    .await?;
                let data = wire::quote(&bytes, &request)?;
                let timestamp = now()?;
                let expiry = data.expiry;
                let value = SwapQuote::new(request, data, timestamp).map_err(|e| {
                    if e == Error::UnavailableData {
                        e
                    } else {
                        wire::invalid_response()
                    }
                })?;
                let observation = Observation::swap_quote(
                    value,
                    self.context(operation, "quote-swap", timestamp)?,
                )?;
                if expiry.unix_seconds() <= now()?.unix_seconds() {
                    return Err(Error::UnavailableData);
                }
                Ok(observation)
            })
            .await
    }
    /// Reads complete external inbound-vault, halt and actual gas-unit source facts.
    ///
    /// # Errors
    /// Rejects inconsistent/duplicate chains, excess items, malformed source data,
    /// network changes and bounded transport failures; no records are truncated.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_inbound_addresses(
        &self,
        limit: CollectionLimit,
    ) -> Result<Observation<InboundAddresses>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .read(&["thorchain", "inbound_addresses"], &[], &budget)
                    .await?;
                Observation::inbound_addresses(
                    wire::inbounds(&bytes, limit)?,
                    self.context(
                        Operation::InboundAddresses { limit },
                        "inbound-addresses",
                        now()?,
                    )?,
                )
            })
            .await
    }
    /// Reads external observation and distinct `THORChain` signing/evaluation heights.
    /// Separate chain records retain actual heights without a fabricated shared hash.
    ///
    /// # Errors
    /// Rejects duplicate/inconsistent heights, excess items, malformed data,
    /// network changes and bounded transport failures.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_last_blocks(
        &self,
        limit: CollectionLimit,
    ) -> Result<Observation<LastBlocks>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.read(&["thorchain", "lastblock"], &[], &budget).await?;
                Observation::last_blocks(
                    wire::lastblocks(&bytes, limit)?,
                    self.context(Operation::LastBlocks { limit }, "lastblock", now()?)?,
                )
            })
            .await
    }
    /// Reads request-bound source inbound/planned/outbound cross-chain progress.
    /// Null or omitted transaction/collections remain unavailable; explicit empty
    /// collections remain empty. Source signing/broadcast is not external inclusion.
    ///
    /// # Errors
    /// Rejects differing transaction identity, invalid stages/chain attribution,
    /// nested collection excess, network changes and bounded transport failures.
    ///
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_transaction_status(
        &self,
        txid: Txid,
        limit: CollectionLimit,
    ) -> Result<Observation<TransactionStatus>, Error> {
        let budget = OperationBudget::new(self.config.http.limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .read(&["thorchain", "tx", "status", txid.as_str()], &[], &budget)
                    .await?;
                let value = wire::status(&bytes, txid.clone(), limit)?;
                Observation::transaction_status(
                    value,
                    self.context(
                        Operation::TransactionStatus { txid, limit },
                        "tx-status",
                        now()?,
                    )?,
                )
            })
            .await
    }
}
fn now() -> Result<Timestamp, Error> {
    Ok(Timestamp::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs(),
    ))
}
fn quote_query(request: &SwapRequest) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("from_asset", request.from_asset().to_string()),
        ("to_asset", request.to_asset().to_string()),
        ("amount", request.amount().raw().to_string()),
    ];
    let p = request.parameters();
    if let Some(v) = &p.destination {
        query.push(("destination", v.as_str().to_owned()));
    }
    if let Some(v) = &p.refund_address {
        query.push(("refund_address", v.as_str().to_owned()));
    }
    if let Some(v) = &p.streaming {
        query.push(("streaming_interval", v.interval.to_string()));
        query.push(("streaming_quantity", v.quantity.to_string()));
    }
    if let Some(v) = p.tolerance_bps {
        query.push(("tolerance_bps", v.get().to_string()));
    }
    if let Some(v) = p.liquidity_tolerance_bps {
        query.push(("liquidity_tolerance_bps", v.get().to_string()));
    }
    query
}
impl ThorchainReader for ThorchainClient {
    async fn get_rune_balance(&self, v: Address) -> Result<Observation<RuneBalance>, Error> {
        Self::get_rune_balance(self, v).await
    }
    async fn get_pool(&self, v: Asset) -> Result<Observation<Pool>, Error> {
        Self::get_pool(self, v).await
    }
    async fn get_pools(&self, v: CollectionLimit) -> Result<Observation<Pools>, Error> {
        Self::get_pools(self, v).await
    }
    async fn get_network(&self) -> Result<Observation<NetworkData>, Error> {
        Self::get_network(self).await
    }
    async fn get_swap_quote(&self, v: SwapRequest) -> Result<Observation<SwapQuote>, Error> {
        Self::get_swap_quote(self, v).await
    }
    async fn get_inbound_addresses(
        &self,
        v: CollectionLimit,
    ) -> Result<Observation<InboundAddresses>, Error> {
        Self::get_inbound_addresses(self, v).await
    }
    async fn get_last_blocks(&self, v: CollectionLimit) -> Result<Observation<LastBlocks>, Error> {
        Self::get_last_blocks(self, v).await
    }
    async fn get_transaction_status(
        &self,
        txid: Txid,
        limit: CollectionLimit,
    ) -> Result<Observation<TransactionStatus>, Error> {
        Self::get_transaction_status(self, txid, limit).await
    }
}
