// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};

use super::{AccountLookup, Hash, NativeBalance, Network, TokenBalance};

/// The explicitly requested Solana read commitment.
///
/// A request label does not independently prove finality of the returned data.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Commitment {
    /// Request the most recent processed state.
    Processed,
    /// Request confirmed state.
    Confirmed,
    /// Request finalized state.
    Finalized,
}

/// Explicit read requirements, separate from the actual reported context slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadOptions {
    commitment: Commitment,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    minimum_context_slot: Option<u64>,
}

impl ReadOptions {
    /// Records a commitment and optional minimum acceptable evaluation slot.
    ///
    /// The minimum is a lower bound, never an exact historical state selector.
    #[must_use]
    pub const fn new(commitment: Commitment, minimum_context_slot: Option<u64>) -> Self {
        Self {
            commitment,
            minimum_context_slot,
        }
    }

    /// Returns the requested commitment without claiming observed finality.
    #[must_use]
    pub const fn commitment(self) -> Commitment {
        self.commitment
    }

    /// Returns the optional lower bound on the actual context slot.
    #[must_use]
    pub const fn minimum_context_slot(self) -> Option<u64> {
        self.minimum_context_slot
    }
}

/// The typed operation represented by a Solana observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// An exact native lamport balance.
    NativeBalance,
    /// An exact balance for one SPL token account.
    TokenBalance,
    /// A present or absent account lookup.
    Account,
}

/// Caller-supplied attribution and the actual reported Solana context slot.
///
/// This type does not contact a source, consult a clock, or verify a network.
/// The actual slot must meet the requested minimum. No block hash, block time,
/// or independently assessed finality is inferred from the slot or commitment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields")]
pub struct Context {
    schema_version: u16,
    operation: Operation,
    network: Network,
    requested_options: ReadOptions,
    slot: u64,
    source: Source,
    retrieved_at: Timestamp,
}

impl Context {
    /// Records explicit operation, network, request, slot, source, and retrieval time.
    ///
    /// # Errors
    /// Rejects an actual slot below the requested minimum context slot.
    pub fn new(
        operation: Operation,
        network: Network,
        requested_options: ReadOptions,
        slot: u64,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        if requested_options
            .minimum_context_slot()
            .is_some_and(|minimum| slot < minimum)
        {
            return Err(ValidationError::ContextSlotBelowMinimum.into());
        }
        Ok(Self {
            schema_version: 1,
            operation,
            network,
            requested_options,
            slot,
            source,
            retrieved_at,
        })
    }

    /// Returns this family's wire schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    /// Returns the declared typed operation.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns the explicitly supplied network and display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }

    /// Returns the requested commitment and lower bound, separately from the slot.
    #[must_use]
    pub const fn requested_options(&self) -> ReadOptions {
        self.requested_options
    }

    /// Returns the actual reported evaluation slot.
    #[must_use]
    pub const fn slot(&self) -> u64 {
        self.slot
    }

    /// Returns the explicitly supplied non-secret source labels.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Returns the explicitly supplied retrieval time, independently of the slot.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFields {
    schema_version: u16,
    operation: Operation,
    network: Network,
    requested_options: ReadOptions,
    slot: u64,
    source: Source,
    retrieved_at: Timestamp,
}

impl TryFrom<ContextFields> for Context {
    type Error = Error;

    fn try_from(fields: ContextFields) -> Result<Self, Self::Error> {
        if fields.schema_version != 1 {
            return Err(ValidationError::UnsupportedSchemaVersion.into());
        }
        Self::new(
            fields.operation,
            fields.network,
            fields.requested_options,
            fields.slot,
            fields.source,
            fields.retrieved_at,
        )
    }
}

/// A typed Solana result with its required family-specific context.
///
/// Serialization places context fields and `value` in one strict object. The
/// constructor and deserializer both require agreement on operation and full
/// genesis identity; differing display aliases remain independently retained.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation<T> {
    #[serde(flatten)]
    context: Context,
    value: T,
}

impl<T> Observation<T> {
    /// Returns the typed value, including explicit absence where applicable.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns required source, request, network, slot, and retrieval context.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }

    fn validated(
        value: T,
        context: Context,
        operation: Operation,
        genesis_hash: Hash,
    ) -> Result<Self, Error> {
        if context.operation() != operation {
            return Err(ValidationError::ObservationOperationMismatch.into());
        }
        if context.network().genesis_hash() != genesis_hash {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(Self { context, value })
    }
}

impl Observation<NativeBalance> {
    /// Attributes an exact native balance to matching operation and network context.
    ///
    /// # Errors
    /// Rejects a different operation or genesis identity.
    pub fn native_balance(value: NativeBalance, context: Context) -> Result<Self, Error> {
        let genesis_hash = value.network().genesis_hash();
        Self::validated(value, context, Operation::NativeBalance, genesis_hash)
    }
}

impl Observation<TokenBalance> {
    /// Attributes an exact token balance to matching operation and network context.
    ///
    /// # Errors
    /// Rejects a different operation or genesis identity.
    pub fn token_balance(value: TokenBalance, context: Context) -> Result<Self, Error> {
        let genesis_hash = value.asset().network().genesis_hash();
        Self::validated(value, context, Operation::TokenBalance, genesis_hash)
    }
}

impl Observation<AccountLookup> {
    /// Attributes a present or absent account to matching operation/network context.
    ///
    /// # Errors
    /// Rejects a different operation or genesis identity.
    pub fn account(value: AccountLookup, context: Context) -> Result<Self, Error> {
        let genesis_hash = value.network().genesis_hash();
        Self::validated(value, context, Operation::Account, genesis_hash)
    }
}

// Flat wire fields preserve duplicate-key detection without flattening a
// deserializer or first parsing through a map that could overwrite duplicates.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationFields<T> {
    schema_version: u16,
    operation: Operation,
    network: Network,
    requested_options: ReadOptions,
    slot: u64,
    source: Source,
    retrieved_at: Timestamp,
    value: T,
}

impl<T> ObservationFields<T> {
    fn into_parts(self) -> Result<(T, Context), Error> {
        let context = Context::try_from(ContextFields {
            schema_version: self.schema_version,
            operation: self.operation,
            network: self.network,
            requested_options: self.requested_options,
            slot: self.slot,
            source: self.source,
            retrieved_at: self.retrieved_at,
        })?;
        Ok((self.value, context))
    }
}

macro_rules! observation_deserialize {
    ($value:ty, $constructor:ident) => {
        impl<'de> Deserialize<'de> for Observation<$value> {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let (value, context) = ObservationFields::<$value>::deserialize(deserializer)?
                    .into_parts()
                    .map_err(serde::de::Error::custom)?;
                Self::$constructor(value, context).map_err(serde::de::Error::custom)
            }
        }
    };
}

observation_deserialize!(NativeBalance, native_balance);
observation_deserialize!(TokenBalance, token_balance);
observation_deserialize!(AccountLookup, account);
