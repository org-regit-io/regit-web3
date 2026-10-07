// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{V3QuoteReader, wire};
use crate::{
    chains::evm::EvmClient,
    config::EvmConfig,
    domain::{
        BlockSelector,
        evm::{AccountNonce, OperationObservation},
        uniswap::{
            RouteComparison, RouteComparisonData, RouteOutcome, V3Deployment, V3QuoteObservation,
            V3QuoteRequest, V3RouteRequest,
        },
    },
    error::{Error, ValidationError},
    transport::OperationBudget,
};
use std::fmt;

/// Direct Uniswap V3 `QuoterV2` reader with declared Universal Router 2.1.2 preparation identities.
///
/// Composes the existing bounded EVM backend; no Uniswap API key or distinct
/// transport is required. Callers own RPC access and a Tokio runtime with I/O
/// and time enabled. A declared deployment is not contract-code attestation.
/// Reads never sign, approve or submit transactions.
pub struct UniswapV3Client {
    evm: EvmClient,
    deployment: V3Deployment,
}
impl UniswapV3Client {
    /// Establishes the bounded EVM client after local expected-chain matching.
    /// # Errors
    /// Reports configuration/deployment mismatch or EVM establishment failures.
    /// # Panics
    /// The caller's Tokio runtime must have I/O and time drivers enabled.
    pub async fn connect(config: EvmConfig, deployment: V3Deployment) -> Result<Self, Error> {
        if config.network().chain_id() != deployment.chain_id() {
            return Err(invalid());
        }
        Self::from_evm_client(EvmClient::connect(config).await?, deployment)
    }
    /// Composes an already established EVM client with exact declared contract identities.
    /// # Errors
    /// Rejects a deployment chain different from the client's explicit expected chain.
    pub fn from_evm_client(evm: EvmClient, deployment: V3Deployment) -> Result<Self, Error> {
        if evm.config().network().chain_id() != deployment.chain_id() {
            return Err(invalid());
        }
        Ok(Self { evm, deployment })
    }
    /// Returns the bounded EVM client's explicit configuration.
    #[must_use]
    pub const fn config(&self) -> &EvmConfig {
        self.evm.config()
    }
    /// Returns the exact declared supported-version contracts.
    #[must_use]
    pub const fn deployment(&self) -> &V3Deployment {
        &self.deployment
    }
    /// Quotes one exact V3 path using a captured canonical hash and frozen-hash retries.
    ///
    /// One configured total budget covers the anchor nonce read, all EVM stages,
    /// retries and bounded ABI decoding. The nonce observation captures state
    /// only; no source nonce replaces the explicit simulated nonce. Source
    /// `QuoterV2` gas is not an actual router transaction gas limit. Finality stays
    /// unknown. The exact returned source call must match every requested field.
    /// # Errors
    /// Reports deployment mismatch, source revert, bad ABI, identity/state or backend failures.
    /// # Panics
    /// The caller's Tokio runtime must have I/O and time drivers enabled.
    pub async fn quote_exact_input(
        &self,
        request: V3QuoteRequest,
        selector: Option<BlockSelector>,
    ) -> Result<V3QuoteObservation, Error> {
        self.validate_deployment(&request.data().deployment)?;
        let budget = OperationBudget::new(self.config().limits())?;
        budget
            .run(async {
                let anchor = self
                    .anchor(request.data().settings.data().from, selector)
                    .await?;
                self.quote_at(request, &anchor).await
            })
            .await
    }
    /// Compares bounded caller-supplied V3 paths in order at the same captured hash.
    ///
    /// Source code-3 reverts retain their exact candidate. Unknown errors abort
    /// the operation; no revert proves a missing pool. Best means highest raw
    /// output among supplied successful quotes, with first-candidate tie breaking.
    /// Every candidate and retry uses the captured hash, with no newer-head fallback.
    /// One total budget spans all reads/candidates/decoding; no write is possible.
    /// # Errors
    /// Reports deployment mismatch, malformed/mismatched responses or backend failures.
    /// # Panics
    /// The caller's Tokio runtime must have I/O and time drivers enabled.
    pub async fn compare_routes(
        &self,
        request: V3RouteRequest,
        selector: Option<BlockSelector>,
    ) -> Result<RouteComparison, Error> {
        self.validate_deployment(&request.data().deployment)?;
        let budget = OperationBudget::new(self.config().limits())?;
        budget
            .run(async {
                let anchor = self
                    .anchor(request.data().settings.data().from, selector)
                    .await?;
                let mut outcomes = Vec::with_capacity(request.data().paths.len());
                for index in 0..request.data().paths.len() {
                    let candidate = request.candidate(index)?;
                    match self.quote_at(candidate.clone(), &anchor).await {
                        Ok(quote) => outcomes.push(RouteOutcome::Quoted {
                            quote: Box::new(quote),
                        }),
                        Err(Error::ExecutionReverted) => outcomes.push(RouteOutcome::Reverted {
                            request: Box::new(candidate),
                        }),
                        Err(error) => return Err(error),
                    }
                }
                RouteComparison::new(RouteComparisonData {
                    request,
                    anchor,
                    outcomes,
                })
            })
            .await
    }
    async fn anchor(
        &self,
        address: crate::domain::Address,
        selector: Option<BlockSelector>,
    ) -> Result<OperationObservation<AccountNonce>, Error> {
        let anchor = self.evm.get_account_nonce(address, selector).await?;
        if anchor.value().address != address
            || anchor.value().chain_id != self.deployment.chain_id()
        {
            return Err(invalid());
        }
        Ok(anchor)
    }
    async fn quote_at(
        &self,
        request: V3QuoteRequest,
        anchor: &OperationObservation<AccountNonce>,
    ) -> Result<V3QuoteObservation, Error> {
        let block = wire::anchor_block(anchor)?;
        let call = self
            .evm
            .call(request.call()?, Some(BlockSelector::Hash(*block.hash())))
            .await?;
        wire::quote(request, anchor, &call)
    }
    fn validate_deployment(&self, deployment: &V3Deployment) -> Result<(), Error> {
        if *deployment != self.deployment {
            return Err(invalid());
        }
        Ok(())
    }
}
impl V3QuoteReader for UniswapV3Client {
    async fn quote_exact_input(
        &self,
        request: V3QuoteRequest,
        selector: Option<BlockSelector>,
    ) -> Result<V3QuoteObservation, Error> {
        Self::quote_exact_input(self, request, selector).await
    }
    async fn compare_routes(
        &self,
        request: V3RouteRequest,
        selector: Option<BlockSelector>,
    ) -> Result<RouteComparison, Error> {
        Self::compare_routes(self, request, selector).await
    }
}
impl fmt::Debug for UniswapV3Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UniswapV3Client").finish_non_exhaustive()
    }
}
fn invalid() -> Error {
    ValidationError::InvalidUniswapDeployment.into()
}
