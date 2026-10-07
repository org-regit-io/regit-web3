// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Cursor, HistoryRequest, MetadataValue, ParseRequest, SortDirection, SourceText};
use crate::{
    domain::solana::{ExecutionBytes, Pubkey, Signature},
    error::Error,
};
use serde::{Deserialize, Serialize};

/// Parser-detected native movement, in exact lamports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTransfer {
    /// Actual source account when known; absence is not the System Program.
    pub from_user_account: Option<Pubkey>,
    /// Actual destination account when known.
    pub to_user_account: Option<Pubkey>,
    /// Exact lamports, never a display SOL amount.
    pub amount: u64,
}
/// Parser-detected SPL or Token-2022 movement, without wallet net-receipt inference.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenTransfer {
    /// Actual source owner/authority when known.
    pub from_user_account: Option<Pubkey>,
    /// Actual destination owner when known.
    pub to_user_account: Option<Pubkey>,
    /// Actual source token account when known.
    pub from_token_account: Option<Pubkey>,
    /// Actual destination token account when known.
    pub to_token_account: Option<Pubkey>,
    /// Exact raw token amount, without scaled UI conversion.
    pub raw_token_amount: u64,
    /// Source mint decimals.
    pub decimals: u8,
    /// Actual source mint identity.
    pub mint: Pubkey,
    /// Source token standard; an unknown standard remains explicit source text.
    pub token_standard: Option<SourceText>,
}
/// Optional provider summary; venue amounts in metadata need not be wallet receipts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    /// Source summary category, preserving unknown future labels.
    pub category: SourceText,
    /// Opaque source narrative, never used as a validation or approval decision.
    pub description: SourceText,
    /// Structured source interpretation; IDL/venue semantics are not independently verified.
    pub parsed_data: Option<MetadataValue>,
}
/// An IDL-named account reported by the parser, without independently verified flags.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedAccount {
    /// Source IDL account name.
    pub name: SourceText,
    /// Canonical encoding-valid account identity.
    pub pubkey: Pubkey,
    /// Source signer flag; not a verified signature.
    pub is_signer: bool,
    /// Source writable flag; not proof of a reviewed intent.
    pub is_writable: bool,
}
/// IDL-decoded fields; numeric strings stay strings and JSON numbers stay exact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedInstruction {
    /// Structured named IDL arguments, without conversion of protocol-specific units.
    pub args: MetadataValue,
    /// Bounded named source account list.
    #[serde(deserialize_with = "super::bounded::list")]
    pub accounts: Vec<NamedAccount>,
}
/// One source instruction, retaining raw bytes even when IDL decoding is unavailable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParsedInstruction {
    /// Source top-level instruction index.
    pub instruction_index: u32,
    /// Optional source inner-instruction index.
    pub inner_instruction_index: Option<u32>,
    /// Optional actual source stack height.
    pub stack_height: Option<u32>,
    /// Actual source program identity.
    pub program_id: Pubkey,
    /// Resolved source account list; lookup-table resolution is not independently proven.
    #[serde(deserialize_with = "super::bounded::list")]
    pub raw_accounts: Vec<Pubkey>,
    /// Exact bounded base58-decoded source instruction bytes.
    pub raw_data: ExecutionBytes,
    /// Optional source summary.
    pub summary: Option<Summary>,
    /// Optional source catalog program name.
    pub program_name: Option<SourceText>,
    /// Optional actual IDL instruction name.
    pub instruction_name: Option<SourceText>,
    /// Explicitly unavailable when the catalog cannot decode the instruction.
    pub decoded: Option<DecodedInstruction>,
}
/// Parser-reported custom program error metadata, distinct from parser failure.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedError {
    /// Source failing top-level instruction index.
    pub instruction_index: u32,
    /// Source failing program identity.
    pub program_id: Pubkey,
    /// Source catalog program name.
    pub program_name: SourceText,
    /// Source custom error code.
    pub code: u32,
    /// Source custom error name.
    pub name: SourceText,
    /// Optional opaque source message, never included in diagnostics.
    pub message: Option<SourceText>,
}
/// Source transaction execution classification, separate from parser success.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    /// Source reports successful execution.
    Succeeded,
    /// Source reports failed execution; raw error metadata can be unavailable.
    Failed,
}
/// Supported exact Parsed Events transaction fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParsedTransactionData {
    /// Actual source transaction inclusion slot; not an evaluation-slot anchor.
    pub slot: u64,
    /// Optional actual source block time in whole Unix seconds, including negative values.
    pub block_time: Option<i64>,
    /// Exact lamport fee reported by the parser source.
    pub fee: u64,
    /// Actual source fee payer when available.
    pub fee_payer: Option<Pubkey>,
    /// Source execution classification, without finality proof.
    pub transaction_status: TransactionStatus,
    /// Optional structured raw source execution error, with no string-based diagnosis.
    pub error: Option<MetadataValue>,
    /// Optional parser-decoded custom program error.
    pub decoded_error: Option<DecodedError>,
    /// Every bounded source native movement.
    #[serde(deserialize_with = "super::bounded::list")]
    pub native_transfers: Vec<NativeTransfer>,
    /// Every bounded source token movement, including Token-2022 interpretations.
    #[serde(deserialize_with = "super::bounded::list")]
    pub token_transfers: Vec<TokenTransfer>,
    /// Optional source summary, absent for unrecognized transaction-level actions.
    pub summary: Option<Summary>,
    /// Source top-level/inner instruction order, without invented inclusion indices.
    #[serde(deserialize_with = "super::bounded::list")]
    pub instructions: Vec<ParsedInstruction>,
}
/// Validated bounded parsed transaction; it is not canonical transaction proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ParsedTransactionData", into = "ParsedTransactionData")]
pub struct ParsedTransaction(ParsedTransactionData);
impl ParsedTransaction {
    /// Validates supported counts, index widths and source execution consistency.
    /// # Errors
    /// Rejects malformed/excessive metadata without echoing source data.
    pub fn new(data: ParsedTransactionData) -> Result<Self, Error> {
        let mut instruction_keys = std::collections::BTreeSet::new();
        if data.native_transfers.len() > 4096
            || data.token_transfers.len() > 4096
            || data.instructions.len() > 4096
            || data.transaction_status == TransactionStatus::Succeeded
                && (data.error.is_some() || data.decoded_error.is_some())
            || data
                .decoded_error
                .as_ref()
                .is_some_and(|e| e.instruction_index >= 4096)
            || data.instructions.iter().any(|i| {
                i.instruction_index >= 4096
                    || i.inner_instruction_index.is_some_and(|v| v >= 4096)
                    || i.stack_height.is_some_and(|v| v == 0 || v > 64)
                    || i.raw_accounts.len() > 256
                    || i.decoded.as_ref().is_some_and(|d| d.accounts.len() > 256)
                    || !instruction_keys.insert((i.instruction_index, i.inner_instruction_index))
            })
        {
            return Err(super::bounded::invalid_transaction());
        }
        Ok(Self(data))
    }
    /// Returns exact source parsed fields and optional interpretations.
    #[must_use]
    pub const fn data(&self) -> &ParsedTransactionData {
        &self.0
    }
}
impl TryFrom<ParsedTransactionData> for ParsedTransaction {
    type Error = Error;
    fn try_from(d: ParsedTransactionData) -> Result<Self, Error> {
        Self::new(d)
    }
}
impl From<ParsedTransaction> for ParsedTransactionData {
    fn from(v: ParsedTransaction) -> Self {
        v.0
    }
}
/// Explicit item-level parser state. Parsing can fail without failing the batch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "parser_status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParseOutcome {
    /// Parser produced a supported interpreted transaction, even if execution failed.
    Ok {
        /// Actual parsed source transaction fields.
        parsed: Box<ParsedTransaction>,
    },
    /// Missing transaction or source parser failure, never silently treated as pending.
    Error {
        /// Actual bounded source error code; message text is deliberately omitted.
        code: SourceText,
    },
}
/// One parsed item bound to its exact canonical encoding-valid query signature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionResult {
    /// Source signature; neither bytes nor cryptographic validity are proven here.
    pub signature: Signature,
    /// Explicit parser state, distinct from execution state.
    pub outcome: ParseOutcome,
}
/// Ordered parsed batch retaining every input position, including duplicates/errors.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ParsedBatchFields")]
pub struct ParsedBatch {
    request: ParseRequest,
    results: Vec<TransactionResult>,
}
fn valid_result(result: &TransactionResult) -> bool {
    match &result.outcome {
        ParseOutcome::Ok { .. } => true,
        ParseOutcome::Error { code } => {
            !code.as_str().is_empty()
                && code.as_str().len() <= 256
                && !code.as_str().chars().any(char::is_control)
        }
    }
}
impl ParsedBatch {
    /// Requires one source result per requested signature in exact input order.
    /// # Errors
    /// Rejects omissions, extra records, reorderings and mismatched identities.
    pub fn new(request: ParseRequest, results: Vec<TransactionResult>) -> Result<Self, Error> {
        if results.len() != request.signatures().len()
            || results
                .iter()
                .zip(request.signatures())
                .any(|(r, s)| r.signature != *s || !valid_result(r))
        {
            return Err(super::bounded::invalid_transaction());
        }
        Ok(Self { request, results })
    }
    /// Returns the immutable original parse query.
    #[must_use]
    pub const fn request(&self) -> &ParseRequest {
        &self.request
    }
    /// Returns results in exact requested order.
    #[must_use]
    pub fn results(&self) -> &[TransactionResult] {
        &self.results
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParsedBatchFields {
    request: ParseRequest,
    #[serde(deserialize_with = "super::bounded::list")]
    results: Vec<TransactionResult>,
}
impl TryFrom<ParsedBatchFields> for ParsedBatch {
    type Error = Error;
    fn try_from(f: ParsedBatchFields) -> Result<Self, Error> {
        Self::new(f.request, f.results)
    }
}
/// One explicit source address-history page, without a complete-history claim.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HistoryPageFields")]
pub struct HistoryPage {
    request: HistoryRequest,
    results: Vec<TransactionResult>,
    pagination_token: Option<Cursor>,
}
impl HistoryPage {
    /// Checks response capacity, duplicate signatures, known slot/time bounds and order.
    /// Parser-error records lack inclusion facts and remain explicit unknowns.
    /// # Errors
    /// Rejects contradictory source facts without inventing absent context.
    pub fn new(
        request: HistoryRequest,
        results: Vec<TransactionResult>,
        pagination_token: Option<Cursor>,
    ) -> Result<Self, Error> {
        let mut signatures = std::collections::BTreeSet::new();
        let mut previous = None;
        if results.len() > usize::from(request.limit())
            || pagination_token
                .as_ref()
                .zip(request.pagination_token())
                .is_some_and(|(a, b)| a == b)
        {
            return Err(super::bounded::invalid_transaction());
        }
        for result in &results {
            if !valid_result(result)
                || !signatures.insert(result.signature)
                || request.signatures().contains(&Some(result.signature))
            {
                return Err(super::bounded::invalid_transaction());
            }
            if let ParseOutcome::Ok { parsed } = &result.outcome {
                let data = parsed.data();
                if request.slot().is_some_and(|b| !b.contains(data.slot))
                    || request
                        .time()
                        .zip(data.block_time)
                        .is_some_and(|(b, t)| u64::try_from(t).map_or(true, |t| !b.contains(t)))
                    || previous.is_some_and(|p| match request.direction() {
                        SortDirection::Asc => p > data.slot,
                        SortDirection::Desc => p < data.slot,
                    })
                {
                    return Err(super::bounded::invalid_transaction());
                }
                previous = Some(data.slot);
            }
        }
        Ok(Self {
            request,
            results,
            pagination_token,
        })
    }
    /// Returns exact query controls.
    #[must_use]
    pub const fn request(&self) -> &HistoryRequest {
        &self.request
    }
    /// Returns every source item, including per-item parser errors.
    #[must_use]
    pub fn results(&self) -> &[TransactionResult] {
        &self.results
    }
    /// Returns exact continuation token; absence is the source's available-range end.
    #[must_use]
    pub const fn pagination_token(&self) -> Option<&Cursor> {
        self.pagination_token.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryPageFields {
    request: HistoryRequest,
    #[serde(deserialize_with = "super::bounded::list")]
    results: Vec<TransactionResult>,
    pagination_token: Option<Cursor>,
}
impl TryFrom<HistoryPageFields> for HistoryPage {
    type Error = Error;
    fn try_from(f: HistoryPageFields) -> Result<Self, Error> {
        Self::new(f.request, f.results, f.pagination_token)
    }
}
