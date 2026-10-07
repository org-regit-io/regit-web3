// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Instruction, Observation, SwapBuild};
use crate::{
    domain::solana::{BlockhashLifetime, Network, Pubkey, UnsignedMessage, UnsignedTransaction},
    error::{Error, ValidationError},
    wallets::Preparation,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use solana_address::Address;
use solana_instruction::{AccountMeta as SdkMeta, Instruction as SdkInstruction};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use std::fmt;

/// Caller-selected insertion point for source `otherInstructions`.
/// The source API does not establish an ordering for this group. This choice
/// stays in review even for an empty group; it is not semantic verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtherInstructionPlacement {
    /// Insert after compute-budget instructions and before source setup.
    BeforeSetup,
    /// Insert after source setup and before the source swap.
    BeforeSwap,
    /// Insert after the source swap and before source cleanup.
    BeforeCleanup,
    /// Insert after source cleanup.
    AfterCleanup,
}
/// Explicit caller resource and signer constraints for compiling the fresh source build.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationSettings {
    /// Explicit CU limit, from 1 through 1,400,000, without a simulation-derived margin.
    pub compute_unit_limit: u32,
    /// Maximum source CU price in micro-lamports; the source's exact price is preserved.
    pub maximum_compute_unit_price: u64,
    /// Mandatory caller insertion choice, without an inferred source ordering.
    pub other_instruction_placement: OtherInstructionPlacement,
    /// Additional explicitly permitted required signers beyond the declared taker/payer.
    /// This is a structural allow-list, not caller approval or signer ownership proof.
    pub additional_signers: Vec<Pubkey>,
}
/// Complete fresh review intent, including the exact newly returned source instructions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwapIntentData {
    /// Exact fresh Metis V0 source build and separately retained API provenance.
    pub build: Observation<SwapBuild>,
    /// Explicit local compilation constraints, never source-selected defaults.
    pub settings: PreparationSettings,
}
/// Immutable review snapshot; provider instructions/lookup facts remain semantically unverified.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SwapIntentData", into = "SwapIntentData")]
pub struct SwapIntent(SwapIntentData);
impl SwapIntent {
    /// Validates explicit resource ceilings and additional required signer identities.
    /// No funding, intent verification, signing, approval or submission occurs.
    /// # Errors
    /// Rejects zero/excess CU limit, source price above ceiling or excessive/duplicate signers.
    pub fn new(data: SwapIntentData) -> Result<Self, Error> {
        let build = data.build.value().data();
        let settings = &data.settings;
        if !(1..=1_400_000).contains(&settings.compute_unit_limit)
            || price(data.build.value())? > settings.maximum_compute_unit_price
            || settings.additional_signers.len() > 16
            || settings
                .additional_signers
                .iter()
                .enumerate()
                .any(|(i, s)| {
                    *s == build.request.data().payer
                        || *s == build.request.data().taker
                        || settings.additional_signers[..i].contains(s)
                })
        {
            return Err(invalid());
        }
        let fee = (u128::from(price(data.build.value())?)
            * u128::from(settings.compute_unit_limit))
        .div_ceil(1_000_000);
        u64::try_from(fee).map_err(|_| invalid())?;
        Ok(Self(data))
    }
    /// Returns all immutable source and caller facts requiring review.
    #[must_use]
    pub const fn data(&self) -> &SwapIntentData {
        &self.0
    }
}
impl fmt::Debug for SwapIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SwapIntent").finish_non_exhaustive()
    }
}
impl TryFrom<SwapIntentData> for SwapIntent {
    type Error = Error;
    fn try_from(v: SwapIntentData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<SwapIntent> for SwapIntentData {
    fn from(v: SwapIntent) -> Self {
        v.0
    }
}
/// Canonical maintained V0 transaction with only zero signature placeholders.
/// Exact source instruction/ALT compilation does not verify trade semantics,
/// lookup-table state, signer ownership, review binding or future execution.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct UnsignedSwap {
    transaction: UnsignedTransaction,
    lifetime: BlockhashLifetime,
}
impl UnsignedSwap {
    /// Returns exact immutable unsigned transaction/message bytes for external verification.
    #[must_use]
    pub const fn transaction(&self) -> &UnsignedTransaction {
        &self.transaction
    }
    /// Returns source blockhash and final-valid height, independent of slots/time.
    #[must_use]
    pub const fn lifetime(&self) -> BlockhashLifetime {
        self.lifetime
    }
}
impl fmt::Debug for UnsignedSwap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnsignedSwap").finish_non_exhaustive()
    }
}
/// Typed immutable Jupiter fresh-build wallet review/handoff preparation.
/// The trusted caller verifier must check actual signed content against this
/// network/intent/payload. Opaque source bytes are not semantic trade verification.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparedSwap {
    intent: SwapIntent,
    unsigned: UnsignedSwap,
}
impl PreparedSwap {
    /// Compiles the exact supplied fresh V0 instruction sequence using maintained SDK types.
    /// Adds only the explicit CU-limit instruction; source CU price stays exact.
    /// No signing, wallet approval, source refresh or submission occurs.
    /// # Errors
    /// Rejects malformed source budget instructions, unauthorized structural signers,
    /// excessive/noncanonical compilation or caller/resource contradictions.
    pub fn new(intent: SwapIntent) -> Result<Self, Error> {
        let data = intent.data();
        let build = data.build.value().data();
        let request = build.request.data();
        let source_price = price(data.build.value())?;
        let mut instructions = vec![budget_instruction(
            2,
            &data.settings.compute_unit_limit.to_le_bytes(),
        )?];
        instructions.push(budget_instruction(3, &source_price.to_le_bytes())?);
        for instruction in ordered(build, data.settings.other_instruction_placement) {
            if instruction.fields().program_id == budget_program()? {
                return Err(invalid());
            }
            if instruction.fields().accounts.iter().any(|m| {
                m.is_signer
                    && m.pubkey != request.taker
                    && m.pubkey != request.payer
                    && !data.settings.additional_signers.contains(&m.pubkey)
            }) {
                return Err(invalid());
            }
            instructions.push(sdk_instruction(instruction));
        }
        let tables: Vec<_> = build
            .lookup_tables
            .iter()
            .map(|table| AddressLookupTableAccount {
                key: address(table.address()),
                addresses: table.entries().iter().map(|p| address(*p)).collect(),
            })
            .collect();
        let message = v0::Message::try_compile(
            &address(request.payer),
            &instructions,
            &tables,
            solana_hash::Hash::new_from_array(build.blockhash.bytes()),
        )
        .map_err(|_| invalid())?;
        let message =
            UnsignedMessage::from_message(VersionedMessage::V0(message)).map_err(|_| invalid())?;
        let transaction = UnsignedTransaction::from_message(message).map_err(|_| invalid())?;
        let lifetime = BlockhashLifetime {
            blockhash: build.blockhash,
            last_valid_block_height: build.last_valid_block_height,
        };
        Ok(Self {
            intent,
            unsigned: UnsignedSwap {
                transaction,
                lifetime,
            },
        })
    }
    /// Returns the immutable fresh reviewed source/caller snapshot.
    #[must_use]
    pub const fn intent(&self) -> &SwapIntent {
        &self.intent
    }
    /// Returns canonical unsigned bytes and actual source block-height expiry.
    #[must_use]
    pub const fn unsigned(&self) -> &UnsignedSwap {
        &self.unsigned
    }
}
fn ordered(
    build: &super::SwapBuildData,
    placement: OtherInstructionPlacement,
) -> Vec<&Instruction> {
    let mut instructions = Vec::new();
    if placement == OtherInstructionPlacement::BeforeSetup {
        instructions.extend(&build.other);
    }
    instructions.extend(&build.setup);
    if placement == OtherInstructionPlacement::BeforeSwap {
        instructions.extend(&build.other);
    }
    instructions.push(&build.swap);
    if placement == OtherInstructionPlacement::BeforeCleanup {
        instructions.extend(&build.other);
    }
    instructions.extend(build.cleanup.iter());
    if placement == OtherInstructionPlacement::AfterCleanup {
        instructions.extend(&build.other);
    }
    instructions
}
impl Preparation for PreparedSwap {
    type Network = Network;
    type Intent = SwapIntent;
    type UnsignedPayload = UnsignedSwap;
    fn network(&self) -> &Network {
        &self
            .intent
            .data()
            .build
            .value()
            .data()
            .request
            .data()
            .network
    }
    fn intent(&self) -> &SwapIntent {
        &self.intent
    }
    fn unsigned_payload(&self) -> &UnsignedSwap {
        &self.unsigned
    }
    fn validate(&self) -> Result<(), Error> {
        if *self != Self::new(self.intent.clone())? {
            return Err(invalid());
        }
        Ok(())
    }
}
impl fmt::Debug for PreparedSwap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedSwap").finish_non_exhaustive()
    }
}
impl Serialize for PreparedSwap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.intent.serialize(s)
    }
}
impl<'de> Deserialize<'de> for PreparedSwap {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(SwapIntent::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
fn address(value: Pubkey) -> Address {
    Address::new_from_array(value.bytes())
}
fn sdk_instruction(value: &Instruction) -> SdkInstruction {
    let fields = value.fields();
    SdkInstruction {
        program_id: address(fields.program_id),
        accounts: fields
            .accounts
            .iter()
            .map(|m| SdkMeta {
                pubkey: address(m.pubkey),
                is_signer: m.is_signer,
                is_writable: m.is_writable,
            })
            .collect(),
        data: fields.data.bytes().to_vec(),
    }
}
fn budget_program() -> Result<Pubkey, Error> {
    Pubkey::parse("ComputeBudget111111111111111111111111111111")
}
// Only these two fixed instruction layouts are supported. Released maintained
// compute-budget-interface3.1.0 specifies tag2/u32LE and tag3/u64LE, with no accounts.
// Message/transaction encoding is exclusively the maintained SDK wrapper's job.
fn budget_instruction(tag: u8, body: &[u8]) -> Result<SdkInstruction, Error> {
    let mut data = vec![tag];
    data.extend(body);
    Ok(SdkInstruction {
        program_id: address(budget_program()?),
        accounts: vec![],
        data,
    })
}
pub(super) fn price(build: &SwapBuild) -> Result<u64, Error> {
    let instructions = &build.data().compute_budget;
    if instructions.len() != 1 {
        return Err(invalid());
    }
    let fields = instructions[0].fields();
    let data = fields.data.bytes();
    if fields.program_id != budget_program()?
        || !fields.accounts.is_empty()
        || data.len() != 9
        || data[0] != 3
    {
        return Err(invalid());
    }
    let value = u64::from_le_bytes(data[1..].try_into().map_err(|_| invalid())?);
    if budget_instruction(3, &value.to_le_bytes())?.data != data {
        return Err(invalid());
    }
    Ok(value)
}
fn invalid() -> Error {
    ValidationError::InvalidJupiterPreparation.into()
}
