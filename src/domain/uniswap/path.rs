// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::evm::{Address, Data},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// Caller-supplied forward Uniswap V3 exact-input token and fee sequence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V3PathData {
    /// Exact token addresses; native assets require an explicit wrapped token.
    pub tokens: Vec<Address>,
    /// Explicit pool fees in millionths, one per hop; no fee tier is assumed.
    pub fees: Vec<u32>,
}

/// Bounded Uniswap V3 forward path, without pool-existence or liquidity proof.
/// Nonadjacent repeated tokens remain legal; no route discovery is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "V3PathData", into = "V3PathData")]
pub struct V3Path(V3PathData);
impl V3Path {
    /// Maximum supported number of pools per explicit path.
    pub const MAX_HOPS: usize = 8;
    /// Validates exact path cardinality, ordinary token identities and fee width.
    /// Factory-enabled custom fee tiers remain representable; no pool is inferred.
    /// # Errors
    /// Rejects empty/excessive paths, cardinality mismatch, zero or adjacent equal
    /// tokens and fee values outside the V3 factory's less-than-one-million bound.
    pub fn new(data: V3PathData) -> Result<Self, Error> {
        if data.fees.is_empty()
            || data.fees.len() > Self::MAX_HOPS
            || data.tokens.len() != data.fees.len() + 1
            || data
                .tokens
                .iter()
                .any(|a| *a == Address::from_bytes([0; 20]))
            || data.tokens.windows(2).any(|p| p[0] == p[1])
            || data.fees.iter().any(|f| *f >= 1_000_000)
        {
            return Err(ValidationError::InvalidUniswapPath.into());
        }
        Ok(Self(data))
    }
    /// Returns all exact tokens and fees without mutable access.
    #[must_use]
    pub const fn data(&self) -> &V3PathData {
        &self.0
    }
    /// Returns the number of pools in the path.
    #[must_use]
    pub fn hops(&self) -> usize {
        self.0.fees.len()
    }
    /// Returns the first exact token identity.
    #[must_use]
    pub fn token_in(&self) -> Address {
        self.0.tokens[0]
    }
    /// Returns the final exact token identity.
    #[must_use]
    pub fn token_out(&self) -> Address {
        self.0.tokens[self.0.fees.len()]
    }
    /// Encodes exact forward packed token/uint24-fee bytes.
    /// # Errors
    /// Reports local byte-bound excess.
    pub fn encoded(&self) -> Result<Data, Error> {
        let mut bytes = Vec::with_capacity(20 + 23 * self.hops());
        bytes.extend(self.token_in().bytes());
        for (index, fee) in self.0.fees.iter().enumerate() {
            bytes.extend(&fee.to_be_bytes()[1..]);
            bytes.extend(self.0.tokens[index + 1].bytes());
        }
        Data::new(bytes)
    }
}
impl TryFrom<V3PathData> for V3Path {
    type Error = Error;
    fn try_from(data: V3PathData) -> Result<Self, Error> {
        Self::new(data)
    }
}
impl From<V3Path> for V3PathData {
    fn from(value: V3Path) -> Self {
        value.0
    }
}
