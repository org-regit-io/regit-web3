// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Serialize};
use std::fmt;

use super::{
    Address, BlockContext, ChainId, Data, FeeTerms, OperationContext, OperationValue,
    PreparedTransaction, Quantity, ReadOperation, ReadState,
};
use crate::error::{Error, ValidationError};

fn invalid() -> Error {
    ValidationError::InvalidEvmPreparation.into()
}

/// Complete explicit local call/estimation parameters, without state or block overrides.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionCallData {
    /// Exact expected server chain.
    pub chain_id: ChainId,
    /// Explicit simulated sender; no key or signature is loaded.
    pub from: Address,
    /// Explicit recipient, or contract creation when absent.
    pub to: Option<Address>,
    /// Exact caller-chosen nonce.
    pub nonce: u64,
    /// Explicit nonzero call/estimation gas cap.
    pub gas_limit: u64,
    /// Exact native wei, including explicit zero.
    pub value: Quantity,
    /// Exact bounded calldata or creation code.
    pub input: Data,
    /// Complete explicit fee terms; no fee default is chosen.
    pub fees: FeeTerms,
}

/// Immutable explicitly scoped `eth_call`/`eth_estimateGas` input.
/// These operations simulate locally at the node and never sign or submit.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionCallData", into = "TransactionCallData")]
pub struct TransactionCall(TransactionCallData);
impl TransactionCall {
    /// Checks local bounds and internally consistent fee choices.
    /// # Errors
    /// Rejects unusable nonce, zero gas or invalid/excessive fee/access-list fields.
    pub fn new(data: TransactionCallData) -> Result<Self, Error> {
        data.fees.validate()?;
        if data.nonce == u64::MAX || data.gas_limit == 0 {
            return Err(invalid());
        }
        Ok(Self(data))
    }
    /// Derives the exact simulation parameters from a reviewed preparation.
    #[must_use]
    pub fn from_prepared(prepared: &PreparedTransaction) -> Self {
        let request = prepared.request().data();
        let unsigned = prepared.unsigned();
        Self(TransactionCallData {
            chain_id: request.chain_id,
            from: request.sender,
            to: Some(unsigned.to()),
            nonce: request.nonce,
            gas_limit: request.gas_limit,
            value: unsigned.value(),
            input: unsigned.input().clone(),
            fees: request.fees.clone(),
        })
    }
    /// Returns all exact immutable call parameters.
    #[must_use]
    pub const fn data(&self) -> &TransactionCallData {
        &self.0
    }
}
impl TryFrom<TransactionCallData> for TransactionCall {
    type Error = Error;
    fn try_from(value: TransactionCallData) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<TransactionCall> for TransactionCallData {
    fn from(value: TransactionCall) -> Self {
        value.0
    }
}
impl fmt::Debug for TransactionCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionCall").finish_non_exhaustive()
    }
}

/// An exact source account nonce at the observation's captured canonical hash.
/// It excludes future/pending transactions and does not reserve a signing nonce.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountNonce {
    /// Exact expected chain.
    pub chain_id: ChainId,
    /// Exact queried account.
    pub address: Address,
    /// Exact source-reported account nonce, including a terminal protocol value.
    pub nonce: u64,
}
impl OperationValue for AccountNonce {
    fn validate_context(&self, context: &OperationContext) -> Result<(), Error> {
        canonical(context, self.chain_id, ReadOperation::AccountNonce)
    }
}

/// Successful source-returned call bytes and their exact requested local parameters.
/// An RPC code3 failure is separately `Error::ExecutionReverted`; returned bytes
/// do not prove a token's boolean result, reviewed signature or future execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallResult {
    /// Exact immutable simulated parameters.
    pub call: TransactionCall,
    /// Exact returned bounded bytes; empty success remains empty.
    pub output: Data,
}
impl OperationValue for CallResult {
    fn validate_context(&self, context: &OperationContext) -> Result<(), Error> {
        canonical(context, self.call.data().chain_id, ReadOperation::Call)
    }
}

/// A node's gas estimate at the exact captured canonical state, not a guarantee.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "GasEstimateFields")]
pub struct GasEstimate {
    call: TransactionCall,
    gas: u64,
}
impl GasEstimate {
    /// Records a positive estimate within the explicit caller gas cap.
    /// # Errors
    /// Rejects zero or an estimate exceeding the supplied cap.
    pub fn new(call: TransactionCall, gas: u64) -> Result<Self, Error> {
        if gas == 0 || gas > call.data().gas_limit {
            return Err(invalid());
        }
        Ok(Self { call, gas })
    }
    /// Returns the exact parameters used for estimation.
    #[must_use]
    pub const fn call(&self) -> &TransactionCall {
        &self.call
    }
    /// Returns exact source-estimated gas units, without an added margin.
    #[must_use]
    pub const fn gas(&self) -> u64 {
        self.gas
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GasEstimateFields {
    call: TransactionCall,
    gas: u64,
}
impl TryFrom<GasEstimateFields> for GasEstimate {
    type Error = Error;
    fn try_from(value: GasEstimateFields) -> Result<Self, Error> {
        Self::new(value.call, value.gas)
    }
}
impl OperationValue for GasEstimate {
    fn validate_context(&self, context: &OperationContext) -> Result<(), Error> {
        canonical(
            context,
            self.call.data().chain_id,
            ReadOperation::GasEstimate,
        )
    }
}

fn canonical(
    context: &OperationContext,
    chain: ChainId,
    operation: ReadOperation,
) -> Result<(), Error> {
    if context.network().chain_id() != chain
        || context.operation() != operation
        || !matches!(context.state(), ReadState::CanonicalHash { .. })
    {
        return Err(ValidationError::ObservationOperationMismatch.into());
    }
    Ok(())
}

/// Actual availability of a separately queried optional node fee suggestion.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "outcome",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum OptionalFeeSuggestion {
    /// Exact source suggestion in wei per gas.
    Available(Quantity),
    /// The source reports the optional method/capability unsupported.
    Unsupported,
    /// The source returned explicit null, rather than a numeric suggestion.
    NullResult,
    /// The source reports the requested data unavailable.
    Unavailable,
}

/// Actual block header base-fee fact queried separately from node price suggestions.
/// Absence may describe a non-EIP1559 header; no base fee is synthesized.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockFee {
    /// The source header's exact number/hash/time, independently identity-matched.
    pub block: BlockContext,
    /// Exact reported base fee in wei per gas, or actual omission.
    pub base_fee_per_gas: Option<Quantity>,
}

/// Independently queried node fee preferences and a matching resolved block header.
/// These are separate point-in-time facts, not one atomic or hash-pinned fee oracle.
/// The caller owns gas margins, fee-cap policy and transaction replacement policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeeSuggestions {
    /// Exact expected chain.
    pub chain_id: ChainId,
    /// Actual `eth_gasPrice` preference in wei per gas.
    pub gas_price: Quantity,
    /// Optional actual `eth_maxPriorityFeePerGas` result.
    pub max_priority_fee_per_gas: OptionalFeeSuggestion,
    /// Actual separately queried source block and optional base fee.
    pub block_fee: BlockFee,
}
impl OperationValue for FeeSuggestions {
    fn validate_context(&self, context: &OperationContext) -> Result<(), Error> {
        if context.network().chain_id() != self.chain_id
            || context.operation() != ReadOperation::FeeSuggestions
            || context.state() != ReadState::Unanchored
        {
            return Err(ValidationError::ObservationOperationMismatch.into());
        }
        Ok(())
    }
}
