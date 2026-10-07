// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Account, Chain, Family, FreshQuote, Identifier, Quote};
use crate::{
    domain::{
        Address, Amount,
        evm::{Data, Quantity},
        solana::{Pubkey, UnsignedTransaction},
    },
    error::Error,
    wallets::Preparation,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

/// Caller-reviewed direct preparation constraints and original source selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationRequestData {
    /// Original selected quote, including original source amounts and caller request.
    pub quote: Quote,
    /// Explicit actual source account; never signed or authenticated by this type.
    pub sender: Account,
    /// Explicit destination account.
    pub receiver: Account,
    /// Explicit source-estimate output floor; the earlier source minimum remains
    /// a separate review fact, without inventing the caller's acceptance policy.
    pub minimum_output: Amount,
    /// Explicit EVM native-call value ceiling; unavailable precision is not assumed.
    /// For Solana this is retained review data and does not verify native spending.
    pub maximum_native_value: Amount,
    /// Mandatory exact EVM router for this supported EVM profile; absent for Solana.
    pub expected_evm_router: Option<Address>,
    /// Explicit exact allowed EVM allowance suggestion, including absence.
    pub expected_approval: Option<Address>,
    /// Explicit exact allowed Permit2 allowance suggestion, including absence.
    pub expected_permit2: Option<Address>,
    /// Exact ordered expected Solana required signers; empty for EVM.
    pub expected_solana_signers: Vec<Pubkey>,
}
/// Immutable validated direct preparation request, without a signing backend.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PreparationRequestData", into = "PreparationRequestData")]
pub struct PreparationRequest(PreparationRequestData);
impl PreparationRequest {
    /// Checks accounts, original floor, exact router/spenders and signer capacities.
    /// # Errors
    /// Rejects contradictory caller choices or unsupported preparation families.
    pub fn new(v: PreparationRequestData) -> Result<Self, Error> {
        let q = v.quote.data();
        let r = q.request.data();
        v.sender.validate(r.source.chain())?;
        v.receiver.validate(r.destination.chain())?;
        if r.sender.as_ref().is_some_and(|a| a != &v.sender)
            || r.receiver.as_ref().is_some_and(|a| a != &v.receiver)
            || v.minimum_output.decimals() != Some(r.destination.decimals())
            || v.maximum_native_value.decimals() != Some(q.fees.native_token.asset.decimals())
            || v.expected_approval != q.approval_address
            || v.expected_permit2 != q.permit2_address
        {
            return Err(super::invalid_preparation());
        }
        match r.source.chain().family() {
            Family::Evm { .. } => {
                if v.expected_evm_router.is_none() || !v.expected_solana_signers.is_empty() {
                    return Err(super::invalid_preparation());
                }
            }
            Family::Solana { .. } => {
                let mut ids = BTreeSet::new();
                if v.expected_evm_router.is_some()
                    || v.expected_approval.is_some()
                    || v.expected_permit2.is_some()
                    || v.expected_solana_signers.is_empty()
                    || v.expected_solana_signers.len() > 16
                    || v.expected_solana_signers
                        .iter()
                        .any(|p| !ids.insert(p.to_string()))
                    || !matches!(&v.sender,Account::Solana(p) if v.expected_solana_signers.first()==Some(p))
                {
                    return Err(super::invalid_preparation());
                }
            }
            Family::Provider { .. } => return Err(Error::UnsupportedCapability),
        }
        Ok(Self(v))
    }
    /// Returns all immutable caller constraints and original source selection.
    #[must_use]
    pub const fn data(&self) -> &PreparationRequestData {
        &self.0
    }
}
impl fmt::Debug for PreparationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparationRequest").finish_non_exhaustive()
    }
}
impl TryFrom<PreparationRequestData> for PreparationRequest {
    type Error = Error;
    fn try_from(v: PreparationRequestData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<PreparationRequest> for PreparationRequestData {
    fn from(v: PreparationRequest) -> Self {
        v.0
    }
}

/// Exact source EVM call fields; these omit nonce/fees and are not signing bytes.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvmPayload {
    /// Exact source router address.
    pub to: Address,
    /// Exact native wei, including explicit zero.
    pub value: Quantity,
    /// Exact bounded source calldata, without ABI-semantic verification.
    pub input: Data,
}
impl fmt::Debug for EvmPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvmPayload").finish_non_exhaustive()
    }
}
/// Supported unsigned family representation; parsing never proves reviewed intent.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "family",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Payload {
    /// Unsigned EVM call fields, with no canonical envelope or general expiry claim.
    Evm(EvmPayload),
    /// Maintained structurally validated canonical zero-signature Solana transaction.
    /// Its embedded blockhash has no source-reported last-valid height here.
    Solana(UnsignedTransaction),
}
impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Payload").finish_non_exhaustive()
    }
}
/// Actual source-expanded filter facts; these are not caller-selected policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ForeignFields")]
pub struct ForeignFilters {
    provider: Identifier,
    excluded: Vec<super::Text>,
}
impl ForeignFilters {
    /// Retains up to128 unique source-expanded provider labels.
    /// # Errors
    /// Rejects excess or duplicates without truncation.
    pub fn new(provider: Identifier, excluded: Vec<super::Text>) -> Result<Self, Error> {
        let mut ids = BTreeSet::new();
        if excluded.len() > 128 || excluded.iter().any(|p| !ids.insert(p.as_str())) {
            return Err(super::invalid_preparation());
        }
        Ok(Self { provider, excluded })
    }
    /// Returns the exact outer provider.
    #[must_use]
    pub const fn provider(&self) -> &Identifier {
        &self.provider
    }
    /// Returns actual applied source labels.
    #[must_use]
    pub fn excluded(&self) -> &[super::Text] {
        &self.excluded
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForeignFields {
    provider: Identifier,
    excluded: Vec<super::Text>,
}
impl TryFrom<ForeignFields> for ForeignFilters {
    type Error = Error;
    fn try_from(v: ForeignFields) -> Result<Self, Error> {
        Self::new(v.provider, v.excluded)
    }
}

/// Full new review snapshot, with original quote and fresh recalculated source facts.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedSwapData {
    /// All caller constraints and original quote, without inferred approval.
    pub request: PreparationRequest,
    /// Fresh source facts; missing token metadata remains unavailable. Original
    /// complete metadata and amounts are retained separately in request.quote.
    pub fresh_quote: FreshQuote,
    /// Actual source-selected integrator address, including unavailable absence.
    pub source_integrator: Option<Address>,
    /// Actual bounded source-expanded foreign blacklists, keyed by provider.
    pub source_foreign_filters: Vec<ForeignFilters>,
    /// Actual bounded provider-generated IDs; these are not approvals or orders
    /// created by this library, and their contents are opaque in Debug.
    pub source_provider_ids: Vec<SourceId>,
    /// Actual optional provider-specific JSON object. Its bounded opaque contents
    /// have no inferred approval, order-creation or executable-intent semantics.
    pub source_additional_data: Option<SourceAdditionalData>,
    /// Supported family-specific exact unsigned representation.
    pub payload: Payload,
}
/// Immutable review/handoff preparation; source estimates and bytes are unverified semantics.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PreparedSwapData", into = "PreparedSwapData")]
pub struct PreparedSwap(PreparedSwapData);
impl PreparedSwap {
    /// Correlates exact caller choices, fresh route identity, floors and payload bounds.
    /// Matching fields do not prove ABI instructions or actual signed intent.
    /// # Errors
    /// Rejects changed route/provider, caller constraints, family/signers or excess value.
    pub fn new(v: PreparedSwapData) -> Result<Self, Error> {
        let req = v.request.data();
        let old = req.quote.data();
        let fresh = v.fresh_quote.data();
        let r = old.request.data();
        let mut ids = BTreeSet::new();
        if fresh.request != old.request
            || fresh.id != old.id
            || fresh.provider != old.provider
            || fresh.kind != old.kind
            || fresh.approval_address != req.expected_approval
            || fresh.permit2_address != req.expected_permit2
            || fresh.estimate.data().minimum_output.raw() < req.minimum_output.raw()
            || r.integrator.is_some_and(|a| Some(a) != v.source_integrator)
            || v.source_foreign_filters.len() > 16
            || v.source_foreign_filters
                .iter()
                .any(|p| !ids.insert(p.provider.clone()))
            || v.source_provider_ids.len() > 32
            || !crate::domain::market::unique(v.source_provider_ids.iter().map(|p| &p.kind))
        {
            return Err(super::invalid_preparation());
        }
        match (&v.payload, r.source.chain().family()) {
            (Payload::Evm(p), Family::Evm { .. }) => {
                if Some(p.to) != req.expected_evm_router
                    || p.value.value() > req.maximum_native_value.raw()
                    || p.input.bytes().is_empty()
                {
                    return Err(super::invalid_preparation());
                }
            }
            (Payload::Solana(p), Family::Solana { .. }) => {
                let keys = p.message().static_accounts();
                let count = usize::from(p.message().required_signatures());
                if keys.get(..count) != Some(req.expected_solana_signers.as_slice()) {
                    return Err(super::invalid_preparation());
                }
            }
            _ => return Err(super::invalid_preparation()),
        }
        Ok(Self(v))
    }
    /// Returns all binding-relevant caller/source/payload facts for deliberate review.
    #[must_use]
    pub const fn data(&self) -> &PreparedSwapData {
        &self.0
    }
}

/// Exact bounded provider-specific JSON object text with redacted diagnostics.
/// Contents remain unverified source facts, including exact lexical numbers.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceAdditionalData(String);
impl SourceAdditionalData {
    /// Validates an object of at most 64 KiB, 32 nesting levels, 8192 nodes and
    /// 4096 entries per collection, rejecting escaped-equivalent duplicate keys.
    /// # Errors
    /// Rejects malformed, excessive or non-object data with a fixed diagnostic.
    pub fn new(value: &str) -> Result<Self, Error> {
        super::json::additional_object(value).map_err(|_| super::invalid_preparation())?;
        Ok(Self(value.into()))
    }
    /// Returns exact source JSON text for deliberate caller review.
    #[must_use]
    pub fn as_json(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SourceAdditionalData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceAdditionalData")
            .finish_non_exhaustive()
    }
}
impl TryFrom<String> for SourceAdditionalData {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::new(&value)
    }
}
impl From<SourceAdditionalData> for String {
    fn from(value: SourceAdditionalData) -> Self {
        value.0
    }
}

/// Actual bounded provider-generated continuation identity with opaque diagnostics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceId {
    /// Source's exact identity-kind label.
    pub kind: Identifier,
    /// Actual opaque source-generated identity, without inferred semantics.
    pub value: super::Text,
}
impl fmt::Debug for PreparedSwap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedSwap").finish_non_exhaustive()
    }
}
impl fmt::Debug for PreparedSwapData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedSwapData").finish_non_exhaustive()
    }
}
impl TryFrom<PreparedSwapData> for PreparedSwap {
    type Error = Error;
    fn try_from(v: PreparedSwapData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<PreparedSwap> for PreparedSwapData {
    fn from(v: PreparedSwap) -> Self {
        v.0
    }
}
impl Preparation for PreparedSwap {
    type Network = Chain;
    type Intent = PreparedSwapData;
    type UnsignedPayload = Payload;
    fn network(&self) -> &Chain {
        self.0
            .request
            .data()
            .quote
            .data()
            .request
            .data()
            .source
            .chain()
    }
    fn intent(&self) -> &PreparedSwapData {
        &self.0
    }
    fn unsigned_payload(&self) -> &Payload {
        &self.0.payload
    }
    fn validate(&self) -> Result<(), Error> {
        Self::new(self.0.clone()).map(|_| ())
    }
}
