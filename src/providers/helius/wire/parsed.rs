// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{List, invalid, metadata::Metadata};
use crate::{
    domain::{
        helius::{
            Bounds, Cursor, DecodedError, DecodedInstruction, HistoryPage, HistoryRequest,
            NamedAccount, NativeTransfer, ParseOutcome, ParseRequest, ParsedInstruction,
            ParsedTransaction, ParsedTransactionData, SortDirection, SourceText, Summary,
            TokenTransfer, TransactionResult, TransactionStatus,
        },
        solana::{Commitment, ExecutionBytes, Pubkey, Signature},
    },
    error::Error,
};
use serde::{Deserialize, Serialize, de::IgnoredAny};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ParseParams<'a> {
    transactions: &'a [Signature],
    commitment: Commitment,
    include_raw_transaction: bool,
}
pub(in super::super) fn parse_params(q: &ParseRequest) -> ParseParams<'_> {
    ParseParams {
        transactions: q.signatures(),
        commitment: q.commitment(),
        include_raw_transaction: false,
    }
}
#[derive(Serialize)]
struct Comparisons {
    #[serde(skip_serializing_if = "Option::is_none")]
    gt: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gte: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lt: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lte: Option<u64>,
}
fn bounds(b: Bounds) -> Comparisons {
    let [gt, gte, lt, lte] = b.comparisons();
    Comparisons { gt, gte, lt, lte }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct HistoryParams<'a> {
    address: Pubkey,
    limit: u8,
    commitment: Commitment,
    sort_order: SortDirection,
    include_raw_transaction: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    before_signature: Option<Signature>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after_signature: Option<Signature>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination_token: Option<&'a Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slot: Option<Comparisons>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time: Option<Comparisons>,
}
pub(in super::super) fn history_params(q: &HistoryRequest) -> HistoryParams<'_> {
    let [before_signature, after_signature] = q.signatures();
    HistoryParams {
        address: q.address(),
        limit: q.limit(),
        commitment: q.commitment(),
        sort_order: q.direction(),
        include_raw_transaction: false,
        before_signature,
        after_signature,
        pagination_token: q.pagination_token(),
        slot: q.slot().map(bounds),
        time: q.time().map(bounds),
    }
}
#[derive(Deserialize)]
struct SummaryWire {
    #[serde(rename = "type")]
    category: SourceText,
    description: SourceText,
    #[serde(rename = "parsedData")]
    parsed_data: Option<Metadata>,
}
impl From<SummaryWire> for Summary {
    fn from(s: SummaryWire) -> Self {
        Self {
            category: s.category,
            description: s.description,
            parsed_data: s.parsed_data.map(|m| m.0),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeWire {
    from_user_account: Option<Pubkey>,
    to_user_account: Option<Pubkey>,
    amount: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenWire {
    from_user_account: Option<Pubkey>,
    to_user_account: Option<Pubkey>,
    from_token_account: Option<Pubkey>,
    to_token_account: Option<Pubkey>,
    #[serde(deserialize_with = "super::raw_units")]
    raw_token_amount: u64,
    decimals: u8,
    mint: Pubkey,
    token_standard: Option<SourceText>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamedWire {
    name: SourceText,
    pubkey: Pubkey,
    is_signer: bool,
    is_writable: bool,
}
#[derive(Deserialize)]
struct DecodedWire {
    args: Metadata,
    accounts: List<NamedWire>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstructionWire {
    instruction_index: u32,
    inner_instruction_index: Option<u32>,
    stack_height: Option<u32>,
    program_id: Pubkey,
    raw_accounts: List<Pubkey>,
    raw_data: String,
    summary: Option<SummaryWire>,
    program_name: Option<SourceText>,
    instruction_name: Option<SourceText>,
    decoded: Option<DecodedWire>,
}
impl InstructionWire {
    fn into_instruction(self) -> Result<ParsedInstruction, Error> {
        if self.raw_data.len() > ExecutionBytes::MAX_BYTES * 2 {
            return Err(invalid());
        }
        let bytes = bs58::decode(&self.raw_data)
            .into_vec()
            .map_err(|_| invalid())?;
        if bs58::encode(&bytes).into_string() != self.raw_data {
            return Err(invalid());
        }
        Ok(ParsedInstruction {
            instruction_index: self.instruction_index,
            inner_instruction_index: self.inner_instruction_index,
            stack_height: self.stack_height,
            program_id: self.program_id,
            raw_accounts: self.raw_accounts.0,
            raw_data: ExecutionBytes::new(bytes).map_err(|_| invalid())?,
            summary: self.summary.map(Into::into),
            program_name: self.program_name,
            instruction_name: self.instruction_name,
            decoded: self.decoded.map(|d| DecodedInstruction {
                args: d.args.0,
                accounts: d
                    .accounts
                    .0
                    .into_iter()
                    .map(|a| NamedAccount {
                        name: a.name,
                        pubkey: a.pubkey,
                        is_signer: a.is_signer,
                        is_writable: a.is_writable,
                    })
                    .collect(),
            }),
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecodedErrorWire {
    instruction_index: u32,
    program_id: Pubkey,
    program_name: SourceText,
    code: u32,
    name: SourceText,
    msg: Option<SourceText>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParsedWire {
    slot: u64,
    block_time: Option<i64>,
    fee: u64,
    fee_payer: Option<Pubkey>,
    transaction_status: String,
    error: Option<Metadata>,
    decoded_error: Option<DecodedErrorWire>,
    native_transfers: List<NativeWire>,
    token_transfers: List<TokenWire>,
    summary: Option<SummaryWire>,
    instructions: List<InstructionWire>,
}
impl ParsedWire {
    fn into_parsed(self) -> Result<ParsedTransaction, Error> {
        let transaction_status = match self.transaction_status.as_str() {
            "OK" => TransactionStatus::Succeeded,
            "ERROR" => TransactionStatus::Failed,
            _ => return Err(invalid()),
        };
        ParsedTransaction::new(ParsedTransactionData {
            slot: self.slot,
            block_time: self.block_time,
            fee: self.fee,
            fee_payer: self.fee_payer,
            transaction_status,
            error: self.error.map(|m| m.0),
            decoded_error: self.decoded_error.map(|e| DecodedError {
                instruction_index: e.instruction_index,
                program_id: e.program_id,
                program_name: e.program_name,
                code: e.code,
                name: e.name,
                message: e.msg,
            }),
            native_transfers: self
                .native_transfers
                .0
                .into_iter()
                .map(|n| NativeTransfer {
                    from_user_account: n.from_user_account,
                    to_user_account: n.to_user_account,
                    amount: n.amount,
                })
                .collect(),
            token_transfers: self
                .token_transfers
                .0
                .into_iter()
                .map(|t| TokenTransfer {
                    from_user_account: t.from_user_account,
                    to_user_account: t.to_user_account,
                    from_token_account: t.from_token_account,
                    to_token_account: t.to_token_account,
                    raw_token_amount: t.raw_token_amount,
                    decimals: t.decimals,
                    mint: t.mint,
                    token_standard: t.token_standard,
                })
                .collect(),
            summary: self.summary.map(Into::into),
            instructions: self
                .instructions
                .0
                .into_iter()
                .map(InstructionWire::into_instruction)
                .collect::<Result<_, _>>()?,
        })
        .map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
struct ErrorWire {
    code: SourceText,
    #[serde(rename = "message")]
    _message: SourceText,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct ResultWire {
    signature: Signature,
    parser_status: String,
    parsed: Option<ParsedWire>,
    parser_error: Option<ErrorWire>,
    raw_transaction: Option<IgnoredAny>,
}
impl ResultWire {
    pub(in super::super) fn into_result(self) -> Result<TransactionResult, Error> {
        if self.raw_transaction.is_some() {
            return Err(invalid());
        }
        let outcome = match (self.parser_status.as_str(), self.parsed, self.parser_error) {
            ("OK", Some(p), None) => ParseOutcome::Ok {
                parsed: Box::new(p.into_parsed()?),
            },
            ("ERROR", None, Some(e))
                if !e.code.as_str().is_empty() && e.code.as_str().len() <= 256 =>
            {
                ParseOutcome::Error { code: e.code }
            }
            _ => return Err(invalid()),
        };
        Ok(TransactionResult {
            signature: self.signature,
            outcome,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct HistoryWire {
    data: List<ResultWire>,
    pagination_token: Option<Cursor>,
}
impl HistoryWire {
    pub(in super::super) fn into_page(self, q: HistoryRequest) -> Result<HistoryPage, Error> {
        HistoryPage::new(
            q,
            self.data
                .0
                .into_iter()
                .map(ResultWire::into_result)
                .collect::<Result<_, _>>()?,
            self.pagination_token,
        )
        .map_err(|_| invalid())
    }
}
