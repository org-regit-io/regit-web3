// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! EVM-specific RPC fields, canonical quantities, and fixed error-code policy.
//!
//! Typed results preserve duplicate-field detection and map required block
//! fields into domain records. HTTP orchestration and shared JSON-RPC envelope
//! decoding live outside this module.

use serde::{Deserialize, Serialize};

pub(super) mod abi;
pub(super) mod reads;

use crate::{
    domain::{BlockContext, BlockHash, Timestamp, U256},
    error::{Error, ProviderError},
};

pub(super) const fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}

pub(super) const fn chain_error(_code: i64) -> Error {
    Error::Provider(ProviderError::Rpc)
}

pub(super) const fn state_error(code: i64) -> Error {
    // EIP-1474 meanings are precise; ambiguous canonicality errors remain RPC
    // failures. Remote diagnostic text is never interpreted.
    match code {
        -32601 | -32004 => Error::UnsupportedCapability,
        -32001 | -32002 => Error::UnavailableData,
        _ => Error::Provider(ProviderError::Rpc),
    }
}

pub(super) fn parse_quantity(quantity: &str) -> Result<U256, Error> {
    let digits = quantity.strip_prefix("0x").ok_or_else(invalid_response)?;
    if digits.is_empty()
        || digits.len() > 64
        || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err(invalid_response());
    }
    U256::from_str_radix(digits, 16).map_err(|_| invalid_response())
}

// Block responses include protocol fields and future extensions. Required
// anchor fields deserialize directly without a generic map intermediary.
#[derive(Deserialize)]
pub(super) struct RpcBlock {
    hash: String,
    number: String,
    timestamp: String,
}

impl RpcBlock {
    pub(super) fn context(self) -> Result<BlockContext, Error> {
        let hash = BlockHash::parse(&self.hash).map_err(|_| invalid_response())?;
        let number =
            u64::try_from(parse_quantity(&self.number)?).map_err(|_| invalid_response())?;
        let timestamp =
            u64::try_from(parse_quantity(&self.timestamp)?).map_err(|_| invalid_response())?;
        Ok(BlockContext::new(
            number,
            hash,
            Timestamp::from_unix_seconds(timestamp),
        ))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CanonicalBlock {
    block_hash: BlockHash,
    require_canonical: bool,
}

impl CanonicalBlock {
    pub(super) const fn new(block_hash: BlockHash) -> Self {
        Self {
            block_hash,
            require_canonical: true,
        }
    }
}
