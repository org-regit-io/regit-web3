// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::error::Error;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Exact TON 256-bit hash bytes, without a transaction or consensus inference.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Hash([u8; 32]);
impl Hash {
    /// All-zero bytes, used by the account history's initial-transaction sentinel.
    pub const ZERO: Self = Self([0; 32]);
    /// Records exact fixed-size bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Parses hexadecimal or canonical standard/URL-safe base64, with optional padding.
    ///
    /// # Errors
    /// Rejects anything other than exactly 32 bytes in a supported encoding.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let hex = text.strip_prefix("0x").unwrap_or(text);
        let mut bytes = [0; 32];
        if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            const_hex::decode_to_slice(hex, &mut bytes).map_err(|_| super::invalid_hash())?;
            return Ok(Self(bytes));
        }
        if !(43..=44).contains(&text.len()) {
            return Err(super::invalid_hash());
        }
        for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
            if let Ok(value) = engine.decode(text)
                && value.len() == 32
                && engine.encode(&value) == text
            {
                bytes.copy_from_slice(&value);
                return Ok(Self(bytes));
            }
        }
        Err(super::invalid_hash())
    }
    /// Returns the exact fixed-size hash bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// Returns lowercase hexadecimal without a prefix.
    #[must_use]
    pub fn to_hex(self) -> String {
        const_hex::encode(self.0)
    }
    /// Returns canonical padded standard base64 for TON Center APIs.
    #[must_use]
    pub fn to_base64(self) -> String {
        STANDARD.encode(self.0)
    }
}
impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TonHash").field(&self.to_hex()).finish()
    }
}
impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}
impl TryFrom<String> for Hash {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<Hash> for String {
    fn from(v: Hash) -> Self {
        v.to_hex()
    }
}

/// Actual TON zero-state identity; it has no block sequence number or shard field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ZeroStateFields")]
pub struct ZeroState {
    workchain: i32,
    root_hash: Hash,
    file_hash: Hash,
}
impl ZeroState {
    /// Records an expected masterchain zero-state using both full hash values.
    ///
    /// # Errors
    /// Rejects a non-masterchain identity or zero hash placeholder.
    pub fn new(workchain: i32, root_hash: Hash, file_hash: Hash) -> Result<Self, Error> {
        if workchain != -1 || root_hash == Hash::ZERO || file_hash == Hash::ZERO {
            return Err(super::invalid_network());
        }
        Ok(Self {
            workchain,
            root_hash,
            file_hash,
        })
    }
    /// Returns the zero-state's workchain, which is masterchain `-1`.
    #[must_use]
    pub const fn workchain(self) -> i32 {
        self.workchain
    }
    /// Returns the expected state-root hash.
    #[must_use]
    pub const fn root_hash(self) -> Hash {
        self.root_hash
    }
    /// Returns the expected zero-state file hash.
    #[must_use]
    pub const fn file_hash(self) -> Hash {
        self.file_hash
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ZeroStateFields {
    workchain: i32,
    root_hash: Hash,
    file_hash: Hash,
}
impl TryFrom<ZeroStateFields> for ZeroState {
    type Error = Error;
    fn try_from(v: ZeroStateFields) -> Result<Self, Error> {
        Self::new(v.workchain, v.root_hash, v.file_hash)
    }
}

/// Explicit network category; friendly address flags do not establish this identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkCategory {
    /// Official mainnet zero-state identity.
    Mainnet,
    /// Official public testnet zero-state identity.
    Testnet,
    /// Caller-supplied separate zero-state; address test-only policy stays explicit.
    Custom,
}
/// Immutable category and exact expected zero-state for an outgoing TON operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct Network {
    category: NetworkCategory,
    zero_state: ZeroState,
}
impl Network {
    /// Checks named public categories against their official zero-state hash pairs.
    /// Custom identities must differ from the two named public networks.
    ///
    /// # Errors
    /// Rejects an incorrectly labelled or incomplete network identity.
    pub fn new(category: NetworkCategory, zero_state: ZeroState) -> Result<Self, Error> {
        let mainnet = public_zero_state(false)?;
        let testnet = public_zero_state(true)?;
        let valid = match category {
            NetworkCategory::Mainnet => zero_state == mainnet,
            NetworkCategory::Testnet => zero_state == testnet,
            NetworkCategory::Custom => zero_state != mainnet && zero_state != testnet,
        };
        if !valid {
            return Err(super::invalid_network());
        }
        Ok(Self {
            category,
            zero_state,
        })
    }
    /// Returns the named public category or explicit custom-network category.
    #[must_use]
    pub const fn category(self) -> NetworkCategory {
        self.category
    }
    /// Returns the full expected zero-state checked by network-aware operations.
    #[must_use]
    pub const fn zero_state(self) -> ZeroState {
        self.zero_state
    }
}
fn public_zero_state(testnet: bool) -> Result<ZeroState, Error> {
    let (root, file) = if testnet {
        (
            "gj+B8wb/AmlPk1z1AhVI484rhrUpgSr2oSFIh56VoSg=",
            "Z+IKwYS54DmmJmesw/nAD5DzWadnOCMzee+kdgSYDOg=",
        )
    } else {
        (
            "F6OpKZKqvqeFp6CQmFomXNMfMj2EnaUSOXN+Mh+wVWk=",
            "XplPz01CXAps5qeSWUtxcyBfdAo5zVb1N979KLSKD24=",
        )
    };
    ZeroState::new(-1, Hash::parse(root)?, Hash::parse(file)?)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkFields {
    category: NetworkCategory,
    zero_state: ZeroState,
}
impl TryFrom<NetworkFields> for Network {
    type Error = Error;
    fn try_from(v: NetworkFields) -> Result<Self, Error> {
        Self::new(v.category, v.zero_state)
    }
}

/// User-friendly metadata, kept separate from the workchain/account identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FriendlyFlags {
    /// Wallet-facing bounce suggestion; transfer preparation uses an explicit choice.
    pub bounceable: bool,
    /// Forbids use by mainnet software; absence does not prove mainnet membership.
    pub test_only: bool,
}
/// Original address form; raw address bytes contain no friendly metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "flags",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AddressFormat {
    /// Workchain and hexadecimal account bytes, with no checksum or flags.
    Raw,
    /// CRC-checked friendly address flags, independent of its base64 alphabet.
    Friendly(FriendlyFlags),
}
/// Standard signed-int8 TON workchain and exact 256-bit account identity.
/// Presentation flags are retained, while `same_account` compares only identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AddressFields")]
pub struct Address {
    workchain: i8,
    account: Hash,
    format: AddressFormat,
}
impl Address {
    /// Records standard account bytes and honest original presentation metadata.
    #[must_use]
    pub const fn new(workchain: i8, account: Hash, format: AddressFormat) -> Self {
        Self {
            workchain,
            account,
            format,
        }
    }
    /// Parses raw or 48-character CRC-checked standard/URL-safe friendly text.
    ///
    /// # Errors
    /// Rejects malformed workchain/hex, checksum, flags or base64 input.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if let Some((workchain, account)) = text.split_once(':') {
            let digits = workchain.strip_prefix('-').unwrap_or(workchain);
            if digits.is_empty()
                || !digits.bytes().all(|b| b.is_ascii_digit())
                || workchain.len() > 4
                || account.len() != 64
                || !account.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(super::invalid_address());
            }
            let workchain = workchain
                .parse::<i8>()
                .map_err(|_| super::invalid_address())?;
            let account = Hash::parse(account).map_err(|_| super::invalid_address())?;
            return Ok(Self::new(workchain, account, AddressFormat::Raw));
        }
        if text.len() != 48 {
            return Err(super::invalid_address());
        }
        let bytes = STANDARD
            .decode(text)
            .or_else(|_| URL_SAFE.decode(text))
            .map_err(|_| super::invalid_address())?;
        if bytes.len() != 36 || (STANDARD.encode(&bytes) != text && URL_SAFE.encode(&bytes) != text)
        {
            return Err(super::invalid_address());
        }
        let tag = bytes[0];
        if ![0x11, 0x51, 0x91, 0xd1].contains(&tag) {
            return Err(super::invalid_address());
        }
        let checksum = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM)
            .checksum(&bytes[..34])
            .to_be_bytes();
        if bytes[34..] != checksum {
            return Err(super::invalid_address());
        }
        let mut account = [0; 32];
        account.copy_from_slice(&bytes[2..34]);
        Ok(Self::new(
            i8::from_be_bytes([bytes[1]]),
            Hash::from_bytes(account),
            AddressFormat::Friendly(FriendlyFlags {
                bounceable: tag & 0x40 == 0,
                test_only: tag & 0x80 != 0,
            }),
        ))
    }
    /// Rejects a test-only friendly address when used against mainnet.
    /// Raw/unmarked addresses rely on the explicit caller-qualified network.
    ///
    /// # Errors
    /// Returns a fixed network failure for a test-only/mainnet contradiction.
    pub fn check_network(self, network: Network) -> Result<(), Error> {
        if matches!(
            self.format,
            AddressFormat::Friendly(FriendlyFlags {
                test_only: true,
                ..
            })
        ) && network.category == NetworkCategory::Mainnet
        {
            return Err(super::invalid_network());
        }
        Ok(())
    }
    /// Returns the signed standard workchain.
    #[must_use]
    pub const fn workchain(self) -> i8 {
        self.workchain
    }
    /// Returns the exact account bytes.
    #[must_use]
    pub const fn account(self) -> Hash {
        self.account
    }
    /// Returns the original raw/friendly metadata without a network inference.
    #[must_use]
    pub const fn format(self) -> AddressFormat {
        self.format
    }
    /// Compares workchain/account identity independently of presentation flags.
    #[must_use]
    pub fn same_account(self, other: Self) -> bool {
        self.workchain == other.workchain && self.account.0 == other.account.0
    }
    /// Returns a canonical raw address for explicit node queries.
    #[must_use]
    pub fn to_raw(self) -> String {
        format!("{}:{}", self.workchain, self.account)
    }
    /// Encodes explicit caller-selected friendly metadata with a maintained CRC.
    #[must_use]
    pub fn to_friendly(self, flags: FriendlyFlags, url_safe: bool) -> String {
        let mut bytes = [0; 36];
        bytes[0] =
            (if flags.bounceable { 0x11 } else { 0x51 }) | if flags.test_only { 0x80 } else { 0 };
        bytes[1] = self.workchain.to_be_bytes()[0];
        bytes[2..34].copy_from_slice(self.account.as_bytes());
        let checksum = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM)
            .checksum(&bytes[..34])
            .to_be_bytes();
        bytes[34..].copy_from_slice(&checksum);
        if url_safe {
            URL_SAFE.encode(bytes)
        } else {
            STANDARD.encode(bytes)
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddressFields {
    workchain: i8,
    account: Hash,
    format: AddressFormat,
}
impl TryFrom<AddressFields> for Address {
    type Error = Error;
    fn try_from(v: AddressFields) -> Result<Self, Error> {
        Ok(Self::new(v.workchain, v.account, v.format))
    }
}
