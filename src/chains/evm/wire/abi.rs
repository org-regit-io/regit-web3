// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Supported ERC-20 standard ABI only, following the Solidity ABI specification.
//! No cryptographic primitive or generic Solidity decoder is implemented here.

use super::invalid_response;
use crate::domain::evm::{Data, MetadataText};
use crate::domain::{Address, U256};
use crate::error::Error;

pub(in crate::chains::evm) const NAME: [u8; 4] = [0x06, 0xfd, 0xde, 0x03];
pub(in crate::chains::evm) const SYMBOL: [u8; 4] = [0x95, 0xd8, 0x9b, 0x41];
pub(in crate::chains::evm) const DECIMALS: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67];

pub(in crate::chains::evm) fn balance(owner: Address) -> Vec<u8> {
    let mut bytes = vec![0x70, 0xa0, 0x82, 0x31];
    address_word(&mut bytes, owner);
    bytes
}
pub(in crate::chains::evm) fn allowance(owner: Address, spender: Address) -> Vec<u8> {
    let mut bytes = vec![0xdd, 0x62, 0xed, 0x3e];
    address_word(&mut bytes, owner);
    address_word(&mut bytes, spender);
    bytes
}
fn address_word(bytes: &mut Vec<u8>, address: Address) {
    bytes.extend_from_slice(&[0; 12]);
    bytes.extend_from_slice(&address.bytes());
}
pub(in crate::chains::evm) fn uint256(data: &Data) -> Result<U256, Error> {
    if data.bytes().len() != 32 {
        return Err(invalid_response());
    }
    Ok(U256::from_be_slice(data.bytes()))
}
pub(in crate::chains::evm) fn uint8(data: &Data) -> Result<u8, Error> {
    let bytes = data.bytes();
    if bytes.len() != 32 || bytes[..31].iter().any(|b| *b != 0) {
        return Err(invalid_response());
    }
    Ok(bytes[31])
}
pub(in crate::chains::evm) fn text(data: &Data) -> Result<MetadataText, Error> {
    let bytes = data.bytes();
    if bytes.len() < 64 || U256::from_be_slice(&bytes[..32]) != U256::from(32) {
        return Err(invalid_response());
    }
    let length =
        usize::try_from(U256::from_be_slice(&bytes[32..64])).map_err(|_| invalid_response())?;
    if length > MetadataText::MAX_BYTES {
        return Err(invalid_response());
    }
    let padded = length
        .checked_add(31)
        .map(|v| v / 32 * 32)
        .ok_or_else(invalid_response)?;
    let expected = 64_usize.checked_add(padded).ok_or_else(invalid_response)?;
    if bytes.len() != expected || bytes[64 + length..].iter().any(|b| *b != 0) {
        return Err(invalid_response());
    }
    let value = std::str::from_utf8(&bytes[64..64 + length]).map_err(|_| invalid_response())?;
    MetadataText::new(value.to_owned()).map_err(|_| invalid_response())
}
