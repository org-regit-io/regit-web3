// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    Address, BlockHash, ChainId, Data, OperationContext, OperationValue, Quantity, ReadOperation,
    ReadState, TransactionId, U256, Word,
};
use crate::error::{Error, ValidationError};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Source-reported inclusion, independent of execution and finality.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inclusion {
    block_hash: BlockHash,
    block_number: u64,
    transaction_index: u64,
}
impl Inclusion {
    /// Records a block identity and transaction position without proving inclusion.
    #[must_use]
    pub const fn new(block_hash: BlockHash, block_number: u64, transaction_index: u64) -> Self {
        Self {
            block_hash,
            block_number,
            transaction_index,
        }
    }
    /// Returns the source-reported block hash.
    #[must_use]
    pub const fn block_hash(self) -> BlockHash {
        self.block_hash
    }
    /// Returns the source-reported block height.
    #[must_use]
    pub const fn block_number(self) -> u64 {
        self.block_number
    }
    /// Returns the source-reported transaction position in its block.
    #[must_use]
    pub const fn transaction_index(self) -> u64 {
        self.transaction_index
    }
}
/// One ordered access-list entry. Duplicate addresses and keys remain legal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessListEntry {
    /// The account being accessed.
    pub address: Address,
    /// Ordered storage keys; duplicates are preserved.
    pub storage_keys: Vec<Word>,
}
/// Signature fields reported by the source, without signature verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureFields {
    /// Exact signature component r.
    pub r: Quantity,
    /// Exact signature component s.
    pub s: Quantity,
    /// Legacy replay/parity value or an optional typed-transaction parity alias.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub v: Option<Quantity>,
    /// Explicit typed-transaction parity, when reported.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub y_parity: Option<bool>,
}
/// Source-reported EIP-7702 authorization, not independently verified.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authorization {
    /// Authorized chain ID; zero has the protocol's chain-independent meaning.
    pub chain_id: ChainId,
    /// Delegation target.
    pub address: Address,
    /// Authority nonce, retained exactly.
    pub nonce: Quantity,
    /// Signature parity.
    pub y_parity: bool,
    /// Signature component r.
    pub r: Quantity,
    /// Signature component s.
    pub s: Quantity,
}
/// Supported source transaction forms; fee values are exact wei per gas.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransactionKind {
    /// A legacy transaction, possibly without replay protection.
    Legacy {
        /// Exact gas price in wei per gas.
        gas_price: Quantity,
    },
    /// An EIP-2930 transaction retaining the ordered access list.
    AccessList {
        /// Exact gas price in wei per gas.
        gas_price: Quantity,
        /// Ordered access list with legal duplicates preserved.
        access_list: Vec<AccessListEntry>,
    },
    /// An EIP-1559 transaction.
    DynamicFee {
        /// Maximum total wei per gas.
        max_fee_per_gas: Quantity,
        /// Maximum priority wei per gas.
        max_priority_fee_per_gas: Quantity,
        /// Ordered access list with legal duplicates preserved.
        access_list: Vec<AccessListEntry>,
    },
    /// EIP-4844 source fields; no blob sidecar or availability proof is implied.
    Blob {
        /// Maximum total wei per execution gas.
        max_fee_per_gas: Quantity,
        /// Maximum priority wei per execution gas.
        max_priority_fee_per_gas: Quantity,
        /// Maximum wei per blob gas.
        max_fee_per_blob_gas: Quantity,
        /// Ordered access list with legal duplicates preserved.
        access_list: Vec<AccessListEntry>,
        /// Versioned hashes reported by the source.
        blob_versioned_hashes: Vec<Word>,
    },
    /// EIP-7702 source fields; authorizations are not independently verified.
    Authorization {
        /// Maximum total wei per gas.
        max_fee_per_gas: Quantity,
        /// Maximum priority wei per gas.
        max_priority_fee_per_gas: Quantity,
        /// Ordered access list with legal duplicates preserved.
        access_list: Vec<AccessListEntry>,
        /// Ordered signed authorizations.
        authorization_list: Vec<Authorization>,
    },
}
impl TransactionKind {
    /// Returns the protocol transaction type byte.
    #[must_use]
    pub const fn type_byte(&self) -> u8 {
        match self {
            Self::Legacy { .. } => 0,
            Self::AccessList { .. } => 1,
            Self::DynamicFee { .. } => 2,
            Self::Blob { .. } => 3,
            Self::Authorization { .. } => 4,
        }
    }
    fn validate(&self) -> Result<(), Error> {
        let (list, fees) = match self {
            Self::Legacy { .. } => (None, None),
            Self::AccessList { access_list, .. } => (Some(access_list), None),
            Self::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            }
            | Self::Blob {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
                ..
            }
            | Self::Authorization {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
                ..
            } => (
                Some(access_list),
                Some((*max_fee_per_gas, *max_priority_fee_per_gas)),
            ),
        };
        if fees.is_some_and(|(max, tip)| max < tip) {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        if let Some(list) = list
            && (list.len() > 1024
                || list
                    .iter()
                    .try_fold(0_usize, |n, e| n.checked_add(e.storage_keys.len()))
                    .is_none_or(|n| n > 4096))
        {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        match self {
            Self::Blob {
                blob_versioned_hashes,
                ..
            } => {
                if blob_versioned_hashes.is_empty()
                    || blob_versioned_hashes.len() > 4096
                    || blob_versioned_hashes.iter().any(|h| h.bytes()[0] != 1)
                {
                    return Err(ValidationError::InvalidEvmRecord.into());
                }
            }
            Self::Authorization {
                authorization_list, ..
            } => {
                if authorization_list.is_empty()
                    || authorization_list.len() > 1024
                    || authorization_list
                        .iter()
                        .any(|a| a.nonce.value() >= U256::from(u64::MAX))
                {
                    return Err(ValidationError::InvalidEvmRecord.into());
                }
            }
            Self::Legacy { .. } | Self::AccessList { .. } | Self::DynamicFee { .. } => {}
        }
        Ok(())
    }
}
/// Unvalidated source transaction fields supplied to [`Transaction::new`].
///
/// No mutation of this input changes a constructed immutable transaction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionData {
    /// Transaction identity reported by the source; not computed from JSON.
    pub hash: TransactionId,
    /// Sender reported by the source; signature recovery is not performed.
    pub from: Address,
    /// Destination, or null for contract creation.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub to: Option<Address>,
    /// Exact account nonce.
    pub nonce: Quantity,
    /// Exact execution gas limit.
    pub gas_limit: Quantity,
    /// Exact native value in wei.
    pub value: Quantity,
    /// Exact opaque transaction input.
    pub input: Data,
    /// Supported transaction-specific fee and auxiliary fields.
    pub kind: TransactionKind,
    /// Actual source chain ID when reported; absence remains explicit.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub reported_chain_id: Option<ChainId>,
    /// Source gas-price alias, optional for dynamic fee transactions.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub reported_gas_price: Option<Quantity>,
    /// Source signature fields; no verification is implied.
    pub signature: SignatureFields,
    /// Actual source inclusion, or none for pending transactions.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub inclusion: Option<Inclusion>,
}
/// Immutable structurally validated source transaction fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionFields")]
pub struct Transaction {
    chain_id: ChainId,
    data: TransactionData,
}
impl Transaction {
    /// Validates source fields against the explicit expected network.
    ///
    /// # Errors
    /// Rejects contradictory network, parity, fee, bounded-list or nonce facts.
    /// This does not verify signatures, sender, transaction hash or canonical bytes.
    pub fn new(chain_id: ChainId, data: TransactionData) -> Result<Self, Error> {
        data.kind.validate()?;
        if data.reported_chain_id.is_some_and(|id| id != chain_id)
            || data.nonce.value() >= U256::from(u64::MAX)
            || data.gas_limit.value() == U256::ZERO
        {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        let sig = data.signature;
        if data.kind.type_byte() == 0 {
            let v = sig.v.ok_or(ValidationError::InvalidEvmRecord)?.value();
            if v != U256::from(27)
                && v != U256::from(28)
                && (v < U256::from(35)
                    || ChainId::new((v - U256::from(35)) / U256::from(2)) != chain_id)
            {
                return Err(ValidationError::InvalidEvmRecord.into());
            }
            if sig.y_parity.is_some() {
                return Err(ValidationError::InvalidEvmRecord.into());
            }
        } else {
            if data.reported_chain_id != Some(chain_id) || sig.v.is_none() && sig.y_parity.is_none()
            {
                return Err(ValidationError::InvalidEvmRecord.into());
            }
            if let Some(v) = sig.v
                && (v.value() > U256::from(1)
                    || sig
                        .y_parity
                        .is_some_and(|p| p != (v.value() == U256::from(1))))
            {
                return Err(ValidationError::InvalidEvmRecord.into());
            }
        }
        if matches!(
            data.kind,
            TransactionKind::Blob { .. } | TransactionKind::Authorization { .. }
        ) && data.to.is_none()
        {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        Ok(Self { chain_id, data })
    }
    /// Returns the explicit expected chain identity.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }
    /// Returns the actual source transaction identity.
    #[must_use]
    pub const fn hash(&self) -> TransactionId {
        self.data.hash
    }
    /// Returns immutable source fields with their declared validation limits.
    #[must_use]
    pub const fn data(&self) -> &TransactionData {
        &self.data
    }
    /// Returns actual inclusion independently of execution/finality.
    #[must_use]
    pub const fn inclusion(&self) -> Option<Inclusion> {
        self.data.inclusion
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionFields {
    chain_id: ChainId,
    data: TransactionData,
}
impl TryFrom<TransactionFields> for Transaction {
    type Error = Error;
    fn try_from(v: TransactionFields) -> Result<Self, Error> {
        Self::new(v.chain_id, v.data)
    }
}

/// A bounded source receipt log with matched inclusion and transaction identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LogFields")]
pub struct Log {
    transaction_hash: TransactionId,
    inclusion: Inclusion,
    #[serde(rename = "log_index")]
    index: u64,
    address: Address,
    data: Data,
    topics: Vec<Word>,
}
impl Log {
    /// Validates a source log without interpreting event semantics.
    ///
    /// # Errors
    /// Rejects more than the protocol's four topics.
    pub fn new(
        transaction_hash: TransactionId,
        inclusion: Inclusion,
        log_index: u64,
        address: Address,
        data: Data,
        topics: Vec<Word>,
    ) -> Result<Self, Error> {
        if topics.len() > 4 {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        Ok(Self {
            transaction_hash,
            inclusion,
            index: log_index,
            address,
            data,
            topics,
        })
    }
    /// Returns the exact transaction identifier.
    #[must_use]
    pub const fn transaction_hash(&self) -> TransactionId {
        self.transaction_hash
    }
    /// Returns source inclusion.
    #[must_use]
    pub const fn inclusion(&self) -> Inclusion {
        self.inclusion
    }
    /// Returns source log position.
    #[must_use]
    pub const fn log_index(&self) -> u64 {
        self.index
    }
    /// Returns emitting contract identity.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }
    /// Returns exact opaque event data.
    #[must_use]
    pub const fn data(&self) -> &Data {
        &self.data
    }
    /// Returns ordered topics without decoding them.
    #[must_use]
    pub fn topics(&self) -> &[Word] {
        &self.topics
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogFields {
    transaction_hash: TransactionId,
    inclusion: Inclusion,
    log_index: u64,
    address: Address,
    data: Data,
    topics: Vec<Word>,
}
impl TryFrom<LogFields> for Log {
    type Error = Error;
    fn try_from(v: LogFields) -> Result<Self, Error> {
        Self::new(
            v.transaction_hash,
            v.inclusion,
            v.log_index,
            v.address,
            v.data,
            v.topics,
        )
    }
}
/// Source receipt top-level execution, independent of inclusion and finality.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "state_root",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ExecutionOutcome {
    /// The source receipt reports status one; inner calls may still have failed.
    Succeeded,
    /// The source receipt reports status zero; reason is not inferred.
    Failed,
    /// A pre-Byzantium state root supplies no top-level success/failure status.
    PreByzantium(Word),
    /// The source did not supply either a status or historical root.
    Unknown,
}
/// Unvalidated source receipt fields supplied to [`Receipt::new`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptData {
    /// Exact transaction identity.
    pub transaction_hash: TransactionId,
    /// Actual source inclusion.
    pub inclusion: Inclusion,
    /// Source sender address.
    pub from: Address,
    /// Source destination, or none for contract creation.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub to: Option<Address>,
    /// Created contract, if actually reported.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub contract_address: Option<Address>,
    /// Source transaction type; none when not reported by an older source.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub transaction_type: Option<u8>,
    /// Top-level execution or explicit uncertainty.
    pub execution: ExecutionOutcome,
    /// Execution gas used by this transaction.
    pub gas_used: Quantity,
    /// Execution gas used by this and preceding transactions in the block.
    pub cumulative_gas_used: Quantity,
    /// Actual wei per execution gas, or none when unavailable.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub effective_gas_price: Option<Quantity>,
    /// Actual blob gas used, separately reported.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub blob_gas_used: Option<Quantity>,
    /// Actual wei per blob gas, separately reported.
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    pub blob_gas_price: Option<Quantity>,
    /// Exact 256-byte bloom supplied by the source; not reconstructed.
    pub logs_bloom: Data,
    /// Ordered typed source logs; no event or internal-call interpretation.
    pub logs: Vec<Log>,
}
/// An immutable structurally validated source receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ReceiptFields")]
pub struct Receipt {
    chain_id: ChainId,
    data: ReceiptData,
}
impl Receipt {
    /// Validates bounded gas/log/inclusion facts without proving execution.
    ///
    /// # Errors
    /// Rejects inconsistent gas, bloom width, contract identity, log identity,
    /// duplicate log positions or a collection exceeding 4096 logs.
    pub fn new(chain_id: ChainId, data: ReceiptData) -> Result<Self, Error> {
        if data.gas_used > data.cumulative_gas_used
            || data.logs_bloom.bytes().len() != 256
            || data.logs.len() > 4096
            || data.to.is_some() && data.contract_address.is_some()
            || data.blob_gas_used.is_some() != data.blob_gas_price.is_some()
        {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        let mut indices = HashSet::with_capacity(data.logs.len());
        for log in &data.logs {
            if log.transaction_hash() != data.transaction_hash
                || log.inclusion() != data.inclusion
                || !indices.insert(log.log_index())
            {
                return Err(ValidationError::InvalidEvmRecord.into());
            }
        }
        Ok(Self { chain_id, data })
    }
    /// Returns expected chain identity.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }
    /// Returns exact transaction identity.
    #[must_use]
    pub const fn transaction_hash(&self) -> TransactionId {
        self.data.transaction_hash
    }
    /// Returns actual inclusion independently of finality.
    #[must_use]
    pub const fn inclusion(&self) -> Inclusion {
        self.data.inclusion
    }
    /// Returns top-level execution with historical uncertainty retained.
    #[must_use]
    pub const fn execution(&self) -> ExecutionOutcome {
        self.data.execution
    }
    /// Returns immutable source receipt fields.
    #[must_use]
    pub const fn data(&self) -> &ReceiptData {
        &self.data
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptFields {
    chain_id: ChainId,
    data: ReceiptData,
}
impl TryFrom<ReceiptFields> for Receipt {
    type Error = Error;
    fn try_from(v: ReceiptFields) -> Result<Self, Error> {
        Self::new(v.chain_id, v.data)
    }
}

/// Transaction lookup retaining a null result separately from an actual record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionLookupFields")]
pub struct TransactionLookup {
    chain_id: ChainId,
    query: TransactionId,
    transaction: Option<Transaction>,
}
impl TransactionLookup {
    /// Validates query/record identity; null means not observed, never failure.
    ///
    /// # Errors
    /// Rejects a record with a different identifier or chain.
    pub fn new(
        chain_id: ChainId,
        query: TransactionId,
        transaction: Option<Transaction>,
    ) -> Result<Self, Error> {
        if transaction
            .as_ref()
            .is_some_and(|t| t.hash() != query || t.chain_id() != chain_id)
        {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        Ok(Self {
            chain_id,
            query,
            transaction,
        })
    }
    /// Returns the exact queried identifier.
    #[must_use]
    pub const fn query(&self) -> TransactionId {
        self.query
    }
    /// Returns the source record, or no observed transaction.
    #[must_use]
    pub const fn transaction(&self) -> Option<&Transaction> {
        self.transaction.as_ref()
    }
    /// Returns the appropriate actual attribution state.
    #[must_use]
    pub fn state(&self) -> ReadState {
        self.transaction
            .as_ref()
            .map_or(ReadState::Unanchored, |t| {
                t.inclusion()
                    .map_or(ReadState::Pending, |inclusion| ReadState::Included {
                        inclusion,
                    })
            })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionLookupFields {
    chain_id: ChainId,
    query: TransactionId,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    transaction: Option<Transaction>,
}
impl TryFrom<TransactionLookupFields> for TransactionLookup {
    type Error = Error;
    fn try_from(v: TransactionLookupFields) -> Result<Self, Error> {
        Self::new(v.chain_id, v.query, v.transaction)
    }
}
/// Receipt lookup retaining null without classifying it as pending or failed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ReceiptLookupFields")]
pub struct ReceiptLookup {
    chain_id: ChainId,
    query: TransactionId,
    receipt: Option<Receipt>,
}
impl ReceiptLookup {
    /// Validates query/record identity without treating absence as success.
    ///
    /// # Errors
    /// Rejects a receipt for a different identifier or chain.
    pub fn new(
        chain_id: ChainId,
        query: TransactionId,
        receipt: Option<Receipt>,
    ) -> Result<Self, Error> {
        if receipt
            .as_ref()
            .is_some_and(|r| r.transaction_hash() != query || r.chain_id() != chain_id)
        {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        Ok(Self {
            chain_id,
            query,
            receipt,
        })
    }
    /// Returns the exact queried identifier.
    #[must_use]
    pub const fn query(&self) -> TransactionId {
        self.query
    }
    /// Returns the receipt, or no observed receipt.
    #[must_use]
    pub const fn receipt(&self) -> Option<&Receipt> {
        self.receipt.as_ref()
    }
    /// Returns actual inclusion or absent attribution.
    #[must_use]
    pub fn state(&self) -> ReadState {
        self.receipt
            .as_ref()
            .map_or(ReadState::Unanchored, |r| ReadState::Included {
                inclusion: r.inclusion(),
            })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptLookupFields {
    chain_id: ChainId,
    query: TransactionId,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    receipt: Option<Receipt>,
}
impl TryFrom<ReceiptLookupFields> for ReceiptLookup {
    type Error = Error;
    fn try_from(v: ReceiptLookupFields) -> Result<Self, Error> {
        Self::new(v.chain_id, v.query, v.receipt)
    }
}
/// Observed transaction lifecycle, retaining missing execution uncertainty.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransactionState {
    /// Neither lookup returned an observed transaction or receipt.
    NotObserved,
    /// An actual source transaction is pending and no receipt was observed.
    Pending,
    /// Source inclusion exists; execution remains independently optional.
    Included {
        /// Actual source inclusion.
        inclusion: Inclusion,
        /// Receipt execution if observed; missing receipt never implies success.
        #[serde(deserialize_with = "crate::domain::deserialize_optional")]
        execution: Option<ExecutionOutcome>,
    },
}
/// Joint source transaction and receipt lookup with matched returned identities.
///
/// The reads are sequential observations, not an atomic snapshot. A pending
/// transaction can gain a receipt during lookup; conflicting inclusion fails.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StatusFields")]
pub struct TransactionStatus {
    transaction: TransactionLookup,
    receipt: ReceiptLookup,
    state: TransactionState,
}
impl TransactionStatus {
    /// Matches identities, inclusion, addresses, type and observed gas facts.
    ///
    /// # Errors
    /// Rejects contradictory source transaction/receipt records.
    pub fn new(transaction: TransactionLookup, receipt: ReceiptLookup) -> Result<Self, Error> {
        if transaction.chain_id != receipt.chain_id || transaction.query != receipt.query {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        if let (Some(t), Some(r)) = (transaction.transaction(), receipt.receipt())
            && (t.inclusion().is_some_and(|i| i != r.inclusion())
                || t.data.from != r.data.from
                || t.data.to != r.data.to
                || r.data
                    .transaction_type
                    .is_some_and(|kind| kind != t.data.kind.type_byte())
                || r.data.gas_used > t.data.gas_limit)
        {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        let state = if let Some(r) = receipt.receipt() {
            TransactionState::Included {
                inclusion: r.inclusion(),
                execution: Some(r.execution()),
            }
        } else if let Some(t) = transaction.transaction() {
            t.inclusion()
                .map_or(TransactionState::Pending, |inclusion| {
                    TransactionState::Included {
                        inclusion,
                        execution: None,
                    }
                })
        } else {
            TransactionState::NotObserved
        };
        Ok(Self {
            transaction,
            receipt,
            state,
        })
    }
    /// Returns exact query identity.
    #[must_use]
    pub const fn query(&self) -> TransactionId {
        self.transaction.query
    }
    /// Returns the independent transaction lookup.
    #[must_use]
    pub const fn transaction(&self) -> &TransactionLookup {
        &self.transaction
    }
    /// Returns the independent receipt lookup.
    #[must_use]
    pub const fn receipt(&self) -> &ReceiptLookup {
        &self.receipt
    }
    /// Returns actual observed lifecycle without finality inference.
    #[must_use]
    pub const fn state(&self) -> TransactionState {
        self.state
    }
    /// Returns appropriate actual context attribution.
    #[must_use]
    pub const fn read_state(&self) -> ReadState {
        match self.state {
            TransactionState::NotObserved => ReadState::Unanchored,
            TransactionState::Pending => ReadState::Pending,
            TransactionState::Included { inclusion, .. } => ReadState::Included { inclusion },
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusFields {
    transaction: TransactionLookup,
    receipt: ReceiptLookup,
    state: TransactionState,
}
impl TryFrom<StatusFields> for TransactionStatus {
    type Error = Error;
    fn try_from(v: StatusFields) -> Result<Self, Error> {
        let result = Self::new(v.transaction, v.receipt)?;
        if result.state != v.state {
            return Err(ValidationError::InvalidEvmRecord.into());
        }
        Ok(result)
    }
}
fn validate_lookup(
    chain: ChainId,
    operation: ReadOperation,
    state: ReadState,
    c: &OperationContext,
) -> Result<(), Error> {
    if c.operation() != operation {
        return Err(ValidationError::ObservationOperationMismatch.into());
    }
    if c.network().chain_id() != chain {
        return Err(ValidationError::NetworkMismatch.into());
    }
    if c.state() != state {
        return Err(ValidationError::InvalidEvmRecord.into());
    }
    Ok(())
}
impl OperationValue for TransactionLookup {
    fn validate_context(&self, c: &OperationContext) -> Result<(), Error> {
        validate_lookup(self.chain_id, ReadOperation::Transaction, self.state(), c)
    }
}
impl OperationValue for ReceiptLookup {
    fn validate_context(&self, c: &OperationContext) -> Result<(), Error> {
        validate_lookup(self.chain_id, ReadOperation::Receipt, self.state(), c)
    }
}
impl OperationValue for TransactionStatus {
    fn validate_context(&self, c: &OperationContext) -> Result<(), Error> {
        validate_lookup(
            self.transaction.chain_id,
            ReadOperation::TransactionStatus,
            self.read_state(),
            c,
        )
    }
}
