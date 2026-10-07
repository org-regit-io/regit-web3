// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::evm::{Address, ChainId},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// Explicit contracts for Uniswap V3 `QuoterV2` and Universal Router 2.1.2.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V3DeploymentData {
    /// Exact expected RPC chain identity.
    pub chain_id: ChainId,
    /// Expected Uniswap V3 factory shared by the declared contracts.
    pub factory: Address,
    /// Explicit deployed `QuoterV2` address, not the earlier Quoter ABI.
    pub quoter_v2: Address,
    /// Explicit Universal Router 2.1.2 address with six-field V3 command input.
    pub universal_router_2_1_2: Address,
    /// The deployment's explicit Permit2 contract.
    pub permit2: Address,
}

/// Validated declared deployment identities, without code or immutable-storage attestation.
///
/// The adapter implements only V3 `QuoterV2` and Universal Router 2.1.2 ABIs.
/// Callers must confirm supplied contracts implement those versions and share
/// the expected factory/Permit2. A network check does not prove contract code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "V3DeploymentData", into = "V3DeploymentData")]
pub struct V3Deployment(V3DeploymentData);
impl V3Deployment {
    /// Validates nonzero, distinct declared contract identities.
    /// # Errors
    /// Rejects an unusable or repeated contract address without echoing input.
    pub fn new(data: V3DeploymentData) -> Result<Self, Error> {
        let addresses = [
            data.factory,
            data.quoter_v2,
            data.universal_router_2_1_2,
            data.permit2,
        ];
        for (index, address) in addresses.iter().enumerate() {
            if *address == Address::from_bytes([0; 20]) || addresses[..index].contains(address) {
                return Err(ValidationError::InvalidUniswapDeployment.into());
            }
        }
        Ok(Self(data))
    }
    /// Returns the explicitly declared immutable contract identities.
    #[must_use]
    pub const fn data(&self) -> &V3DeploymentData {
        &self.0
    }
    /// Returns the expected chain, independent of aliases.
    #[must_use]
    pub const fn chain_id(&self) -> ChainId {
        self.0.chain_id
    }
    pub(super) const fn chain_id_ref(&self) -> &ChainId {
        &self.0.chain_id
    }
    /// Returns documented Ethereum mainnet V3/Universal Router 2.1.2 identities.
    ///
    /// Values follow the official deployment feed's 2026-09-22 source inventory.
    /// Choosing this declaration does not attest live contract code or liquidity.
    /// # Errors
    /// Reports invalid compiled deployment identities.
    pub fn ethereum_mainnet() -> Result<Self, Error> {
        Self::new(V3DeploymentData {
            chain_id: ChainId::from(1),
            factory: Address::parse("0x1f98431c8ad98523631ae4a59f267346ea31f984")?,
            quoter_v2: Address::parse("0x61ffe014ba17989e743c5f6cb21bf9697530b21e")?,
            universal_router_2_1_2: Address::parse("0x23617e59a5925b2a4bf75d73ff6711cd0b29de85")?,
            permit2: Address::parse("0x000000000022d473030f116ddee9f6b43ac78ba3")?,
        })
    }
}
impl TryFrom<V3DeploymentData> for V3Deployment {
    type Error = Error;
    fn try_from(data: V3DeploymentData) -> Result<Self, Error> {
        Self::new(data)
    }
}
impl From<V3Deployment> for V3DeploymentData {
    fn from(value: V3Deployment) -> Self {
        value.0
    }
}
