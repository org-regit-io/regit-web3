// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Runtime-independent Cardano indexed read capabilities.
//!
//! Implementations verify expected network magic and retain indexed provenance.
//! An address header cannot identify one particular testnet. Current indexed
//! state has no promised hash-selected snapshot; creating-block fields describe
//! output origin only. Wider transaction, fee, epoch/staking and preparation
//! operations remain required and are not implemented by these read contracts.

use std::future::Future;

use crate::{
    domain::cardano::{
        AddressBalance, Network, Observation, PageRequest, PaymentAddress, UtxoPage,
    },
    error::Error,
};

/// A replaceable reader of current indexed ADA and native-asset balances.
pub trait BalanceReader {
    /// Returns caller-configured expected identity, without proving a remote source.
    fn network(&self) -> &Network;
    /// Reads unique exact asset entries for the requested payment address.
    ///
    /// # Errors
    /// Returns typed validation, network or provider failures; missing data is not zero.
    fn get_balance(
        &self,
        address: PaymentAddress,
    ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + Send;
}

/// A replaceable reader of bounded current indexed unspent-output pages.
pub trait UtxoReader {
    /// Returns caller-configured expected network identity and alias.
    fn network(&self) -> &Network;
    /// Reads one explicit page, preserving its request and dynamic-index limitations.
    ///
    /// # Errors
    /// Returns typed failures for unavailable, excessive or inconsistent data.
    fn get_utxos(
        &self,
        address: PaymentAddress,
        page: PageRequest,
    ) -> impl Future<Output = Result<Observation<UtxoPage>, Error>> + Send;
}
