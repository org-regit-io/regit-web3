// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    AddressPolicy, AtomicAmount, BlockHash, Bytes, MAXIMUM_ITEMS, MempoolDelta, SourceText, Txid,
    bounded_vec, invalid,
};
use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Exact indexed balance and lifetime-flow facts in the family's native atomic units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
pub struct BalanceData<A: AddressPolicy> {
    /// Confirmed lifetime funding, which may exceed a transaction money limit.
    pub total_received: AtomicAmount<A>,
    /// Confirmed lifetime spending.
    pub total_sent: AtomicAmount<A>,
    /// Confirmed funding minus spending.
    pub confirmed: AtomicAmount<A>,
    /// Signed source-reported unconfirmed delta, never an unsigned balance.
    pub unconfirmed: MempoolDelta<A>,
    /// Source-reported confirmed balance plus unconfirmed delta.
    pub final_balance: AtomicAmount<A>,
    /// Source-reported number of confirmed transactions.
    pub confirmed_transactions: u64,
    /// Source-reported number of unconfirmed transactions.
    pub unconfirmed_transactions: u64,
    /// Source-reported total transaction count.
    pub final_transactions: u64,
}
impl<A: AddressPolicy> BalanceData<A> {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.total_received.raw().checked_sub(self.total_sent.raw())
            != Some(self.confirmed.raw())
            || i128::from(self.confirmed.raw()).checked_add(self.unconfirmed.raw())
                != Some(i128::from(self.final_balance.raw()))
            || self
                .confirmed_transactions
                .checked_add(self.unconfirmed_transactions)
                != Some(self.final_transactions)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

/// An immutable exact balance associated with its requested qualified address.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BalanceFields<A>", bound(deserialize = "A: AddressPolicy"))]
pub struct AddressBalance<A: AddressPolicy> {
    address: A,
    data: BalanceData<A>,
}
impl<A: AddressPolicy> AddressBalance<A> {
    /// Correlates an address and checked exact source balance facts.
    /// # Errors
    /// Rejects inconsistent arithmetic or transaction counts.
    pub fn new(address: A, data: BalanceData<A>) -> Result<Self, Error> {
        data.validate()?;
        Ok(Self { address, data })
    }
    /// Returns the explicitly qualified queried address.
    #[must_use]
    pub const fn address(&self) -> &A {
        &self.address
    }
    /// Returns the exact indexed facts.
    #[must_use]
    pub const fn data(&self) -> &BalanceData<A> {
        &self.data
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
struct BalanceFields<A: AddressPolicy> {
    address: A,
    data: BalanceData<A>,
}
impl<A: AddressPolicy> TryFrom<BalanceFields<A>> for AddressBalance<A> {
    type Error = Error;
    fn try_from(v: BalanceFields<A>) -> Result<Self, Error> {
        Self::new(v.address, v.data)
    }
}

/// Exact reported fee buckets per 1000 serialized bytes, with separate source-tip facts.
/// `BlockCypher` describes high/medium/low as 1–2, 3–6 and 7+ block preferences;
/// these labels do not guarantee a confirmation time or provide a fee per vbyte.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
pub struct FeeEstimates<A: AddressPolicy> {
    /// High preference in native atomic units per 1000 bytes.
    pub high_per_kilobyte: AtomicAmount<A>,
    /// Medium preference in native atomic units per 1000 bytes.
    pub medium_per_kilobyte: AtomicAmount<A>,
    /// Low preference in native atomic units per 1000 bytes.
    pub low_per_kilobyte: AtomicAmount<A>,
    /// Actual source-reported current tip height at the chain-resource read.
    pub tip_height: u64,
    /// Actual source-reported current tip hash; this is not a balance-state pin.
    pub tip_hash: BlockHash,
    /// Optional exact source update time text, without an inferred local clock.
    pub updated_at: Option<SourceText>,
}

/// Height-based history inputs with distinct provider page minimum and local resource cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryRequestFields")]
pub struct HistoryRequest {
    before_height: Option<u64>,
    minimum_entries: u16,
    maximum_entries: u32,
}
impl HistoryRequest {
    /// Records the exclusive before-height cursor, minimum page size and hard local capacity.
    /// The provider can exceed the minimum to include an entire boundary block.
    /// # Errors
    /// Rejects zero/excessive sizes, capacity below the minimum or an oversized height.
    pub fn new(
        before_height: Option<u64>,
        minimum_entries: u16,
        maximum_entries: u32,
    ) -> Result<Self, Error> {
        if minimum_entries == 0
            || minimum_entries > 2000
            || maximum_entries < u32::from(minimum_entries)
            || maximum_entries > 100_000
            || before_height.is_some_and(|h| h > i64::MAX.cast_unsigned())
        {
            return Err(invalid());
        }
        Ok(Self {
            before_height,
            minimum_entries,
            maximum_entries,
        })
    }
    /// Returns the exact exclusive cursor, or the recent page when absent.
    #[must_use]
    pub const fn before_height(self) -> Option<u64> {
        self.before_height
    }
    /// Returns the provider's requested minimum; it is not an exact response maximum.
    #[must_use]
    pub const fn minimum_entries(self) -> u16 {
        self.minimum_entries
    }
    /// Returns the caller's hard combined confirmed/unconfirmed reference capacity.
    #[must_use]
    pub const fn maximum_entries(self) -> u32 {
        self.maximum_entries
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryRequestFields {
    before_height: Option<u64>,
    minimum_entries: u16,
    maximum_entries: u32,
}
impl TryFrom<HistoryRequestFields> for HistoryRequest {
    type Error = Error;
    fn try_from(v: HistoryRequestFields) -> Result<Self, Error> {
        Self::new(v.before_height, v.minimum_entries, v.maximum_entries)
    }
}

/// Actual source-reported inclusion, independent of lasting finality or consensus proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceInclusion {
    /// The actual source-reported block height.
    pub height: u64,
    /// The actual reported hash, or absence when the endpoint does not supply one.
    pub block_hash: Option<BlockHash>,
    /// The actual reported transaction index, or absence when not supplied.
    pub transaction_index: Option<u32>,
    /// Exact source confirmation-time text, retained separately from retrieval time.
    pub confirmed_at: Option<SourceText>,
}

/// Which concrete input/output of a transaction relates to the queried address.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReferenceDirection {
    /// A transaction input associated with this address.
    Input {
        /// The source-reported input position.
        index: u32,
    },
    /// A transaction output associated with this address.
    Output {
        /// The source-reported output position.
        index: u32,
    },
}

/// One exact indexed transaction input/output reference, not a duplicate-collapsed transaction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
pub struct TransactionReference<A: AddressPolicy> {
    /// The actual source transaction identifier.
    pub txid: Txid,
    /// Distinct input/output position; one transaction can legitimately have multiple references.
    pub direction: ReferenceDirection,
    /// Exact associated native value.
    pub value: AtomicAmount<A>,
    /// Reported inclusion, absent only for explicitly unconfirmed references.
    pub inclusion: Option<ReferenceInclusion>,
    /// Actual reported confirmation count, never derived from a separate tip read.
    pub confirmations: u64,
    /// Optional source-reported past balance at this reference.
    pub balance_at_reference: Option<AtomicAmount<A>>,
    /// Optional actual spent state; absence is distinct from false.
    pub spent: Option<bool>,
    /// Optional source-reported spending transaction identifier.
    pub spent_by: Option<Txid>,
    /// Actual provider double-spend flag; it is not independent validation.
    pub double_spend: bool,
    /// Optional source-reported conflicting transaction.
    pub double_spend_transaction: Option<Txid>,
    /// Optional exact source script bytes.
    pub script: Option<Bytes>,
}
impl<A: AddressPolicy> TransactionReference<A> {
    fn validate(&self) -> Result<(), Error> {
        if self.value.raw() > A::TRANSACTION_MAXIMUM
            || (self.inclusion.is_some() != (self.confirmations > 0))
            || self.spent == Some(false) && self.spent_by.is_some()
        {
            return Err(invalid());
        }
        Ok(())
    }
}

/// A bounded source history page with explicit continuation facts and exact balance context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryFields<A>", bound(deserialize = "A: AddressPolicy"))]
pub struct HistoryPage<A: AddressPolicy> {
    address: A,
    request: HistoryRequest,
    balance: BalanceData<A>,
    confirmed: Vec<TransactionReference<A>>,
    unconfirmed: Vec<TransactionReference<A>>,
    has_more: Option<bool>,
}
impl<A: AddressPolicy> HistoryPage<A> {
    /// Records complete source page arrays without truncating a boundary block.
    /// # Errors
    /// Rejects capacity excess, duplicate references, invalid inclusion, unordered
    /// or out-of-range heights and nonadvancing continuation. Same-block references
    /// and distinct input/output positions from one transaction remain valid.
    pub fn new(
        address: A,
        request: HistoryRequest,
        balance: BalanceData<A>,
        confirmed: Vec<TransactionReference<A>>,
        unconfirmed: Vec<TransactionReference<A>>,
        has_more: Option<bool>,
    ) -> Result<Self, Error> {
        balance.validate()?;
        let length = confirmed
            .len()
            .checked_add(unconfirmed.len())
            .ok_or_else(invalid)?;
        if length > request.maximum_entries as usize {
            return Err(invalid());
        }
        let mut identities = BTreeSet::new();
        let mut previous_height = None;
        for entry in &confirmed {
            entry.validate()?;
            let height = entry.inclusion.as_ref().ok_or_else(invalid)?.height;
            if previous_height.is_some_and(|last| height > last)
                || request.before_height.is_some_and(|before| height >= before)
                || !identities.insert((entry.txid, entry.direction))
            {
                return Err(invalid());
            }
            previous_height = Some(height);
        }
        for entry in &unconfirmed {
            entry.validate()?;
            if entry.inclusion.is_some() || !identities.insert((entry.txid, entry.direction)) {
                return Err(invalid());
            }
        }
        if has_more == Some(true) && (previous_height.is_none() || previous_height == Some(0)) {
            return Err(invalid());
        }
        Ok(Self {
            address,
            request,
            balance,
            confirmed,
            unconfirmed,
            has_more,
        })
    }
    /// Returns the exact qualified queried address.
    #[must_use]
    pub const fn address(&self) -> &A {
        &self.address
    }
    /// Returns the exact page inputs and capacity.
    #[must_use]
    pub const fn request(&self) -> HistoryRequest {
        self.request
    }
    /// Returns source balance facts accompanying this page, not a historical height pin.
    #[must_use]
    pub const fn balance(&self) -> &BalanceData<A> {
        &self.balance
    }
    /// Returns all supplied confirmed references, including the complete boundary block.
    #[must_use]
    pub fn confirmed(&self) -> &[TransactionReference<A>] {
        &self.confirmed
    }
    /// Returns separately supplied unconfirmed references; they are not paginated by block height.
    #[must_use]
    pub fn unconfirmed(&self) -> &[TransactionReference<A>] {
        &self.unconfirmed
    }
    /// Returns actual continuation metadata; absence does not prove exhaustion.
    #[must_use]
    pub const fn has_more(&self) -> Option<bool> {
        self.has_more
    }
    /// Returns the next strictly exclusive confirmed-height cursor only when more is reported.
    #[must_use]
    pub fn next_before_height(&self) -> Option<u64> {
        if self.has_more == Some(true) {
            self.confirmed.last()?.inclusion.as_ref().map(|v| v.height)
        } else {
            None
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
struct HistoryFields<A: AddressPolicy> {
    address: A,
    request: HistoryRequest,
    balance: BalanceData<A>,
    #[serde(deserialize_with = "bounded_vec")]
    confirmed: Vec<TransactionReference<A>>,
    #[serde(deserialize_with = "bounded_vec")]
    unconfirmed: Vec<TransactionReference<A>>,
    has_more: Option<bool>,
}
impl<A: AddressPolicy> TryFrom<HistoryFields<A>> for HistoryPage<A> {
    type Error = Error;
    fn try_from(v: HistoryFields<A>) -> Result<Self, Error> {
        Self::new(
            v.address,
            v.request,
            v.balance,
            v.confirmed,
            v.unconfirmed,
            v.has_more,
        )
    }
}

/// Exact indexed transaction confirmation and conflict facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StatusFields")]
pub struct TransactionStatus {
    txid: Txid,
    inclusion: Option<ReferenceInclusion>,
    confirmations: u64,
    double_spend: bool,
    double_spend_transaction: Option<Txid>,
}
impl TransactionStatus {
    /// Retains actual source facts without computing identity, confirmations or finality.
    /// # Errors
    /// Rejects inconsistent confirmed/unconfirmed inclusion facts.
    pub fn new(
        txid: Txid,
        inclusion: Option<ReferenceInclusion>,
        confirmations: u64,
        double_spend: bool,
        double_spend_transaction: Option<Txid>,
    ) -> Result<Self, Error> {
        if inclusion.is_some() != (confirmations > 0) {
            return Err(invalid());
        }
        Ok(Self {
            txid,
            inclusion,
            confirmations,
            double_spend,
            double_spend_transaction,
        })
    }
    /// Returns the source-reported identifier correlated with the exact query.
    #[must_use]
    pub const fn txid(&self) -> Txid {
        self.txid
    }
    /// Returns actual reported inclusion; an unknown transaction is unavailable, not unconfirmed.
    #[must_use]
    pub const fn inclusion(&self) -> Option<&ReferenceInclusion> {
        self.inclusion.as_ref()
    }
    /// Returns the actual provider confirmation count.
    #[must_use]
    pub const fn confirmations(&self) -> u64 {
        self.confirmations
    }
    /// Returns the source double-spend flag, not an independent finding.
    #[must_use]
    pub const fn double_spend(&self) -> bool {
        self.double_spend
    }
    /// Returns an explicitly supplied conflicting identity.
    #[must_use]
    pub const fn double_spend_transaction(&self) -> Option<Txid> {
        self.double_spend_transaction
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusFields {
    txid: Txid,
    inclusion: Option<ReferenceInclusion>,
    confirmations: u64,
    double_spend: bool,
    double_spend_transaction: Option<Txid>,
}
impl TryFrom<StatusFields> for TransactionStatus {
    type Error = Error;
    fn try_from(v: StatusFields) -> Result<Self, Error> {
        Self::new(
            v.txid,
            v.inclusion,
            v.confirmations,
            v.double_spend,
            v.double_spend_transaction,
        )
    }
}

/// A concrete transaction output reference, distinct from the coinbase sentinel.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutPoint {
    /// Previous transaction identity reported by the source.
    pub txid: Txid,
    /// Exact unsigned output position.
    pub output_index: u32,
}

/// The exact provider shape used to classify an indexed input as coinbase.
/// This source interpretation is not independent raw decoding or consensus proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoinbaseSource {
    /// The actual indexed output-index -1 sentinel with absent/zero hash and absent value.
    ExplicitSentinel,
    /// The documented simultaneous omission of previous hash, output index and previous value.
    OmittedPrevoutFields,
}

/// Typed indexed input fields; no raw transaction decoder or signature validation is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
pub struct TransactionInput<A: AddressPolicy> {
    /// An actual indexed outpoint; absence requires a separate source coinbase classification.
    pub previous_output: Option<OutPoint>,
    /// The exact source coinbase shape, absent for ordinary concrete outpoints.
    pub coinbase_source: Option<CoinbaseSource>,
    /// Optional actual indexed previous-output value; unknown remains absent.
    pub previous_output_value: Option<AtomicAmount<A>>,
    /// Actual source sequence, or absence when not supplied.
    pub sequence: Option<u32>,
    /// Actual source script bytes, or absence when not supplied.
    pub script: Option<Bytes>,
    /// Actual supplied witness stack, never inferred from raw bytes or other families.
    #[serde(default, deserialize_with = "super::bounded_optional_witness")]
    pub witness: Option<Vec<Bytes>>,
    /// Optional source-derived, network-validated addresses; no address is invented from scripts.
    #[serde(default, deserialize_with = "super::bounded_optional_addresses")]
    pub addresses: Option<Vec<A>>,
    /// Optional opaque source script classification.
    pub script_type: Option<SourceText>,
}

/// Typed indexed output fields with independently reported spending state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
pub struct TransactionOutput<A: AddressPolicy> {
    /// Exact native atomic-unit value.
    pub value: AtomicAmount<A>,
    /// Exact output script bytes without script execution or policy validation.
    pub script: Bytes,
    /// Optional source-derived, network-validated addresses.
    #[serde(default, deserialize_with = "super::bounded_optional_addresses")]
    pub addresses: Option<Vec<A>>,
    /// Optional actual spending transaction identity.
    pub spent_by: Option<Txid>,
    /// Optional opaque source script classification.
    pub script_type: Option<SourceText>,
}

/// Complete supplied indexed transaction fields, distinct from canonical raw decoding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
pub struct TransactionData<A: AddressPolicy> {
    /// The separately source-reported transaction/status identity and inclusion facts.
    pub status: TransactionStatus,
    /// Actual source transaction version, without interpreting block `AuxPoW` flags.
    pub version: i32,
    /// Actual source lock time; absence remains unavailable, not synthesized zero.
    pub lock_time: Option<u32>,
    /// Actual source-reported serialized size in bytes.
    pub size: u32,
    /// Optional reported virtual size; Dogecoin absence is not replaced by serialized size.
    pub virtual_size: Option<u32>,
    /// Actual indexed sum of all outputs.
    pub total_output: AtomicAmount<A>,
    /// Optional actual indexed fee; coinbase zero remains a source convention.
    pub reported_fee: Option<AtomicAmount<A>>,
    /// All source inputs; incomplete pages are rejected rather than truncated.
    #[serde(deserialize_with = "bounded_vec")]
    pub inputs: Vec<TransactionInput<A>>,
    /// All source outputs; incomplete pages are rejected rather than truncated.
    #[serde(deserialize_with = "bounded_vec")]
    pub outputs: Vec<TransactionOutput<A>>,
    /// Optional bounded opaque source transaction bytes, without independent decoding or identity proof.
    pub raw: Option<Bytes>,
}

/// An immutable, structurally consistent complete indexed transaction for one family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    try_from = "TransactionFields<A>",
    bound(deserialize = "A: AddressPolicy")
)]
pub struct Transaction<A: AddressPolicy> {
    network: A::Network,
    data: TransactionData<A>,
}
impl<A: AddressPolicy> Transaction<A> {
    /// Checks indexed structure, money ranges, known-prevout consistency and family addresses.
    /// # Errors
    /// Rejects invalid counts, duplicate outpoints, malformed coinbase structure,
    /// impossible sums, family-incompatible witness/address facts or raw size disagreement.
    /// This does not validate canonical serialization, `MWEB`, `AuxPoW`, signatures or consensus.
    pub fn new(network: A::Network, data: TransactionData<A>) -> Result<Self, Error> {
        if data.inputs.is_empty()
            || data.outputs.is_empty()
            || data.inputs.len() > MAXIMUM_ITEMS
            || data.outputs.len() > MAXIMUM_ITEMS
            || data.size == 0
            || data.virtual_size.is_some_and(|v| v == 0 || v > data.size)
            || data
                .raw
                .as_ref()
                .is_some_and(|b| b.as_slice().len() != data.size as usize)
        {
            return Err(invalid());
        }
        let mut total_output = 0_u64;
        for output in &data.outputs {
            total_output = total_output
                .checked_add(output.value.raw())
                .ok_or_else(invalid)?;
            if total_output > A::TRANSACTION_MAXIMUM {
                return Err(invalid());
            }
            validate_addresses(output.addresses.as_deref(), network)?;
        }
        if total_output != data.total_output.raw()
            || data
                .reported_fee
                .as_ref()
                .is_some_and(|fee| fee.raw() > A::TRANSACTION_MAXIMUM)
        {
            return Err(invalid());
        }
        let coinbase = data.inputs[0].previous_output.is_none();
        let mut known_input_sum = 0_u64;
        let mut complete_inputs = true;
        let mut outpoints = BTreeSet::new();
        for input in &data.inputs {
            if input.previous_output.is_none() != input.coinbase_source.is_some() {
                return Err(invalid());
            }
            if input.previous_output.is_none()
                && (!coinbase || data.inputs.len() != 1 || input.previous_output_value.is_some())
            {
                return Err(invalid());
            }
            if let Some(outpoint) = input.previous_output
                && ((!outpoints.insert(outpoint))
                    || outpoint.txid.display_bytes() == &[0; 32]
                        && outpoint.output_index == u32::MAX)
            {
                return Err(invalid());
            }
            if let Some(value) = &input.previous_output_value {
                if value.raw() > A::TRANSACTION_MAXIMUM {
                    return Err(invalid());
                }
                known_input_sum = known_input_sum
                    .checked_add(value.raw())
                    .ok_or_else(invalid)?;
                if known_input_sum > A::TRANSACTION_MAXIMUM {
                    return Err(invalid());
                }
            } else {
                complete_inputs = false;
            }
            validate_addresses(input.addresses.as_deref(), network)?;
            if let Some(witness) = &input.witness {
                if witness.len() > 10_000 || !A::SUPPORTS_WITNESS && !witness.is_empty() {
                    return Err(invalid());
                }
                let sum = witness
                    .iter()
                    .try_fold(0_usize, |s, b| s.checked_add(b.as_slice().len()))
                    .ok_or_else(invalid)?;
                if sum > Bytes::MAXIMUM_BYTES {
                    return Err(invalid());
                }
            }
        }
        if coinbase {
            if data.reported_fee.as_ref().is_some_and(|fee| fee.raw() != 0) {
                return Err(invalid());
            }
        } else if let Some(fee) = &data.reported_fee {
            let implied_inputs = total_output.checked_add(fee.raw()).ok_or_else(invalid)?;
            if implied_inputs > A::TRANSACTION_MAXIMUM
                || known_input_sum > implied_inputs
                || complete_inputs && known_input_sum != implied_inputs
            {
                return Err(invalid());
            }
        } else if complete_inputs && known_input_sum < total_output {
            return Err(invalid());
        }
        Ok(Self { network, data })
    }
    /// Returns the family's explicit network qualification.
    #[must_use]
    pub const fn network(&self) -> A::Network {
        self.network
    }
    /// Returns complete indexed fields without a claim of canonical raw decoding.
    #[must_use]
    pub const fn data(&self) -> &TransactionData<A> {
        &self.data
    }
    /// Returns exact arithmetic fee only when all ordinary previous-output values are available.
    /// Coinbase and incomplete previous-output data remain unavailable.
    #[must_use]
    pub fn derived_fee(&self) -> Option<AtomicAmount<A>> {
        self.data.inputs[0].previous_output?;
        let inputs = self.data.inputs.iter().try_fold(0_u64, |sum, input| {
            sum.checked_add(input.previous_output_value.as_ref()?.raw())
        })?;
        Some(AtomicAmount::new(
            inputs.checked_sub(self.data.total_output.raw())?,
        ))
    }
}
fn validate_addresses<A: AddressPolicy>(
    addresses: Option<&[A]>,
    network: A::Network,
) -> Result<(), Error> {
    if let Some(addresses) = addresses {
        if addresses.len() > 100 || addresses.iter().any(|a| a.network() != network) {
            return Err(invalid());
        }
        for (index, address) in addresses.iter().enumerate() {
            if addresses[..index].contains(address) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
struct TransactionFields<A: AddressPolicy> {
    network: A::Network,
    data: TransactionData<A>,
}
impl<A: AddressPolicy> TryFrom<TransactionFields<A>> for Transaction<A> {
    type Error = Error;
    fn try_from(v: TransactionFields<A>) -> Result<Self, Error> {
        Self::new(v.network, v.data)
    }
}
