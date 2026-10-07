// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeMap};

use crate::domain::evm::{
    Address, BlockContext, BlockFee, BlockHash, FeeTerms, Quantity, Timestamp, TransactionCall,
    Word,
};
use crate::error::Error;

use super::{invalid_response, parse_quantity};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::chains::evm) struct FeeBlock {
    hash: String,
    number: String,
    timestamp: String,
    #[serde(default, deserialize_with = "present")]
    base_fee_per_gas: Option<String>,
}

fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}

impl FeeBlock {
    pub(in crate::chains::evm) fn into_domain(
        self,
        expected: BlockContext,
    ) -> Result<BlockFee, Error> {
        let hash = BlockHash::parse(&self.hash).map_err(|_| invalid_response())?;
        let number =
            u64::try_from(parse_quantity(&self.number)?).map_err(|_| invalid_response())?;
        let timestamp =
            u64::try_from(parse_quantity(&self.timestamp)?).map_err(|_| invalid_response())?;
        let actual = BlockContext::new(number, hash, Timestamp::from_unix_seconds(timestamp));
        if actual != expected {
            return Err(invalid_response());
        }
        Ok(BlockFee {
            block: actual,
            base_fee_per_gas: self
                .base_fee_per_gas
                .map(|v| parse_quantity(&v).map(Quantity::new))
                .transpose()?,
        })
    }
}

pub(in crate::chains::evm) struct Call<'a>(pub(in crate::chains::evm) &'a TransactionCall);

impl Serialize for Call<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let call = self.0.data();
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("chainId", &format!("0x{:x}", call.chain_id.value()))?;
        map.serialize_entry("from", &call.from)?;
        if let Some(to) = call.to {
            map.serialize_entry("to", &to)?;
        }
        map.serialize_entry("nonce", &format!("0x{:x}", call.nonce))?;
        map.serialize_entry("gas", &format!("0x{:x}", call.gas_limit))?;
        map.serialize_entry("value", &format!("0x{:x}", call.value.value()))?;
        map.serialize_entry("data", &call.input)?;
        map.serialize_entry("type", &format!("0x{:x}", call.fees.type_byte()))?;
        match &call.fees {
            FeeTerms::Legacy { gas_price } => {
                map.serialize_entry("gasPrice", &format!("0x{:x}", gas_price.value()))?;
            }
            FeeTerms::AccessList {
                gas_price,
                access_list,
            } => {
                map.serialize_entry("gasPrice", &format!("0x{:x}", gas_price.value()))?;
                map.serialize_entry("accessList", &access_entries(access_list))?;
            }
            FeeTerms::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => {
                map.serialize_entry("maxFeePerGas", &format!("0x{:x}", max_fee_per_gas.value()))?;
                map.serialize_entry(
                    "maxPriorityFeePerGas",
                    &format!("0x{:x}", max_priority_fee_per_gas.value()),
                )?;
                map.serialize_entry("accessList", &access_entries(access_list))?;
            }
        }
        map.end()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccessEntry<'a> {
    address: Address,
    storage_keys: &'a [Word],
}

fn access_entries(entries: &[crate::domain::evm::AccessListEntry]) -> Vec<AccessEntry<'_>> {
    entries
        .iter()
        .map(|entry| AccessEntry {
            address: entry.address,
            storage_keys: &entry.storage_keys,
        })
        .collect()
}
