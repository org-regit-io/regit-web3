// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Functional Dogecoin indexed balances, reference history, fees and transaction/status contracts.
//! Family identities, native units and address validation stay independent of
//! Bitcoin consensus decoding. The optional backend uses explicit caller-owned
//! `BlockCypher` settings and retains point-in-time indexed source facts.

use crate::{
    domain::dogecoin::{
        Address, AddressBalance, FeeEstimates, HistoryPage, HistoryRequest, Observation,
        Transaction, TransactionStatus, Txid,
    },
    error::Error,
};
use std::future::Future;

/// Replaceable runtime-independent Dogecoin indexed read capability.
/// Implementors need not be Send/Sync; each returned operation future is Send.
pub trait DogecoinReader {
    /// Reads exact confirmed balance and separate signed unconfirmed delta.
    fn get_address_balance(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + Send;
    /// Reads all supplied references in a bounded height page without truncating the boundary block.
    fn get_address_history(
        &self,
        address: Address,
        request: HistoryRequest,
    ) -> impl Future<Output = Result<Observation<HistoryPage>, Error>> + Send;
    /// Reads source fee buckets in native atomic units per 1000 bytes.
    fn get_fee_estimates(
        &self,
    ) -> impl Future<Output = Result<Observation<FeeEstimates>, Error>> + Send;
    /// Reads exact source inclusion/count/conflict facts for one transaction.
    fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + Send;
    /// Reads complete typed indexed inputs/outputs and optional opaque raw bytes.
    fn get_transaction(
        &self,
        txid: Txid,
        maximum_entries: u32,
    ) -> impl Future<Output = Result<Observation<Transaction>, Error>> + Send;
}

/// Explicit supported Dogecoin network/genesis and bounded `BlockCypher` backend configuration.
#[cfg(feature = "dogecoin-http")]
pub type BlockCypherConfig = super::blockcypher::BlockCypherConfig<Address>;
/// Functional optional Dogecoin `BlockCypher` reader with a single bounded deadline per operation.
#[cfg(feature = "dogecoin-http")]
pub type BlockCypherClient = super::blockcypher::BlockCypherClient<Address>;

#[cfg(feature = "dogecoin-http")]
impl DogecoinReader for BlockCypherClient {
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
