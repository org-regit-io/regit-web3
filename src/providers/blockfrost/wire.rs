// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Private remote DTO decoding and Cardano mapping; no transport or runtime.

use serde::{Deserialize, Deserializer};

use crate::{
    domain::{
        Amount,
        cardano::{
            AddressBalance, AssetAmount, AssetId, AssetKind, Hash, HexData, Network, NetworkId,
            OutputData, PageRequest, PaymentAddress, ScriptHash, Utxo, UtxoPage,
        },
    },
    error::{Error, ProviderError},
};

pub(super) fn genesis(bytes: &[u8], network: NetworkId) -> Result<(), Error> {
    let value: Genesis = serde_json::from_slice(bytes).map_err(|_| invalid_response())?;
    if value.network_magic != network.network_magic() {
        return Err(Error::Provider(ProviderError::ChainMismatch));
    }
    Ok(())
}

pub(super) fn balance(
    bytes: &[u8],
    network: &Network,
    address: &PaymentAddress,
) -> Result<AddressBalance, Error> {
    let fields: AddressFields = serde_json::from_slice(bytes).map_err(|_| invalid_response())?;
    let returned = PaymentAddress::parse(&fields.address).map_err(|_| invalid_response())?;
    if &returned != address {
        return Err(invalid_response());
    }
    AddressBalance::new(
        network.clone(),
        returned,
        assets(fields.amount, network.identity())?,
    )
    .map_err(|_| invalid_response())
}

pub(super) fn utxos(
    bytes: &[u8],
    network: &Network,
    address: &PaymentAddress,
    page: PageRequest,
) -> Result<UtxoPage, Error> {
    let fields: Vec<UtxoFields> = serde_json::from_slice(bytes).map_err(|_| invalid_response())?;
    if fields.len() > usize::from(page.count()) {
        return Err(invalid_response());
    }
    let mut outputs = Vec::with_capacity(fields.len());
    for field in fields {
        let returned = PaymentAddress::parse(&field.address).map_err(|_| invalid_response())?;
        if &returned != address
            || field
                .tx_index
                .is_some_and(|index| index != field.output_index)
        {
            return Err(invalid_response());
        }
        let data = OutputData::new(
            field
                .data_hash
                .as_deref()
                .map(Hash::parse)
                .transpose()
                .map_err(|_| invalid_response())?,
            field
                .inline_datum
                .as_deref()
                .map(HexData::parse)
                .transpose()
                .map_err(|_| invalid_response())?,
            field
                .reference_script_hash
                .as_deref()
                .map(ScriptHash::parse)
                .transpose()
                .map_err(|_| invalid_response())?,
        );
        outputs.push(
            Utxo::new(
                network.clone(),
                returned,
                Hash::parse(&field.tx_hash).map_err(|_| invalid_response())?,
                field.output_index,
                assets(field.amount, network.identity())?,
                Hash::parse(&field.block).map_err(|_| invalid_response())?,
                data,
            )
            .map_err(|_| invalid_response())?,
        );
    }
    UtxoPage::new(network.clone(), address.clone(), page, outputs).map_err(|_| invalid_response())
}

fn assets(fields: Vec<UnitQuantity>, network: NetworkId) -> Result<Vec<AssetAmount>, Error> {
    if fields.len() > 10_000 {
        return Err(invalid_response());
    }
    fields
        .into_iter()
        .map(|field| {
            let identity =
                AssetId::from_unit(network, &field.unit).map_err(|_| invalid_response())?;
            let raw = Amount::from_decimal(&field.quantity, None)
                .map_err(|_| invalid_response())?
                .raw();
            let decimals = if matches!(identity.asset(), AssetKind::Native) {
                Some(6)
            } else {
                None
            };
            AssetAmount::new(identity, raw, decimals).map_err(|_| invalid_response())
        })
        .collect()
}

fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(deserializer)
}

#[derive(Deserialize)]
struct Genesis {
    network_magic: u32,
}
#[derive(Deserialize)]
struct UnitQuantity {
    unit: String,
    quantity: String,
}
#[derive(Deserialize)]
struct AddressFields {
    address: String,
    amount: Vec<UnitQuantity>,
}
#[derive(Deserialize)]
struct UtxoFields {
    address: String,
    tx_hash: String,
    #[serde(default)]
    tx_index: Option<u16>,
    output_index: u16,
    amount: Vec<UnitQuantity>,
    block: String,
    #[serde(deserialize_with = "required_option")]
    data_hash: Option<String>,
    #[serde(deserialize_with = "required_option")]
    inline_datum: Option<String>,
    #[serde(deserialize_with = "required_option")]
    reference_script_hash: Option<String>,
}
