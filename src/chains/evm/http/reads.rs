// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::wire::{
    CanonicalBlock, abi, invalid_response,
    reads::{Call, WireReceipt, WireTransaction},
    state_error,
};
use super::{EvmClient, SystemTime, UNIX_EPOCH};
use crate::{
    domain::evm::{
        Data, Erc20Allowance, Erc20Balance, Erc20Metadata, MetadataUnavailable, MetadataValue,
        OperationContext, OperationObservation, ReadOperation, ReadState, ReceiptLookup,
        TransactionId, TransactionLookup, TransactionStatus,
    },
    domain::{Address, BlockSelector, Source, Timestamp},
    error::Error,
    transport::OperationBudget,
};

fn call_error(code: i64) -> Error {
    if code == 3 {
        Error::ExecutionReverted
    } else {
        state_error(code)
    }
}
impl EvmClient {
    /// Reads exact ERC-20 raw units using one frozen canonical block hash.
    ///
    /// No precision is assumed. Each operation verifies the expected chain and
    /// resolves its selector once; call retries retain exact calldata and hash.
    ///
    /// # Errors
    /// Reports source execution revert, null data, bad ABI and fixed backend errors.
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_erc20_balance(
        &self,
        contract: Address,
        owner: Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<Erc20Balance>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                let (selector, block) = self.token_state(selector, &budget).await?;
                let bytes = self
                    .token_call(contract, abi::balance(owner), *block.hash(), &budget)
                    .await?
                    .ok_or(Error::UnavailableData)?;
                let value =
                    Erc20Balance::new(self.chain_id, contract, owner, abi::uint256(&bytes)?);
                OperationObservation::new(
                    value,
                    self.read_context(
                        ReadOperation::Erc20Balance,
                        ReadState::CanonicalHash {
                            requested_selector: selector,
                            block,
                        },
                        "eth_call",
                    )?,
                )
            })
            .await
    }
    /// Reads exact ERC-20 allowance using one frozen canonical block hash.
    ///
    /// Owner/spender are explicit and zero values remain actual raw values.
    /// # Errors
    /// Reports source execution revert, null data, bad ABI and fixed backend errors.
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_erc20_allowance(
        &self,
        contract: Address,
        owner: Address,
        spender: Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<Erc20Allowance>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                let (selector, block) = self.token_state(selector, &budget).await?;
                let bytes = self
                    .token_call(
                        contract,
                        abi::allowance(owner, spender),
                        *block.hash(),
                        &budget,
                    )
                    .await?
                    .ok_or(Error::UnavailableData)?;
                let value = Erc20Allowance::new(
                    self.chain_id,
                    contract,
                    owner,
                    spender,
                    abi::uint256(&bytes)?,
                );
                OperationObservation::new(
                    value,
                    self.read_context(
                        ReadOperation::Erc20Allowance,
                        ReadState::CanonicalHash {
                            requested_selector: selector,
                            block,
                        },
                        "eth_call",
                    )?,
                )
            })
            .await
    }
    /// Reads separate optional standard-string name/symbol and uint8 precision.
    ///
    /// All calls use one captured canonical hash and one total operation budget.
    /// Bytes32 metadata is unsupported ABI, not silently reinterpreted. Exact
    /// RPC code3 records source-reported revert; diagnostic text is not read.
    /// Unknown RPC and transport failures fail the operation. No token identity,
    /// precision default or execution guarantee is inferred from metadata.
    /// # Errors
    /// Reports chain/state/protocol/transport failures; supported optional method
    /// absence, revert and invalid ABI remain separate per-field outcomes.
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_erc20_metadata(
        &self,
        contract: Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<Erc20Metadata>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                let (selector, block) = self.token_state(selector, &budget).await?;
                let name = self
                    .metadata_field(contract, abi::NAME, *block.hash(), &budget, abi::text)
                    .await?;
                let symbol = self
                    .metadata_field(contract, abi::SYMBOL, *block.hash(), &budget, abi::text)
                    .await?;
                let decimals = self
                    .metadata_field(contract, abi::DECIMALS, *block.hash(), &budget, abi::uint8)
                    .await?;
                let value = Erc20Metadata::new(self.chain_id, contract, name, symbol, decimals);
                OperationObservation::new(
                    value,
                    self.read_context(
                        ReadOperation::Erc20Metadata,
                        ReadState::CanonicalHash {
                            requested_selector: selector,
                            block,
                        },
                        "eth_call",
                    )?,
                )
            })
            .await
    }
    /// Retrieves actual typed transaction fields or an explicit null lookup.
    ///
    /// Supported current source forms are legacy and types1–4. JSON identity and
    /// signatures are source facts, not independently computed/recovered. Pending
    /// inclusion is null; contradictory partial inclusion is rejected.
    /// # Errors
    /// Reports identity/network/field mismatch, unsupported type and backend errors.
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_transaction(
        &self,
        hash: TransactionId,
    ) -> Result<OperationObservation<TransactionLookup>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                self.verify_chain(&budget).await?;
                let value = self.transaction_lookup(hash, &budget).await?;
                let state = value.state();
                OperationObservation::new(
                    value,
                    self.read_context(
                        ReadOperation::Transaction,
                        state,
                        "eth_getTransactionByHash",
                    )?,
                )
            })
            .await
    }
    /// Retrieves an exact receipt or explicit absence, retaining top-level outcome.
    ///
    /// Status0 is failure, status1 top-level success; historical root or absent
    /// status remains explicit uncertainty. Logs are bounded and identity-matched.
    /// # Errors
    /// Reports malformed/mismatched receipt/logs and fixed backend failures.
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_receipt(
        &self,
        hash: TransactionId,
    ) -> Result<OperationObservation<ReceiptLookup>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                self.verify_chain(&budget).await?;
                let value = self.receipt_lookup(hash, &budget).await?;
                let state = value.state();
                OperationObservation::new(
                    value,
                    self.read_context(ReadOperation::Receipt, state, "eth_getTransactionReceipt")?,
                )
            })
            .await
    }
    /// Matches sequential transaction/receipt facts under one total budget.
    ///
    /// These lookups are not atomic. Matching receipt inclusion can supersede an
    /// earlier pending transaction; conflicting included blocks fail. Missing
    /// receipt never means execution success. Finality remains unknown.
    /// # Errors
    /// Reports contradictory identities/inclusion/addresses/type/gas and backend errors.
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_transaction_status(
        &self,
        hash: TransactionId,
    ) -> Result<OperationObservation<TransactionStatus>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                self.verify_chain(&budget).await?;
                let transaction = self.transaction_lookup(hash, &budget).await?;
                let receipt = self.receipt_lookup(hash, &budget).await?;
                let value =
                    TransactionStatus::new(transaction, receipt).map_err(|_| invalid_response())?;
                let state = value.read_state();
                OperationObservation::new(
                    value,
                    self.read_context(
                        ReadOperation::TransactionStatus,
                        state,
                        "eth_getTransactionByHash+eth_getTransactionReceipt",
                    )?,
                )
            })
            .await
    }
    async fn token_state(
        &self,
        selector: Option<BlockSelector>,
        budget: &OperationBudget,
    ) -> Result<(BlockSelector, crate::domain::BlockContext), Error> {
        self.verify_chain(budget).await?;
        let selector = selector.unwrap_or_else(|| self.config.default_selector());
        let block = self.resolve_block(selector, budget).await?;
        Ok((selector, block))
    }
    async fn token_call(
        &self,
        contract: Address,
        data: Vec<u8>,
        hash: crate::domain::BlockHash,
        budget: &OperationBudget,
    ) -> Result<Option<Data>, Error> {
        self.http
            .read_rpc(
                budget,
                "eth_call",
                &(
                    Call {
                        to: contract,
                        data: const_hex::encode_prefixed(data),
                    },
                    CanonicalBlock::new(hash),
                ),
                call_error,
            )
            .await
    }
    async fn metadata_field<T>(
        &self,
        contract: Address,
        selector: [u8; 4],
        hash: crate::domain::BlockHash,
        budget: &OperationBudget,
        decode: fn(&Data) -> Result<T, Error>,
    ) -> Result<MetadataValue<T>, Error> {
        match self
            .token_call(contract, selector.to_vec(), hash, budget)
            .await
        {
            Ok(Some(bytes)) => {
                Ok(decode(&bytes).map_or(MetadataValue::InvalidAbi, MetadataValue::Available))
            }
            Ok(None) => Ok(MetadataValue::Unavailable(MetadataUnavailable::NullResult)),
            Err(Error::ExecutionReverted) => Ok(MetadataValue::Reverted),
            Err(Error::UnsupportedCapability) => {
                Ok(MetadataValue::Unavailable(MetadataUnavailable::Unsupported))
            }
            Err(Error::UnavailableData) => Ok(MetadataValue::Unavailable(
                MetadataUnavailable::StateUnavailable,
            )),
            Err(error) => Err(error),
        }
    }
    async fn transaction_lookup(
        &self,
        hash: TransactionId,
        budget: &OperationBudget,
    ) -> Result<TransactionLookup, Error> {
        let result: Option<WireTransaction> = self
            .http
            .read_rpc(budget, "eth_getTransactionByHash", &(hash,), state_error)
            .await?;
        let transaction = result
            .map(|wire| wire.into_domain(self.chain_id, hash))
            .transpose()?;
        TransactionLookup::new(self.chain_id, hash, transaction).map_err(|_| invalid_response())
    }
    async fn receipt_lookup(
        &self,
        hash: TransactionId,
        budget: &OperationBudget,
    ) -> Result<ReceiptLookup, Error> {
        let result: Option<WireReceipt> = self
            .http
            .read_rpc(budget, "eth_getTransactionReceipt", &(hash,), state_error)
            .await?;
        let receipt = result
            .map(|wire| wire.into_domain(self.chain_id, hash))
            .transpose()?;
        ReceiptLookup::new(self.chain_id, hash, receipt).map_err(|_| invalid_response())
    }
    pub(super) fn read_context(
        &self,
        operation: ReadOperation,
        state: ReadState,
        method: &str,
    ) -> Result<OperationContext, Error> {
        let retrieved = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        OperationContext::new(
            operation,
            self.config.network().clone(),
            state,
            Source::new(self.config.provider_id(), method, env!("CARGO_PKG_VERSION"))?,
            Timestamp::from_unix_seconds(retrieved),
        )
    }
}
impl super::super::Erc20Reader for EvmClient {
    async fn get_erc20_balance(
        &self,
        contract: Address,
        owner: Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<Erc20Balance>, Error> {
        Self::get_erc20_balance(self, contract, owner, selector).await
    }
    async fn get_erc20_allowance(
        &self,
        contract: Address,
        owner: Address,
        spender: Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<Erc20Allowance>, Error> {
        Self::get_erc20_allowance(self, contract, owner, spender, selector).await
    }
    async fn get_erc20_metadata(
        &self,
        contract: Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<Erc20Metadata>, Error> {
        Self::get_erc20_metadata(self, contract, selector).await
    }
}
impl super::super::TransactionReader for EvmClient {
    async fn get_transaction(
        &self,
        hash: TransactionId,
    ) -> Result<OperationObservation<TransactionLookup>, Error> {
        Self::get_transaction(self, hash).await
    }
    async fn get_receipt(
        &self,
        hash: TransactionId,
    ) -> Result<OperationObservation<ReceiptLookup>, Error> {
        Self::get_receipt(self, hash).await
    }
    async fn get_transaction_status(
        &self,
        hash: TransactionId,
    ) -> Result<OperationObservation<TransactionStatus>, Error> {
        Self::get_transaction_status(self, hash).await
    }
}
