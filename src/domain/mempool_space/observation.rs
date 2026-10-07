// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    domain::{Source, Timestamp, bitcoin::NetworkId},
    error::{Error, ValidationError},
};

use super::{
    MempoolSummary, RecentTransactions, RecommendedFees, TransactionIds, TransactionLimit,
};

/// The typed mempool operation and its actual local request bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Backlog totals and source fee-distribution bins.
    Summary,
    /// At most ten recent source arrivals.
    RecentTransactions,
    /// An unpaged full ID list with an explicit local capacity.
    TransactionIds {
        /// The maximum number of returned IDs the caller accepts.
        limit: TransactionLimit,
    },
    /// Independent source fee suggestions in sat/vB.
    RecommendedFees,
}

/// Explicit full Bitcoin genesis identity, operation and retrieval attribution.
/// No historical mempool snapshot, arrival time or confirmation promise is inferred.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    network: NetworkId,
    operation: Operation,
    source: Source,
    retrieved_at: Timestamp,
}
impl Context {
    /// Records explicit caller-supplied facts without a network or clock.
    #[must_use]
    pub const fn new(
        network: NetworkId,
        operation: Operation,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Self {
        Self {
            network,
            operation,
            source,
            retrieved_at,
        }
    }
    /// Returns the expected full Bitcoin genesis identity and separate alias.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns the typed operation and its supplied collection capacity.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }
    /// Returns explicit non-secret provider/method/version attribution.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns retrieval time in Unix seconds, not a mempool state timestamp.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}

/// A schema-versioned mempool result with matching operation and source context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation<T> {
    schema_version: u16,
    context: Context,
    value: T,
}
impl<T> Observation<T> {
    /// Returns the supported serialization schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns expected network, operation and source retrieval context.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns the exact typed source value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
    fn new(value: T, context: Context, operation: Operation) -> Result<Self, Error> {
        if context.operation() != operation {
            return Err(ValidationError::ObservationOperationMismatch.into());
        }
        Ok(Self {
            schema_version: 1,
            context,
            value,
        })
    }
}
impl Observation<MempoolSummary> {
    /// Attributes a backlog response without an invented state anchor.
    /// # Errors
    /// Rejects differing context operation.
    pub fn summary(value: MempoolSummary, context: Context) -> Result<Self, Error> {
        Self::new(value, context, Operation::Summary)
    }
}
impl Observation<RecentTransactions> {
    /// Attributes recent entries without implying an exhaustive mempool listing.
    /// # Errors
    /// Rejects differing context operation.
    pub fn recent_transactions(value: RecentTransactions, context: Context) -> Result<Self, Error> {
        Self::new(value, context, Operation::RecentTransactions)
    }
}
impl Observation<TransactionIds> {
    /// Attributes every source ID with the exact supplied local capacity.
    /// # Errors
    /// Rejects a differing context operation or collection bound.
    pub fn transaction_ids(value: TransactionIds, context: Context) -> Result<Self, Error> {
        let operation = Operation::TransactionIds {
            limit: value.limit(),
        };
        Self::new(value, context, operation)
    }
}
impl Observation<RecommendedFees> {
    /// Attributes independent suggestions without promising confirmation time.
    /// # Errors
    /// Rejects differing context operation.
    pub fn recommended_fees(value: RecommendedFees, context: Context) -> Result<Self, Error> {
        Self::new(value, context, Operation::RecommendedFees)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields<T> {
    schema_version: u16,
    context: Context,
    value: T,
}
macro_rules! observation_serde {
    ($type:ty, $constructor:ident) => {
        impl<'de> Deserialize<'de> for Observation<$type> {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let fields = Fields::<$type>::deserialize(d)?;
                if fields.schema_version != 1 {
                    return Err(serde::de::Error::custom(
                        ValidationError::UnsupportedSchemaVersion,
                    ));
                }
                Self::$constructor(fields.value, fields.context).map_err(serde::de::Error::custom)
            }
        }
    };
}
observation_serde!(MempoolSummary, summary);
observation_serde!(RecentTransactions, recent_transactions);
observation_serde!(TransactionIds, transaction_ids);
observation_serde!(RecommendedFees, recommended_fees);
