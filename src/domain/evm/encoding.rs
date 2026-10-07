// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use alloy_rlp::Header;
use sha3::{Digest, Keccak256};

use super::{AccessListEntry, Address, Data, Quantity, U256, Word};
use crate::error::{Error, ValidationError};

pub(super) fn invalid() -> Error {
    ValidationError::InvalidEvmSignedTransaction.into()
}

pub(super) fn hash(bytes: &[u8]) -> Word {
    Word::from_bytes(Keccak256::digest(bytes).into())
}

pub(super) fn bytes(value: &[u8]) -> Vec<u8> {
    alloy_rlp::encode(value)
}

pub(super) fn integer(value: U256) -> Vec<u8> {
    let bytes = value.to_be_bytes::<32>();
    let first = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len());
    self::bytes(&bytes[first..])
}

pub(super) fn list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload_length = items.iter().map(Vec::len).sum();
    let mut out = Vec::new();
    Header {
        list: true,
        payload_length,
    }
    .encode(&mut out);
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

pub(super) fn access_list(entries: &[AccessListEntry]) -> Vec<u8> {
    list(
        &entries
            .iter()
            .map(|entry| {
                list(&[
                    bytes(&entry.address.bytes()),
                    list(
                        &entry
                            .storage_keys
                            .iter()
                            .map(|key| bytes(&key.bytes()))
                            .collect::<Vec<_>>(),
                    ),
                ])
            })
            .collect::<Vec<_>>(),
    )
}

pub(super) fn validate_access_list(entries: &[AccessListEntry]) -> Result<(), Error> {
    if entries.len() > 1024
        || entries
            .iter()
            .try_fold(0_usize, |n, entry| n.checked_add(entry.storage_keys.len()))
            .is_none_or(|n| n > 4096)
    {
        return Err(ValidationError::InvalidEvmPreparation.into());
    }
    Ok(())
}

// Walk only the supported fixed-depth schema. Maintained Header decoding checks
// canonical lengths before every bounded slice; unknown recursive structures
// are rejected rather than decoded into an unbounded generic tree.
pub(super) fn item<'a>(input: &mut &'a [u8], is_list: bool) -> Result<&'a [u8], Error> {
    Header::decode_bytes(input, is_list).map_err(|_| invalid())
}

pub(super) fn uint(input: &mut &[u8]) -> Result<Quantity, Error> {
    let value = item(input, false)?;
    if value.len() > 32 || value.first() == Some(&0) {
        return Err(invalid());
    }
    Ok(Quantity::new(U256::from_be_slice(value)))
}

pub(super) fn uint64(input: &mut &[u8]) -> Result<u64, Error> {
    u64::try_from(uint(input)?.value()).map_err(|_| invalid())
}

pub(super) fn address(input: &mut &[u8]) -> Result<Option<Address>, Error> {
    let value = item(input, false)?;
    if value.is_empty() {
        return Ok(None);
    }
    let value: [u8; 20] = value.try_into().map_err(|_| invalid())?;
    Ok(Some(Address::from_bytes(value)))
}

pub(super) fn data(input: &mut &[u8]) -> Result<Data, Error> {
    Data::new(item(input, false)?.to_vec()).map_err(|_| invalid())
}

pub(super) fn decode_access_list(input: &mut &[u8]) -> Result<Vec<AccessListEntry>, Error> {
    let mut payload = item(input, true)?;
    let mut entries = Vec::new();
    let mut keys = 0_usize;
    while !payload.is_empty() {
        if entries.len() == 1024 {
            return Err(invalid());
        }
        let mut entry = item(&mut payload, true)?;
        let address = address(&mut entry)?.ok_or_else(invalid)?;
        let mut key_bytes = item(&mut entry, true)?;
        if !entry.is_empty() {
            return Err(invalid());
        }
        let mut storage_keys = Vec::new();
        while !key_bytes.is_empty() {
            if keys == 4096 {
                return Err(invalid());
            }
            let key = item(&mut key_bytes, false)?;
            storage_keys.push(Word::from_bytes(key.try_into().map_err(|_| invalid())?));
            keys += 1;
        }
        entries.push(AccessListEntry {
            address,
            storage_keys,
        });
    }
    Ok(entries)
}
