// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    AssetAmount, AssetKind, Hash, Network, PaymentAddress, ProtocolParameters, TransactionCbor,
    Utxo,
};
use crate::{
    domain::U256,
    error::{Error, ValidationError},
    wallets::Preparation,
};
use pallas_codec::{
    minicbor,
    utils::{KeepRaw, NonEmptySet, Nullable, PositiveCoin, Set},
};
use pallas_primitives::{TransactionInput, conway};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

fn invalid() -> Error {
    ValidationError::InvalidCardanoPreparation.into()
}

/// Explicit ordinary-payment output, including caller-selected change outputs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PaymentOutputFields")]
pub struct PaymentOutput {
    address: PaymentAddress,
    assets: Vec<AssetAmount>,
}
impl PaymentOutput {
    /// Checks exact output values without inventing change, data or decimal precision.
    ///
    /// # Errors
    /// Rejects duplicates, missing ADA, zero token entries, width or item overflows.
    pub fn new(address: PaymentAddress, assets: Vec<AssetAmount>) -> Result<Self, Error> {
        if assets.len() > 256 {
            return Err(invalid());
        }
        let network = assets.first().ok_or_else(invalid)?.asset_id().network();
        if !address.is_compatible_with(network) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        super::value::validate_assets(&assets, network, true)?;
        Ok(Self { address, assets })
    }
    /// Returns exact destination address.
    #[must_use]
    pub const fn address(&self) -> &PaymentAddress {
        &self.address
    }
    /// Returns all exact attached quantities, including explicit change values.
    #[must_use]
    pub fn assets(&self) -> &[AssetAmount] {
        &self.assets
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaymentOutputFields {
    address: PaymentAddress,
    #[serde(deserialize_with = "output_assets")]
    assets: Vec<AssetAmount>,
}
fn output_assets<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<AssetAmount>, D::Error> {
    super::value::bounded_values::<D, AssetAmount, 256>(d)
}
impl TryFrom<PaymentOutputFields> for PaymentOutput {
    type Error = Error;
    fn try_from(v: PaymentOutputFields) -> Result<Self, Error> {
        Self::new(v.address, v.assets)
    }
}

/// Explicit Conway key-spend payment with immutable selected inputs and all outputs.
/// No automatic input selection, change calculation, signing or clock access occurs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PaymentIntentFields")]
pub struct PaymentIntent {
    network: Network,
    inputs: Vec<Utxo>,
    outputs: Vec<PaymentOutput>,
    fee: u64,
    invalid_before: Option<u64>,
    invalid_hereafter: u64,
    witness_count: u8,
}
impl PaymentIntent {
    /// Validates ordinary key-spend inputs, exact asset conservation and caller policy.
    ///
    /// # Errors
    /// Rejects duplicate/mismatched inputs, unsupported script/Byron spend inputs,
    /// datum/reference-script inputs, more than 100 inputs/outputs/witnesses,
    /// insufficient witness count, unbalanced values or invalid slot intervals.
    pub fn new(
        network: Network,
        inputs: Vec<Utxo>,
        outputs: Vec<PaymentOutput>,
        fee: u64,
        invalid_before: Option<u64>,
        invalid_hereafter: u64,
        witness_count: u8,
    ) -> Result<Self, Error> {
        if inputs.is_empty()
            || inputs.len() > 100
            || outputs.is_empty()
            || outputs.len() > 100
            || !(1..=100).contains(&witness_count)
            || invalid_hereafter == 0
            || invalid_before.is_some_and(|start| start >= invalid_hereafter)
        {
            return Err(invalid());
        }
        let mut points = HashSet::new();
        let mut keys = HashSet::new();
        let mut input_values = BTreeMap::new();
        let mut output_values = BTreeMap::new();
        for input in &inputs {
            if input.network().identity() != network.identity()
                || !points.insert((input.transaction_id(), input.output_index()))
            {
                return Err(invalid());
            }
            if input.datum_hash().is_some()
                || input.inline_datum().is_some()
                || input.reference_script_hash().is_some()
            {
                return Err(Error::UnsupportedCapability);
            }
            keys.insert(payment_key(input.address())?);
            sum_assets(&mut input_values, input.assets())?;
        }
        if keys.len() > usize::from(witness_count) {
            return Err(invalid());
        }
        for output in &outputs {
            if !output.address.is_compatible_with(network.identity())
                || output
                    .assets
                    .iter()
                    .any(|a| a.asset_id().network() != network.identity())
            {
                return Err(ValidationError::NetworkMismatch.into());
            }
            sum_assets(&mut output_values, &output.assets)?;
        }
        let native = output_values.entry("lovelace".into()).or_default();
        *native = native.checked_add(U256::from(fee)).ok_or_else(invalid)?;
        if input_values != output_values {
            return Err(invalid());
        }
        Ok(Self {
            network,
            inputs,
            outputs,
            fee,
            invalid_before,
            invalid_hereafter,
            witness_count,
        })
    }
    /// Returns the exact caller-qualified tag/magic network and alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns every explicitly selected source output; current unspentness is not proved.
    #[must_use]
    pub fn inputs(&self) -> &[Utxo] {
        &self.inputs
    }
    /// Returns every explicitly supplied payment/change output in review order.
    #[must_use]
    pub fn outputs(&self) -> &[PaymentOutput] {
        &self.outputs
    }
    /// Returns the exact caller-selected fee in lovelaces.
    #[must_use]
    pub const fn fee(&self) -> u64 {
        self.fee
    }
    /// Returns the optional inclusive validity-start slot.
    #[must_use]
    pub const fn invalid_before(&self) -> Option<u64> {
        self.invalid_before
    }
    /// Returns the mandatory exclusive validity-end slot; no TTL is invented.
    #[must_use]
    pub const fn invalid_hereafter(&self) -> u64 {
        self.invalid_hereafter
    }
    /// Returns the caller's exact distinct key-witness size profile.
    #[must_use]
    pub const fn witness_count(&self) -> u8 {
        self.witness_count
    }
}
fn payment_key(address: &PaymentAddress) -> Result<[u8; 28], Error> {
    let bytes = address.bytes();
    if !matches!(bytes[0] >> 4, 0 | 2 | 4 | 6) || bytes.len() < 29 {
        return Err(Error::UnsupportedCapability);
    }
    bytes[1..29].try_into().map_err(|_| invalid())
}
fn sum_assets(totals: &mut BTreeMap<String, U256>, assets: &[AssetAmount]) -> Result<(), Error> {
    for asset in assets {
        let total = totals
            .entry(super::indexed::asset_unit(asset.asset_id()))
            .or_default();
        *total = total
            .checked_add(asset.amount().raw())
            .ok_or_else(invalid)?;
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaymentIntentFields {
    network: Network,
    #[serde(deserialize_with = "inputs")]
    inputs: Vec<Utxo>,
    #[serde(deserialize_with = "outputs")]
    outputs: Vec<PaymentOutput>,
    fee: u64,
    invalid_before: Option<u64>,
    invalid_hereafter: u64,
    witness_count: u8,
}
fn inputs<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Utxo>, D::Error> {
    super::value::bounded_values::<D, Utxo, 100>(d)
}
fn outputs<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<PaymentOutput>, D::Error> {
    super::value::bounded_values::<D, PaymentOutput, 100>(d)
}
impl TryFrom<PaymentIntentFields> for PaymentIntent {
    type Error = Error;
    fn try_from(v: PaymentIntentFields) -> Result<Self, Error> {
        Self::new(
            v.network,
            v.inputs,
            v.outputs,
            v.fee,
            v.invalid_before,
            v.invalid_hereafter,
            v.witness_count,
        )
    }
}

/// Exact local ordinary-payment fee/output calculation for explicit indexed parameters.
/// Dummy witnesses determine serialization size only and are never signing evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PaymentEstimateFields")]
pub struct PaymentEstimate {
    intent: PaymentIntent,
    parameters: ProtocolParameters,
    minimum_fee: u64,
    signed_size_bytes: u32,
    output_minimum_lovelaces: Vec<u64>,
}
impl PaymentEstimate {
    /// Encodes the explicit body/witness-size profile and checks ordinary output limits.
    /// It reports the minimum fee without silently changing fee or change outputs.
    ///
    /// # Errors
    /// Rejects wrong-network/unsupported parameters, size/value/minimum-ADA violations
    /// or checked arithmetic overflow. Caller must build a fresh review after adjustment.
    pub fn new(intent: PaymentIntent, parameters: ProtocolParameters) -> Result<Self, Error> {
        if intent.network.identity() != parameters.network().identity() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let p = parameters.data();
        if !matches!(p.protocol_major, 9 | 10) {
            return Err(Error::UnsupportedCapability);
        }
        let cost = p.coins_per_utxo_size.ok_or(Error::UnsupportedCapability)?;
        let max_value = p.max_value_bytes.ok_or(Error::UnsupportedCapability)?;
        let body = body(&intent)?;
        let mut minimum = Vec::with_capacity(body.outputs.len());
        for (output, reviewed) in body.outputs.iter().zip(&intent.outputs) {
            let encoded = minicbor::to_vec(output).map_err(|_| invalid())?;
            let conway::TransactionOutput::PostAlonzo(out) = output else {
                return Err(invalid());
            };
            let value = minicbor::to_vec(&out.value).map_err(|_| invalid())?;
            if value.len() > usize::try_from(max_value).map_err(|_| invalid())? {
                return Err(invalid());
            }
            let bytes = u64::try_from(encoded.len())
                .map_err(|_| invalid())?
                .checked_add(160)
                .ok_or_else(invalid)?;
            let required = cost.checked_mul(bytes).ok_or_else(invalid)?;
            if native_coin(reviewed.assets())? < required {
                return Err(invalid());
            }
            minimum.push(required);
        }
        let witnesses = (0..intent.witness_count)
            .map(|index| conway::VKeyWitness {
                vkey: vec![index; 32].into(),
                signature: vec![0; 64].into(),
            })
            .collect::<Vec<_>>();
        let signed = encoded_tx(
            body,
            Some(NonEmptySet::try_from(witnesses).map_err(|_| invalid())?),
        )?;
        let signed_size_bytes = u32::try_from(signed.len()).map_err(|_| invalid())?;
        if signed.len() > TransactionCbor::MAX_BYTES || signed_size_bytes > p.max_transaction_bytes
        {
            return Err(invalid());
        }
        let minimum_fee = p
            .min_fee_coefficient
            .checked_mul(u64::from(signed_size_bytes))
            .and_then(|fee| fee.checked_add(p.min_fee_constant))
            .ok_or_else(invalid)?;
        Ok(Self {
            intent,
            parameters,
            minimum_fee,
            signed_size_bytes,
            output_minimum_lovelaces: minimum,
        })
    }
    /// Returns the immutable original proposal, including its caller-selected fee.
    #[must_use]
    pub const fn intent(&self) -> &PaymentIntent {
        &self.intent
    }
    /// Returns exact supplied source parameter facts and their source epoch.
    #[must_use]
    pub const fn parameters(&self) -> &ProtocolParameters {
        &self.parameters
    }
    /// Returns calculated minimum ordinary fee in lovelaces for this exact profile.
    #[must_use]
    pub const fn minimum_fee(&self) -> u64 {
        self.minimum_fee
    }
    /// Returns full serialized size with the explicit placeholder witness count.
    #[must_use]
    pub const fn signed_size_bytes(&self) -> u32 {
        self.signed_size_bytes
    }
    /// Returns each output's exact byte-based minimum ADA requirement.
    #[must_use]
    pub fn output_minimum_lovelaces(&self) -> &[u64] {
        &self.output_minimum_lovelaces
    }
    /// Returns original expected network attribution.
    #[must_use]
    pub const fn network(&self) -> &Network {
        self.intent.network()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaymentEstimateFields {
    intent: PaymentIntent,
    parameters: ProtocolParameters,
    minimum_fee: u64,
    signed_size_bytes: u32,
    #[serde(deserialize_with = "minimum_outputs")]
    output_minimum_lovelaces: Vec<u64>,
}
fn minimum_outputs<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u64>, D::Error> {
    super::value::bounded_values::<D, u64, 100>(d)
}
impl TryFrom<PaymentEstimateFields> for PaymentEstimate {
    type Error = Error;
    fn try_from(v: PaymentEstimateFields) -> Result<Self, Error> {
        let e = Self::new(v.intent, v.parameters)?;
        if e.minimum_fee != v.minimum_fee
            || e.signed_size_bytes != v.signed_size_bytes
            || e.output_minimum_lovelaces != v.output_minimum_lovelaces
        {
            return Err(invalid());
        }
        Ok(e)
    }
}

/// Ordinary unsigned Conway transaction bound to explicit payment/parameter review.
/// The full transaction has an empty witness set; no signature or approval is supplied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PaymentPreparationFields")]
pub struct PaymentPreparation {
    estimate: PaymentEstimate,
    unsigned_transaction: TransactionCbor,
}
impl PaymentPreparation {
    /// Encodes a balanced supported payment after checking its explicit fee profile.
    ///
    /// # Errors
    /// Rejects insufficient fee or any unsupported/inconsistent estimate/preparation.
    pub fn new(intent: PaymentIntent, parameters: ProtocolParameters) -> Result<Self, Error> {
        let estimate = PaymentEstimate::new(intent, parameters)?;
        if estimate.intent.fee < estimate.minimum_fee {
            return Err(invalid());
        }
        let unsigned_transaction =
            TransactionCbor::from_bytes(encoded_tx(body(&estimate.intent)?, None)?)?;
        Ok(Self {
            estimate,
            unsigned_transaction,
        })
    }
    /// Returns exact local fee/size/output calculations used by this review.
    #[must_use]
    pub const fn estimate(&self) -> &PaymentEstimate {
        &self.estimate
    }
    /// Returns computed original unsigned-body transaction identity.
    #[must_use]
    pub const fn transaction_id(&self) -> Hash {
        self.unsigned_transaction.transaction_id()
    }
}
impl Preparation for PaymentPreparation {
    type Network = Network;
    type Intent = PaymentIntent;
    type UnsignedPayload = TransactionCbor;
    fn network(&self) -> &Network {
        self.estimate.intent.network()
    }
    fn intent(&self) -> &PaymentIntent {
        &self.estimate.intent
    }
    fn unsigned_payload(&self) -> &TransactionCbor {
        &self.unsigned_transaction
    }
    fn validate(&self) -> Result<(), Error> {
        if Self::new(
            self.estimate.intent.clone(),
            self.estimate.parameters.clone(),
        )? != *self
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaymentPreparationFields {
    estimate: PaymentEstimate,
    unsigned_transaction: TransactionCbor,
}
impl TryFrom<PaymentPreparationFields> for PaymentPreparation {
    type Error = Error;
    fn try_from(v: PaymentPreparationFields) -> Result<Self, Error> {
        let p = Self::new(v.estimate.intent, v.estimate.parameters)?;
        if p.unsigned_transaction != v.unsigned_transaction {
            return Err(invalid());
        }
        Ok(p)
    }
}

fn native_coin(assets: &[AssetAmount]) -> Result<u64, Error> {
    let asset = assets
        .iter()
        .find(|a| matches!(a.asset_id().asset(), AssetKind::Native))
        .ok_or_else(invalid)?;
    u64::try_from(asset.amount().raw()).map_err(|_| invalid())
}
fn value(assets: &[AssetAmount]) -> Result<conway::Value, Error> {
    let coin = native_coin(assets)?;
    let mut tokens = BTreeMap::new();
    for asset in assets {
        if let AssetKind::Token {
            policy_id,
            asset_name,
        } = asset.asset_id().asset()
        {
            let raw = u64::try_from(asset.amount().raw()).map_err(|_| invalid())?;
            let quantity = PositiveCoin::try_from(raw).map_err(|_| invalid())?;
            tokens
                .entry(pallas_crypto::hash::Hash::<28>::from(policy_id.bytes()))
                .or_insert_with(BTreeMap::new)
                .insert(asset_name.bytes().to_vec().into(), quantity);
        }
    }
    Ok(if tokens.is_empty() {
        conway::Value::Coin(coin)
    } else {
        conway::Value::Multiasset(coin, tokens)
    })
}
fn body(intent: &PaymentIntent) -> Result<conway::TransactionBody<'static>, Error> {
    let mut inputs = intent
        .inputs
        .iter()
        .map(|input| TransactionInput {
            transaction_id: pallas_crypto::hash::Hash::from(input.transaction_id().bytes()),
            index: u64::from(input.output_index()),
        })
        .collect::<Vec<_>>();
    inputs.sort_by_key(|input| (input.transaction_id, input.index));
    let outputs = intent
        .outputs
        .iter()
        .map(|out| {
            Ok(conway::TransactionOutput::PostAlonzo(
                conway::PostAlonzoTransactionOutput {
                    address: out.address.bytes().to_vec().into(),
                    value: value(&out.assets)?,
                    datum_option: None,
                    script_ref: None,
                }
                .into(),
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(conway::TransactionBody {
        inputs: Set::from(inputs),
        outputs,
        fee: intent.fee,
        ttl: Some(intent.invalid_hereafter),
        certificates: None,
        withdrawals: None,
        auxiliary_data_hash: None,
        validity_interval_start: intent.invalid_before,
        mint: None,
        script_data_hash: None,
        collateral: None,
        required_signers: None,
        network_id: Some(if intent.network.identity().network_tag() == 1 {
            pallas_primitives::NetworkId::Mainnet
        } else {
            pallas_primitives::NetworkId::Testnet
        }),
        collateral_return: None,
        total_collateral: None,
        reference_inputs: None,
        voting_procedures: None,
        proposal_procedures: None,
        treasury_value: None,
        donation: None,
    })
}
fn encoded_tx(
    body: conway::TransactionBody<'static>,
    vkeywitness: Option<NonEmptySet<conway::VKeyWitness>>,
) -> Result<Vec<u8>, Error> {
    let tx = conway::Tx {
        transaction_body: KeepRaw::from(body),
        transaction_witness_set: KeepRaw::from(conway::WitnessSet {
            vkeywitness,
            native_script: None,
            bootstrap_witness: None,
            plutus_v1_script: None,
            plutus_data: None,
            redeemer: None,
            plutus_v2_script: None,
            plutus_v3_script: None,
        }),
        success: true,
        auxiliary_data: Nullable::Null,
    };
    minicbor::to_vec(tx).map_err(|_| invalid())
}

/// Caller-asserted signed ordinary transaction with exact reviewed-body binding.
/// Witness byte shape/key hashes are checked; cryptographic signatures/approval are not.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SignedSubmissionFields")]
pub struct SignedSubmission {
    preparation: PaymentPreparation,
    transaction: TransactionCbor,
}
impl SignedSubmission {
    /// Checks unchanged reviewed body, supported witnesses and actual signed size/fee.
    ///
    /// # Errors
    /// Rejects changed body, unsupported scripts/auxiliary data, malformed or duplicate
    /// key witnesses, missing input key identities, incorrect count or fee/size overflow.
    pub fn new(
        preparation: PaymentPreparation,
        transaction: TransactionCbor,
    ) -> Result<Self, Error> {
        preparation.validate()?;
        if transaction.body_bytes() != preparation.unsigned_transaction.body_bytes()
            || transaction.encoded_script_validity() != Some(true)
        {
            return Err(invalid());
        }
        if !transaction.ordinary_witnesses() {
            return Err(Error::UnsupportedCapability);
        }
        let tx: conway::Tx<'_> = minicbor::decode(transaction.bytes()).map_err(|_| invalid())?;
        let witnesses = &tx.transaction_witness_set;
        if witnesses.native_script.is_some()
            || witnesses.bootstrap_witness.is_some()
            || witnesses.plutus_v1_script.is_some()
            || witnesses.plutus_data.is_some()
            || witnesses.redeemer.is_some()
            || witnesses.plutus_v2_script.is_some()
            || witnesses.plutus_v3_script.is_some()
            || !matches!(tx.auxiliary_data, Nullable::Null)
        {
            return Err(Error::UnsupportedCapability);
        }
        let keys = witnesses.vkeywitness.as_ref().ok_or_else(invalid)?;
        if keys.len() != usize::from(preparation.estimate.intent.witness_count) {
            return Err(invalid());
        }
        let mut hashes = HashSet::new();
        for key in keys {
            if key.vkey.len() != 32
                || key.signature.len() != 64
                || !hashes.insert(*pallas_crypto::hash::Hasher::<224>::hash(&key.vkey))
            {
                return Err(invalid());
            }
        }
        for input in &preparation.estimate.intent.inputs {
            if !hashes.contains(&payment_key(input.address())?) {
                return Err(invalid());
            }
        }
        let p = preparation.estimate.parameters.data();
        let len = u64::try_from(transaction.bytes().len()).map_err(|_| invalid())?;
        let fee = p
            .min_fee_coefficient
            .checked_mul(len)
            .and_then(|v| v.checked_add(p.min_fee_constant))
            .ok_or_else(invalid)?;
        if len > u64::from(p.max_transaction_bytes) || preparation.estimate.intent.fee < fee {
            return Err(invalid());
        }
        Ok(Self {
            preparation,
            transaction,
        })
    }
    /// Returns original immutable ordinary-payment review.
    #[must_use]
    pub const fn preparation(&self) -> &PaymentPreparation {
        &self.preparation
    }
    /// Returns actual bounded externally supplied transaction bytes.
    #[must_use]
    pub const fn transaction(&self) -> &TransactionCbor {
        &self.transaction
    }
    /// Returns the expected caller-qualified network for genesis verification.
    #[must_use]
    pub const fn network(&self) -> &Network {
        self.preparation.estimate.intent.network()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedSubmissionFields {
    preparation: PaymentPreparation,
    transaction: TransactionCbor,
}
impl TryFrom<SignedSubmissionFields> for SignedSubmission {
    type Error = Error;
    fn try_from(v: SignedSubmissionFields) -> Result<Self, Error> {
        Self::new(v.preparation, v.transaction)
    }
}

/// Exact provider acknowledgement of a submitted body hash, separate from execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SubmissionResultFields")]
pub struct SubmissionResult {
    submission: SignedSubmission,
    acknowledged_transaction: Hash,
}
impl SubmissionResult {
    /// Correlates actual acknowledgement with the submitted original body identity.
    ///
    /// # Errors
    /// Rejects a source acknowledgement for a different transaction.
    pub fn new(
        submission: SignedSubmission,
        acknowledged_transaction: Hash,
    ) -> Result<Self, Error> {
        if submission.transaction.transaction_id() != acknowledged_transaction {
            return Err(invalid());
        }
        Ok(Self {
            submission,
            acknowledged_transaction,
        })
    }
    /// Returns the immutable exact submitted review and payload.
    #[must_use]
    pub const fn submission(&self) -> &SignedSubmission {
        &self.submission
    }
    /// Returns acknowledged body identity, not a success/finality claim.
    #[must_use]
    pub const fn acknowledged_transaction(&self) -> Hash {
        self.acknowledged_transaction
    }
    /// Returns original expected network attribution.
    #[must_use]
    pub const fn network(&self) -> &Network {
        self.submission.network()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmissionResultFields {
    submission: SignedSubmission,
    acknowledged_transaction: Hash,
}
impl TryFrom<SubmissionResultFields> for SubmissionResult {
    type Error = Error;
    fn try_from(v: SubmissionResultFields) -> Result<Self, Error> {
        Self::new(v.submission, v.acknowledged_transaction)
    }
}
