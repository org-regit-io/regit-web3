// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::super::{Commitment, Network, TransactionVersion};
use super::{
    BlockHeight, BlockhashValidity, ExecutionRequest, LatestBlockhash, MessageFee, Simulation,
    StatusLookup, Submission, TransactionLookup,
};
use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Deserializer, Serialize};

/// Required attribution with only the context actually provided by each RPC.
/// Inclusion slots and last-valid block heights remain separate value fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields")]
pub struct ExecutionContext {
    schema_version: u16,
    network: Network,
    request: ExecutionRequest,
    evaluation_slot: Option<u64>,
    source: Source,
    retrieved_at: Timestamp,
}
impl ExecutionContext {
    /// Records method-specific request and actual evaluation context facts.
    /// # Errors
    /// Rejects missing/invented evaluation slots and violated request lower bounds.
    pub fn new(
        network: Network,
        request: ExecutionRequest,
        evaluation_slot: Option<u64>,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        if request.has_evaluation_slot() != evaluation_slot.is_some() {
            return Err(super::invalid());
        }
        if request
            .evaluation_options()
            .and_then(super::super::ReadOptions::minimum_context_slot)
            .is_some_and(|m| evaluation_slot.is_none_or(|s| s < m))
        {
            return Err(ValidationError::ContextSlotBelowMinimum.into());
        }
        Ok(Self {
            schema_version: 1,
            network,
            request,
            evaluation_slot,
            source,
            retrieved_at,
        })
    }
    /// Returns the validated schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns caller-qualified full genesis identity and display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns actual supported request controls and frozen query values.
    #[must_use]
    pub const fn request(&self) -> &ExecutionRequest {
        &self.request
    }
    /// Returns an evaluation slot only for RPCs which actually report one.
    #[must_use]
    pub const fn evaluation_slot(&self) -> Option<u64> {
        self.evaluation_slot
    }
    /// Returns the explicit provider/method/library source labels.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns local retrieval time, independent of chain inclusion or lifetime.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFields {
    schema_version: u16,
    network: Network,
    request: ExecutionRequest,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    evaluation_slot: Option<u64>,
    source: Source,
    retrieved_at: Timestamp,
}
impl TryFrom<ContextFields> for ExecutionContext {
    type Error = Error;
    fn try_from(f: ContextFields) -> Result<Self, Error> {
        if f.schema_version != 1 {
            return Err(ValidationError::UnsupportedSchemaVersion.into());
        }
        Self::new(
            f.network,
            f.request,
            f.evaluation_slot,
            f.source,
            f.retrieved_at,
        )
    }
}

/// Typed attributed source result with constructor/serde request correlation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutionObservation<T> {
    context: ExecutionContext,
    value: T,
}
impl<T> ExecutionObservation<T> {
    /// Returns the actual method-specific context and attribution.
    #[must_use]
    pub const fn context(&self) -> &ExecutionContext {
        &self.context
    }
    /// Returns the exact source value, including explicit lookup/fee absence.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields<T> {
    context: ExecutionContext,
    value: T,
}
macro_rules! observation {
    ($ty:ty, $name:ident, $validate:expr) => {
        impl ExecutionObservation<$ty> {
            /// Attributes the typed value to its matching immutable method request.
            /// # Errors
            /// Rejects request/value identity or source-context inconsistencies.
            pub fn $name(value: $ty, context: ExecutionContext) -> Result<Self, Error> {
                ($validate)(&value, &context)?;
                Ok(Self { context, value })
            }
        }
        impl<'de> Deserialize<'de> for ExecutionObservation<$ty> {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let f = Fields::<$ty>::deserialize(d)?;
                Self::$name(f.value, f.context).map_err(serde::de::Error::custom)
            }
        }
    };
}
observation!(
    TransactionLookup,
    transaction,
    |v: &TransactionLookup, c: &ExecutionContext| {
        let ExecutionRequest::Transaction { signature, options } = c.request() else {
            return Err(super::invalid());
        };
        if *signature != v.signature {
            return Err(super::invalid());
        }
        if let Some(t) = &v.transaction
            && (t.body().signature() != *signature
                || matches!(t.body().message().version(), TransactionVersion::V1)
                    && options.maximum_supported_version() < 1)
        {
            return Err(super::invalid());
        }
        Ok(())
    }
);
observation!(
    StatusLookup,
    status,
    |v: &StatusLookup, c: &ExecutionContext| {
        let ExecutionRequest::Status { signature, .. } = c.request() else {
            return Err(super::invalid());
        };
        if *signature != v.signature {
            return Err(super::invalid());
        }
        if let Some(s) = &v.status
            && (c
                .evaluation_slot()
                .is_none_or(|slot| s.inclusion_slot > slot)
                || s.confirmation_status == Some(Commitment::Finalized)
                    && s.confirmations.is_some())
        {
            return Err(super::invalid());
        }
        Ok(())
    }
);
observation!(
    LatestBlockhash,
    latest_blockhash,
    |_: &LatestBlockhash, c: &ExecutionContext| if matches!(
        c.request(),
        ExecutionRequest::LatestBlockhash { .. }
    ) {
        Ok(())
    } else {
        Err(super::invalid())
    }
);
observation!(
    BlockhashValidity,
    blockhash_validity,
    |v: &BlockhashValidity, c: &ExecutionContext| if matches!(c.request(), ExecutionRequest::BlockhashValidity {blockhash,..} if *blockhash == v.blockhash)
    {
        Ok(())
    } else {
        Err(super::invalid())
    }
);
observation!(
    BlockHeight,
    block_height,
    |_: &BlockHeight, c: &ExecutionContext| if matches!(
        c.request(),
        ExecutionRequest::BlockHeight { .. }
    ) {
        Ok(())
    } else {
        Err(super::invalid())
    }
);
observation!(
    MessageFee,
    message_fee,
    |_: &MessageFee, c: &ExecutionContext| if matches!(
        c.request(),
        ExecutionRequest::MessageFee { .. }
    ) {
        Ok(())
    } else {
        Err(super::invalid())
    }
);
observation!(
    Simulation,
    simulation,
    |v: &Simulation, c: &ExecutionContext| {
        let ExecutionRequest::Simulation { transaction, .. } = c.request() else {
            return Err(super::invalid());
        };
        v.outcome().validate_message(transaction.message())
    }
);
observation!(
    Submission,
    submission,
    |v: &Submission, c: &ExecutionContext| if matches!(c.request(), ExecutionRequest::Submission {signature,..} if *signature == v.signature)
    {
        Ok(())
    } else {
        Err(super::invalid())
    }
);
