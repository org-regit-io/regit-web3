// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! `THORChain` RUNE balances, pools, network data, swap quotes and cross-chain state.
//!
//! Pure capabilities retain exact protocol values and real family/source context.
//! The optional HTTP backend verifies explicit Cosmos identity without imposing
//! EVM hash semantics. Reads/quotes never sign, fund, prepare or submit transfers.

use std::future::Future;

use crate::{
    domain::thorchain::{
        Address, Asset, CollectionLimit, InboundAddresses, LastBlocks, NetworkData, Observation,
        Pool, Pools, RuneBalance, SwapQuote, SwapRequest, TransactionStatus, Txid,
    },
    error::Error,
};

#[cfg(feature = "thorchain-http")]
mod http;
#[cfg(feature = "thorchain-http")]
mod wire;
#[cfg(feature = "thorchain-http")]
pub use http::{ThorchainClient, ThorchainHttpConfig};

/// Runtime-independent `THORNode` read/quote capability, with independently implementable backends.
///
/// Futures are `Send` on native targets and host-local on JavaScript WebAssembly.
/// Implementations need not themselves be `Send` or `Sync`.
/// Collection reads return complete source data or fail at their explicit ceiling.
pub trait ThorchainReader {
    /// Reads one explicit RUNE denomination balance in protocol 1e8 units.
    fn get_rune_balance(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<Observation<RuneBalance>, Error>> + crate::future::MaybeSend;
    /// Reads the pool for one exact full layer-one asset identity.
    fn get_pool(
        &self,
        asset: Asset,
    ) -> impl Future<Output = Result<Observation<Pool>, Error>> + crate::future::MaybeSend;
    /// Reads the complete source pool catalogue without silent truncation.
    fn get_pools(
        &self,
        limit: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<Pools>, Error>> + crate::future::MaybeSend;
    /// Reads exact network reserve/bond/fee/price facts without choosing policy.
    fn get_network(
        &self,
    ) -> impl Future<Output = Result<Observation<NetworkData>, Error>> + crate::future::MaybeSend;
    /// Reads a request-associated quote with explicit input-resolution metadata and expiry.
    fn get_swap_quote(
        &self,
        request: SwapRequest,
    ) -> impl Future<Output = Result<Observation<SwapQuote>, Error>> + crate::future::MaybeSend;
    /// Reads complete source inbound vault/halt/gas-unit state.
    fn get_inbound_addresses(
        &self,
        limit: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<InboundAddresses>, Error>> + crate::future::MaybeSend;
    /// Reads actual external observation and distinct `THORChain` signing heights.
    fn get_last_blocks(
        &self,
        limit: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<LastBlocks>, Error>> + crate::future::MaybeSend;
    /// Reads inbound/planned/outbound stages for one exact query identity.
    fn get_transaction_status(
        &self,
        txid: Txid,
        limit: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + crate::future::MaybeSend;
}
