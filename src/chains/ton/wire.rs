// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::{
        Amount,
        ton::{
            AccountBalance, AccountBalanceData, AccountState, Address, Block, Boc, Currency,
            Cursor, ExtraCurrency, FeePart, Hash, HistoryPage, HistoryRequest, LogicalTime,
            Nanotons, Network, NetworkData, ProviderFees, SourceMessageFees, Transaction,
            ZeroState,
        },
    },
    error::{Error, ProviderError},
};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::value::RawValue;

pub(super) const MAX_BODY: usize = 2 * 1024 * 1024;
pub(super) fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
#[derive(Deserialize)]
struct Envelope {
    ok: bool,
    result: Option<Box<RawValue>>,
}
pub(super) fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T, Error> {
    let envelope: Envelope = serde_json::from_slice(body).map_err(|_| invalid())?;
    if !envelope.ok {
        return Err(Error::Provider(ProviderError::Rpc));
    }
    let result = envelope.result.ok_or_else(invalid)?;
    serde_json::from_str(result.get()).map_err(|_| invalid())
}
#[derive(Deserialize)]
pub(super) struct BlockWire {
    workchain: i32,
    shard: String,
    seqno: u32,
    root_hash: String,
    file_hash: String,
}
impl BlockWire {
    fn into_domain(self) -> Result<Block, Error> {
        Block::new(
            self.workchain,
            &self.shard,
            self.seqno,
            Hash::parse(&self.root_hash).map_err(|_| invalid())?,
            Hash::parse(&self.file_hash).map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
struct ZeroWire {
    workchain: i32,
    root_hash: String,
    file_hash: String,
}
#[derive(Deserialize)]
pub(super) struct Master {
    last: BlockWire,
    state_root_hash: String,
    init: ZeroWire,
}
impl Master {
    pub(super) fn into_domain(self, network: Network) -> Result<NetworkData, Error> {
        let zero = ZeroState::new(
            self.init.workchain,
            Hash::parse(&self.init.root_hash).map_err(|_| invalid())?,
            Hash::parse(&self.init.file_hash).map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?;
        if zero != network.zero_state() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        NetworkData::new(
            network,
            self.last.into_domain()?,
            Hash::parse(&self.state_root_hash).map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
struct CursorWire {
    lt: String,
    hash: String,
}
impl CursorWire {
    fn optional(self) -> Result<Option<Cursor>, Error> {
        let hash = Hash::parse(&self.hash).map_err(|_| invalid())?;
        if self.lt == "0" && hash == Hash::ZERO {
            return Ok(None);
        }
        Cursor::new(LogicalTime::parse(&self.lt).map_err(|_| invalid())?, hash)
            .map(Some)
            .map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
struct ExtraWire {
    id: i32,
    amount: String,
}
#[derive(Deserialize)]
pub(super) struct Account {
    balance: String,
    extra_currencies: Vec<ExtraWire>,
    last_transaction_id: CursorWire,
    block_id: BlockWire,
    code: String,
    data: String,
    frozen_hash: String,
    sync_utime: u64,
    state: String,
    suspended: Option<bool>,
}
impl Account {
    pub(super) fn into_domain(
        self,
        address: Address,
        selected: &Block,
    ) -> Result<AccountBalance, Error> {
        if &self.block_id.into_domain()? != selected {
            return Err(invalid());
        }
        let extra = self
            .extra_currencies
            .into_iter()
            .map(|v| {
                Ok(ExtraCurrency {
                    id: u32::from_be_bytes(v.id.to_be_bytes()),
                    amount: Amount::from_decimal(&v.amount, None).map_err(|_| invalid())?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let state = match self.state.as_str() {
            "active" => AccountState::Active,
            "frozen" => AccountState::Frozen,
            "uninitialized" => AccountState::Uninitialized,
            "nonexist" => AccountState::Nonexistent,
            _ => return Err(invalid()),
        };
        AccountBalance::new(AccountBalanceData {
            address,
            balance: Currency::new(
                Nanotons::parse(&self.balance).map_err(|_| invalid())?,
                extra,
            )
            .map_err(|_| invalid())?,
            state,
            last_transaction: self.last_transaction_id.optional()?,
            code: optional_boc(&self.code)?,
            data: optional_boc(&self.data)?,
            frozen_hash: if self.frozen_hash.is_empty() {
                None
            } else {
                Some(Hash::parse(&self.frozen_hash).map_err(|_| invalid())?)
            },
            suspended: self.suspended,
            sync_unix_seconds: self.sync_utime,
        })
        .map_err(|_| invalid())
    }
}
fn optional_boc(text: &str) -> Result<Option<Boc>, Error> {
    if text.is_empty() {
        Ok(None)
    } else {
        Boc::parse(text).map(Some).map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
struct Echo {
    #[serde(default)]
    hash: String,
}
#[derive(Deserialize)]
struct OutgoingEcho {
    hash: String,
    fwd_fee: String,
    ihr_fee: String,
}
#[derive(Deserialize)]
pub(super) struct TransactionWire {
    account: String,
    utime: u32,
    data: String,
    transaction_id: CursorWire,
    fee: String,
    storage_fee: String,
    other_fee: String,
    in_msg: Option<Echo>,
    out_msgs: Vec<OutgoingEcho>,
}
impl TransactionWire {
    fn into_domain(self, address: Address) -> Result<Transaction, Error> {
        let echoed = Address::parse(&self.account).map_err(|_| invalid())?;
        if !echoed.same_account(address) {
            return Err(invalid());
        }
        let tx = Transaction::decode(address, Boc::parse(&self.data).map_err(|_| invalid())?)
            .map_err(|_| invalid())?;
        let fee = Nanotons::parse(&self.fee).map_err(|_| invalid())?;
        let storage = Nanotons::parse(&self.storage_fee).map_err(|_| invalid())?;
        let other = Nanotons::parse(&self.other_fee).map_err(|_| invalid())?;
        if self.transaction_id.optional()? != Some(tx.cursor())
            || self.utime != tx.created_unix_seconds()
            || storage.raw().checked_add(other.raw()) != Some(fee.raw())
            || self.out_msgs.len() != tx.outgoing().len()
        {
            return Err(invalid());
        }
        let incoming = self
            .in_msg
            .and_then(|v| {
                if v.hash.is_empty() {
                    None
                } else {
                    Some(v.hash)
                }
            })
            .map(|v| Hash::parse(&v).map_err(|_| invalid()))
            .transpose()?;
        if tx
            .incoming()
            .map(crate::domain::ton::Message::hash)
            .transpose()
            .map_err(|_| invalid())?
            != incoming
        {
            return Err(invalid());
        }
        let outgoing = self
            .out_msgs
            .into_iter()
            .map(|echo| {
                Ok(SourceMessageFees {
                    message_hash: Hash::parse(&echo.hash).map_err(|_| invalid())?,
                    forwarding_fee: Nanotons::parse(&echo.fwd_fee).map_err(|_| invalid())?,
                    source_ihr_fee: Nanotons::parse(&echo.ihr_fee).map_err(|_| invalid())?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        tx.with_provider_fees(
            ProviderFees::new(fee, storage, other, outgoing).map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())
    }
}
pub(super) fn page(
    rows: Vec<TransactionWire>,
    request: HistoryRequest,
) -> Result<HistoryPage, Error> {
    if rows.len() > usize::from(request.limit()) {
        return Err(invalid());
    }
    let txs = rows
        .into_iter()
        .map(|r| r.into_domain(request.address()))
        .collect::<Result<Vec<_>, _>>()?;
    HistoryPage::new(request, txs).map_err(|_| invalid())
}
#[derive(Deserialize)]
pub(super) struct Fees {
    source_fees: FeeWire,
    destination_fees: Vec<FeeWire>,
}
#[derive(Deserialize)]
struct FeeWire {
    #[serde(rename = "in_fwd_fee")]
    incoming: NumericNano,
    #[serde(rename = "storage_fee")]
    storage: NumericNano,
    #[serde(rename = "gas_fee")]
    gas: NumericNano,
    #[serde(rename = "fwd_fee")]
    forwarding: NumericNano,
}
impl Fees {
    pub(super) fn into_parts(self) -> Result<(FeePart, Vec<FeePart>), Error> {
        if self.destination_fees.len() > 256 {
            return Err(invalid());
        }
        Ok((
            self.source_fees.into_domain(),
            self.destination_fees
                .into_iter()
                .map(FeeWire::into_domain)
                .collect(),
        ))
    }
}
impl FeeWire {
    fn into_domain(self) -> FeePart {
        FeePart {
            incoming_forwarding_fee: self.incoming.0,
            storage_fee: self.storage.0,
            gas_fee: self.gas.0,
            forwarding_fee: self.forwarding.0,
        }
    }
}
struct NumericNano(Nanotons);
impl<'de> Deserialize<'de> for NumericNano {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = Box::<RawValue>::deserialize(d)?;
        let value = if raw.get().starts_with('"') {
            serde_json::from_str::<String>(raw.get()).map_err(serde::de::Error::custom)?
        } else {
            raw.get().to_owned()
        };
        Nanotons::parse(&value)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}
#[derive(Deserialize)]
pub(super) struct SendResult {
    pub(super) hash: String,
}
