// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Synthetic ordinary payment vectors built by the maintained Cardano codec.

use pallas_codec::{
    minicbor,
    utils::{KeepRaw, NonEmptySet, Nullable},
};
use pallas_primitives::conway;
use regit_web3::{
    domain::{
        U256,
        cardano::{
            AssetAmount, AssetId, AssetName, Hash, Lovelace, Network, NetworkId, OutputData,
            PaymentAddress, PaymentIntent, PaymentOutput, PaymentParameters, PaymentPreparation,
            PolicyId, ProtocolParameters, SignedSubmission, TransactionCbor,
        },
    },
    error::Error,
    wallets::Preparation,
};

pub(crate) fn network() -> Result<Network, Error> {
    Network::new(NetworkId::mainnet(), "fixture")
}
pub(crate) fn address(key: u8) -> Result<PaymentAddress, Error> {
    let hash = pallas_crypto::hash::Hasher::<224>::hash(&[key; 32]);
    let mut bytes = vec![0x61];
    bytes.extend_from_slice(hash.as_ref());
    PaymentAddress::from_bytes(&bytes)
}
pub(crate) fn token() -> Result<AssetId, Error> {
    Ok(AssetId::token(
        NetworkId::mainnet(),
        PolicyId::from_bytes([3; 28]),
        AssetName::from_bytes(vec![0, 255])?,
    ))
}
pub(crate) fn assets(lovelaces: u64, tokens: u64) -> Result<Vec<AssetAmount>, Error> {
    Ok(vec![
        AssetAmount::new(
            AssetId::native(NetworkId::mainnet()),
            U256::from(lovelaces),
            Some(6),
        )?,
        AssetAmount::new(token()?, U256::from(tokens), None)?,
    ])
}
pub(crate) fn intent(fee: u64) -> Result<PaymentIntent, Error> {
    let input = regit_web3::domain::cardano::Utxo::new(
        network()?,
        address(1)?,
        Hash::from_bytes([4; 32]),
        0,
        assets(10_000_000, 7)?,
        Hash::from_bytes([5; 32]),
        OutputData::new(None, None, None),
    )?;
    PaymentIntent::new(
        network()?,
        vec![input],
        vec![PaymentOutput::new(
            address(2)?,
            assets(10_000_000 - fee, 7)?,
        )?],
        fee,
        Some(199_000_000),
        200_000_000,
        1,
    )
}
pub(crate) fn parameters() -> Result<ProtocolParameters, Error> {
    ProtocolParameters::new(
        network()?,
        PaymentParameters {
            epoch: 600,
            min_fee_coefficient: 44,
            min_fee_constant: 155_381,
            max_transaction_bytes: 16_384,
            max_value_bytes: Some(5000),
            coins_per_utxo_size: Some(4310),
            protocol_major: 10,
            protocol_minor: 0,
            key_deposit: Lovelace::parse("2000000")?,
            pool_deposit: Lovelace::parse("500000000")?,
            script_memory_price: None,
            script_step_price: None,
        },
    )
}
pub(crate) fn preparation() -> Result<PaymentPreparation, Error> {
    PaymentPreparation::new(intent(500_000)?, parameters()?)
}
pub(crate) fn signed_bytes(prep: &PaymentPreparation, key: u8) -> Result<Vec<u8>, Error> {
    let mut tx: conway::Tx<'_> =
        minicbor::decode(prep.unsigned_payload().bytes()).map_err(|_| Error::Configuration)?;
    tx.transaction_witness_set = KeepRaw::from(conway::WitnessSet {
        vkeywitness: Some(
            NonEmptySet::try_from(vec![conway::VKeyWitness {
                vkey: vec![key; 32].into(),
                signature: vec![0; 64].into(),
            }])
            .map_err(|_| Error::Configuration)?,
        ),
        native_script: None,
        bootstrap_witness: None,
        plutus_v1_script: None,
        plutus_data: None,
        redeemer: None,
        plutus_v2_script: None,
        plutus_v3_script: None,
    });
    tx.auxiliary_data = Nullable::Null;
    minicbor::to_vec(tx).map_err(|_| Error::Configuration)
}
pub(crate) fn signed() -> Result<SignedSubmission, Error> {
    let prep = preparation()?;
    let bytes = signed_bytes(&prep, 1)?;
    SignedSubmission::new(prep, TransactionCbor::from_bytes(bytes)?)
}
