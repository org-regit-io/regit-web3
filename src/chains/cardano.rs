// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Runtime-independent Cardano indexed read capabilities.
//!
//! Implementations verify expected network magic and retain indexed provenance.
//! An address header cannot identify one particular testnet. Current indexed
//! state has no promised hash-selected snapshot; creating-block fields describe
//! output origin only. Ordinary Conway key-spend preparation is pure and explicit;
//! indexed reads, fresh fee estimates and one-shot externally signed submission
//! are separate capabilities. No cryptographic signing or approval occurs here.

use std::future::Future;

use crate::{
    domain::cardano::{
        AddressBalance, AddressDetails, AssetDetails, AssetEntry, AssetHolder, AssetId, Epoch,
        EpochSelector, Hash, IndexPage, Network, NetworkData, Observation, PageRequest,
        PaymentAddress, PaymentEstimate, PaymentIntent, ProtocolParameters, Reward,
        SignedSubmission, StakeAccount, StakeAddress, SubmissionResult, Transaction,
        TransactionReference, TransactionStatus, TransactionUtxos, UtxoPage,
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
    ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + crate::future::MaybeSend;
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
    ) -> impl Future<Output = Result<Observation<UtxoPage>, Error>> + crate::future::MaybeSend;
}

/// Replaceable bounded indexed-data and fee-estimate operations.
/// Retrieval facts are separate from ledger validity and finality.
pub trait CardanoReader: BalanceReader + UtxoReader {
    /// Reads exact indexed supply and stake with explicit retrieval attribution.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_network_data(
        &self,
    ) -> impl Future<Output = Result<Observation<NetworkData>, Error>> + crate::future::MaybeSend;
    /// Reads latest or explicitly numbered epoch facts.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_epoch(
        &self,
        selector: EpochSelector,
    ) -> impl Future<Output = Result<Observation<Epoch>, Error>> + crate::future::MaybeSend;
    /// Reads selected epoch parameters needed by the ordinary payment profile.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_protocol_parameters(
        &self,
        selector: EpochSelector,
    ) -> impl Future<Output = Result<Observation<ProtocolParameters>, Error>> + crate::future::MaybeSend;
    /// Reads source address classification, stake credential and exact balance.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_address_details(
        &self,
        address: PaymentAddress,
    ) -> impl Future<Output = Result<Observation<AddressDetails>, Error>> + crate::future::MaybeSend;
    /// Reads one bounded dynamic-index transaction page for a payment address.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_address_transactions(
        &self,
        address: PaymentAddress,
        page: PageRequest,
    ) -> impl Future<Output = Result<Observation<IndexPage<TransactionReference>>, Error>>
    + crate::future::MaybeSend;
    /// Reads typed exact staking-account amounts and delegation metadata.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_stake_account(
        &self,
        address: StakeAddress,
    ) -> impl Future<Output = Result<Observation<StakeAccount>, Error>> + crate::future::MaybeSend;
    /// Reads one bounded staking reward page with exact lovelace units.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_stake_rewards(
        &self,
        address: StakeAddress,
        page: PageRequest,
    ) -> impl Future<Output = Result<Observation<IndexPage<Reward>>, Error>> + crate::future::MaybeSend;
    /// Reads one bounded indexed native-asset catalogue page.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_assets(
        &self,
        page: PageRequest,
    ) -> impl Future<Output = Result<Observation<IndexPage<AssetEntry>>, Error>> + crate::future::MaybeSend;
    /// Reads exact native-token identity, quantity and named metadata.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_asset(
        &self,
        asset: AssetId,
    ) -> impl Future<Output = Result<Observation<AssetDetails>, Error>> + crate::future::MaybeSend;
    /// Reads one bounded token transaction page.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_asset_transactions(
        &self,
        asset: AssetId,
        page: PageRequest,
    ) -> impl Future<Output = Result<Observation<IndexPage<TransactionReference>>, Error>>
    + crate::future::MaybeSend;
    /// Reads one bounded token holder page.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_asset_holders(
        &self,
        asset: AssetId,
        page: PageRequest,
    ) -> impl Future<Output = Result<Observation<IndexPage<AssetHolder>>, Error>>
    + crate::future::MaybeSend;
    /// Reads original transaction CBOR correlated with indexed source facts.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_transaction(
        &self,
        id: Hash,
    ) -> impl Future<Output = Result<Observation<Transaction>, Error>> + crate::future::MaybeSend;
    /// Reads bounded indexed transaction inputs and outputs with their actual roles.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_transaction_utxos(
        &self,
        id: Hash,
    ) -> impl Future<Output = Result<Observation<TransactionUtxos>, Error>> + crate::future::MaybeSend;
    /// Reads indexed inclusion/execution or unavailable lookup without invented finality.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn get_transaction_status(
        &self,
        id: Hash,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + crate::future::MaybeSend;
    /// Estimates exact ordinary-payment minimum fee using fresh selected parameters.
    ///
    /// # Errors
    /// Returns typed network, validation, unavailable-data or provider failures.
    fn estimate_payment_fee(
        &self,
        intent: PaymentIntent,
        selector: EpochSelector,
    ) -> impl Future<Output = Result<Observation<PaymentEstimate>, Error>> + crate::future::MaybeSend;
}

/// Explicit one-shot externally signed ordinary-payment submission.
/// Signing, secrets and approval policy belong to the caller.
pub trait CardanoSubmitter {
    /// Returns configured expected network, without remote verification.
    fn network(&self) -> &Network;
    /// Sends the exact reviewed-body-bound payload once. Acknowledgement is not execution.
    ///
    /// # Errors
    /// Before dispatch returns ordinary failures; an uncertain dispatched result
    /// returns `SubmissionOutcomeUnknown`, preserving possible submission.
    fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> impl Future<Output = Result<Observation<SubmissionResult>, Error>> + crate::future::MaybeSend;
}
