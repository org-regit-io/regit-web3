// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure Solana identities, exact values, canonical bytes and unsigned preparation.
//!
//! Construction records caller-supplied facts without network access or a
//! runtime. Network identity is the full genesis hash, independent of its
//! display alias. Observations retain the requested commitment and minimum
//! context slot separately from the actual reported slot; they do not invent
//! a block hash, block time, or independently verified finality.
//! Execution observations use method-specific controls: transaction inclusion,
//! status evaluation and last-valid block height are distinct source facts.
//! Canonical decoding and structural validation do not verify signatures or
//! consensus. Local native/classic SPL preparations use explicit legacy
//! recent-blockhash profiles; retrieval supports legacy, v0 and v1 bytes.

mod account;
mod execution;
mod identity;
mod observation;
mod preparation;
mod transaction;

pub use account::{
    Account, AccountLookup, NativeBalance, TokenAccountState, TokenAsset, TokenBalance,
    TokenIdentity,
};
#[cfg(feature = "solana-http")]
pub(crate) use execution::bounded_entries as bounded_execution_entries;
pub use execution::{
    BlockHeight, BlockhashValidity, CompiledInnerInstruction, ExecutionBytes, ExecutionContext,
    ExecutionObservation, ExecutionOutcome, ExecutionRequest, InnerInstructions, LatestBlockhash,
    LoadedAddresses, LogMessage, MessageFee, ReturnData, Reward, RewardKind, SignatureStatus,
    Simulation, StatusLookup, StatusOptions, Submission, SubmitOptions, TokenBalanceRecord,
    Transaction, TransactionLookup, TransactionMetadata, TransactionMetadataData,
    TransactionReadOptions,
};
pub use identity::{Hash, Network, Pubkey, Signature};
pub use observation::{Commitment, Context, Observation, Operation, ReadOptions};
pub use preparation::{BlockhashLifetime, TransferIntent, TransferPreparation, TransferReview};
pub use transaction::{
    SignedTransaction, TransactionVersion, UnsignedMessage, UnsignedTransaction,
};
