// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::invalid_response;
use crate::{
    domain::solana::{
        BlockhashLifetime, Commitment, CompiledInnerInstruction, ExecutionBytes, ExecutionOutcome,
        Hash, InnerInstructions, LoadedAddresses, LogMessage, Pubkey, ReturnData, Reward,
        RewardKind, SignatureStatus, SignedTransaction, Simulation, StatusOptions, SubmitOptions,
        TokenBalanceRecord, Transaction, TransactionMetadata, TransactionMetadataData,
        TransactionReadOptions, TransactionVersion, bounded_execution_entries,
    },
    error::Error,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize};
use solana_transaction_error::TransactionError;
fn required_optional<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

fn accounts<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    bounded_execution_entries::<D, T, 256>(d)
}
fn instructions<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    bounded_execution_entries::<D, T, 1024>(d)
}
fn groups<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    bounded_execution_entries::<D, T, 256>(d)
}
// Explicit wrappers preserve missing/null versus empty source recordings and
// enforce list caps during deserialization rather than after materialization.
fn optional_tokens<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<TokenWire>>, D::Error> {
    #[derive(Deserialize)]
    struct V(#[serde(deserialize_with = "accounts")] Vec<TokenWire>);
    Ok(Option::<V>::deserialize(d)?.map(|v| v.0))
}
fn optional_inner<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<InnerWire>>, D::Error> {
    #[derive(Deserialize)]
    struct V(#[serde(deserialize_with = "groups")] Vec<InnerWire>);
    Ok(Option::<V>::deserialize(d)?.map(|v| v.0))
}
fn optional_logs<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<LogMessage>>, D::Error> {
    #[derive(Deserialize)]
    struct V(#[serde(deserialize_with = "instructions")] Vec<LogMessage>);
    Ok(Option::<V>::deserialize(d)?.map(|v| v.0))
}
fn optional_rewards<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<RewardWire>>, D::Error> {
    #[derive(Deserialize)]
    struct V(#[serde(deserialize_with = "instructions")] Vec<RewardWire>);
    Ok(Option::<V>::deserialize(d)?.map(|v| v.0))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransactionOptions {
    encoding: &'static str,
    commitment: Commitment,
    max_supported_transaction_version: u8,
}
impl TransactionOptions {
    pub(crate) const fn new(o: TransactionReadOptions) -> Self {
        Self {
            encoding: "base64",
            commitment: o.commitment(),
            max_supported_transaction_version: o.maximum_supported_version(),
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusRequest {
    search_transaction_history: bool,
}
impl From<StatusOptions> for StatusRequest {
    fn from(o: StatusOptions) -> Self {
        Self {
            search_transaction_history: o.search_transaction_history,
        }
    }
}
#[derive(Serialize)]
pub(crate) struct HeightOptions {
    pub(crate) commitment: Commitment,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulationOptions {
    pub(crate) encoding: &'static str,
    pub(crate) commitment: Commitment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) min_context_slot: Option<u64>,
    pub(crate) sig_verify: bool,
    pub(crate) replace_recent_blockhash: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmissionOptions {
    encoding: &'static str,
    preflight_commitment: Commitment,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_context_slot: Option<u64>,
    skip_preflight: bool,
    max_retries: u8,
}
impl From<SubmitOptions> for SubmissionOptions {
    fn from(o: SubmitOptions) -> Self {
        Self {
            encoding: "base64",
            preflight_commitment: o.preflight_commitment,
            min_context_slot: o.minimum_context_slot,
            skip_preflight: o.skip_preflight,
            max_retries: 0,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum VersionWire {
    Number(u8),
    Text(String),
}
impl VersionWire {
    fn value(self) -> Result<TransactionVersion, Error> {
        match self {
            Self::Number(0) => Ok(TransactionVersion::V0),
            Self::Number(1) => Ok(TransactionVersion::V1),
            Self::Text(v) if v == "legacy" => Ok(TransactionVersion::Legacy),
            _ => Err(invalid_response()),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransactionWire {
    transaction: [String; 2],
    slot: u64,
    #[serde(deserialize_with = "required_optional")]
    block_time: Option<i64>,
    #[serde(default)]
    transaction_index: Option<u32>,
    #[serde(deserialize_with = "required_optional")]
    meta: Option<MetadataWire>,
    version: VersionWire,
}
impl TransactionWire {
    pub(crate) fn into_transaction(self) -> Result<Transaction, Error> {
        let [encoded, encoding] = self.transaction;
        if encoding != "base64" || encoded.len() > 4096_usize.div_ceil(3) * 4 {
            return Err(invalid_response());
        }
        let bytes = STANDARD.decode(&encoded).map_err(|_| invalid_response())?;
        if STANDARD.encode(&bytes) != encoded {
            return Err(invalid_response());
        }
        let body = SignedTransaction::from_bytes(bytes).map_err(|_| invalid_response())?;
        if body.message().version() != self.version.value()? {
            return Err(invalid_response());
        }
        Transaction::new(
            body,
            self.slot,
            self.block_time,
            self.transaction_index,
            self.meta.map(MetadataWire::into_metadata).transpose()?,
        )
        .map_err(|_| invalid_response())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataWire {
    #[serde(deserialize_with = "required_optional")]
    err: Option<TransactionError>,
    #[serde(default)]
    status: Option<Result<(), TransactionError>>,
    fee: u64,
    #[serde(deserialize_with = "accounts")]
    pre_balances: Vec<u64>,
    #[serde(deserialize_with = "accounts")]
    post_balances: Vec<u64>,
    #[serde(default)]
    loaded_addresses: Option<LoadedAddresses>,
    #[serde(default, deserialize_with = "optional_tokens")]
    pre_token_balances: Option<Vec<TokenWire>>,
    #[serde(default, deserialize_with = "optional_tokens")]
    post_token_balances: Option<Vec<TokenWire>>,
    #[serde(default, deserialize_with = "optional_inner")]
    inner_instructions: Option<Vec<InnerWire>>,
    #[serde(default, deserialize_with = "optional_logs")]
    log_messages: Option<Vec<LogMessage>>,
    #[serde(default)]
    return_data: Option<ReturnWire>,
    #[serde(default, deserialize_with = "optional_rewards")]
    rewards: Option<Vec<RewardWire>>,
    #[serde(default)]
    compute_units_consumed: Option<u64>,
    #[serde(default)]
    cost_units: Option<u64>,
}
fn check_status(
    error: Option<&TransactionError>,
    status: Option<&Result<(), TransactionError>>,
) -> Result<(), Error> {
    if status.is_some_and(|s| s.as_ref().err() != error) {
        return Err(invalid_response());
    }
    Ok(())
}
impl MetadataWire {
    fn into_metadata(self) -> Result<TransactionMetadata, Error> {
        check_status(self.err.as_ref(), self.status.as_ref())?;
        TransactionMetadata::new(TransactionMetadataData {
            outcome: ExecutionOutcome::from_error(self.err),
            fee_lamports: self.fee,
            pre_balances: self.pre_balances,
            post_balances: self.post_balances,
            loaded_addresses: self.loaded_addresses,
            pre_token_balances: self
                .pre_token_balances
                .map(|v| v.into_iter().map(TokenWire::into_record).collect())
                .transpose()?,
            post_token_balances: self
                .post_token_balances
                .map(|v| v.into_iter().map(TokenWire::into_record).collect())
                .transpose()?,
            inner_instructions: self
                .inner_instructions
                .map(|v| v.into_iter().map(InnerWire::into_record).collect())
                .transpose()?,
            log_messages: self.log_messages,
            return_data: self.return_data.map(ReturnWire::into_record).transpose()?,
            rewards: self
                .rewards
                .map(|v| v.into_iter().map(RewardWire::into_record).collect()),
            compute_units_consumed: self.compute_units_consumed,
            cost_units: self.cost_units,
        })
        .map_err(|_| invalid_response())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenWire {
    account_index: u8,
    mint: Pubkey,
    #[serde(default)]
    owner: Option<Pubkey>,
    #[serde(default)]
    program_id: Option<Pubkey>,
    ui_token_amount: TokenAmountWire,
}
#[derive(Deserialize)]
struct TokenAmountWire {
    amount: String,
    decimals: u8,
}
impl TokenWire {
    fn into_record(self) -> Result<TokenBalanceRecord, Error> {
        let text = self.ui_token_amount.amount;
        if text.is_empty()
            || text.len() > 20
            || text.bytes().any(|b| !b.is_ascii_digit())
            || text.len() > 1 && text.starts_with('0')
        {
            return Err(invalid_response());
        }
        Ok(TokenBalanceRecord {
            account_index: self.account_index,
            mint: self.mint,
            owner: self.owner,
            program_id: self.program_id,
            raw_amount: text.parse().map_err(|_| invalid_response())?,
            decimals: self.ui_token_amount.decimals,
        })
    }
}
#[derive(Deserialize)]
struct InnerWire {
    index: u8,
    #[serde(deserialize_with = "instructions")]
    instructions: Vec<InstructionWire>,
}
impl InnerWire {
    fn into_record(self) -> Result<InnerInstructions, Error> {
        Ok(InnerInstructions {
            index: self.index,
            instructions: self
                .instructions
                .into_iter()
                .map(InstructionWire::into_record)
                .collect::<Result<_, _>>()?,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstructionWire {
    program_id_index: u8,
    #[serde(deserialize_with = "accounts")]
    accounts: Vec<u8>,
    data: String,
    #[serde(default)]
    stack_height: Option<u32>,
}
impl InstructionWire {
    fn into_record(self) -> Result<CompiledInnerInstruction, Error> {
        Ok(CompiledInnerInstruction {
            program_id_index: self.program_id_index,
            accounts: self.accounts,
            data: ExecutionBytes::from_base58(&self.data).map_err(|_| invalid_response())?,
            stack_height: self.stack_height,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReturnWire {
    program_id: Pubkey,
    data: [String; 2],
}
impl ReturnWire {
    fn into_record(self) -> Result<ReturnData, Error> {
        let [encoded, encoding] = self.data;
        if encoding != "base64" || encoded.len() > ExecutionBytes::MAX_BYTES.div_ceil(3) * 4 {
            return Err(invalid_response());
        }
        let bytes = STANDARD.decode(&encoded).map_err(|_| invalid_response())?;
        if STANDARD.encode(&bytes) != encoded {
            return Err(invalid_response());
        }
        Ok(ReturnData {
            program_id: self.program_id,
            data: ExecutionBytes::new(bytes).map_err(|_| invalid_response())?,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewardWire {
    pubkey: Pubkey,
    lamports: i64,
    post_balance: u64,
    #[serde(default)]
    reward_type: Option<RewardKind>,
    #[serde(default)]
    commission: Option<u8>,
}
impl RewardWire {
    fn into_record(self) -> Reward {
        Reward {
            pubkey: self.pubkey,
            lamports: self.lamports,
            post_balance: self.post_balance,
            reward_type: self.reward_type,
            commission: self.commission,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusWire {
    slot: u64,
    #[serde(deserialize_with = "required_optional")]
    confirmations: Option<u64>,
    #[serde(deserialize_with = "required_optional")]
    err: Option<TransactionError>,
    #[serde(default)]
    status: Option<Result<(), TransactionError>>,
    #[serde(default)]
    confirmation_status: Option<Commitment>,
}
impl StatusWire {
    pub(crate) fn into_status(self) -> Result<SignatureStatus, Error> {
        check_status(self.err.as_ref(), self.status.as_ref())?;
        Ok(SignatureStatus {
            inclusion_slot: self.slot,
            confirmations: self.confirmations,
            confirmation_status: self.confirmation_status,
            outcome: ExecutionOutcome::from_error(self.err),
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LatestWire {
    blockhash: Hash,
    last_valid_block_height: u64,
}
impl LatestWire {
    pub(crate) const fn lifetime(self) -> BlockhashLifetime {
        BlockhashLifetime {
            blockhash: self.blockhash,
            last_valid_block_height: self.last_valid_block_height,
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulationWire {
    #[serde(deserialize_with = "required_optional")]
    err: Option<TransactionError>,
    #[serde(default)]
    units_consumed: Option<u64>,
    #[serde(default)]
    fee: Option<u64>,
    #[serde(deserialize_with = "optional_logs")]
    logs: Option<Vec<LogMessage>>,
    #[serde(default)]
    return_data: Option<ReturnWire>,
    #[serde(default)]
    replacement_blockhash: Option<LatestWire>,
}
impl SimulationWire {
    pub(crate) fn into_simulation(self) -> Result<Simulation, Error> {
        if self.replacement_blockhash.is_some() {
            return Err(invalid_response());
        }
        Simulation::new(
            ExecutionOutcome::from_error(self.err),
            self.units_consumed,
            self.fee,
            self.logs,
            self.return_data.map(ReturnWire::into_record).transpose()?,
        )
        .map_err(|_| invalid_response())
    }
}
