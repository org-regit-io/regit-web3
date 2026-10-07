// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure Solana identities, exact account values, and attributed observations.
//!
//! Construction records caller-supplied facts without network access or a
//! runtime. Network identity is the full genesis hash, independent of its
//! display alias. Observations retain the requested commitment and minimum
//! context slot separately from the actual reported slot; they do not invent
//! a block hash, block time, or independently verified finality.

mod account;
mod identity;
mod observation;

pub use account::{
    Account, AccountLookup, NativeBalance, TokenAccountState, TokenAsset, TokenBalance,
    TokenIdentity,
};
pub use identity::{Hash, Network, Pubkey, Signature};
pub use observation::{Commitment, Context, Observation, Operation, ReadOptions};
