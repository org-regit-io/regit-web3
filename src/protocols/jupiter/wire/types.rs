// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{bounded::Bounded, invalid};
use crate::{
    domain::{
        ExactDecimal,
        jupiter::{
            AccountMeta, Instruction, InstructionData, InstructionFields, Label, LookupTable,
            RouteStep, RouteStepData,
        },
        solana::Pubkey,
    },
    error::Error,
};
use serde::{
    Deserialize, Deserializer,
    de::{IgnoredAny, MapAccess, Visitor},
};
use serde_json::value::RawValue;
use std::fmt;
#[derive(Deserialize)]
pub(super) struct Units(#[serde(deserialize_with = "units")] pub(super) u64);
fn units<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let s = String::deserialize(d)?;
    if s.is_empty()
        || s.len() > 20
        || s.bytes().any(|b| !b.is_ascii_digit())
        || s.len() > 1 && s.starts_with('0')
    {
        return Err(serde::de::Error::custom("invalid source units"));
    }
    s.parse()
        .map_err(|_| serde::de::Error::custom("invalid source units"))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Leg {
    swap_info: SwapInfo,
    percent: Box<RawValue>,
    bps: u16,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwapInfo {
    amm_key: Pubkey,
    label: Label,
    input_mint: Pubkey,
    output_mint: Pubkey,
    in_amount: Units,
    out_amount: Units,
}
impl Leg {
    pub(super) fn typed(self) -> Result<RouteStep, Error> {
        let percent = self.percent.get();
        if percent.starts_with('"') || percent.starts_with('{') {
            return Err(invalid());
        }
        RouteStep::new(RouteStepData {
            amm: self.swap_info.amm_key,
            label: self.swap_info.label,
            input_mint: self.swap_info.input_mint,
            output_mint: self.swap_info.output_mint,
            in_amount: self.swap_info.in_amount.0,
            out_amount: self.swap_info.out_amount.0,
            percent: ExactDecimal::parse(percent).map_err(|_| invalid())?,
            bps: self.bps,
        })
        .map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstructionWire {
    program_id: Pubkey,
    accounts: Bounded<AccountWire, 256>,
    data: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountWire {
    pubkey: Pubkey,
    is_signer: bool,
    is_writable: bool,
}
impl InstructionWire {
    pub(super) fn typed(self) -> Result<Instruction, Error> {
        Instruction::new(InstructionFields {
            program_id: self.program_id,
            accounts: self
                .accounts
                .0
                .into_iter()
                .map(|a| AccountMeta {
                    pubkey: a.pubkey,
                    is_signer: a.is_signer,
                    is_writable: a.is_writable,
                })
                .collect(),
            data: InstructionData::parse(&self.data).map_err(|_| invalid())?,
        })
        .map_err(|_| invalid())
    }
}
pub(super) struct Tables(pub(super) Vec<LookupTable>);
impl<'de> Deserialize<'de> for Tables {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Map;
        impl<'de> Visitor<'de> for Map {
            type Value = Tables;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded lookup tables")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<Tables, M::Error> {
                let mut result = Vec::new();
                while let Some(key) = m.next_key::<Pubkey>()? {
                    if result.len() >= 64 || result.iter().any(|t: &LookupTable| t.address() == key)
                    {
                        return Err(serde::de::Error::custom("invalid source tables"));
                    }
                    let entries = m.next_value::<Bounded<Pubkey, 256>>()?.0;
                    result.push(LookupTable::new(key, entries).map_err(serde::de::Error::custom)?);
                }
                Ok(Tables(result))
            }
        }
        d.deserialize_map(Map)
    }
}
pub(super) fn null_only<'de, D: Deserializer<'de>>(d: D) -> Result<(), D::Error> {
    if Option::<IgnoredAny>::deserialize(d)?.is_some() {
        return Err(serde::de::Error::custom("unexpected source payload"));
    }
    Ok(())
}
