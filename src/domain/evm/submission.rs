// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Serialize};
use std::fmt;

use super::{
    Address, ChainId, Data, FeeTerms, OperationContext, OperationValue, Quantity, ReadOperation,
    ReadState, TransactionId, U256, encoding,
};
use crate::error::{Error, ValidationError};

/// Structurally checked signed legacy/type1/type2 source envelope fields.
/// Signature range/parity checks do not verify the signature or recover a sender.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedTransactionFields {
    /// Actual replay-protected chain identity decoded from the envelope.
    pub chain_id: ChainId,
    /// Exact transaction nonce, below the protocol's unusable maximum.
    pub nonce: u64,
    /// Exact nonzero gas limit.
    pub gas_limit: u64,
    /// Exact supported fee terms and ordered access list.
    pub fees: FeeTerms,
    /// Actual recipient, or explicit empty destination for contract creation.
    pub to: Option<Address>,
    /// Exact native wei.
    pub value: Quantity,
    /// Exact opaque calldata or creation code.
    pub input: Data,
    /// Signature parity decoded from typed parity or protected legacy v.
    pub y_parity: bool,
    /// Structurally range-checked r; no signature correctness claim.
    pub r: Quantity,
    /// Structurally range-checked low-s; no signature correctness claim.
    pub s: Quantity,
}

/// Explicit caller-supplied signed envelope and its maintained Keccak-256 identity.
///
/// Supported forms are EIP-155 protected legacy and EIP-2930/type1 or EIP-1559/type2.
/// Unprotected legacy and type3/type4 or future forms are unsupported. Blob network
/// wrappers are never treated as ordinary transaction-hash input. The validated
/// RLP shape, chain ID and component ranges do not prove sender recovery, a valid
/// signature, reviewed intent, funding or execution. A wallet semantic verifier
/// must separately establish those binding facts before confirming a handoff.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SubmissionFields", into = "SubmissionFields")]
pub struct SignedSubmission {
    expected_chain_id: ChainId,
    payload: Data,
    transaction_id: TransactionId,
    fields: SignedTransactionFields,
}
impl SignedSubmission {
    /// Checks the supported canonical envelope and exact expected replay-protected chain.
    /// # Errors
    /// Rejects malformed, unbounded, noncanonical or mismatched envelopes and unsupported forms.
    pub fn new(expected_chain_id: ChainId, payload: Data) -> Result<Self, Error> {
        let fields = decode(payload.bytes())?;
        if fields.chain_id != expected_chain_id {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let transaction_id = TransactionId::from_bytes(encoding::hash(payload.bytes()).bytes());
        Ok(Self {
            expected_chain_id,
            payload,
            transaction_id,
            fields,
        })
    }
    /// Returns the exact caller expected and envelope-matched chain identity.
    #[must_use]
    pub const fn expected_chain_id(&self) -> ChainId {
        self.expected_chain_id
    }
    /// Returns the exact bytes that a separate explicit submit operation may dispatch.
    #[must_use]
    pub const fn payload(&self) -> &Data {
        &self.payload
    }
    /// Returns Keccak-256 of the complete supported signed envelope bytes.
    #[must_use]
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }
    /// Returns immutable structurally decoded fields without signature or sender proof.
    #[must_use]
    pub const fn fields(&self) -> &SignedTransactionFields {
        &self.fields
    }
}
impl fmt::Debug for SignedSubmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedSubmission").finish_non_exhaustive()
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmissionFields {
    expected_chain_id: ChainId,
    payload: Data,
}
impl TryFrom<SubmissionFields> for SignedSubmission {
    type Error = Error;
    fn try_from(value: SubmissionFields) -> Result<Self, Error> {
        Self::new(value.expected_chain_id, value.payload)
    }
}
impl From<SignedSubmission> for SubmissionFields {
    fn from(value: SignedSubmission) -> Self {
        Self {
            expected_chain_id: value.expected_chain_id,
            payload: value.payload,
        }
    }
}

// secp256k1's published group order. This constant is only used for envelope
// scalar-range/low-s structural checks (EIP-2), never custom signature arithmetic.
const ORDER: U256 = U256::from_limbs([
    0xbfd2_5e8c_d036_4141,
    0xbaae_dce6_af48_a03b,
    0xffff_ffff_ffff_fffe,
    0xffff_ffff_ffff_ffff,
]);

fn decode(raw: &[u8]) -> Result<SignedTransactionFields, Error> {
    let first = *raw.first().ok_or_else(encoding::invalid)?;
    let (kind, mut source) = if first >= 0xc0 {
        (0, raw)
    } else if matches!(first, 1 | 2) {
        (first, &raw[1..])
    } else {
        return Err(Error::UnsupportedCapability);
    };
    let mut fields = encoding::item(&mut source, true)?;
    if !source.is_empty() {
        return Err(encoding::invalid());
    }
    let chain_id = if kind == 0 {
        None
    } else {
        Some(ChainId::new(encoding::uint(&mut fields)?.value()))
    };
    let nonce = encoding::uint64(&mut fields)?;
    let first_fee = encoding::uint(&mut fields)?;
    let second_fee = if kind == 2 {
        Some(encoding::uint(&mut fields)?)
    } else {
        None
    };
    let gas_limit = encoding::uint64(&mut fields)?;
    let to = encoding::address(&mut fields)?;
    let value = encoding::uint(&mut fields)?;
    let input = encoding::data(&mut fields)?;
    let access_list = if kind == 0 {
        Vec::new()
    } else {
        encoding::decode_access_list(&mut fields)?
    };
    let v = encoding::uint(&mut fields)?.value();
    let r = encoding::uint(&mut fields)?;
    let s = encoding::uint(&mut fields)?;
    if !fields.is_empty()
        || nonce == u64::MAX
        || gas_limit == 0
        || r.value() == U256::ZERO
        || r.value() >= ORDER
        || s.value() == U256::ZERO
        || s.value() > ORDER / U256::from(2)
    {
        return Err(encoding::invalid());
    }
    let (chain_id, y_parity) = if let Some(chain_id) = chain_id {
        if v > U256::from(1) {
            return Err(encoding::invalid());
        }
        (chain_id, v == U256::from(1))
    } else {
        if matches!(v, value if value == U256::from(27) || value == U256::from(28)) {
            return Err(Error::UnsupportedCapability);
        }
        let protected = v
            .checked_sub(U256::from(35))
            .ok_or_else(encoding::invalid)?;
        (
            ChainId::new(protected / U256::from(2)),
            protected % U256::from(2) == U256::from(1),
        )
    };
    let fees = match kind {
        0 => FeeTerms::Legacy {
            gas_price: first_fee,
        },
        1 => FeeTerms::AccessList {
            gas_price: first_fee,
            access_list,
        },
        2 => FeeTerms::DynamicFee {
            max_fee_per_gas: second_fee.ok_or_else(encoding::invalid)?,
            max_priority_fee_per_gas: first_fee,
            access_list,
        },
        _ => return Err(Error::UnsupportedCapability),
    };
    fees.validate().map_err(|_| encoding::invalid())?;
    Ok(SignedTransactionFields {
        chain_id,
        nonce,
        gas_limit,
        fees,
        to,
        value,
        input,
        y_parity,
        r,
        s,
    })
}

/// A matching transaction hash acknowledged by the responding node after one dispatch.
/// This is not a mempool, inclusion, execution, signature or lasting-finality proof.
/// Callers may separately query the existing transaction/status reader with this ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmissionAcknowledgment {
    /// Exact replay-protected chain matched locally and verified at preflight.
    pub chain_id: ChainId,
    /// Maintained-computed transaction ID exactly matched to the node's response.
    pub transaction_id: TransactionId,
}
impl OperationValue for SubmissionAcknowledgment {
    fn validate_context(&self, context: &OperationContext) -> Result<(), Error> {
        if context.operation() != ReadOperation::SignedSubmission
            || context.network().chain_id() != self.chain_id
            || context.state() != ReadState::Unanchored
        {
            return Err(ValidationError::ObservationOperationMismatch.into());
        }
        Ok(())
    }
}
