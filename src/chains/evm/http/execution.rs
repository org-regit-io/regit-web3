// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::wire::{
    CanonicalBlock,
    execution::{Call, FeeBlock},
    invalid_response, parse_quantity, state_error,
};
use super::EvmClient;
use crate::{
    domain::evm::{
        AccountNonce, CallResult, FeeSuggestions, GasEstimate, OperationObservation,
        OptionalFeeSuggestion, Quantity, ReadOperation, ReadState, TransactionCall,
    },
    domain::{Address, BlockContext, BlockSelector},
    error::{Error, ProviderError},
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
    /// Queries separate node price suggestions and an identity-matched latest header.
    ///
    /// Gas price, optional priority price and base fee are independently sourced;
    /// no atomic fee snapshot or transaction fee policy is implied. Missing base
    /// fee remains absent. All stages and read retries share one total budget.
    /// # Errors
    /// Reports chain/header mismatch, malformed required prices and backend failures.
    /// Unknown optional-method RPC failures fail rather than invent a suggestion.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_fee_suggestions(&self) -> Result<OperationObservation<FeeSuggestions>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                self.verify_chain(&budget).await?;
                let block = self.resolve_block(BlockSelector::Latest, &budget).await?;
                let header: FeeBlock = self
                    .http
                    .read_rpc(
                        &budget,
                        "eth_getBlockByHash",
                        &(block.hash().to_string(), false),
                        state_error,
                    )
                    .await?
                    .ok_or(Error::UnavailableData)?;
                let block_fee = header.into_domain(block)?;
                let gas_price: String = self
                    .http
                    .read_rpc(&budget, "eth_gasPrice", &[] as &[(); 0], state_error)
                    .await?
                    .ok_or(Error::UnavailableData)?;
                let gas_price = Quantity::new(parse_quantity(&gas_price)?);
                let max_priority_fee_per_gas = self.priority_fee(&budget).await?;
                OperationObservation::new(
                    FeeSuggestions {
                        chain_id: self.chain_id,
                        gas_price,
                        max_priority_fee_per_gas,
                        block_fee,
                    },
                    self.read_context(
                        ReadOperation::FeeSuggestions,
                        ReadState::Unanchored,
                        "eth_gasPrice+eth_maxPriorityFeePerGas+eth_getBlockByHash",
                    )?,
                )
            })
            .await
    }

    /// Reads the exact account nonce at one captured canonical block hash.
    ///
    /// Pending transactions are excluded. This does not reserve or automatically
    /// choose a signing nonce. Retries retain the identical account and hash.
    /// # Errors
    /// Reports unavailable state, inconsistent identity, nonce overflow and backend failures.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_account_nonce(
        &self,
        address: Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<AccountNonce>, Error> {
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                let (selector, block) = self.execution_state(selector, &budget).await?;
                let result: String = self
                    .http
                    .read_rpc(
                        &budget,
                        "eth_getTransactionCount",
                        &(address, CanonicalBlock::new(*block.hash())),
                        state_error,
                    )
                    .await?
                    .ok_or(Error::UnavailableData)?;
                let nonce =
                    u64::try_from(parse_quantity(&result)?).map_err(|_| invalid_response())?;
                OperationObservation::new(
                    AccountNonce {
                        chain_id: self.chain_id,
                        address,
                        nonce,
                    },
                    self.read_context(
                        ReadOperation::AccountNonce,
                        ReadState::CanonicalHash {
                            requested_selector: selector,
                            block,
                        },
                        "eth_getTransactionCount",
                    )?,
                )
            })
            .await
    }

    /// Executes explicit parameters locally at one captured canonical state.
    ///
    /// No transaction is signed or submitted. Returned bytes are exact source
    /// output, without token boolean interpretation or a future-success guarantee.
    /// Sender, nonce, gas cap, value and fees are explicit; no overrides are used.
    /// # Errors
    /// Reports source code3 revert, null result, chain/state and fixed backend failures.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn call(
        &self,
        call: TransactionCall,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<CallResult>, Error> {
        self.check_call_chain(&call)?;
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                let (selector, block) = self.execution_state(selector, &budget).await?;
                let output = self
                    .http
                    .read_rpc(
                        &budget,
                        "eth_call",
                        &(Call(&call), CanonicalBlock::new(*block.hash())),
                        call_error,
                    )
                    .await?
                    .ok_or(Error::UnavailableData)?;
                OperationObservation::new(
                    CallResult { call, output },
                    self.read_context(
                        ReadOperation::Call,
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

    /// Estimates gas with explicit parameters at one captured canonical block hash.
    ///
    /// This requires the Geth-compatible hash-selector extension: the execution
    /// API standard does not require hash selection for `eth_estimateGas`. An
    /// unsupported selector remains a typed backend failure, with no fallback to
    /// height/latest. RPC `-32602` remains an ambiguous fixed RPC error. Estimates
    /// retain the caller's cap and do not guarantee later execution or add margin.
    /// # Errors
    /// Reports source revert, unsupported capability, malformed/out-of-cap estimate,
    /// unavailable state and backend failures. No signing or submission occurs.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn estimate_gas(
        &self,
        call: TransactionCall,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<GasEstimate>, Error> {
        self.check_call_chain(&call)?;
        let budget = OperationBudget::new(self.config.limits())?;
        budget
            .run(async {
                let (selector, block) = self.execution_state(selector, &budget).await?;
                let result: String = self
                    .http
                    .read_rpc(
                        &budget,
                        "eth_estimateGas",
                        &(Call(&call), CanonicalBlock::new(*block.hash())),
                        call_error,
                    )
                    .await?
                    .ok_or(Error::UnavailableData)?;
                let gas =
                    u64::try_from(parse_quantity(&result)?).map_err(|_| invalid_response())?;
                let value = GasEstimate::new(call, gas).map_err(|_| invalid_response())?;
                OperationObservation::new(
                    value,
                    self.read_context(
                        ReadOperation::GasEstimate,
                        ReadState::CanonicalHash {
                            requested_selector: selector,
                            block,
                        },
                        "eth_estimateGas",
                    )?,
                )
            })
            .await
    }

    fn check_call_chain(&self, call: &TransactionCall) -> Result<(), Error> {
        if call.data().chain_id != self.chain_id {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(())
    }

    async fn execution_state(
        &self,
        selector: Option<BlockSelector>,
        budget: &OperationBudget,
    ) -> Result<(BlockSelector, BlockContext), Error> {
        self.verify_chain(budget).await?;
        let selector = selector.unwrap_or_else(|| self.config.default_selector());
        let block = self.resolve_block(selector, budget).await?;
        Ok((selector, block))
    }

    async fn priority_fee(&self, budget: &OperationBudget) -> Result<OptionalFeeSuggestion, Error> {
        let result: Result<Option<String>, Error> = self
            .http
            .read_rpc(
                budget,
                "eth_maxPriorityFeePerGas",
                &[] as &[(); 0],
                state_error,
            )
            .await;
        match result {
            Ok(Some(value)) => Ok(OptionalFeeSuggestion::Available(Quantity::new(
                parse_quantity(&value)?,
            ))),
            Ok(None) => Ok(OptionalFeeSuggestion::NullResult),
            Err(Error::UnsupportedCapability) => Ok(OptionalFeeSuggestion::Unsupported),
            Err(Error::UnavailableData) => Ok(OptionalFeeSuggestion::Unavailable),
            Err(error) => Err(error),
        }
    }
}

impl super::super::FeeReader for EvmClient {
    async fn get_fee_suggestions(&self) -> Result<OperationObservation<FeeSuggestions>, Error> {
        Self::get_fee_suggestions(self).await
    }
}
impl super::super::ExecutionReader for EvmClient {
    async fn get_account_nonce(
        &self,
        address: Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<AccountNonce>, Error> {
        Self::get_account_nonce(self, address, selector).await
    }
    async fn call(
        &self,
        call: TransactionCall,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<CallResult>, Error> {
        Self::call(self, call, selector).await
    }
    async fn estimate_gas(
        &self,
        call: TransactionCall,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<GasEstimate>, Error> {
        Self::estimate_gas(self, call, selector).await
    }
}
