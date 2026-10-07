// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use super::{AccessListEntry, Address, ChainId, Data, Quantity, U256, Word, encoding};
use crate::{
    error::{Error, ValidationError},
    wallets::Preparation,
};

/// Caller-selected transaction fee terms; no fee or access list is inferred.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FeeTerms {
    /// EIP-155 replay-protected legacy fee terms, in wei per gas.
    Legacy {
        /// Exact wei per gas.
        gas_price: Quantity,
    },
    /// EIP-2930 ordered access-list transaction fee terms.
    AccessList {
        /// Exact wei per gas.
        gas_price: Quantity,
        /// Ordered entries and keys; legal duplicates are retained.
        access_list: Vec<AccessListEntry>,
    },
    /// EIP-1559 transaction fee bounds, in wei per gas.
    DynamicFee {
        /// Exact maximum total wei per gas, at least the priority cap.
        max_fee_per_gas: Quantity,
        /// Exact maximum priority wei per gas.
        max_priority_fee_per_gas: Quantity,
        /// Explicit ordered access list, including an explicitly empty list.
        access_list: Vec<AccessListEntry>,
    },
}
impl FeeTerms {
    /// Returns the supported protocol transaction type byte.
    #[must_use]
    pub const fn type_byte(&self) -> u8 {
        match self {
            Self::Legacy { .. } => 0,
            Self::AccessList { .. } => 1,
            Self::DynamicFee { .. } => 2,
        }
    }
    pub(super) fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Legacy { .. } => Ok(()),
            Self::AccessList { access_list, .. } => encoding::validate_access_list(access_list),
            Self::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => {
                if max_fee_per_gas < max_priority_fee_per_gas {
                    return Err(invalid());
                }
                encoding::validate_access_list(access_list)
            }
        }
    }
}

/// Exact ordinary transfer or approval intent; ERC-20 values are raw token units.
/// Zero transfers and zero approvals remain explicit, valid caller choices.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransferIntent {
    /// Native value transfer, with no calldata.
    Native {
        /// Exact recipient.
        to: Address,
        /// Exact native wei, with no floating-point conversion.
        value: Quantity,
    },
    /// Standard `transfer(address,uint256)` calldata; execution is not guaranteed.
    Erc20Transfer {
        /// Explicit token contract.
        contract: Address,
        /// Exact transfer recipient.
        to: Address,
        /// Exact raw token units; no metadata or precision default is applied.
        amount: Quantity,
    },
    /// Standard `approve(address,uint256)` calldata; caller owns allowance-change policy.
    Erc20Approval {
        /// Explicit token contract.
        contract: Address,
        /// Exact authorized spender.
        spender: Address,
        /// Exact raw allowance; zero can request revocation.
        amount: Quantity,
    },
}
impl TransferIntent {
    fn fields(&self) -> Result<(Address, Quantity, Data), Error> {
        match *self {
            Self::Native { to, value } => Ok((to, value, Data::new(Vec::new())?)),
            Self::Erc20Transfer {
                contract,
                to,
                amount,
            } => Ok((
                contract,
                Quantity::from(0),
                calldata([0xa9, 0x05, 0x9c, 0xbb], to, amount)?,
            )),
            Self::Erc20Approval {
                contract,
                spender,
                amount,
            } => Ok((
                contract,
                Quantity::from(0),
                calldata([0x09, 0x5e, 0xa7, 0xb3], spender, amount)?,
            )),
        }
    }
}
fn calldata(selector: [u8; 4], address: Address, amount: Quantity) -> Result<Data, Error> {
    let mut bytes = Vec::with_capacity(68);
    bytes.extend_from_slice(&selector);
    bytes.extend_from_slice(&[0; 12]);
    bytes.extend_from_slice(&address.bytes());
    bytes.extend_from_slice(&amount.value().to_be_bytes::<32>());
    Data::new(bytes)
}
fn invalid() -> Error {
    ValidationError::InvalidEvmPreparation.into()
}

/// Complete caller-owned signing intent and resource/fee choices.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionRequestData {
    /// Exact expected chain identity; aliases are not replay protection.
    pub chain_id: ChainId,
    /// Caller-reviewed expected sender; the signing bytes do not independently bind/recover it.
    pub sender: Address,
    /// Exact caller-chosen nonce, below the protocol's unusable `u64::MAX` value.
    pub nonce: u64,
    /// Exact caller-chosen nonzero maximum gas units; not an estimate or automatic margin.
    pub gas_limit: u64,
    /// Complete caller-selected fee terms and ordered access list.
    pub fees: FeeTerms,
    /// Exact ordinary action to review.
    pub intent: TransferIntent,
}

/// Immutable validated transaction request; no nonce, fee, precision or expiry is invented.
/// Ordinary EVM envelopes have no general expiry field. External timing/approval policy
/// remains caller-owned and this type does not promise successful execution.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionRequestData", into = "TransactionRequestData")]
pub struct TransactionRequest(TransactionRequestData);
impl TransactionRequest {
    /// Checks exact explicit signing choices without fetching, approving or signing.
    /// # Errors
    /// Rejects unusable nonce, zero gas, excessive access lists, inverted fee bounds
    /// or a legacy chain ID whose protected v value cannot fit 256 bits.
    pub fn new(data: TransactionRequestData) -> Result<Self, Error> {
        data.fees.validate()?;
        if data.nonce == u64::MAX
            || data.gas_limit == 0
            || matches!(data.fees, FeeTerms::Legacy { .. })
                && data.chain_id.value() > (U256::MAX - U256::from(36)) / U256::from(2)
        {
            return Err(invalid());
        }
        Ok(Self(data))
    }
    /// Returns all exact immutable caller choices.
    #[must_use]
    pub const fn data(&self) -> &TransactionRequestData {
        &self.0
    }
    /// Derives a canonical supported signing payload without signing or submission.
    /// # Errors
    /// Rejects a payload exceeding the local byte bound.
    pub fn prepare(self) -> Result<PreparedTransaction, Error> {
        PreparedTransaction::new(self)
    }
}
impl fmt::Debug for TransactionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionRequest").finish_non_exhaustive()
    }
}
impl TryFrom<TransactionRequestData> for TransactionRequest {
    type Error = Error;
    fn try_from(value: TransactionRequestData) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<TransactionRequest> for TransactionRequestData {
    fn from(value: TransactionRequest) -> Self {
        value.0
    }
}

/// Exact unsigned supported transaction fields and canonical bytes to hash for signing.
/// The expected sender is reviewed separately; these bytes contain no signature.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct UnsignedTransaction {
    chain_id: ChainId,
    nonce: u64,
    gas_limit: u64,
    fees: FeeTerms,
    to: Address,
    value: Quantity,
    input: Data,
    signing_payload: Data,
}
impl UnsignedTransaction {
    fn from_request(request: &TransactionRequest) -> Result<Self, Error> {
        let data = request.data();
        let (to, value, input) = data.intent.fields()?;
        let nonce = encoding::integer(U256::from(data.nonce));
        let gas = encoding::integer(U256::from(data.gas_limit));
        let recipient = encoding::bytes(&to.bytes());
        let amount = encoding::integer(value.value());
        let calldata = encoding::bytes(input.bytes());
        let chain = encoding::integer(data.chain_id.value());
        let mut payload = match &data.fees {
            FeeTerms::Legacy { gas_price } => encoding::list(&[
                nonce,
                encoding::integer(gas_price.value()),
                gas,
                recipient,
                amount,
                calldata,
                chain,
                encoding::integer(U256::ZERO),
                encoding::integer(U256::ZERO),
            ]),
            FeeTerms::AccessList {
                gas_price,
                access_list,
            } => encoding::list(&[
                chain,
                nonce,
                encoding::integer(gas_price.value()),
                gas,
                recipient,
                amount,
                calldata,
                encoding::access_list(access_list),
            ]),
            FeeTerms::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => encoding::list(&[
                chain,
                nonce,
                encoding::integer(max_priority_fee_per_gas.value()),
                encoding::integer(max_fee_per_gas.value()),
                gas,
                recipient,
                amount,
                calldata,
                encoding::access_list(access_list),
            ]),
        };
        if data.fees.type_byte() != 0 {
            payload.insert(0, data.fees.type_byte());
        }
        Ok(Self {
            chain_id: data.chain_id,
            nonce: data.nonce,
            gas_limit: data.gas_limit,
            fees: data.fees.clone(),
            to,
            value,
            input,
            signing_payload: Data::new(payload)?,
        })
    }
    /// Returns the exact expected chain identity embedded in the signing payload.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }
    /// Returns the exact chosen nonce.
    #[must_use]
    pub const fn nonce(&self) -> u64 {
        self.nonce
    }
    /// Returns the exact chosen gas limit.
    #[must_use]
    pub const fn gas_limit(&self) -> u64 {
        self.gas_limit
    }
    /// Returns the complete chosen fee terms.
    #[must_use]
    pub const fn fees(&self) -> &FeeTerms {
        &self.fees
    }
    /// Returns the actual transaction recipient, including a token contract.
    #[must_use]
    pub const fn to(&self) -> Address {
        self.to
    }
    /// Returns actual native wei; token operations send explicit zero native value.
    #[must_use]
    pub const fn value(&self) -> Quantity {
        self.value
    }
    /// Returns exact calldata derived from the intent.
    #[must_use]
    pub const fn input(&self) -> &Data {
        &self.input
    }
    /// Returns canonical legacy EIP-155 or type1/type2 bytes to hash for signing.
    #[must_use]
    pub const fn signing_payload(&self) -> &Data {
        &self.signing_payload
    }
    /// Computes the maintained Keccak-256 signing digest, not a signed transaction ID.
    #[must_use]
    pub fn signing_hash(&self) -> Word {
        encoding::hash(self.signing_payload.bytes())
    }
}
impl fmt::Debug for UnsignedTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnsignedTransaction")
            .finish_non_exhaustive()
    }
}

/// Immutable EVM preparation for generic review and external signing handoff.
/// A caller-supplied verifier must check the actual returned signed content,
/// including the recovered sender, against this request before declaring binding.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparedTransaction {
    request: TransactionRequest,
    unsigned: UnsignedTransaction,
}
impl PreparedTransaction {
    /// Derives immutable typed unsigned fields from a validated request without side effects.
    /// # Errors
    /// Rejects local payload resource excess.
    pub fn new(request: TransactionRequest) -> Result<Self, Error> {
        let unsigned = UnsignedTransaction::from_request(&request)?;
        Ok(Self { request, unsigned })
    }
    /// Returns all exact caller signing choices and action intent.
    #[must_use]
    pub const fn request(&self) -> &TransactionRequest {
        &self.request
    }
    /// Returns exact unsigned fields and canonical signing payload.
    #[must_use]
    pub const fn unsigned(&self) -> &UnsignedTransaction {
        &self.unsigned
    }
}
impl Preparation for PreparedTransaction {
    type Network = ChainId;
    type Intent = TransactionRequest;
    type UnsignedPayload = UnsignedTransaction;
    fn network(&self) -> &ChainId {
        &self.request.0.chain_id
    }
    fn intent(&self) -> &TransactionRequest {
        &self.request
    }
    fn unsigned_payload(&self) -> &UnsignedTransaction {
        &self.unsigned
    }
    fn validate(&self) -> Result<(), Error> {
        Ok(())
    }
}
impl fmt::Debug for PreparedTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedTransaction")
            .finish_non_exhaustive()
    }
}
impl Serialize for PreparedTransaction {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.request.serialize(s)
    }
}
impl<'de> Deserialize<'de> for PreparedTransaction {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(TransactionRequest::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
