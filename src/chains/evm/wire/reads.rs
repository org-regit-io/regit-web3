// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{invalid_response, parse_quantity};
use crate::{
    domain::evm::{
        AccessListEntry, Authorization, Data, ExecutionOutcome, Inclusion, Log, Quantity, Receipt,
        ReceiptData, SignatureFields, Transaction, TransactionData, TransactionId, TransactionKind,
        Word,
    },
    domain::{Address, BlockHash, ChainId, U256},
    error::Error,
};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy)]
pub(in crate::chains::evm) struct HexQuantity(U256);
impl<'de> Deserialize<'de> for HexQuantity {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        parse_quantity(&String::deserialize(d)?)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}
impl HexQuantity {
    fn quantity(self) -> Quantity {
        Quantity::new(self.0)
    }
    fn u64(self) -> Result<u64, Error> {
        u64::try_from(self.0).map_err(|_| invalid_response())
    }
    fn byte(self) -> Result<u8, Error> {
        u8::try_from(self.0).map_err(|_| invalid_response())
    }
    fn parity(self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid_response()),
        }
    }
}
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}
fn inclusion(
    hash: Option<BlockHash>,
    number: Option<HexQuantity>,
    index: Option<HexQuantity>,
) -> Result<Option<Inclusion>, Error> {
    match (hash, number, index) {
        (None, None, None) => Ok(None),
        (Some(h), Some(n), Some(i)) => Ok(Some(Inclusion::new(h, n.u64()?, i.u64()?))),
        _ => Err(invalid_response()),
    }
}

#[derive(Serialize)]
pub(in crate::chains::evm) struct Call {
    pub(in crate::chains::evm) to: Address,
    pub(in crate::chains::evm) data: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAccessList {
    address: Address,
    storage_keys: Vec<Word>,
}
impl WireAccessList {
    fn into_domain(self) -> AccessListEntry {
        AccessListEntry {
            address: self.address,
            storage_keys: self.storage_keys,
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAuthorization {
    chain_id: HexQuantity,
    address: Address,
    nonce: HexQuantity,
    y_parity: HexQuantity,
    r: HexQuantity,
    s: HexQuantity,
}
impl WireAuthorization {
    fn into_domain(self) -> Result<Authorization, Error> {
        Ok(Authorization {
            chain_id: ChainId::new(self.chain_id.0),
            address: self.address,
            nonce: self.nonce.quantity(),
            y_parity: self.y_parity.parity()?,
            r: self.r.quantity(),
            s: self.s.quantity(),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::chains::evm) struct WireTransaction {
    hash: TransactionId,
    from: Address,
    #[serde(deserialize_with = "nullable")]
    to: Option<Address>,
    nonce: HexQuantity,
    gas: HexQuantity,
    value: HexQuantity,
    input: Data,
    #[serde(default, rename = "type", deserialize_with = "present")]
    transaction_type: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    chain_id: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    gas_price: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    max_fee_per_gas: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    max_priority_fee_per_gas: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    max_fee_per_blob_gas: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    access_list: Option<Vec<WireAccessList>>,
    #[serde(default, deserialize_with = "present")]
    blob_versioned_hashes: Option<Vec<Word>>,
    #[serde(default, deserialize_with = "present")]
    authorization_list: Option<Vec<WireAuthorization>>,
    r: HexQuantity,
    s: HexQuantity,
    #[serde(default, deserialize_with = "present")]
    v: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    y_parity: Option<HexQuantity>,
    #[serde(deserialize_with = "nullable")]
    block_hash: Option<BlockHash>,
    #[serde(deserialize_with = "nullable")]
    block_number: Option<HexQuantity>,
    #[serde(deserialize_with = "nullable")]
    transaction_index: Option<HexQuantity>,
}
impl WireTransaction {
    pub(in crate::chains::evm) fn into_domain(
        self,
        chain: ChainId,
        query: TransactionId,
    ) -> Result<Transaction, Error> {
        if self.hash != query {
            return Err(invalid_response());
        }
        let kind = self
            .transaction_type
            .map(HexQuantity::byte)
            .transpose()?
            .unwrap_or(0);
        let gas_price = self.gas_price.map(HexQuantity::quantity);
        let max_fee = self.max_fee_per_gas.map(HexQuantity::quantity);
        let max_priority = self.max_priority_fee_per_gas.map(HexQuantity::quantity);
        let max_blob = self.max_fee_per_blob_gas.map(HexQuantity::quantity);
        let access = self.access_list.map(|list| {
            list.into_iter()
                .map(WireAccessList::into_domain)
                .collect::<Vec<_>>()
        });
        if kind < 2 && (max_fee.is_some() || max_priority.is_some() || max_blob.is_some())
            || kind != 3 && self.blob_versioned_hashes.is_some()
            || kind != 4 && self.authorization_list.is_some()
            || kind != 3 && max_blob.is_some()
            || kind == 0 && access.as_ref().is_some_and(|v| !v.is_empty())
        {
            return Err(invalid_response());
        }
        let transaction_kind = match kind {
            0 => TransactionKind::Legacy {
                gas_price: gas_price.ok_or_else(invalid_response)?,
            },
            1 => TransactionKind::AccessList {
                gas_price: gas_price.ok_or_else(invalid_response)?,
                access_list: access.ok_or_else(invalid_response)?,
            },
            2 => TransactionKind::DynamicFee {
                max_fee_per_gas: max_fee.ok_or_else(invalid_response)?,
                max_priority_fee_per_gas: max_priority.ok_or_else(invalid_response)?,
                access_list: access.ok_or_else(invalid_response)?,
            },
            3 => TransactionKind::Blob {
                max_fee_per_gas: max_fee.ok_or_else(invalid_response)?,
                max_priority_fee_per_gas: max_priority.ok_or_else(invalid_response)?,
                max_fee_per_blob_gas: max_blob.ok_or_else(invalid_response)?,
                access_list: access.ok_or_else(invalid_response)?,
                blob_versioned_hashes: self.blob_versioned_hashes.ok_or_else(invalid_response)?,
            },
            4 => TransactionKind::Authorization {
                max_fee_per_gas: max_fee.ok_or_else(invalid_response)?,
                max_priority_fee_per_gas: max_priority.ok_or_else(invalid_response)?,
                access_list: access.ok_or_else(invalid_response)?,
                authorization_list: self
                    .authorization_list
                    .ok_or_else(invalid_response)?
                    .into_iter()
                    .map(WireAuthorization::into_domain)
                    .collect::<Result<Vec<_>, _>>()?,
            },
            _ => return Err(Error::UnsupportedCapability),
        };
        let data = TransactionData {
            hash: self.hash,
            from: self.from,
            to: self.to,
            nonce: self.nonce.quantity(),
            gas_limit: self.gas.quantity(),
            value: self.value.quantity(),
            input: self.input,
            kind: transaction_kind,
            reported_chain_id: self.chain_id.map(|q| ChainId::new(q.0)),
            reported_gas_price: gas_price,
            signature: SignatureFields {
                r: self.r.quantity(),
                s: self.s.quantity(),
                v: self.v.map(HexQuantity::quantity),
                y_parity: self.y_parity.map(HexQuantity::parity).transpose()?,
            },
            inclusion: inclusion(self.block_hash, self.block_number, self.transaction_index)?,
        };
        Transaction::new(chain, data).map_err(|_| invalid_response())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireLog {
    transaction_hash: TransactionId,
    block_hash: BlockHash,
    block_number: HexQuantity,
    transaction_index: HexQuantity,
    log_index: HexQuantity,
    address: Address,
    data: Data,
    topics: Vec<Word>,
    #[serde(default, deserialize_with = "present")]
    removed: Option<bool>,
}
impl WireLog {
    fn into_domain(self) -> Result<Log, Error> {
        if self.removed == Some(true) {
            return Err(invalid_response());
        }
        Log::new(
            self.transaction_hash,
            Inclusion::new(
                self.block_hash,
                self.block_number.u64()?,
                self.transaction_index.u64()?,
            ),
            self.log_index.u64()?,
            self.address,
            self.data,
            self.topics,
        )
        .map_err(|_| invalid_response())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::chains::evm) struct WireReceipt {
    transaction_hash: TransactionId,
    block_hash: BlockHash,
    block_number: HexQuantity,
    transaction_index: HexQuantity,
    from: Address,
    #[serde(deserialize_with = "nullable")]
    to: Option<Address>,
    #[serde(deserialize_with = "nullable")]
    contract_address: Option<Address>,
    #[serde(default, rename = "type", deserialize_with = "present")]
    transaction_type: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    status: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    root: Option<Word>,
    gas_used: HexQuantity,
    cumulative_gas_used: HexQuantity,
    #[serde(default, deserialize_with = "present")]
    effective_gas_price: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    blob_gas_used: Option<HexQuantity>,
    #[serde(default, deserialize_with = "present")]
    blob_gas_price: Option<HexQuantity>,
    logs_bloom: Data,
    logs: Vec<WireLog>,
}
impl WireReceipt {
    pub(in crate::chains::evm) fn into_domain(
        self,
        chain: ChainId,
        query: TransactionId,
    ) -> Result<Receipt, Error> {
        if self.transaction_hash != query {
            return Err(invalid_response());
        }
        let execution = match (self.status, self.root) {
            (Some(status), None) => match status.byte()? {
                0 => ExecutionOutcome::Failed,
                1 => ExecutionOutcome::Succeeded,
                _ => return Err(invalid_response()),
            },
            (None, Some(root)) => ExecutionOutcome::PreByzantium(root),
            (None, None) => ExecutionOutcome::Unknown,
            (Some(_), Some(_)) => return Err(invalid_response()),
        };
        let data = ReceiptData {
            transaction_hash: self.transaction_hash,
            inclusion: Inclusion::new(
                self.block_hash,
                self.block_number.u64()?,
                self.transaction_index.u64()?,
            ),
            from: self.from,
            to: self.to,
            contract_address: self.contract_address,
            transaction_type: self.transaction_type.map(HexQuantity::byte).transpose()?,
            execution,
            gas_used: self.gas_used.quantity(),
            cumulative_gas_used: self.cumulative_gas_used.quantity(),
            effective_gas_price: self.effective_gas_price.map(HexQuantity::quantity),
            blob_gas_used: self.blob_gas_used.map(HexQuantity::quantity),
            blob_gas_price: self.blob_gas_price.map(HexQuantity::quantity),
            logs_bloom: self.logs_bloom,
            logs: self
                .logs
                .into_iter()
                .map(WireLog::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
        };
        Receipt::new(chain, data).map_err(|_| invalid_response())
    }
}
