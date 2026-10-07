// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Solana-specific RPC fields, account decoding, and validated domain mapping.
//!
//! Required result fields deserialize directly to retain duplicate detection.
//! HTTP orchestration and shared JSON-RPC envelope decoding remain separate.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    domain::solana::{
        Account, Commitment, Network, Pubkey, ReadOptions, TokenAccountState, TokenAsset,
        TokenBalance,
    },
    error::{Error, ProviderError},
};

pub(super) fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

pub(super) fn error_policy(code: i64) -> Error {
    if code == -32601 {
        Error::UnsupportedCapability
    } else {
        Error::Provider(ProviderError::Rpc)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RequestOptions {
    commitment: Commitment,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_context_slot: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding: Option<&'static str>,
}

impl RequestOptions {
    pub(super) const fn new(options: ReadOptions, encoding: Option<&'static str>) -> Self {
        Self {
            commitment: options.commitment(),
            min_context_slot: options.minimum_context_slot(),
            encoding,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct SlotContext {
    pub(super) slot: u64,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(super) struct SlotValue<T> {
    pub(super) context: SlotContext,
    #[serde(deserialize_with = "required_value")]
    pub(super) value: T,
}

fn required_value<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    T::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BinaryAccount {
    data: [String; 2],
    executable: bool,
    lamports: u64,
    owner: Pubkey,
    rent_epoch: u64,
    space: Option<u64>,
}

impl BinaryAccount {
    pub(super) fn into_account(self, network: Network, address: Pubkey) -> Result<Account, Error> {
        let [encoded, encoding] = self.data;
        // Reject oversized encoded data before allocating the decoded buffer.
        let maximum_encoded = Account::MAX_DATA_BYTES.div_ceil(3) * 4;
        if encoding != "base64" || encoded.len() > maximum_encoded {
            return Err(invalid_response());
        }
        let data = STANDARD.decode(encoded).map_err(|_| invalid_response())?;
        if self.space.is_some_and(|space| space != data.len() as u64) {
            return Err(invalid_response());
        }
        Account::new(
            network,
            address,
            self.owner,
            self.lamports,
            self.executable,
            data,
            self.rent_epoch,
        )
        .map_err(|_| invalid_response())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ParsedAccount {
    data: ParsedData,
    executable: bool,
    #[serde(rename = "lamports")]
    _lamports: u64,
    owner: Pubkey,
    #[serde(rename = "rentEpoch")]
    _rent_epoch: u64,
    space: Option<u64>,
}

#[derive(Deserialize)]
struct ParsedData {
    program: String,
    parsed: ParsedToken,
    space: u64,
}

#[derive(Deserialize)]
struct ParsedToken {
    #[serde(rename = "type")]
    kind: String,
    info: TokenInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenInfo {
    mint: Pubkey,
    owner: Pubkey,
    state: TokenAccountState,
    token_amount: TokenAmount,
}

#[derive(Deserialize)]
struct TokenAmount {
    amount: String,
    decimals: u8,
}

impl ParsedAccount {
    pub(super) fn into_balance(
        self,
        network: Network,
        token_account: Pubkey,
    ) -> Result<TokenBalance, Error> {
        let expected_program = match self.data.program.as_str() {
            "spl-token" => "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "spl-token-2022" => "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
            _ => return Err(invalid_response()),
        };
        if self.owner != Pubkey::parse(expected_program).map_err(|_| invalid_response())?
            || self.executable
            || self.data.parsed.kind != "account"
            || self.data.space > Account::MAX_DATA_BYTES as u64
            || self.space.is_some_and(|space| space != self.data.space)
        {
            return Err(invalid_response());
        }
        let info = self.data.parsed.info;
        let raw = parse_raw(&info.token_amount.amount)?;
        let asset = TokenAsset::new(
            network,
            info.mint,
            self.owner,
            Some(info.token_amount.decimals),
        );
        Ok(TokenBalance::new(
            token_account,
            info.owner,
            asset,
            raw,
            info.state,
        ))
    }
}

fn parse_raw(value: &str) -> Result<u64, Error> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(invalid_response());
    }
    value.parse().map_err(|_| invalid_response())
}
