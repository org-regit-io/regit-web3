// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::wire::{chain_error, invalid_response};
use super::EvmClient;
use crate::{
    domain::evm::{
        OperationObservation, ReadOperation, ReadState, SignedSubmission, SubmissionAcknowledgment,
        TransactionId,
    },
    error::{Error, ProviderError},
    transport::{OperationBudget, decode_response, encode_request, submission_unknown},
};

const REQUEST_ID: u64 = 1;

impl EvmClient {
    /// Explicitly submits one caller-supplied supported signed envelope exactly once.
    ///
    /// Local canonical-envelope/range checks and maintained-computed hash are
    /// structural/identity facts, not signature, recovered-sender or reviewed-intent
    /// proof. Signing, funding and caller semantic verification remain external.
    /// Chain verification precedes dispatch under the same total budget. Only the
    /// verification read may retry; `eth_sendRawTransaction` never retries, including
    /// HTTP429/5xx, timeout or disconnect. Preparation/call/estimation never invoke it.
    ///
    /// A matching response hash acknowledges the request without proving mempool
    /// acceptance, inclusion, execution or finality. Unresolved errors after the
    /// execute attempt, including RPC errors, malformed/null/wrong-ID responses and
    /// deadline expiry, retain possible submission outcome. The future owns no
    /// detached task: cancellation stops local polling but cannot prove no dispatch.
    /// # Errors
    /// Returns ordinary local/preflight errors before any write. All unresolved
    /// post-dispatch failures are `Error::SubmissionOutcomeUnknown` with fixed causes.
    /// # Panics
    /// On native targets, Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> Result<OperationObservation<SubmissionAcknowledgment>, Error> {
        if submission.expected_chain_id() != self.chain_id {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        let budget = OperationBudget::new(self.config.limits())?;
        budget.run(self.verify_chain(&budget)).await?;
        let body = encode_request(
            REQUEST_ID,
            "eth_sendRawTransaction",
            &(submission.payload(),),
        )?;
        let response = self.http.write_once(&[], &[], &body, &budget).await?;
        // This outer deadline covers only post-dispatch validation. It must not
        // erase the distinction between a preflight timeout and a possible write.
        budget
            .run(async {
                let bytes = response.into_success()?;
                let actual: TransactionId = decode_response(&bytes, REQUEST_ID, chain_error)?
                    .ok_or_else(invalid_response)?;
                if actual != submission.transaction_id() {
                    return Err(invalid_response());
                }
                OperationObservation::new(
                    SubmissionAcknowledgment {
                        chain_id: self.chain_id,
                        transaction_id: actual,
                    },
                    self.read_context(
                        ReadOperation::SignedSubmission,
                        ReadState::Unanchored,
                        "eth_sendRawTransaction",
                    )?,
                )
            })
            .await
            .map_err(submission_unknown)
    }
}

impl super::super::EvmSubmitter for EvmClient {
    async fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> Result<OperationObservation<SubmissionAcknowledgment>, Error> {
        Self::submit_signed(self, submission).await
    }
}
