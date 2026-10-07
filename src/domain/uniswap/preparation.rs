// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{CallSettings, V3QuoteObservation, encoding, quote::canonical_block};
use crate::{
    domain::evm::{Address, ChainId, Quantity, Timestamp, TransactionCall, U256},
    error::{Error, ValidationError},
    wallets::Preparation,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Explicit caller-chosen slippage tolerance, in basis points (0 through 10,000).
/// A value of 10,000 deliberately allows zero minimum output; no policy is invented.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct SlippageBps(u16);
impl SlippageBps {
    /// Validates the exact caller tolerance without choosing a default.
    /// # Errors
    /// Rejects a value above 10,000.
    pub fn new(value: u16) -> Result<Self, Error> {
        if value > 10_000 {
            return Err(invalid());
        }
        Ok(Self(value))
    }
    /// Returns the explicit tolerance.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
    /// Computes floor(output * (10,000 - tolerance) / 10,000) without U256 overflow.
    #[must_use]
    pub fn minimum_output(self, output: Quantity) -> Quantity {
        let scale = U256::from(10_000);
        let retained = U256::from(10_000 - self.0);
        Quantity::new(
            (output.value() / scale) * retained + (output.value() % scale) * retained / scale,
        )
    }
}
impl TryFrom<u16> for SlippageBps {
    type Error = Error;
    fn try_from(value: u16) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<SlippageBps> for u16 {
    fn from(value: SlippageBps) -> Self {
        value.0
    }
}

/// Complete reviewed V3 swap intent for Universal Router 2.1.2.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwapIntentData {
    /// Original exact quote, including deployment/path/input/output and captured state.
    pub quote: V3QuoteObservation,
    /// Literal token-output recipient; router special-marker addresses are rejected.
    pub recipient: Address,
    /// Explicit tolerance used to derive the aggregate output floor.
    pub slippage: SlippageBps,
    /// Explicit Unix-second router deadline, no earlier than quoted block time.
    pub deadline: Timestamp,
    /// Explicit actual transaction sender, nonce, gas limit and fees.
    pub settings: CallSettings,
}
/// Immutable intent with no automatic approval, signing, quote refresh or submission.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SwapIntentData", into = "SwapIntentData")]
pub struct SwapIntent(SwapIntentData);
impl SwapIntent {
    /// Validates literal recipient, Permit2 uint160 input and quoted-time deadline.
    /// Current wall time and future liquidity are not inferred from this offline check.
    /// # Errors
    /// Rejects reserved recipients, input beyond Permit2 transfer width or stale quoted-time deadline.
    pub fn new(data: SwapIntentData) -> Result<Self, Error> {
        let recipient = U256::from_be_slice(&data.recipient.bytes());
        if recipient <= U256::from(2)
            || data
                .quote
                .value()
                .data()
                .request
                .data()
                .amount_in
                .value()
                .bit_len()
                > 160
            || data.deadline < canonical_block(data.quote.context())?.timestamp()
        {
            return Err(invalid());
        }
        Ok(Self(data))
    }
    /// Returns the full original quote and immutable reviewed choices.
    #[must_use]
    pub const fn data(&self) -> &SwapIntentData {
        &self.0
    }
    /// Returns exact floor-rounded aggregate minimum output raw units.
    #[must_use]
    pub fn minimum_output(&self) -> Quantity {
        self.0
            .slippage
            .minimum_output(self.0.quote.value().data().amount_out)
    }
}
impl fmt::Debug for SwapIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SwapIntent").finish_non_exhaustive()
    }
}
impl TryFrom<SwapIntentData> for SwapIntent {
    type Error = Error;
    fn try_from(value: SwapIntentData) -> Result<Self, Error> {
        Self::new(value)
    }
}
impl From<SwapIntent> for SwapIntentData {
    fn from(value: SwapIntent) -> Self {
        value.0
    }
}

/// Explicit authorization preconditions for payerIsUser V3 swapping.
/// These are required balances/allowances to arrange separately, not approval transactions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AllowanceRequirements {
    /// Exact input token contract.
    pub token: Address,
    /// Actual reviewed token owner and transaction sender.
    pub owner: Address,
    /// ERC-20 allowance spender: the declared Permit2 contract.
    pub erc20_spender: Address,
    /// Permit2 allowance spender: the declared Universal Router 2.1.2 contract.
    pub permit2_spender: Address,
    /// Exact raw amount required by this unsigned swap.
    pub amount: Quantity,
}
/// Immutable fields-based Universal Router 2.1.2 call and authorization requirements.
/// This is not a canonical EVM signing payload and does not verify allowances,
/// sender recovery, signatures or future execution. No native wrapping is implicit.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct UnsignedSwap {
    call: TransactionCall,
    allowances: AllowanceRequirements,
}
impl UnsignedSwap {
    /// Returns the exact transaction fields, including router, calldata and zero native value.
    #[must_use]
    pub const fn call(&self) -> &TransactionCall {
        &self.call
    }
    /// Returns explicit separate ERC-20 and Permit2 allowance preconditions.
    #[must_use]
    pub const fn allowances(&self) -> &AllowanceRequirements {
        &self.allowances
    }
}
impl fmt::Debug for UnsignedSwap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnsignedSwap").finish_non_exhaustive()
    }
}

/// Immutable prepared V3 swap for typed wallet review and external handoff.
/// Callers own approval, signing and semantic verification of returned signed bytes.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparedSwap {
    intent: SwapIntent,
    unsigned: UnsignedSwap,
}
impl PreparedSwap {
    /// Derives exact router command/calldata and allowance preconditions from validated intent.
    /// No network operation, signing, approval or submission occurs.
    /// # Errors
    /// Reports local encoding/resource or explicit envelope validation failures.
    pub fn new(intent: SwapIntent) -> Result<Self, Error> {
        let request = intent.data().quote.value().data().request.data();
        let deployment = request.deployment.data();
        let call = intent.data().settings.call(
            deployment.chain_id,
            deployment.universal_router_2_1_2,
            encoding::swap_call(&intent)?,
        )?;
        let allowances = AllowanceRequirements {
            token: request.path.token_in(),
            owner: intent.data().settings.data().from,
            erc20_spender: deployment.permit2,
            permit2_spender: deployment.universal_router_2_1_2,
            amount: request.amount_in,
        };
        Ok(Self {
            intent,
            unsigned: UnsignedSwap { call, allowances },
        })
    }
    /// Returns the complete reviewed immutable swap intent.
    #[must_use]
    pub const fn intent(&self) -> &SwapIntent {
        &self.intent
    }
    /// Returns derived unsigned fields and explicit authorization preconditions.
    #[must_use]
    pub const fn unsigned(&self) -> &UnsignedSwap {
        &self.unsigned
    }
}
impl Preparation for PreparedSwap {
    type Network = ChainId;
    type Intent = SwapIntent;
    type UnsignedPayload = UnsignedSwap;
    fn network(&self) -> &ChainId {
        self.intent
            .data()
            .quote
            .value()
            .data()
            .request
            .data()
            .deployment
            .chain_id_ref()
    }
    fn intent(&self) -> &SwapIntent {
        &self.intent
    }
    fn unsigned_payload(&self) -> &UnsignedSwap {
        &self.unsigned
    }
    fn validate(&self) -> Result<(), Error> {
        if *self != Self::new(self.intent.clone())? {
            return Err(invalid());
        }
        Ok(())
    }
}
impl fmt::Debug for PreparedSwap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedSwap").finish_non_exhaustive()
    }
}
impl Serialize for PreparedSwap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.intent.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for PreparedSwap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(SwapIntent::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
fn invalid() -> Error {
    ValidationError::InvalidUniswapPreparation.into()
}
