// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::Hash;
use crate::error::Error;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use serde::{Deserialize, Serialize};
use std::fmt;
use tycho_types::{boc::Boc as Codec, cell::Cell};

/// One bounded, completely framed TON bag of cells, with a maintained representation hash.
///
/// This checks cell-container structure, CRC when supplied, one root, at most
/// 64 KiB/8192 cells/256 levels. It does not verify signatures, a message's
/// wallet intent or transaction consensus inclusion. Indexed encodings retain
/// their exact bytes; this type makes no canonical-container claim.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Boc {
    bytes: Vec<u8>,
}
impl Boc {
    /// Maximum encoded byte length before allocating or decoding a cell graph.
    pub const MAX_BYTES: usize = 65_536;
    /// Parses canonical standard/URL-safe base64 with or without padding.
    ///
    /// # Errors
    /// Rejects oversized, malformed, trailing, multi-root or excessively deep BOCs.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.len() > Self::MAX_BYTES.div_ceil(3) * 4 {
            return Err(super::invalid_boc());
        }
        for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
            if let Ok(bytes) = engine.decode(text)
                && engine.encode(&bytes) == text
            {
                return Self::from_bytes(bytes);
            }
        }
        Err(super::invalid_boc())
    }
    /// Checks complete container framing before maintained cell decoding.
    ///
    /// # Errors
    /// Rejects invalid container bounds, checksum or cell structure.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        framing(&bytes)?;
        let root = Codec::decode(&bytes).map_err(|_| super::invalid_boc())?;
        if root.repr_depth() > 256 {
            return Err(super::invalid_boc());
        }
        Ok(Self { bytes })
    }
    /// Returns the exact bytes, which need not use the codec's preferred container layout.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Returns standard padded base64 for an explicit TON API request.
    #[must_use]
    pub fn to_base64(&self) -> String {
        STANDARD.encode(&self.bytes)
    }
    /// Returns the root-cell representation hash, independently of BOC framing.
    ///
    /// # Errors
    /// Returns a fixed failure if maintained decoding cannot reproduce the graph.
    pub fn hash(&self) -> Result<Hash, Error> {
        Ok(Hash::from_bytes(self.root()?.repr_hash().0))
    }
    pub(crate) fn root(&self) -> Result<Cell, Error> {
        Codec::decode(&self.bytes).map_err(|_| super::invalid_boc())
    }
    pub(crate) fn from_cell(cell: &Cell) -> Result<Self, Error> {
        Self::from_bytes(Codec::encode(cell))
    }
}
impl fmt::Debug for Boc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TonBoc")
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}
impl TryFrom<String> for Boc {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<Boc> for String {
    fn from(v: Boc) -> Self {
        v.to_base64()
    }
}

// Maintained decoder accepts a valid prefix; validate total framing ourselves.
// All arithmetic and slicing remain bounded before invoking that decoder.
fn framing(bytes: &[u8]) -> Result<(), Error> {
    let fail = super::invalid_boc;
    if bytes.len() < 6 || bytes.len() > Boc::MAX_BYTES {
        return Err(fail());
    }
    let (index, crc, ref_size, generic) = match bytes.get(..4) {
        Some([0xb5, 0xee, 0x9c, 0x72]) => {
            if bytes[4] & 0x18 != 0 {
                return Err(fail());
            }
            (
                bytes[4] & 0x80 != 0,
                bytes[4] & 0x40 != 0,
                usize::from(bytes[4] & 7),
                true,
            )
        }
        Some([0x68, 0xff, 0x65, 0xf3]) => (true, false, usize::from(bytes[4]), false),
        Some([0xac, 0xc3, 0xa7, 0x28]) => (true, true, usize::from(bytes[4]), false),
        _ => return Err(fail()),
    };
    let offset_size = usize::from(bytes[5]);
    if !(1..=4).contains(&ref_size) || !(1..=8).contains(&offset_size) {
        return Err(fail());
    }
    let mut offset = 6;
    let cells = read_uint(bytes, &mut offset, ref_size)?;
    let roots = read_uint(bytes, &mut offset, ref_size)?;
    let absent = read_uint(bytes, &mut offset, ref_size)?;
    let cell_bytes = read_uint(bytes, &mut offset, offset_size)?;
    if cells == 0 || cells > 8192 || roots != 1 || absent != 0 {
        return Err(fail());
    }
    let roots_size = if generic { ref_size } else { 0 };
    let index_size = if index {
        cells.checked_mul(offset_size).ok_or_else(fail)?
    } else {
        0
    };
    let length = offset
        .checked_add(roots_size)
        .and_then(|n| n.checked_add(index_size))
        .and_then(|n| n.checked_add(cell_bytes))
        .and_then(|n| n.checked_add(if crc { 4 } else { 0 }))
        .ok_or_else(fail)?;
    if length != bytes.len() {
        return Err(fail());
    }
    Ok(())
}
fn read_uint(bytes: &[u8], offset: &mut usize, size: usize) -> Result<usize, Error> {
    let end = offset.checked_add(size).ok_or_else(super::invalid_boc)?;
    let chunk = bytes.get(*offset..end).ok_or_else(super::invalid_boc)?;
    let mut n = 0usize;
    for b in chunk {
        n = n
            .checked_mul(256)
            .and_then(|n| n.checked_add(usize::from(*b)))
            .ok_or_else(super::invalid_boc)?;
    }
    *offset = end;
    Ok(n)
}
