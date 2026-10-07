// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Address, Boc, Cursor, Hash, HistoryPage, Nanotons, Network};
use crate::error::Error;
use serde::{Deserialize, Serialize};
use tycho_types::{
    cell::Load,
    models::{Message, MsgInfo},
};

/// Explicit caller-provided wallet-body fee-estimation request.
/// It is distinct from an unsigned internal transfer message: callers supply
/// the actual body and optional deployment code/data for their external wallet.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FeeRequestFields")]
pub struct FeeRequest {
    network: Network,
    address: Address,
    body: Boc,
    init_code: Option<Boc>,
    init_data: Option<Boc>,
    ignore_signature: bool,
}
impl FeeRequest {
    /// Checks explicit account/network policy without constructing a wallet signing body.
    ///
    /// # Errors
    /// Rejects test-only/mainnet address contradictions.
    pub fn new(
        network: Network,
        address: Address,
        body: Boc,
        init_code: Option<Boc>,
        init_data: Option<Boc>,
        ignore_signature: bool,
    ) -> Result<Self, Error> {
        address.check_network(network)?;
        Ok(Self {
            network,
            address,
            body,
            init_code,
            init_data,
            ignore_signature,
        })
    }
    /// Returns caller-qualified expected network.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
    /// Returns external wallet account to evaluate.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }
    /// Returns exact caller-supplied external wallet body.
    #[must_use]
    pub const fn body(&self) -> &Boc {
        &self.body
    }
    /// Returns optional deployment code.
    #[must_use]
    pub const fn init_code(&self) -> Option<&Boc> {
        self.init_code.as_ref()
    }
    /// Returns optional deployment data.
    #[must_use]
    pub const fn init_data(&self) -> Option<&Boc> {
        self.init_data.as_ref()
    }
    /// Returns explicit signature-check bypass used for unsigned estimates.
    #[must_use]
    pub const fn ignore_signature(&self) -> bool {
        self.ignore_signature
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeeRequestFields {
    network: Network,
    address: Address,
    body: Boc,
    init_code: Option<Boc>,
    init_data: Option<Boc>,
    ignore_signature: bool,
}
impl TryFrom<FeeRequestFields> for FeeRequest {
    type Error = Error;
    fn try_from(v: FeeRequestFields) -> Result<Self, Error> {
        Self::new(
            v.network,
            v.address,
            v.body,
            v.init_code,
            v.init_data,
            v.ignore_signature,
        )
    }
}

/// Source-reported fee components in exact nanotons, without margin or policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeePart {
    /// Exact incoming-forwarding fee estimate, distinct from outgoing forwarding.
    pub incoming_forwarding_fee: Nanotons,
    /// Exact storage fee estimate.
    pub storage_fee: Nanotons,
    /// Exact gas fee estimate.
    pub gas_fee: Nanotons,
    /// Exact forwarding fee estimate.
    pub forwarding_fee: Nanotons,
}
/// Original immutable estimation request plus every bounded source fee component.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FeeEstimateFields")]
pub struct FeeEstimate {
    request: FeeRequest,
    source: FeePart,
    destinations: Vec<FeePart>,
}
impl FeeEstimate {
    /// Retains at most 256 destination components without treating estimates as execution.
    ///
    /// # Errors
    /// Rejects an oversized destination list instead of truncating it.
    pub fn new(
        request: FeeRequest,
        source: FeePart,
        destinations: Vec<FeePart>,
    ) -> Result<Self, Error> {
        if destinations.len() > 256 {
            return Err(super::invalid_record());
        }
        Ok(Self {
            request,
            source,
            destinations,
        })
    }
    /// Returns the unchanged exact caller wallet-body request.
    #[must_use]
    pub const fn request(&self) -> &FeeRequest {
        &self.request
    }
    /// Returns actual source-account estimated fee components.
    #[must_use]
    pub const fn source(&self) -> FeePart {
        self.source
    }
    /// Returns every source destination estimate, without destination-account inference.
    #[must_use]
    pub fn destinations(&self) -> &[FeePart] {
        &self.destinations
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeeEstimateFields {
    request: FeeRequest,
    source: FeePart,
    destinations: Vec<FeePart>,
}
impl TryFrom<FeeEstimateFields> for FeeEstimate {
    type Error = Error;
    fn try_from(v: FeeEstimateFields) -> Result<Self, Error> {
        Self::new(v.request, v.source, v.destinations)
    }
}

/// One explicit bounded incoming-message scan, not a global message-status oracle.
/// Empty matches mean only that no matching incoming hash appears in this page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "MessageStatusFields")]
pub struct MessageStatus {
    message_hash: Hash,
    page: HistoryPage,
    observed: Vec<Cursor>,
}
impl MessageStatus {
    /// Computes every matching incoming-message identity in the supplied bounded page.
    ///
    /// # Errors
    /// Rejects a zero query hash or undecodable retained message evidence.
    pub fn scan(message_hash: Hash, page: HistoryPage) -> Result<Self, Error> {
        if message_hash == Hash::ZERO {
            return Err(super::invalid_record());
        }
        let mut observed = Vec::new();
        for tx in page.transactions() {
            if tx.incoming().map(super::Message::hash).transpose()? == Some(message_hash) {
                observed.push(tx.cursor());
            }
        }
        Ok(Self {
            message_hash,
            page,
            observed,
        })
    }
    /// Returns the exact requested representation hash.
    #[must_use]
    pub const fn message_hash(&self) -> Hash {
        self.message_hash
    }
    /// Returns the exact searched account page and continuation bound.
    #[must_use]
    pub const fn page(&self) -> &HistoryPage {
        &self.page
    }
    /// Returns all matching account transaction identities in this page.
    #[must_use]
    pub fn observed(&self) -> &[Cursor] {
        &self.observed
    }
    /// Returns true only for no match within this explicit page.
    #[must_use]
    pub fn not_observed_within_page(&self) -> bool {
        self.observed.is_empty()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageStatusFields {
    message_hash: Hash,
    page: HistoryPage,
    observed: Vec<Cursor>,
}
impl TryFrom<MessageStatusFields> for MessageStatus {
    type Error = Error;
    fn try_from(v: MessageStatusFields) -> Result<Self, Error> {
        let result = Self::scan(v.message_hash, v.page)?;
        if result.observed != v.observed {
            return Err(super::invalid_record());
        }
        Ok(result)
    }
}

/// Explicit caller assertion that one external message is ready for submission.
/// Maintained container/message structure, destination and representation hash
/// are checked. Signatures, outer-wallet expiry, account ownership and reviewed
/// intent are not verified; callers may use a trusted wallet verifier separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SubmissionFields")]
pub struct SignedSubmission {
    network: Network,
    destination: Address,
    boc: Boc,
    message_hash: Hash,
}
impl SignedSubmission {
    /// Checks one externally supplied external-incoming message without signing.
    ///
    /// # Errors
    /// Rejects unsupported/non-external message headers, destination or network
    /// mismatch. Successful construction never proves a valid signature.
    pub fn new(network: Network, destination: Address, boc: Boc) -> Result<Self, Error> {
        destination.check_network(network)?;
        let root = boc.root()?;
        let mut slice = root.as_slice().map_err(|_| super::invalid_record())?;
        let message = Message::load_from(&mut slice).map_err(|_| super::invalid_record())?;
        if !slice.is_empty() {
            return Err(super::invalid_record());
        }
        let MsgInfo::ExtIn(info) = message.info else {
            return Err(super::invalid_record());
        };
        let actual = super::transaction::standard(&info.dst).ok_or_else(super::invalid_record)?;
        if !actual.same_account(destination) {
            return Err(super::invalid_record());
        }
        let message_hash = boc.hash()?;
        Ok(Self {
            network,
            destination,
            boc,
            message_hash,
        })
    }
    /// Returns explicit expected network, not a signature/embedded network proof.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
    /// Returns checked external-message destination.
    #[must_use]
    pub const fn destination(&self) -> Address {
        self.destination
    }
    /// Returns exact bytes dispatched once.
    #[must_use]
    pub const fn boc(&self) -> &Boc {
        &self.boc
    }
    /// Returns maintained external-message representation hash for later bounded lookup.
    #[must_use]
    pub const fn message_hash(&self) -> Hash {
        self.message_hash
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmissionFields {
    network: Network,
    destination: Address,
    boc: Boc,
    message_hash: Hash,
}
impl TryFrom<SubmissionFields> for SignedSubmission {
    type Error = Error;
    fn try_from(v: SubmissionFields) -> Result<Self, Error> {
        let result = Self::new(v.network, v.destination, v.boc)?;
        if result.message_hash != v.message_hash {
            return Err(super::invalid_record());
        }
        Ok(result)
    }
}

/// Provider acknowledgement of one message hash; not transaction inclusion or success.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SubmissionResultFields")]
pub struct SubmissionResult {
    submission: SignedSubmission,
    source_message_hash: Hash,
}
impl SubmissionResult {
    /// Checks that the source acknowledgement refers to exactly the dispatched message.
    ///
    /// # Errors
    /// Rejects a different source message hash.
    pub fn new(submission: SignedSubmission, source_message_hash: Hash) -> Result<Self, Error> {
        if submission.message_hash() != source_message_hash {
            return Err(super::invalid_record());
        }
        Ok(Self {
            submission,
            source_message_hash,
        })
    }
    /// Returns the explicit original submitted message snapshot.
    #[must_use]
    pub const fn submission(&self) -> &SignedSubmission {
        &self.submission
    }
    /// Returns actual source-acknowledged representation hash.
    #[must_use]
    pub const fn source_message_hash(&self) -> Hash {
        self.source_message_hash
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmissionResultFields {
    submission: SignedSubmission,
    source_message_hash: Hash,
}
impl TryFrom<SubmissionResultFields> for SubmissionResult {
    type Error = Error;
    fn try_from(v: SubmissionResultFields) -> Result<Self, Error> {
        Self::new(v.submission, v.source_message_hash)
    }
}
