// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, ValidationError};

macro_rules! base58_type {
    ($name:ident, $inner:ty, $size:literal, $reason:ident, $description:literal, $from:expr, $bytes:expr) => {
        #[doc = $description]
        ///
        /// Parsing validates canonical base58 encoding and byte width only.
        /// It does not prove curve membership, ownership, or cryptographic validity.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name($inner);

        impl $name {
            /// Parses a canonical base58 string of the required byte width.
            ///
            /// # Errors
            /// Rejects invalid encoding or width with a fixed diagnostic.
            pub fn parse(value: &str) -> Result<Self, Error> {
                let inner =
                    <$inner>::from_str(value).map_err(|_| Error::from(ValidationError::$reason))?;
                if inner.to_string() != value {
                    return Err(ValidationError::$reason.into());
                }
                Ok(Self(inner))
            }

            /// Constructs a value from its exact bytes without verification.
            #[must_use]
            pub fn from_bytes(bytes: [u8; $size]) -> Self {
                Self(($from)(bytes))
            }

            /// Returns the exact bytes.
            #[must_use]
            pub fn bytes(self) -> [u8; $size] {
                ($bytes)(self.0)
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

base58_type!(
    Pubkey,
    solana_address::Address,
    32,
    InvalidSolanaPubkey,
    "An encoding-valid 32-byte Solana account address, including off-curve addresses.",
    solana_address::Address::new_from_array,
    |value: solana_address::Address| value.to_bytes()
);
base58_type!(
    Hash,
    solana_hash::Hash,
    32,
    InvalidSolanaHash,
    "An encoding-valid 32-byte Solana hash with canonical base58 serialization.",
    solana_hash::Hash::new_from_array,
    |value: solana_hash::Hash| value.to_bytes()
);
base58_type!(
    Signature,
    solana_signature::Signature,
    64,
    InvalidSolanaSignature,
    "An encoding-valid 64-byte Solana signature, without cryptographic verification.",
    solana_signature::Signature::from,
    |value: solana_signature::Signature| *value.as_array()
);

/// A caller-supplied full genesis hash and independently retained display alias.
///
/// Construction does not fetch or verify the genesis hash. Technical network
/// identity is [`Self::genesis_hash`], rather than the alias or a shortened hash.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NetworkFields")]
pub struct Network {
    genesis_hash: Hash,
    alias: String,
}

impl Network {
    /// Records a full genesis hash and a 1–64 character display label.
    ///
    /// # Errors
    /// Rejects labels outside ASCII letters, digits, `-`, `_`, `.`, and `+`.
    pub fn new(genesis_hash: Hash, alias: impl Into<String>) -> Result<Self, Error> {
        let alias = alias.into();
        if !super::super::valid_label(&alias, 64) {
            return Err(ValidationError::InvalidNetworkAlias.into());
        }
        Ok(Self {
            genesis_hash,
            alias,
        })
    }

    /// Returns the full technical network identity supplied by the caller.
    #[must_use]
    pub const fn genesis_hash(&self) -> Hash {
        self.genesis_hash
    }

    /// Returns the independently retained display alias.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkFields {
    genesis_hash: Hash,
    alias: String,
}

impl TryFrom<NetworkFields> for Network {
    type Error = Error;

    fn try_from(fields: NetworkFields) -> Result<Self, Self::Error> {
        Self::new(fields.genesis_hash, fields.alias)
    }
}
