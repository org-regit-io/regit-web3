// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::wire::{
    RequestOptions, SlotValue, error_policy,
    execution::{
        HeightOptions, LatestWire, SimulationOptions, SimulationWire, StatusRequest, StatusWire,
        SubmissionOptions, TransactionOptions, TransactionWire,
    },
    invalid_response,
};
use super::SolanaClient;
use crate::{
    domain::{
        Source, Timestamp,
        solana::{
            BlockHeight, BlockhashValidity, Commitment, ExecutionContext, ExecutionObservation,
            ExecutionRequest, Hash, LatestBlockhash, MessageFee, ReadOptions, Signature,
            SignedTransaction, Simulation, StatusLookup, StatusOptions, Submission, SubmitOptions,
            TransactionLookup, TransactionReadOptions, UnsignedMessage, UnsignedTransaction,
        },
    },
    error::Error,
    transport::{OperationBudget, decode_response, encode_request, submission_unknown},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::time::{SystemTime, UNIX_EPOCH};

impl SolanaClient {
    /// Retrieves canonical legacy/v0/v1 source bytes and exact execution metadata.
    /// Null means unknown at the selected confirmed/finalized state, not pending.
    /// This RPC supplies inclusion slot, not evaluation slot or minimum-slot controls.
    /// # Errors
    /// Returns bounded source, identity, structural, deadline or unsupported failures.
    /// # Panics
    /// Tokio may panic without caller-provided I/O and time drivers.
    pub async fn get_transaction(
        &self,
        signature: Signature,
        options: TransactionReadOptions,
    ) -> Result<ExecutionObservation<TransactionLookup>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let value: Option<TransactionWire> = self
                    .http
                    .read_rpc(
                        &budget,
                        "getTransaction",
                        &(signature, TransactionOptions::new(options)),
                        error_policy,
                    )
                    .await?;
                let transaction = value.map(TransactionWire::into_transaction).transpose()?;
                ExecutionObservation::transaction(
                    TransactionLookup {
                        signature,
                        transaction,
                    },
                    self.execution_context(
                        ExecutionRequest::Transaction { signature, options },
                        None,
                        "getTransaction",
                    )?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }
    /// Looks up one exact signature in the explicitly selected cache/history search.
    /// Context evaluation slot and source inclusion/execution facts remain separate.
    /// This RPC supports no requested commitment or minimum context slot.
    /// # Errors
    /// Returns fixed bounded-provider, correlation or deadline failures.
    /// # Panics
    /// Tokio may panic without caller-provided I/O and time drivers.
    pub async fn get_transaction_status(
        &self,
        signature: Signature,
        options: StatusOptions,
    ) -> Result<ExecutionObservation<StatusLookup>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let value: SlotValue<[Option<StatusWire>; 1]> = self
                    .http
                    .read_rpc(
                        &budget,
                        "getSignatureStatuses",
                        &([signature], StatusRequest::from(options)),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                let [status] = value.value;
                ExecutionObservation::status(
                    StatusLookup {
                        signature,
                        status: status.map(StatusWire::into_status).transpose()?,
                    },
                    self.execution_context(
                        ExecutionRequest::Status { signature, options },
                        Some(value.context.slot),
                        "getSignatureStatuses",
                    )?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }
    /// Reads a recent blockhash and last-valid BLOCK HEIGHT with actual context slot.
    /// No implicit refresh, wall-clock expiry or hash-pinned history is claimed.
    /// # Errors
    /// Returns fixed provider, network, lower-bound or deadline failures.
    /// # Panics
    /// Tokio may panic without caller-provided I/O and time drivers.
    pub async fn get_latest_blockhash(
        &self,
        options: ReadOptions,
    ) -> Result<ExecutionObservation<LatestBlockhash>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let value: SlotValue<LatestWire> = self
                    .http
                    .read_rpc(
                        &budget,
                        "getLatestBlockhash",
                        &(RequestOptions::new(options, None),),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                ExecutionObservation::latest_blockhash(
                    LatestBlockhash {
                        lifetime: value.value.lifetime(),
                    },
                    self.execution_context(
                        ExecutionRequest::LatestBlockhash { options },
                        Some(value.context.slot),
                        "getLatestBlockhash",
                    )?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }
    /// Checks source validity of the exact frozen caller-supplied blockhash.
    /// # Errors
    /// Returns fixed provider, network, lower-bound or deadline failures.
    /// # Panics
    /// Tokio may panic without caller-provided I/O and time drivers.
    pub async fn is_blockhash_valid(
        &self,
        blockhash: Hash,
        options: ReadOptions,
    ) -> Result<ExecutionObservation<BlockhashValidity>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let value: SlotValue<bool> = self
                    .http
                    .read_rpc(
                        &budget,
                        "isBlockhashValid",
                        &(blockhash, RequestOptions::new(options, None)),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                ExecutionObservation::blockhash_validity(
                    BlockhashValidity {
                        blockhash,
                        valid: value.value,
                    },
                    self.execution_context(
                        ExecutionRequest::BlockhashValidity { blockhash, options },
                        Some(value.context.slot),
                        "isBlockhashValid",
                    )?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }
    /// Reads exact source BLOCK HEIGHT for explicit commitment, without inventing a slot.
    /// # Errors
    /// Returns fixed provider, identity, width or deadline failures.
    /// # Panics
    /// Tokio may panic without caller-provided I/O and time drivers.
    pub async fn get_block_height(
        &self,
        commitment: Commitment,
    ) -> Result<ExecutionObservation<BlockHeight>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let height: u64 = self
                    .http
                    .read_rpc(
                        &budget,
                        "getBlockHeight",
                        &(HeightOptions { commitment },),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                ExecutionObservation::block_height(
                    BlockHeight { height },
                    self.execution_context(
                        ExecutionRequest::BlockHeight { commitment },
                        None,
                        "getBlockHeight",
                    )?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }
    /// Estimates exact lamport fees for immutable canonical message bytes.
    /// Explicit null retains unavailable fee data; no zero fee or refreshed hash is invented.
    /// # Errors
    /// Returns fixed provider, identity, lower-bound or deadline failures.
    /// # Panics
    /// Tokio may panic without caller-provided I/O and time drivers.
    pub async fn get_fee_for_message(
        &self,
        message: UnsignedMessage,
        options: ReadOptions,
    ) -> Result<ExecutionObservation<MessageFee>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let value: SlotValue<Option<u64>> = self
                    .http
                    .read_rpc(
                        &budget,
                        "getFeeForMessage",
                        &(
                            STANDARD.encode(message.bytes()),
                            RequestOptions::new(options, None),
                        ),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                ExecutionObservation::message_fee(
                    MessageFee {
                        lamports: value.value,
                    },
                    self.execution_context(
                        ExecutionRequest::MessageFee { message, options },
                        Some(value.context.slot),
                        "getFeeForMessage",
                    )?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }
    /// Simulates exact zero-placeholder bytes with `sigVerify=false` and replacement off.
    /// Source execution failures remain typed results; this performs no write, signing,
    /// implicit blockhash refresh, account creation, funding or approval decision.
    /// # Errors
    /// Returns fixed provider, identity, malformed/bounded-source or deadline failures.
    /// # Panics
    /// Tokio may panic without caller-provided I/O and time drivers.
    pub async fn simulate_transaction(
        &self,
        transaction: UnsignedTransaction,
        options: ReadOptions,
    ) -> Result<ExecutionObservation<Simulation>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_genesis(&budget).await?;
                let controls = SimulationOptions {
                    encoding: "base64",
                    commitment: options.commitment(),
                    min_context_slot: options.minimum_context_slot(),
                    sig_verify: false,
                    replace_recent_blockhash: false,
                };
                let value: SlotValue<SimulationWire> = self
                    .http
                    .read_rpc(
                        &budget,
                        "simulateTransaction",
                        &(STANDARD.encode(transaction.bytes()), controls),
                        error_policy,
                    )
                    .await?
                    .ok_or_else(invalid_response)?;
                ExecutionObservation::simulation(
                    value.value.into_simulation()?,
                    self.execution_context(
                        ExecutionRequest::Simulation {
                            transaction,
                            options,
                        },
                        Some(value.context.slot),
                        "simulateTransaction",
                    )?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }
    /// Sends exact caller-signed bytes once after full genesis verification.
    /// Every signature slot must be nonzero; cryptographic signature/intent checks and
    /// approval are caller-owned. Solana bytes encode no genesis identity. Node
    /// `maxRetries=0` and local one-shot transport prevent automatic resubmission.
    /// Matching source first signature acknowledges the write without proving inclusion
    /// or execution. Cancellation cannot establish whether a write reached the source.
    /// # Errors
    /// Before dispatch, returns ordinary fixed failures. Unresolved post-dispatch
    /// failures, including RPC errors and mismatched acknowledgements, retain possible
    /// submission as `SubmissionOutcomeUnknown` without exposing source diagnostics.
    /// # Panics
    /// Tokio may panic without caller-provided I/O and time drivers.
    pub async fn submit_signed(
        &self,
        transaction: SignedTransaction,
        options: SubmitOptions,
    ) -> Result<ExecutionObservation<Submission>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget.run(self.verify_genesis(&budget)).await?;
        let signature = transaction.signature();
        let body = encode_request(
            1,
            "sendTransaction",
            &(
                STANDARD.encode(transaction.bytes()),
                SubmissionOptions::from(options),
            ),
        )?;
        let response = self.http.write_once(&[], &[], &body, &budget).await?;
        budget
            .run(async {
                let bytes = response.into_success()?;
                let actual: Signature =
                    decode_response(&bytes, 1, error_policy)?.ok_or_else(invalid_response)?;
                if actual != signature {
                    return Err(invalid_response());
                }
                ExecutionObservation::submission(
                    Submission { signature: actual },
                    self.execution_context(
                        ExecutionRequest::Submission { signature, options },
                        None,
                        "sendTransaction",
                    )?,
                )
                .map_err(|_| invalid_response())
            })
            .await
            .map_err(submission_unknown)
    }
    fn execution_context(
        &self,
        request: ExecutionRequest,
        slot: Option<u64>,
        method: &'static str,
    ) -> Result<ExecutionContext, Error> {
        let retrieved_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| Timestamp::from_unix_seconds(d.as_secs()))
            .map_err(|_| Error::Configuration)?;
        ExecutionContext::new(
            self.config.network().clone(),
            request,
            slot,
            Source::new(
                self.config.http_config().provider_id(),
                method,
                env!("CARGO_PKG_VERSION"),
            )?,
            retrieved_at,
        )
        .map_err(|_| invalid_response())
    }
}

impl super::super::TransactionReader for SolanaClient {
    fn network(&self) -> &crate::domain::solana::Network {
        self.config.network()
    }
    async fn get_transaction(
        &self,
        s: Signature,
        o: TransactionReadOptions,
    ) -> Result<ExecutionObservation<TransactionLookup>, Error> {
        Self::get_transaction(self, s, o).await
    }
    async fn get_transaction_status(
        &self,
        s: Signature,
        o: StatusOptions,
    ) -> Result<ExecutionObservation<StatusLookup>, Error> {
        Self::get_transaction_status(self, s, o).await
    }
}
impl super::super::BlockhashReader for SolanaClient {
    fn network(&self) -> &crate::domain::solana::Network {
        self.config.network()
    }
    async fn get_latest_blockhash(
        &self,
        o: ReadOptions,
    ) -> Result<ExecutionObservation<LatestBlockhash>, Error> {
        Self::get_latest_blockhash(self, o).await
    }
    async fn is_blockhash_valid(
        &self,
        h: Hash,
        o: ReadOptions,
    ) -> Result<ExecutionObservation<BlockhashValidity>, Error> {
        Self::is_blockhash_valid(self, h, o).await
    }
    async fn get_block_height(
        &self,
        c: Commitment,
    ) -> Result<ExecutionObservation<BlockHeight>, Error> {
        Self::get_block_height(self, c).await
    }
}
impl super::super::ExecutionReader for SolanaClient {
    fn network(&self) -> &crate::domain::solana::Network {
        self.config.network()
    }
    async fn get_fee_for_message(
        &self,
        m: UnsignedMessage,
        o: ReadOptions,
    ) -> Result<ExecutionObservation<MessageFee>, Error> {
        Self::get_fee_for_message(self, m, o).await
    }
    async fn simulate_transaction(
        &self,
        t: UnsignedTransaction,
        o: ReadOptions,
    ) -> Result<ExecutionObservation<Simulation>, Error> {
        Self::simulate_transaction(self, t, o).await
    }
}
impl super::super::SolanaSubmitter for SolanaClient {
    fn network(&self) -> &crate::domain::solana::Network {
        self.config.network()
    }
    async fn submit_signed(
        &self,
        t: SignedTransaction,
        o: SubmitOptions,
    ) -> Result<ExecutionObservation<Submission>, Error> {
        Self::submit_signed(self, t, o).await
    }
}
