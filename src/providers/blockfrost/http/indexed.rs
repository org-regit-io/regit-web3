// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{BlockfrostClient, wire};
use crate::{
    domain::cardano::{
        AddressDetails, AssetDetails, AssetEntry, AssetHolder, AssetId, AssetKind, Epoch,
        EpochSelector, Hash, IndexPage, NetworkData, Observation, Operation, Order, PageRequest,
        PageTarget, PaymentAddress, PaymentEstimate, PaymentIntent, ProtocolParameters, Reward,
        SignedSubmission, StakeAccount, StakeAddress, SubmissionResult, Transaction,
        TransactionReference, TransactionStatus, TransactionUtxos,
    },
    error::{Error, ValidationError},
    transport::{OperationBudget, submission_unknown},
};

impl BlockfrostClient {
    async fn indexed_bytes(
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
    fn payment_address(&self, address: &PaymentAddress) -> Result<(), Error> {
        if !address.is_compatible_with(self.network().identity()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(())
    }
    fn stake_address(&self, address: &StakeAddress) -> Result<(), Error> {
        if !address.is_compatible_with(self.network().identity()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(())
    }
    fn token_asset(&self, asset: &AssetId) -> Result<String, Error> {
        if asset.network() != self.network().identity() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        match asset.asset() {
            AssetKind::Native => Err(Error::UnsupportedCapability),
            AssetKind::Token {
                policy_id,
                asset_name,
            } => Ok(format!("{policy_id}{asset_name}")),
        }
    }
    async fn epoch_bytes(
        &self,
        selector: EpochSelector,
        parameters: bool,
        budget: &OperationBudget,
    ) -> Result<Vec<u8>, Error> {
        let selected = match selector {
            EpochSelector::Latest => "latest".into(),
            EpochSelector::Number(n) => n.to_string(),
        };
        if parameters {
            self.indexed_bytes(&["epochs", &selected, "parameters"], &[], budget)
                .await
        } else {
            self.indexed_bytes(&["epochs", &selected], &[], budget)
                .await
        }
    }
    async fn page_bytes(
        &self,
        path: &[&str],
        page: PageRequest,
        budget: &OperationBudget,
    ) -> Result<Vec<u8>, Error> {
        let number = page.page().to_string();
        let count = page.count().to_string();
        let order = match page.order() {
            Order::Asc => "asc",
            Order::Desc => "desc",
        };
        self.indexed_bytes(
            path,
            &[("page", &number), ("count", &count), ("order", order)],
            budget,
        )
        .await
    }
    /// Retrieves exact indexed source supply/stake, without a claimed evaluation block.
    ///
    /// # Errors
    /// Returns fixed network, provider, malformed-data or whole-operation deadline failures.
    pub async fn get_network_data(&self) -> Result<Observation<NetworkData>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.indexed_bytes(&["network"], &[], &budget).await?;
                Observation::new(
                    wire::indexed::network(&bytes, self.network())?,
                    self.context(Operation::NetworkData)?,
                )
            })
            .await
    }
    /// Retrieves latest or exact indexed epoch facts and their actual time units.
    ///
    /// # Errors
    /// Rejects mismatched selected epoch, malformed data, provider/network/deadline failures.
    pub async fn get_epoch(&self, selector: EpochSelector) -> Result<Observation<Epoch>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.epoch_bytes(selector, false, &budget).await?;
                Observation::new(
                    wire::indexed::epoch(&bytes, self.network(), selector)?,
                    self.context(Operation::Epoch)?,
                )
            })
            .await
    }
    /// Retrieves explicit source protocol parameters; prices retain exact decimal values.
    ///
    /// # Errors
    /// Rejects selected-epoch mismatch, malformed/excessive data and provider/deadline failures.
    pub async fn get_protocol_parameters(
        &self,
        selector: EpochSelector,
    ) -> Result<Observation<ProtocolParameters>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.epoch_bytes(selector, true, &budget).await?;
                Observation::new(
                    wire::indexed::parameters(&bytes, self.network(), selector)?,
                    self.context(Operation::ProtocolParameters)?,
                )
            })
            .await
    }
    /// Retrieves supporting address classification/stake metadata and all indexed balances.
    ///
    /// # Errors
    /// Rejects address/network/echo mismatch, malformed or unavailable data and deadline failures.
    pub async fn get_address_details(
        &self,
        address: PaymentAddress,
    ) -> Result<Observation<AddressDetails>, Error> {
        self.payment_address(&address)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .indexed_bytes(&["addresses", address.as_str()], &[], &budget)
                    .await?;
                Observation::new(
                    wire::indexed::address(&bytes, self.network(), &address)?,
                    self.context(Operation::AddressDetails)?,
                )
            })
            .await
    }
    /// Retrieves one exact address's bounded transaction page without hidden fetching.
    ///
    /// # Errors
    /// Rejects incompatible addresses, excess/duplicate/out-of-order rows and provider failures.
    pub async fn get_address_transactions(
        &self,
        address: PaymentAddress,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<TransactionReference>>, Error> {
        self.payment_address(&address)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .page_bytes(
                        &["addresses", address.as_str(), "transactions"],
                        page,
                        &budget,
                    )
                    .await?;
                Observation::new(
                    wire::indexed::transaction_page(
                        &bytes,
                        self.network(),
                        PageTarget::AddressTransactions { address },
                        page,
                    )?,
                    self.context(Operation::AddressTransactions)?,
                )
            })
            .await
    }
    /// Retrieves actual source stake registration/delegation and exact reward balances.
    ///
    /// # Errors
    /// Rejects wrong-network/echo identities, malformed/null substitution and provider failures.
    pub async fn get_stake_account(
        &self,
        address: StakeAddress,
    ) -> Result<Observation<StakeAccount>, Error> {
        self.stake_address(&address)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .indexed_bytes(&["accounts", address.as_str()], &[], &budget)
                    .await?;
                Observation::new(
                    wire::indexed::account(&bytes, self.network(), &address)?,
                    self.context(Operation::StakeAccount)?,
                )
            })
            .await
    }
    /// Retrieves one explicit reward page; rewards are not assumed withdrawable.
    ///
    /// # Errors
    /// Rejects wrong-network identities, excessive/duplicate rows and provider/deadline failures.
    pub async fn get_stake_rewards(
        &self,
        address: StakeAddress,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<Reward>>, Error> {
        self.stake_address(&address)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .page_bytes(&["accounts", address.as_str(), "rewards"], page, &budget)
                    .await?;
                Observation::new(
                    wire::indexed::reward_page(&bytes, self.network(), address, page)?,
                    self.context(Operation::Rewards)?,
                )
            })
            .await
    }
    /// Retrieves one bounded native-asset catalogue page with exact base quantities.
    ///
    /// # Errors
    /// Rejects excess/duplicate/malformed asset rows and provider/network/deadline failures.
    pub async fn get_assets(
        &self,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<AssetEntry>>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.page_bytes(&["assets"], page, &budget).await?;
                Observation::new(
                    wire::indexed::asset_page(&bytes, self.network(), page)?,
                    self.context(Operation::Assets)?,
                )
            })
            .await
    }
    /// Retrieves a full exact token identity/supply/provenance and source display metadata.
    ///
    /// # Errors
    /// Rejects ADA, mismatched policy/name/fingerprint, malformed data and provider failures.
    pub async fn get_asset(&self, asset: AssetId) -> Result<Observation<AssetDetails>, Error> {
        let unit = self.token_asset(&asset)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.indexed_bytes(&["assets", &unit], &[], &budget).await?;
                Observation::new(
                    wire::indexed::asset(&bytes, self.network(), &asset)?,
                    self.context(Operation::AssetDetails)?,
                )
            })
            .await
    }
    /// Retrieves one bounded token transaction page without claiming an atomic index.
    ///
    /// # Errors
    /// Rejects ADA/wrong-network inputs, excess/duplicate/out-of-order rows and provider failures.
    pub async fn get_asset_transactions(
        &self,
        asset: AssetId,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<TransactionReference>>, Error> {
        let unit = self.token_asset(&asset)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .page_bytes(&["assets", &unit, "transactions"], page, &budget)
                    .await?;
                Observation::new(
                    wire::indexed::transaction_page(
                        &bytes,
                        self.network(),
                        PageTarget::AssetTransactions { asset },
                        page,
                    )?,
                    self.context(Operation::AssetTransactions)?,
                )
            })
            .await
    }
    /// Retrieves one exact token's bounded holder page with precision unreported.
    ///
    /// # Errors
    /// Rejects ADA/wrong-network identities, excess/duplicate rows and provider failures.
    pub async fn get_asset_holders(
        &self,
        asset: AssetId,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<AssetHolder>>, Error> {
        let unit = self.token_asset(&asset)?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self
                    .page_bytes(&["assets", &unit, "addresses"], page, &budget)
                    .await?;
                Observation::new(
                    wire::indexed::holder_page(&bytes, self.network(), asset, page)?,
                    self.context(Operation::AssetHolders)?,
                )
            })
            .await
    }
    async fn transaction_data(
        &self,
        id: Hash,
        budget: &OperationBudget,
    ) -> Result<Transaction, Error> {
        let hash = id.to_string();
        let bytes = self.indexed_bytes(&["txs", &hash], &[], budget).await?;
        let summary = wire::transaction::Summary::decode(&bytes, id)?;
        let bytes = self
            .indexed_bytes(&["txs", &hash, "cbor"], &[], budget)
            .await?;
        summary.into_domain(self.network(), wire::transaction::cbor(&bytes)?)
    }
    /// Retrieves full source transaction summary and bounded original-body-hash CBOR.
    ///
    /// # Errors
    /// Rejects source/payload identity, fee/validity mismatch, malformed data and deadline failures.
    pub async fn get_transaction(&self, id: Hash) -> Result<Observation<Transaction>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let tx = self.transaction_data(id, &budget).await?;
                Observation::new(tx, self.context(Operation::Transaction)?)
            })
            .await
    }
    /// Retrieves complete bounded source input/output facts, including separate role metadata.
    ///
    /// # Errors
    /// Rejects source hash/identity/quantity mismatch, more than 1000 rows or provider failures.
    pub async fn get_transaction_utxos(
        &self,
        id: Hash,
    ) -> Result<Observation<TransactionUtxos>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let hash = id.to_string();
                let bytes = self
                    .indexed_bytes(&["txs", &hash, "utxos"], &[], &budget)
                    .await?;
                Observation::new(
                    wire::transaction::utxos(&bytes, self.network(), id)?,
                    self.context(Operation::TransactionUtxos)?,
                )
            })
            .await
    }
    /// Returns actual indexed inclusion/contract validity or explicit current unavailability.
    /// It does not invent confirmations, permanent absence, rejection or finality.
    ///
    /// # Errors
    /// Rejects malformed/cross-field source data and network/provider/deadline failures.
    pub async fn get_transaction_status(
        &self,
        id: Hash,
    ) -> Result<Observation<TransactionStatus>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let hash = id.to_string();
                let bytes = match self.indexed_bytes(&["txs", &hash], &[], &budget).await {
                    Ok(bytes) => bytes,
                    Err(Error::UnavailableData) => {
                        return Observation::new(
                            TransactionStatus::NotIndexed {
                                network: self.network().clone(),
                                transaction_id: id,
                            },
                            self.context(Operation::TransactionStatus)?,
                        );
                    }
                    Err(error) => return Err(error),
                };
                let summary = wire::transaction::Summary::decode(&bytes, id)?;
                let bytes = self
                    .indexed_bytes(&["txs", &hash, "cbor"], &[], &budget)
                    .await?;
                let transaction =
                    summary.into_domain(self.network(), wire::transaction::cbor(&bytes)?)?;
                let status = TransactionStatus::Indexed {
                    transaction: Box::new(transaction),
                };
                Observation::new(status, self.context(Operation::TransactionStatus)?)
            })
            .await
    }
    /// Calculates the exact ordinary payment profile with caller-selected fresh epoch parameters.
    /// No scripts are remotely simulated and no fee/change/output is silently altered.
    ///
    /// # Errors
    /// Rejects unsupported profiles, wrong network, inconsistent values/limits and provider failures.
    pub async fn estimate_payment_fee(
        &self,
        intent: PaymentIntent,
        selector: EpochSelector,
    ) -> Result<Observation<PaymentEstimate>, Error> {
        if intent.network().identity() != self.network().identity() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let bytes = self.epoch_bytes(selector, true, &budget).await?;
                let params = wire::indexed::parameters(&bytes, self.network(), selector)?;
                Observation::new(
                    PaymentEstimate::new(intent, params)?,
                    self.context(Operation::PaymentEstimate)?,
                )
            })
            .await
    }
    /// Submits one explicit externally signed supported ordinary transaction as raw CBOR.
    /// Source acknowledgement is not execution. Safe-read retry settings never retry this write.
    ///
    /// # Errors
    /// Preflight failures remain ordinary errors; after dispatch, any unestablished
    /// outcome retains possible submission through `SubmissionOutcomeUnknown`.
    pub async fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> Result<Observation<SubmissionResult>, Error> {
        if submission.network().identity() != self.network().identity() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget.run(self.verify_network(&budget)).await?;
        let response = self
            .http
            .write_once_cbor(
                &["tx", "submit"],
                &[],
                submission.transaction().bytes(),
                &budget,
            )
            .await?;
        let finish = || {
            let id = wire::transaction::acknowledged(&response.into_success()?)?;
            let result = SubmissionResult::new(submission, id)?;
            let observed = Observation::new(result, self.context(Operation::Submission)?)?;
            budget.check_remaining()?;
            Ok(observed)
        };
        finish().map_err(submission_unknown)
    }
}

impl crate::chains::cardano::CardanoReader for BlockfrostClient {
    async fn get_network_data(&self) -> Result<Observation<NetworkData>, Error> {
        Self::get_network_data(self).await
    }
    async fn get_epoch(&self, selector: EpochSelector) -> Result<Observation<Epoch>, Error> {
        Self::get_epoch(self, selector).await
    }
    async fn get_protocol_parameters(
        &self,
        selector: EpochSelector,
    ) -> Result<Observation<ProtocolParameters>, Error> {
        Self::get_protocol_parameters(self, selector).await
    }
    async fn get_address_details(
        &self,
        address: PaymentAddress,
    ) -> Result<Observation<AddressDetails>, Error> {
        Self::get_address_details(self, address).await
    }
    async fn get_address_transactions(
        &self,
        address: PaymentAddress,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<TransactionReference>>, Error> {
        Self::get_address_transactions(self, address, page).await
    }
    async fn get_stake_account(
        &self,
        address: StakeAddress,
    ) -> Result<Observation<StakeAccount>, Error> {
        Self::get_stake_account(self, address).await
    }
    async fn get_stake_rewards(
        &self,
        address: StakeAddress,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<Reward>>, Error> {
        Self::get_stake_rewards(self, address, page).await
    }
    async fn get_assets(
        &self,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<AssetEntry>>, Error> {
        Self::get_assets(self, page).await
    }
    async fn get_asset(&self, asset: AssetId) -> Result<Observation<AssetDetails>, Error> {
        Self::get_asset(self, asset).await
    }
    async fn get_asset_transactions(
        &self,
        asset: AssetId,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<TransactionReference>>, Error> {
        Self::get_asset_transactions(self, asset, page).await
    }
    async fn get_asset_holders(
        &self,
        asset: AssetId,
        page: PageRequest,
    ) -> Result<Observation<IndexPage<AssetHolder>>, Error> {
        Self::get_asset_holders(self, asset, page).await
    }
    async fn get_transaction(&self, id: Hash) -> Result<Observation<Transaction>, Error> {
        Self::get_transaction(self, id).await
    }
    async fn get_transaction_utxos(
        &self,
        id: Hash,
    ) -> Result<Observation<TransactionUtxos>, Error> {
        Self::get_transaction_utxos(self, id).await
    }
    async fn get_transaction_status(
        &self,
        id: Hash,
    ) -> Result<Observation<TransactionStatus>, Error> {
        Self::get_transaction_status(self, id).await
    }
    async fn estimate_payment_fee(
        &self,
        intent: PaymentIntent,
        selector: EpochSelector,
    ) -> Result<Observation<PaymentEstimate>, Error> {
        Self::estimate_payment_fee(self, intent, selector).await
    }
}

impl crate::chains::cardano::CardanoSubmitter for BlockfrostClient {
    fn network(&self) -> &crate::domain::cardano::Network {
        Self::network(self)
    }
    async fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> Result<Observation<SubmissionResult>, Error> {
        Self::submit_signed(self, submission).await
    }
}
