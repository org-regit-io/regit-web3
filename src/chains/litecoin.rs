// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Functional Litecoin indexed balances, reference history, fees and transaction/status contracts.
//! Family identities, native units and address validation stay independent of
//! Bitcoin consensus decoding. The optional backend uses explicit caller-owned
//! `BlockCypher` settings and retains point-in-time indexed source facts.

use crate::{
    domain::litecoin::{
        Address, AddressBalance, FeeEstimates, HistoryPage, HistoryRequest, Observation,
        Transaction, TransactionStatus, Txid,
    },
    error::Error,
};
use std::future::Future;

/// Replaceable runtime-independent Litecoin indexed read capability.
/// Futures are `Send` on native targets and host-local on JavaScript WebAssembly.
/// Implementations need not themselves be `Send` or `Sync`.
pub trait LitecoinReader {
    /// Reads exact confirmed balance and separate signed unconfirmed delta.
    fn get_address_balance(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + crate::future::MaybeSend;
    /// Reads all supplied references in a bounded height page without truncating the boundary block.
    fn get_address_history(
        &self,
        address: Address,
        request: HistoryRequest,
    ) -> impl Future<Output = Result<Observation<HistoryPage>, Error>> + crate::future::MaybeSend;
    /// Reads source fee buckets in native atomic units per 1000 bytes.
    fn get_fee_estimates(
        &self,
    ) -> impl Future<Output = Result<Observation<FeeEstimates>, Error>> + crate::future::MaybeSend;
    /// Reads exact source inclusion/count/conflict facts for one transaction.
    fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + crate::future::MaybeSend;
    /// Reads complete typed indexed inputs/outputs and optional opaque raw bytes.
    fn get_transaction(
        &self,
        txid: Txid,
        maximum_entries: u32,
    ) -> impl Future<Output = Result<Observation<Transaction>, Error>> + crate::future::MaybeSend;
}

/// Explicit supported Litecoin network/genesis and bounded `BlockCypher` backend configuration.
#[cfg(feature = "litecoin-http")]
pub type BlockCypherConfig = super::blockcypher::BlockCypherConfig<Address>;
/// Functional optional Litecoin `BlockCypher` reader with a single bounded deadline per operation.
#[cfg(feature = "litecoin-http")]
pub type BlockCypherClient = super::blockcypher::BlockCypherClient<Address>;

#[cfg(feature = "litecoin-http")]
impl LitecoinReader for BlockCypherClient {
    async fn get_address_balance(
        &self,
        address: Address,
    ) -> Result<Observation<AddressBalance>, Error> {
        Self::get_address_balance(self, address).await
    }
    async fn get_address_history(
        &self,
        address: Address,
        request: HistoryRequest,
    ) -> Result<Observation<HistoryPage>, Error> {
        Self::get_address_history(self, address, request).await
    }
    async fn get_fee_estimates(&self) -> Result<Observation<FeeEstimates>, Error> {
        Self::get_fee_estimates(self).await
    }
    async fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> Result<Observation<TransactionStatus>, Error> {
        Self::get_transaction_status(self, txid).await
    }
    async fn get_transaction(
        &self,
        txid: Txid,
        maximum_entries: u32,
    ) -> Result<Observation<Transaction>, Error> {
        Self::get_transaction(self, txid, maximum_entries).await
    }
}
