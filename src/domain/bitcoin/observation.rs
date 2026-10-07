// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Serialize};

use super::{
    Address, AddressBalance, FeeEstimates, HistoryCursor, HistoryPage, NetworkId,
    TransactionStatus, Txid,
};
use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};

/// The exact indexed operation and caller-supplied query identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// An address balance query.
    AddressBalance {
        /// The explicitly qualified requested address.
        address: Address,
    },
    /// A bounded address history query.
    AddressHistory {
        /// The explicitly qualified requested address.
        address: Address,
        /// The exact requested history cursor.
        cursor: HistoryCursor,
    },
    /// Exact fee estimates by confirmation horizon.
    FeeEstimates,
    /// A transaction-status lookup.
    TransactionStatus {
        /// The exact requested transaction identifier.
        txid: Txid,
    },
}

/// Schema-versioned source and retrieval facts for an indexed Bitcoin read.
///
/// These facts do not establish a historical state pin, confirmation count,
/// or lasting finality. The domain never reads a clock or network.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields")]
pub struct Context {
    schema_version: u16,
    network: NetworkId,
    operation: Operation,
    source: Source,
    retrieved_at: Timestamp,
}
impl Context {
    /// Records an explicit operation, expected network, source and retrieval time.
    ///
    /// # Errors
    /// Rejects a qualified address whose network differs from the context.
    pub fn new(
        network: NetworkId,
        operation: Operation,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        let address = match &operation {
            Operation::AddressBalance { address } | Operation::AddressHistory { address, .. } => {
                Some(address)
            }
            _ => None,
        };
        if address.is_some_and(|address| address.network() != network.network()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(Self {
            schema_version: 1,
            network,
            operation,
            source,
            retrieved_at,
        })
    }
    /// Returns the supported schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns complete network identity and display metadata.
    #[must_use]
    pub const fn network(&self) -> &NetworkId {
        &self.network
    }
    /// Returns the exact query operation and its identity.
    #[must_use]
    pub const fn operation(&self) -> &Operation {
        &self.operation
    }
    /// Returns non-secret source attribution.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns caller-supplied whole Unix seconds after retrieval.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFields {
    schema_version: u16,
    network: NetworkId,
    operation: Operation,
    source: Source,
    retrieved_at: Timestamp,
}
impl TryFrom<ContextFields> for Context {
    type Error = Error;
    fn try_from(value: ContextFields) -> Result<Self, Error> {
        if value.schema_version != 1 {
            return Err(ValidationError::UnsupportedBitcoinSchema.into());
        }
        Self::new(
            value.network,
            value.operation,
            value.source,
            value.retrieved_at,
        )
    }
}

/// A typed value with validated indexed-source context and query identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation<T> {
    context: Context,
    value: T,
}
impl<T> Observation<T> {
    /// Returns the exact query and source context.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns the typed indexed value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
struct ObservationFields<T> {
    context: Context,
    value: T,
}

macro_rules! observation_type {
    ($ty:ty, $method:ident, $validator:expr, $doc:literal) => {
        impl Observation<$ty> {
            #[doc = $doc]
            ///
            /// # Errors
            /// Rejects an operation or query identity differing from its value.
            pub fn $method(value: $ty, context: Context) -> Result<Self, Error> {
                let valid: fn(&$ty, &Operation) -> bool = $validator;
                if !valid(&value, &context.operation) {
                    return Err(ValidationError::ObservationOperationMismatch.into());
                }
                Ok(Self { context, value })
            }
        }
        impl<'de> Deserialize<'de> for Observation<$ty> {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let fields = ObservationFields::<$ty>::deserialize(deserializer)?;
                Self::$method(fields.value, fields.context).map_err(serde::de::Error::custom)
            }
        }
    };
}
observation_type!(
    AddressBalance,
    address_balance,
    |value, operation| matches!(operation, Operation::AddressBalance { address } if address == value.address()),
    "Records a balance with its matching requested address."
);
observation_type!(
    HistoryPage,
    address_history,
    |value, operation| matches!(operation, Operation::AddressHistory { address, cursor } if address == value.address() && *cursor == value.cursor()),
    "Records a page with its matching address and exact cursor."
);
observation_type!(
    FeeEstimates,
    fee_estimates,
    |_value, operation| matches!(operation, Operation::FeeEstimates),
    "Records fee estimates with their matching operation."
);
observation_type!(
    TransactionStatus,
    transaction_status,
    |_value, operation| matches!(operation, Operation::TransactionStatus { .. }),
    "Records transaction status while retaining its exact requested identifier."
);
