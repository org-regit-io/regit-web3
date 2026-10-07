// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Address, Boc, Hash, Network, Transaction};
use crate::{domain::Amount, error::Error};
use serde::{Deserialize, Serialize};
use tycho_types::models::ShardIdent;

/// Exact nonnegative nanotons within TON's `VarUInteger 16` range.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Nanotons(u128);
impl Nanotons {
    /// Zero nanotons.
    pub const ZERO: Self = Self(0);
    /// Checks the protocol's maximum 15-byte integer.
    ///
    /// # Errors
    /// Rejects values greater than `2^120-1`.
    pub fn new(value: u128) -> Result<Self, Error> {
        if value >= 1u128 << 120 {
            return Err(super::invalid_record());
        }
        Ok(Self(value))
    }
    /// Parses canonical unsigned decimal nanotons without floating point.
    ///
    /// # Errors
    /// Rejects signs, whitespace, leading zeros or out-of-range values.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty()
            || !value.bytes().all(|b| b.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(super::invalid_record());
        }
        Self::new(value.parse().map_err(|_| super::invalid_record())?)
    }
    /// Returns exact integer nanotons.
    #[must_use]
    pub const fn raw(self) -> u128 {
        self.0
    }
    /// Returns a general exact amount with TON's nine decimal places.
    ///
    /// # Errors
    /// Returns a fixed amount failure if conversion cannot preserve exact units.
    pub fn amount(self) -> Result<Amount, Error> {
        Amount::from_decimal(&self.0.to_string(), Some(9))
    }
}
impl TryFrom<String> for Nanotons {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<Nanotons> for String {
    fn from(v: Nanotons) -> Self {
        v.0.to_string()
    }
}

/// A nonzero account logical time, serialized as an exact decimal string.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LogicalTime(u64);
impl LogicalTime {
    /// Checks one nonzero account logical-time value.
    ///
    /// # Errors
    /// Rejects the initial-account sentinel zero.
    pub fn new(value: u64) -> Result<Self, Error> {
        if value == 0 {
            return Err(super::invalid_record());
        }
        Ok(Self(value))
    }
    /// Parses canonical unsigned decimal text.
    ///
    /// # Errors
    /// Rejects zero, signs, whitespace, leading zeros or overflow.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) || value.starts_with('0')
        {
            return Err(super::invalid_record());
        }
        Self::new(value.parse().map_err(|_| super::invalid_record())?)
    }
    /// Returns exact logical time, without a timestamp interpretation.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}
impl TryFrom<String> for LogicalTime {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<LogicalTime> for String {
    fn from(v: LogicalTime) -> Self {
        v.0.to_string()
    }
}

/// Exact account-history identity: account, logical time and representation hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CursorFields")]
pub struct Cursor {
    logical_time: LogicalTime,
    hash: Hash,
}
impl Cursor {
    /// Records an exact nonzero history cursor.
    ///
    /// # Errors
    /// Rejects an all-zero transaction hash.
    pub fn new(logical_time: LogicalTime, hash: Hash) -> Result<Self, Error> {
        if hash == Hash::ZERO {
            return Err(super::invalid_record());
        }
        Ok(Self { logical_time, hash })
    }
    /// Returns exact account logical time.
    #[must_use]
    pub const fn logical_time(self) -> LogicalTime {
        self.logical_time
    }
    /// Returns the transaction representation hash.
    #[must_use]
    pub const fn hash(self) -> Hash {
        self.hash
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CursorFields {
    logical_time: LogicalTime,
    hash: Hash,
}
impl TryFrom<CursorFields> for Cursor {
    type Error = Error;
    fn try_from(v: CursorFields) -> Result<Self, Error> {
        Self::new(v.logical_time, v.hash)
    }
}

/// Full source-reported TON block identity, including signed shard bits.
/// Hash agreement is not an independently checked consensus proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BlockFields")]
pub struct Block {
    workchain: i32,
    shard: String,
    seqno: u32,
    root_hash: Hash,
    file_hash: Hash,
}
impl Block {
    /// Checks the maintained shard grammar and full nonzero block hashes.
    ///
    /// # Errors
    /// Rejects invalid shards, zero sequence or missing hashes.
    pub fn new(
        workchain: i32,
        shard: &str,
        seqno: u32,
        root_hash: Hash,
        file_hash: Hash,
    ) -> Result<Self, Error> {
        let digits = shard.strip_prefix('-').unwrap_or(shard);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || shard.len() > 20 {
            return Err(super::invalid_record());
        }
        let id = shard.parse::<i64>().map_err(|_| super::invalid_record())?;
        if ShardIdent::new(workchain, u64::from_be_bytes(id.to_be_bytes())).is_none()
            || seqno == 0
            || root_hash == Hash::ZERO
            || file_hash == Hash::ZERO
        {
            return Err(super::invalid_record());
        }
        Ok(Self {
            workchain,
            shard: id.to_string(),
            seqno,
            root_hash,
            file_hash,
        })
    }
    /// Returns actual signed workchain.
    #[must_use]
    pub const fn workchain(&self) -> i32 {
        self.workchain
    }
    /// Returns canonical signed decimal shard bits.
    #[must_use]
    pub fn shard(&self) -> &str {
        &self.shard
    }
    /// Returns actual block sequence number.
    #[must_use]
    pub const fn seqno(&self) -> u32 {
        self.seqno
    }
    /// Returns source block root hash.
    #[must_use]
    pub const fn root_hash(&self) -> Hash {
        self.root_hash
    }
    /// Returns source block file hash.
    #[must_use]
    pub const fn file_hash(&self) -> Hash {
        self.file_hash
    }
    /// Checks the masterchain's full-shard identity.
    #[must_use]
    pub fn is_masterchain(&self) -> bool {
        self.workchain == -1 && self.shard == "-9223372036854775808"
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockFields {
    workchain: i32,
    shard: String,
    seqno: u32,
    root_hash: Hash,
    file_hash: Hash,
}
impl TryFrom<BlockFields> for Block {
    type Error = Error;
    fn try_from(v: BlockFields) -> Result<Self, Error> {
        Self::new(v.workchain, &v.shard, v.seqno, v.root_hash, v.file_hash)
    }
}

/// One exact non-native TON extra-currency denomination.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraCurrency {
    /// Protocol 32-bit extra-currency ID, not a jetton contract identity.
    pub id: u32,
    /// Exact base-unit amount; its precision is unreported.
    pub amount: Amount,
}
/// Native nanotons plus bounded, duplicate-free extra-currency values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CurrencyFields")]
pub struct Currency {
    native: Nanotons,
    extra: Vec<ExtraCurrency>,
}
impl Currency {
    /// Checks at most 256 distinct protocol extra currencies with unknown precision.
    ///
    /// # Errors
    /// Rejects duplicate IDs, known precision or `VarUInteger 32` overflow.
    pub fn new(native: Nanotons, extra: Vec<ExtraCurrency>) -> Result<Self, Error> {
        if extra.len() > 256 {
            return Err(super::invalid_record());
        }
        let mut ids = std::collections::BTreeSet::new();
        for item in &extra {
            if !ids.insert(item.id)
                || item.amount.decimals().is_some()
                || item.amount.raw().bit_len() > 248
            {
                return Err(super::invalid_record());
            }
        }
        Ok(Self { native, extra })
    }
    /// Returns exact native nanotons.
    #[must_use]
    pub const fn native(&self) -> Nanotons {
        self.native
    }
    /// Returns exact protocol extra-currency values without jetton claims.
    #[must_use]
    pub fn extra(&self) -> &[ExtraCurrency] {
        &self.extra
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrencyFields {
    native: Nanotons,
    extra: Vec<ExtraCurrency>,
}
impl TryFrom<CurrencyFields> for Currency {
    type Error = Error;
    fn try_from(v: CurrencyFields) -> Result<Self, Error> {
        Self::new(v.native, v.extra)
    }
}

/// Actual source-reported account state, independent of a wallet classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    /// Account has no initialized contract.
    Uninitialized,
    /// Account contract is active.
    Active,
    /// Account contract is frozen.
    Frozen,
    /// Account does not exist at this evaluation.
    Nonexistent,
}
/// Exact account data evaluated at a selected full masterchain block.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AccountFields")]
pub struct AccountBalance {
    address: Address,
    balance: Currency,
    state: AccountState,
    last_transaction: Option<Cursor>,
    code: Option<Boc>,
    data: Option<Boc>,
    frozen_hash: Option<Hash>,
    suspended: Option<bool>,
    sync_unix_seconds: u64,
}
impl AccountBalance {
    /// Records source facts after explicit address/network qualification.
    ///
    /// # Errors
    /// Rejects active accounts without code/data or frozen accounts without a hash.
    pub fn new(fields: AccountBalanceData) -> Result<Self, Error> {
        if (fields.state == AccountState::Active
            && (fields.code.is_none() || fields.data.is_none()))
            || (fields.state == AccountState::Frozen && fields.frozen_hash.is_none())
        {
            return Err(super::invalid_record());
        }
        Ok(Self {
            address: fields.address,
            balance: fields.balance,
            state: fields.state,
            last_transaction: fields.last_transaction,
            code: fields.code,
            data: fields.data,
            frozen_hash: fields.frozen_hash,
            suspended: fields.suspended,
            sync_unix_seconds: fields.sync_unix_seconds,
        })
    }
    /// Returns queried account identity and original presentation flags.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }
    /// Returns exact native and extra-currency balances.
    #[must_use]
    pub const fn balance(&self) -> &Currency {
        &self.balance
    }
    /// Returns source contract state without assuming wallet ownership.
    #[must_use]
    pub const fn state(&self) -> AccountState {
        self.state
    }
    /// Returns exact last history cursor, or the initial-account sentinel.
    #[must_use]
    pub const fn last_transaction(&self) -> Option<Cursor> {
        self.last_transaction
    }
    /// Returns bounded source code BOC when present.
    #[must_use]
    pub const fn code(&self) -> Option<&Boc> {
        self.code.as_ref()
    }
    /// Returns bounded source data BOC when present.
    #[must_use]
    pub const fn data(&self) -> Option<&Boc> {
        self.data.as_ref()
    }
    /// Returns actual source frozen-state hash when present.
    #[must_use]
    pub const fn frozen_hash(&self) -> Option<Hash> {
        self.frozen_hash
    }
    /// Returns actual source suspension flag, preserving unreported absence.
    #[must_use]
    pub const fn suspended(&self) -> Option<bool> {
        self.suspended
    }
    /// Returns actual provider synchronization time in Unix seconds.
    #[must_use]
    pub const fn sync_unix_seconds(&self) -> u64 {
        self.sync_unix_seconds
    }
}
/// Constructor data for exact source account facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountBalanceData {
    /// Queried account.
    pub address: Address,
    /// Exact balances.
    pub balance: Currency,
    /// Source state.
    pub state: AccountState,
    /// Last transaction, with genesis absence explicit.
    pub last_transaction: Option<Cursor>,
    /// Source contract code.
    pub code: Option<Boc>,
    /// Source contract data.
    pub data: Option<Boc>,
    /// Source frozen-state hash.
    pub frozen_hash: Option<Hash>,
    /// Source suspension flag, if reported.
    pub suspended: Option<bool>,
    /// Actual synchronization timestamp, Unix seconds.
    pub sync_unix_seconds: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountFields {
    address: Address,
    balance: Currency,
    state: AccountState,
    last_transaction: Option<Cursor>,
    code: Option<Boc>,
    data: Option<Boc>,
    frozen_hash: Option<Hash>,
    suspended: Option<bool>,
    sync_unix_seconds: u64,
}
impl TryFrom<AccountFields> for AccountBalance {
    type Error = Error;
    fn try_from(v: AccountFields) -> Result<Self, Error> {
        Self::new(AccountBalanceData {
            address: v.address,
            balance: v.balance,
            state: v.state,
            last_transaction: v.last_transaction,
            code: v.code,
            data: v.data,
            frozen_hash: v.frozen_hash,
            suspended: v.suspended,
            sync_unix_seconds: v.sync_unix_seconds,
        })
    }
}

/// One explicitly bounded account history page request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryFields")]
pub struct HistoryRequest {
    address: Address,
    start: Option<Cursor>,
    limit: u8,
    archival: bool,
}
impl HistoryRequest {
    /// Records an optional inclusive starting identity and a limit of 1–100.
    /// `archival` is an explicit provider search policy, not an exhaustiveness guarantee.
    ///
    /// # Errors
    /// Rejects zero or over-100 cardinality.
    pub fn new(
        address: Address,
        start: Option<Cursor>,
        limit: u8,
        archival: bool,
    ) -> Result<Self, Error> {
        if !(1..=100).contains(&limit) {
            return Err(super::invalid_record());
        }
        Ok(Self {
            address,
            start,
            limit,
            archival,
        })
    }
    /// Returns exact queried account.
    #[must_use]
    pub const fn address(self) -> Address {
        self.address
    }
    /// Returns optional exact inclusive account cursor.
    #[must_use]
    pub const fn start(self) -> Option<Cursor> {
        self.start
    }
    /// Returns the explicit page ceiling.
    #[must_use]
    pub const fn limit(self) -> u8 {
        self.limit
    }
    /// Returns explicit archival-provider policy.
    #[must_use]
    pub const fn archival(self) -> bool {
        self.archival
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryFields {
    address: Address,
    start: Option<Cursor>,
    limit: u8,
    archival: bool,
}
impl TryFrom<HistoryFields> for HistoryRequest {
    type Error = Error;
    fn try_from(v: HistoryFields) -> Result<Self, Error> {
        Self::new(v.address, v.start, v.limit, v.archival)
    }
}

/// A bounded history page checked against account identity and adjacent previous hashes.
/// Linkage is not a block-inclusion or whole-history-exhaustiveness proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PageFields")]
pub struct HistoryPage {
    request: HistoryRequest,
    transactions: Vec<Transaction>,
    next: Option<Cursor>,
}
impl HistoryPage {
    /// Checks order, exact cursor correlation and previous-transaction linkage.
    /// An empty page cannot prove genesis; a nonempty final transaction with
    /// zero previous identity explicitly establishes the source's chain end.
    ///
    /// # Errors
    /// Rejects excess rows, mismatched account/hash/cursor or broken linkage.
    pub fn new(request: HistoryRequest, transactions: Vec<Transaction>) -> Result<Self, Error> {
        if transactions.len() > usize::from(request.limit) {
            return Err(super::invalid_record());
        }
        for tx in &transactions {
            if !tx.account().same_account(request.address) {
                return Err(super::invalid_record());
            }
        }
        if let (Some(start), Some(first)) = (request.start, transactions.first())
            && first.cursor() != start
        {
            return Err(super::invalid_record());
        }
        for pair in transactions.windows(2) {
            if pair[0].previous() != Some(pair[1].cursor())
                || pair[0].cursor().logical_time() <= pair[1].cursor().logical_time()
            {
                return Err(super::invalid_record());
            }
        }
        let next = transactions.last().and_then(Transaction::previous);
        Ok(Self {
            request,
            transactions,
            next,
        })
    }
    /// Returns the exact page request.
    #[must_use]
    pub const fn request(&self) -> HistoryRequest {
        self.request
    }
    /// Returns every source transaction within the bound, without truncation.
    #[must_use]
    pub fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }
    /// Returns the oldest row's previous identity, avoiding an inclusive duplicate.
    #[must_use]
    pub const fn next(&self) -> Option<Cursor> {
        self.next
    }
    /// Returns true only for a nonempty page whose oldest transaction has no predecessor.
    #[must_use]
    pub fn reaches_account_origin(&self) -> bool {
        self.transactions
            .last()
            .is_some_and(|tx| tx.previous().is_none())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageFields {
    request: HistoryRequest,
    transactions: Vec<Transaction>,
    next: Option<Cursor>,
}
impl TryFrom<PageFields> for HistoryPage {
    type Error = Error;
    fn try_from(v: PageFields) -> Result<Self, Error> {
        let result = Self::new(v.request, v.transactions)?;
        if result.next != v.next {
            return Err(super::invalid_record());
        }
        Ok(result)
    }
}

/// Complete source masterchain state identity and the expected zero-state network.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct NetworkData {
    network: Network,
    last: Block,
    state_root_hash: Hash,
}
impl NetworkData {
    /// Records full masterchain facts after expected zero-state verification.
    ///
    /// # Errors
    /// Rejects a non-masterchain last block or zero state-root hash.
    pub fn new(network: Network, last: Block, state_root_hash: Hash) -> Result<Self, Error> {
        if !last.is_masterchain() || state_root_hash == Hash::ZERO {
            return Err(super::invalid_record());
        }
        Ok(Self {
            network,
            last,
            state_root_hash,
        })
    }
    /// Returns the caller-qualified full zero-state identity.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
    /// Returns the full latest masterchain block reported by this source.
    #[must_use]
    pub const fn last(&self) -> &Block {
        &self.last
    }
    /// Returns actual masterchain state-root hash.
    #[must_use]
    pub const fn state_root_hash(&self) -> Hash {
        self.state_root_hash
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkFields {
    network: Network,
    last: Block,
    state_root_hash: Hash,
}
impl TryFrom<NetworkFields> for NetworkData {
    type Error = Error;
    fn try_from(v: NetworkFields) -> Result<Self, Error> {
        Self::new(v.network, v.last, v.state_root_hash)
    }
}
