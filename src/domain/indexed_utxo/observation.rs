// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{
    AddressBalance, AddressPolicy, FeeEstimates, HistoryPage, HistoryRequest, NetworkId,
    Transaction, TransactionStatus, Txid,
};
use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Serialize};

/// The exact indexed read and caller-owned query, without a common historical state pin.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    deny_unknown_fields,
    bound(deserialize = "A: AddressPolicy")
)]
pub enum Operation<A: AddressPolicy> {
    /// Exact confirmed balance and a separate signed unconfirmed delta.
    AddressBalance {
        /// The requested qualified address.
        address: A,
    },
    /// A bounded, height-based address reference page.
    AddressHistory {
        /// The requested qualified address.
        address: A,
        /// The exact requested page inputs and resource capacity.
        request: HistoryRequest,
    },
    /// Source fee-preference buckets in native atomic units per 1000 bytes.
    FeeEstimates,
    /// Source-reported status for one exact identifier.
    TransactionStatus {
        /// The requested transaction identity.
        txid: Txid,
    },
    /// Complete typed indexed transaction fields, without canonical raw decoding.
    Transaction {
        /// The requested transaction identity.
        txid: Txid,
        /// Caller capacity for each complete input/output collection.
        maximum_entries: u32,
    },
}

/// Schema-versioned family/network/query/source/retrieval context for an indexed read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields<A>", bound(deserialize = "A: AddressPolicy"))]
pub struct Context<A: AddressPolicy> {
    schema_version: u16,
    network: NetworkId<A::Network>,
    operation: Operation<A>,
    source: Source,
    retrieved_at: Timestamp,
}
impl<A: AddressPolicy> Context<A> {
    /// Records explicit expected network, exact query, attribution and retrieval time.
    /// # Errors
    /// Rejects an incompatible address or invalid transaction resource capacity.
    pub fn new(
        network: NetworkId<A::Network>,
        operation: Operation<A>,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        match &operation {
            Operation::AddressBalance { address } | Operation::AddressHistory { address, .. }
                if address.network() != network.network() =>
            {
                return Err(ValidationError::NetworkMismatch.into());
            }
            Operation::Transaction {
                maximum_entries, ..
            } if *maximum_entries == 0 || *maximum_entries > 2000 => return Err(super::invalid()),
            _ => {}
        }
        Ok(Self {
            schema_version: 1,
            network,
            operation,
            source,
            retrieved_at,
        })
    }
    /// Returns the supported observation schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns the complete expected genesis and independent caller display alias.
    #[must_use]
    pub const fn network(&self) -> &NetworkId<A::Network> {
        &self.network
    }
    /// Returns the exact caller operation and query identity.
    #[must_use]
    pub const fn operation(&self) -> &Operation<A> {
        &self.operation
    }
    /// Returns non-secret source attribution and library version.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns actual whole Unix seconds after retrieval.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "A: AddressPolicy"))]
struct ContextFields<A: AddressPolicy> {
    schema_version: u16,
    network: NetworkId<A::Network>,
    operation: Operation<A>,
    source: Source,
    retrieved_at: Timestamp,
}
impl<A: AddressPolicy> TryFrom<ContextFields<A>> for Context<A> {
    type Error = Error;
    fn try_from(v: ContextFields<A>) -> Result<Self, Error> {
        if v.schema_version != 1 {
            return Err(ValidationError::UnsupportedSchemaVersion.into());
        }
        Self::new(v.network, v.operation, v.source, v.retrieved_at)
    }
}

/// An immutable checked typed value and its matching family/query/source context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(bound(serialize = "T: Serialize, A: AddressPolicy"))]
pub struct Observation<T, A: AddressPolicy> {
    context: Context<A>,
    value: T,
}
impl<T, A: AddressPolicy> Observation<T, A> {
    /// Returns explicit family network, exact query and retrieval attribution.
    #[must_use]
    pub const fn context(&self) -> &Context<A> {
        &self.context
    }
    /// Returns the checked typed value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
}
#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "T: Deserialize<'de>, A: AddressPolicy")
)]
struct ObservationFields<T, A: AddressPolicy> {
    context: Context<A>,
    value: T,
}
macro_rules! observation_type {
    ($record:ty, $method:ident, $validator:expr, $doc:literal) => {
        impl<A: AddressPolicy> Observation<$record, A> {
            #[doc = $doc]
            /// # Errors
            /// Rejects an operation, query identity, family network or resource limit differing from its value.
            pub fn $method(value: $record, context: Context<A>) -> Result<Self, Error> {
                let check: fn(&$record, &Context<A>) -> bool = $validator;
                if !check(&value, &context) {
                    return Err(ValidationError::ObservationOperationMismatch.into());
                }
                Ok(Self { context, value })
            }
        }
        impl<'de, A: AddressPolicy> Deserialize<'de> for Observation<$record, A> {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = ObservationFields::<$record, A>::deserialize(d)?;
                Self::$method(v.value, v.context).map_err(serde::de::Error::custom)
            }
        }
    };
}
observation_type!(
    AddressBalance<A>,
    address_balance,
    |v, c| matches!(&c.operation, Operation::AddressBalance { address } if address == v.address()),
    "Records a balance with its exact queried qualified address."
);
observation_type!(
    HistoryPage<A>,
    address_history,
    |v, c| matches!(&c.operation, Operation::AddressHistory { address, request } if address == v.address() && *request == v.request()),
    "Records a complete bounded page with its exact queried address and page inputs."
);
observation_type!(
    FeeEstimates<A>,
    fee_estimates,
    |_v, c| matches!(c.operation, Operation::FeeEstimates),
    "Records exact native-unit fee buckets with the corresponding fee operation."
);
observation_type!(
    TransactionStatus,
    transaction_status,
    |v, c| matches!(c.operation, Operation::TransactionStatus { txid } if txid == v.txid()),
    "Records transaction status with its exact source/query identifier."
);
observation_type!(
    Transaction<A>,
    transaction,
    |v, c| v.network() == c.network.network()
        && matches!(c.operation, Operation::Transaction { txid, maximum_entries } if txid == v.data().status.txid() && v.data().inputs.len() <= maximum_entries as usize && v.data().outputs.len() <= maximum_entries as usize),
    "Records a complete indexed transaction with matching family, source/query identity and capacity."
);
