// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    AccountState, Address, AddressFormat, Boc, Currency, Cursor, ExtraCurrency, Hash, LogicalTime,
    Nanotons,
};
use crate::{domain::Amount, error::Error};
use serde::{Deserialize, Serialize};
use tycho_types::{
    cell::{Cell, Load},
    models::{self, IntAddr, MsgInfo, TxInfo},
};

/// Exact bounded message BOC, whose hash is independent of header interpretation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    boc: Boc,
}
impl Message {
    /// Retains source message evidence without assuming wallet intent or signatures.
    #[must_use]
    pub const fn new(boc: Boc) -> Self {
        Self { boc }
    }
    /// Returns exact bounded source message bytes.
    #[must_use]
    pub const fn boc(&self) -> &Boc {
        &self.boc
    }
    /// Computes the maintained message representation hash.
    ///
    /// # Errors
    /// Returns a fixed decoding failure for invalid cell evidence.
    pub fn hash(&self) -> Result<Hash, Error> {
        self.boc.hash()
    }
    /// Decodes the stable forwarding-fee field where maintained header parsing succeeds.
    ///
    /// `None` explicitly means unsupported header interpretation. External messages
    /// report zero. This does not interpret or verify the physically ambiguous
    /// historical IHR-fee/current extra-flags field.
    ///
    /// # Errors
    /// Returns a fixed error for malformed bounded cell evidence or fee encoding.
    pub fn forwarding_fee(&self) -> Result<Option<Nanotons>, Error> {
        let root = self.boc.root()?;
        let mut slice = root.as_slice().map_err(|_| super::invalid_record())?;
        let Ok(message) = models::Message::load_from(&mut slice) else {
            return Ok(None);
        };
        if !slice.is_empty() {
            return Ok(None);
        }
        match message.info {
            MsgInfo::Int(info) => Nanotons::new(info.fwd_fee.into_inner()).map(Some),
            MsgInfo::ExtIn(_) | MsgInfo::ExtOut(_) => Ok(Some(Nanotons::ZERO)),
        }
    }
    /// Interprets supported modern standard headers, preserving unsupported evidence.
    /// The physical historical IHR-fee/current extra-flags field is deliberately
    /// not exposed: small historical values can also decode as modern flags.
    /// Unsupported headers and variable/anycast addresses remain uninterpreted.
    ///
    /// # Errors
    /// Returns a fixed error for malformed bounded cell evidence.
    pub fn info(&self) -> Result<MessageInfo, Error> {
        let root = self.boc.root()?;
        let mut slice = root.as_slice().map_err(|_| super::invalid_record())?;
        let Ok(message) = models::Message::load_from(&mut slice) else {
            return Ok(MessageInfo::Uninterpreted);
        };
        if !slice.is_empty() {
            return Ok(MessageInfo::Uninterpreted);
        }
        match message.info {
            MsgInfo::Int(info) => {
                let (Some(source), Some(destination)) = (standard(&info.src), standard(&info.dst))
                else {
                    return Ok(MessageInfo::Uninterpreted);
                };
                Ok(MessageInfo::Internal {
                    source,
                    destination,
                    value: currency(&info.value)?,
                    bounce: info.bounce,
                    bounced: info.bounced,
                    created_lt: info.created_lt,
                    created_unix_seconds: info.created_at,
                })
            }
            MsgInfo::ExtIn(info) => {
                let Some(destination) = standard(&info.dst) else {
                    return Ok(MessageInfo::Uninterpreted);
                };
                Ok(MessageInfo::ExternalIncoming {
                    destination,
                    import_fee: Nanotons::new(info.import_fee.into_inner())?,
                })
            }
            MsgInfo::ExtOut(info) => {
                let Some(source) = standard(&info.src) else {
                    return Ok(MessageInfo::Uninterpreted);
                };
                Ok(MessageInfo::ExternalOutgoing {
                    source,
                    created_lt: info.created_lt,
                    created_unix_seconds: info.created_at,
                })
            }
        }
    }
}
/// Header facts supported by the maintained current TON model.
/// Absence of interpretation is explicit; the complete message BOC is retained.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MessageInfo {
    /// Native/extra currency-bearing internal message.
    Internal {
        /// Source account.
        source: Address,
        /// Destination account.
        destination: Address,
        /// Exact attached values.
        value: Currency,
        /// Encoded bounce request.
        bounce: bool,
        /// Encoded bounce-result marker.
        bounced: bool,
        /// Exact source logical time; zero is possible for prepared messages.
        created_lt: u64,
        /// Source message creation time in Unix seconds.
        created_unix_seconds: u32,
    },
    /// External message entering an account; its signature is not verified.
    ExternalIncoming {
        /// Destination account.
        destination: Address,
        /// Exact source import fee.
        import_fee: Nanotons,
    },
    /// External message emitted by an account, with external-address bytes in the BOC.
    ExternalOutgoing {
        /// Source account.
        source: Address,
        /// Source logical time.
        created_lt: u64,
        /// Source Unix seconds.
        created_unix_seconds: u32,
    },
    /// Valid bounded cell evidence has an unsupported or uninterpreted header.
    Uninterpreted,
}

/// Actual supported compute-phase facts, independent of overall transaction success.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Compute {
    /// Source skipped execution; no VM exit code exists.
    Skipped {
        /// Exact supported skip reason.
        reason: ComputeSkip,
    },
    /// Source VM execution facts.
    Executed {
        /// Actual success flag.
        success: bool,
        /// Actual signed VM exit code.
        exit_code: i32,
        /// Exact gas cost in nanotons.
        gas_fees: Nanotons,
    },
}
/// Supported TON compute-skip reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeSkip {
    /// Missing contract state.
    NoState,
    /// Invalid contract state.
    BadState,
    /// Insufficient gas.
    NoGas,
    /// Source suspended account.
    Suspended,
}
/// Actual decoded action-phase facts without a fabricated transfer outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    /// Source action success flag.
    pub success: bool,
    /// Source action-list validity flag.
    pub valid: bool,
    /// Source insufficient-funds flag.
    pub no_funds: bool,
    /// Actual signed action result code.
    pub result_code: i32,
    /// Actual number of messages created.
    pub messages_created: u16,
}
/// Supported execution facts or an explicit uninterpreted description.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Execution {
    /// Ordinary account transaction execution, separate from consensus inclusion.
    Ordinary {
        /// Source reverted-transaction flag.
        aborted: bool,
        /// Source destroyed-account flag.
        destroyed: bool,
        /// Actual compute facts.
        compute: Compute,
        /// Actual action facts when this phase was present.
        action: Option<Action>,
    },
    /// Masterchain tick/tock execution facts, not an ordinary transfer.
    TickTock {
        /// Source aborted flag.
        aborted: bool,
        /// Source destroyed flag.
        destroyed: bool,
        /// Actual compute facts.
        compute: Compute,
        /// Actual optional action facts.
        action: Option<Action>,
    },
    /// A source description unsupported by the maintained current model.
    Uninterpreted {
        /// Exact initial description bits, not a success classification.
        prefix: u8,
        /// Complete bounded description cell evidence.
        description: Boc,
    },
}

/// Provider-reported outgoing-message fee facts, independent of legacy header interpretation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMessageFees {
    /// Source message representation hash, correlated with the outgoing BOC.
    pub message_hash: Hash,
    /// Provider-labelled forwarding fee in exact nanotons.
    pub forwarding_fee: Nanotons,
    /// Provider-labelled IHR fee in exact nanotons; not decoded as current extra flags.
    pub source_ihr_fee: Nanotons,
}
/// Provider transaction aggregate, retained separately from decoded TL-B `total_fees`.
///
/// The TON library adds each outgoing source forwarding/IHR fee to `total_fees`
/// when reporting its API aggregate. IHR labels remain source facts; an ambiguous
/// or unsupported physical header does not establish their decoded interpretation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ProviderFeesFields")]
pub struct ProviderFees {
    aggregate: Nanotons,
    storage: Nanotons,
    other: Nanotons,
    outgoing: Vec<SourceMessageFees>,
}
impl ProviderFees {
    /// Retains an exact provider aggregate and bounded source message components.
    /// Transaction attachment additionally checks ordered BOC hashes, sums and
    /// forwarding fields where the maintained parser supports interpretation.
    ///
    /// # Errors
    /// Rejects more than 256 messages or storage/other components that do not sum.
    pub fn new(
        aggregate: Nanotons,
        storage: Nanotons,
        other: Nanotons,
        outgoing: Vec<SourceMessageFees>,
    ) -> Result<Self, Error> {
        if outgoing.len() > 256 || storage.raw().checked_add(other.raw()) != Some(aggregate.raw()) {
            return Err(super::invalid_record());
        }
        Ok(Self {
            aggregate,
            storage,
            other,
            outgoing,
        })
    }
    /// Returns the provider-labelled aggregate, not the decoded transaction `total_fees`.
    #[must_use]
    pub const fn aggregate(&self) -> Nanotons {
        self.aggregate
    }
    /// Returns the source storage component.
    #[must_use]
    pub const fn storage(&self) -> Nanotons {
        self.storage
    }
    /// Returns the source aggregate minus storage, not an invented compute-phase cost.
    #[must_use]
    pub const fn other(&self) -> Nanotons {
        self.other
    }
    /// Returns all source outgoing fee facts in the transaction's dictionary order.
    #[must_use]
    pub fn outgoing(&self) -> &[SourceMessageFees] {
        &self.outgoing
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderFeesFields {
    aggregate: Nanotons,
    storage: Nanotons,
    other: Nanotons,
    outgoing: Vec<SourceMessageFees>,
}
impl TryFrom<ProviderFeesFields> for ProviderFees {
    type Error = Error;
    fn try_from(v: ProviderFeesFields) -> Result<Self, Error> {
        Self::new(v.aggregate, v.storage, v.other, v.outgoing)
    }
}

/// Decoded TON account transaction with exact maintained BOC/hash identity.
/// The account is caller-qualified because its workchain is absent from transaction TL-B.
/// History linkage and execution facts do not establish a block or finality proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionFields")]
pub struct Transaction {
    account: Address,
    boc: Boc,
    cursor: Cursor,
    previous: Option<Cursor>,
    created_unix_seconds: u32,
    fees: Currency,
    provider_fees: Option<ProviderFees>,
    original_state: AccountState,
    end_state: AccountState,
    incoming: Option<Message>,
    outgoing: Vec<Message>,
    execution: Execution,
}
impl Transaction {
    /// Fully consumes a supported common transaction envelope and derives every retained fact.
    ///
    /// # Errors
    /// Rejects account mismatch, malformed envelope, incoherent initial predecessor,
    /// more than 256 outgoing messages, mismatched dictionary count or malformed
    /// supported ordinary/tick-tock execution. Other descriptions stay uninterpreted.
    pub fn decode(account: Address, boc: Boc) -> Result<Self, Error> {
        let root = boc.root()?;
        let mut slice = root.as_slice().map_err(|_| super::invalid_record())?;
        let tx = models::Transaction::load_from(&mut slice).map_err(|_| super::invalid_record())?;
        if !slice.is_empty() || tx.account.0 != *account.account().as_bytes() {
            return Err(super::invalid_record());
        }
        let mut update = tx
            .state_update
            .inner()
            .as_slice()
            .map_err(|_| super::invalid_record())?;
        models::HashUpdate::load_from(&mut update).map_err(|_| super::invalid_record())?;
        if !update.is_empty() {
            return Err(super::invalid_record());
        }
        let cursor = Cursor::new(LogicalTime::new(tx.lt)?, boc.hash()?)?;
        let previous = if tx.prev_trans_lt == 0 {
            if tx.prev_trans_hash.0 != [0; 32] {
                return Err(super::invalid_record());
            }
            None
        } else {
            if tx.prev_trans_lt >= tx.lt {
                return Err(super::invalid_record());
            }
            Some(Cursor::new(
                LogicalTime::new(tx.prev_trans_lt)?,
                Hash::from_bytes(tx.prev_trans_hash.0),
            )?)
        };
        let incoming = tx.in_msg.as_ref().map(message).transpose()?;
        if tx.out_msg_count.into_inner() > 256 {
            return Err(super::invalid_record());
        }
        let mut outgoing = Vec::new();
        for entry in tx.out_msgs.iter() {
            let (index, cell) = entry.map_err(|_| super::invalid_record())?;
            if outgoing.len() >= 256 {
                return Err(super::invalid_record());
            }
            if usize::from(index.into_inner()) != outgoing.len() {
                return Err(super::invalid_record());
            }
            outgoing.push(message(&cell)?);
        }
        if outgoing.len() != usize::from(tx.out_msg_count.into_inner()) {
            return Err(super::invalid_record());
        }
        let execution = execution(&tx)?;
        Ok(Self {
            account,
            boc,
            cursor,
            previous,
            created_unix_seconds: tx.now,
            fees: currency(&tx.total_fees)?,
            provider_fees: None,
            original_state: account_state(tx.orig_status),
            end_state: account_state(tx.end_status),
            incoming,
            outgoing,
            execution,
        })
    }
    /// Attaches explicit provider fee facts without replacing decoded `total_fees`.
    /// IHR amounts remain source-labelled facts. Unsupported forwarding-header
    /// interpretation remains unavailable through [`Message::forwarding_fee`].
    ///
    /// # Errors
    /// Rejects ordered message hash/count mismatch, inconsistent aggregate sums or
    /// forwarding-field mismatch where maintained header interpretation succeeds.
    pub fn with_provider_fees(mut self, fees: ProviderFees) -> Result<Self, Error> {
        if fees.outgoing.len() != self.outgoing.len() {
            return Err(super::invalid_record());
        }
        let mut aggregate = self.fees.native().raw();
        for (source, message) in fees.outgoing.iter().zip(&self.outgoing) {
            if source.message_hash != message.hash()?
                || message
                    .forwarding_fee()?
                    .is_some_and(|fee| fee != source.forwarding_fee)
            {
                return Err(super::invalid_record());
            }
            aggregate = aggregate
                .checked_add(source.forwarding_fee.raw())
                .and_then(|v| v.checked_add(source.source_ihr_fee.raw()))
                .ok_or_else(super::invalid_record)?;
        }
        if aggregate != fees.aggregate.raw() {
            return Err(super::invalid_record());
        }
        self.provider_fees = Some(fees);
        Ok(self)
    }
    /// Returns checked provider fee attribution, absent for pure BOC-only decoding.
    #[must_use]
    pub const fn provider_fees(&self) -> Option<&ProviderFees> {
        self.provider_fees.as_ref()
    }
    /// Returns account identity with its explicit workchain qualification.
    #[must_use]
    pub const fn account(&self) -> Address {
        self.account
    }
    /// Returns exact bounded source transaction bytes.
    #[must_use]
    pub const fn boc(&self) -> &Boc {
        &self.boc
    }
    /// Returns computed representation hash and decoded logical time.
    #[must_use]
    pub const fn cursor(&self) -> Cursor {
        self.cursor
    }
    /// Returns decoded predecessor identity, or the explicit initial sentinel.
    #[must_use]
    pub const fn previous(&self) -> Option<Cursor> {
        self.previous
    }
    /// Returns decoded transaction Unix seconds, not retrieval or block time.
    #[must_use]
    pub const fn created_unix_seconds(&self) -> u32 {
        self.created_unix_seconds
    }
    /// Returns exact decoded native/extra total fees.
    #[must_use]
    pub const fn fees(&self) -> &Currency {
        &self.fees
    }
    /// Returns the actual account state before this transaction.
    #[must_use]
    pub const fn original_state(&self) -> AccountState {
        self.original_state
    }
    /// Returns the actual account state after this transaction.
    #[must_use]
    pub const fn end_state(&self) -> AccountState {
        self.end_state
    }
    /// Returns decoded incoming message evidence.
    #[must_use]
    pub const fn incoming(&self) -> Option<&Message> {
        self.incoming.as_ref()
    }
    /// Returns all decoded outgoing message evidence without truncation.
    #[must_use]
    pub fn outgoing(&self) -> &[Message] {
        &self.outgoing
    }
    /// Returns supported actual execution facts or uninterpreted source evidence.
    #[must_use]
    pub const fn execution(&self) -> &Execution {
        &self.execution
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionFields {
    account: Address,
    boc: Boc,
    cursor: Cursor,
    previous: Option<Cursor>,
    created_unix_seconds: u32,
    fees: Currency,
    provider_fees: Option<ProviderFees>,
    original_state: AccountState,
    end_state: AccountState,
    incoming: Option<Message>,
    outgoing: Vec<Message>,
    execution: Execution,
}
impl TryFrom<TransactionFields> for Transaction {
    type Error = Error;
    fn try_from(v: TransactionFields) -> Result<Self, Error> {
        let mut tx = Self::decode(v.account, v.boc)?;
        if let Some(fees) = v.provider_fees {
            tx = tx.with_provider_fees(fees)?;
        }
        if tx.cursor != v.cursor
            || tx.previous != v.previous
            || tx.created_unix_seconds != v.created_unix_seconds
            || tx.fees != v.fees
            || tx.original_state != v.original_state
            || tx.end_state != v.end_state
            || tx.incoming != v.incoming
            || tx.outgoing != v.outgoing
            || tx.execution != v.execution
        {
            return Err(super::invalid_record());
        }
        Ok(tx)
    }
}

/// Exact account transaction status retaining source evidence and actual execution facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionStatus {
    transaction: Transaction,
}
impl TransactionStatus {
    /// Records decoded source execution without inferring block inclusion or finality.
    #[must_use]
    pub const fn new(transaction: Transaction) -> Self {
        Self { transaction }
    }
    /// Returns actual decoded transaction and execution facts.
    #[must_use]
    pub const fn transaction(&self) -> &Transaction {
        &self.transaction
    }
}
pub(crate) fn standard(address: &IntAddr) -> Option<Address> {
    match address {
        IntAddr::Std(s) if s.anycast.is_none() => Some(Address::new(
            s.workchain,
            Hash::from_bytes(s.address.0),
            AddressFormat::Raw,
        )),
        _ => None,
    }
}
pub(crate) fn currency(value: &models::CurrencyCollection) -> Result<Currency, Error> {
    let mut extra = Vec::new();
    for entry in value.other.as_dict().iter() {
        let (id, value) = entry.map_err(|_| super::invalid_record())?;
        if extra.len() >= 256 {
            return Err(super::invalid_record());
        }
        extra.push(ExtraCurrency {
            id,
            amount: Amount::from_decimal(&value.to_string(), None)?,
        });
    }
    Currency::new(Nanotons::new(value.tokens.into_inner())?, extra)
}
fn message(cell: &Cell) -> Result<Message, Error> {
    Ok(Message::new(Boc::from_cell(cell)?))
}
fn account_state(state: models::AccountStatus) -> AccountState {
    match state {
        models::AccountStatus::Uninit => AccountState::Uninitialized,
        models::AccountStatus::Frozen => AccountState::Frozen,
        models::AccountStatus::Active => AccountState::Active,
        models::AccountStatus::NotExists => AccountState::Nonexistent,
    }
}
fn compute(phase: models::ComputePhase) -> Result<Compute, Error> {
    Ok(match phase {
        models::ComputePhase::Skipped(p) => Compute::Skipped {
            reason: match p.reason {
                models::ComputePhaseSkipReason::NoState => ComputeSkip::NoState,
                models::ComputePhaseSkipReason::BadState => ComputeSkip::BadState,
                models::ComputePhaseSkipReason::NoGas => ComputeSkip::NoGas,
                models::ComputePhaseSkipReason::Suspended => ComputeSkip::Suspended,
            },
        },
        models::ComputePhase::Executed(p) => Compute::Executed {
            success: p.success,
            exit_code: p.exit_code,
            gas_fees: Nanotons::new(p.gas_fees.into_inner())?,
        },
    })
}
fn action(phase: Option<models::ActionPhase>) -> Option<Action> {
    phase.map(|p| Action {
        success: p.success,
        valid: p.valid,
        no_funds: p.no_funds,
        result_code: p.result_code,
        messages_created: p.messages_created,
    })
}
fn execution(tx: &models::Transaction) -> Result<Execution, Error> {
    let cell = tx.info.inner();
    let mut slice = cell.as_slice().map_err(|_| super::invalid_record())?;
    let prefix = slice
        .load_small_uint(3)
        .map_err(|_| super::invalid_record())?;
    if prefix != 0 && prefix != 1 {
        return Ok(Execution::Uninterpreted {
            prefix,
            description: Boc::from_cell(cell)?,
        });
    }
    if prefix == 0 && slice.load_bit().map_err(|_| super::invalid_record())? {
        return Ok(Execution::Uninterpreted {
            prefix,
            description: Boc::from_cell(cell)?,
        });
    }
    let mut full = cell.as_slice().map_err(|_| super::invalid_record())?;
    let info = TxInfo::load_from(&mut full).map_err(|_| super::invalid_record())?;
    if !full.is_empty() {
        return Err(super::invalid_record());
    }
    Ok(match info {
        TxInfo::Ordinary(p) => Execution::Ordinary {
            aborted: p.aborted,
            destroyed: p.destroyed,
            compute: compute(p.compute_phase)?,
            action: action(p.action_phase),
        },
        TxInfo::TickTock(p) => Execution::TickTock {
            aborted: p.aborted,
            destroyed: p.destroyed,
            compute: compute(p.compute_phase)?,
            action: action(p.action_phase),
        },
    })
}
