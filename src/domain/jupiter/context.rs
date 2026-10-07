// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::{Source, Timestamp, solana::Network},
    error::{Error, ValidationError},
};
use serde::{Deserialize, Deserializer, Serialize};
/// The actual source API operation, without inferred chain execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Quote-only V2 `/order` selection, with no returned transaction.
    OrderQuote,
    /// A fresh Metis V2 `/build` response.
    Build,
}
/// API provenance with declared mainnet identity, no reported slot or genesis proof.
/// Source quote selection, blockhash expiry and later RPC evaluation are distinct facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Fields")]
pub struct Context {
    schema_version: u16,
    operation: Operation,
    network: Network,
    source: Source,
    retrieved_at: Timestamp,
}
impl Context {
    /// Records actual API provenance and expected supported mainnet identity.
    /// # Errors
    /// Rejects unsupported network declarations.
    pub fn new(
        operation: Operation,
        network: Network,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        super::request::validate_network(&network)?;
        Ok(Self {
            schema_version: 1,
            operation,
            network,
            source,
            retrieved_at,
        })
    }
    /// Returns the public schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns the actual attributed API operation.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }
    /// Returns declared expected network identity without asserting source verification.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns source attribution.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns local retrieval time, independently of source blockhash fetch time.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields {
    schema_version: u16,
    operation: Operation,
    network: Network,
    source: Source,
    retrieved_at: Timestamp,
}
impl TryFrom<Fields> for Context {
    type Error = Error;
    fn try_from(v: Fields) -> Result<Self, Error> {
        if v.schema_version != 1 {
            return Err(ValidationError::UnsupportedSchemaVersion.into());
        }
        Self::new(v.operation, v.network, v.source, v.retrieved_at)
    }
}
/// Immutable domain record's structural API attribution contract.
/// Implementations do not independently authenticate provider facts or program semantics.
pub trait OperationValue {
    /// Validates exact request/network and API operation attribution.
    /// # Errors
    /// Rejects inconsistent declared identity or operation.
    fn validate_context(&self, context: &Context) -> Result<(), Error>;
}
/// Typed immutable Jupiter source record with actual API attribution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation<T: OperationValue> {
    value: T,
    context: Context,
}
impl<T: OperationValue> Observation<T> {
    /// Checks structural record/provenance binding.
    /// # Errors
    /// Rejects mismatched request/network/operation facts.
    pub fn new(value: T, context: Context) -> Result<Self, Error> {
        value.validate_context(&context)?;
        Ok(Self { value, context })
    }
    /// Returns the immutable typed source record.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
    /// Returns its separately retained API provenance.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
}
impl<'de, T: OperationValue + Deserialize<'de>> Deserialize<'de> for Observation<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct F<T> {
            value: T,
            context: Context,
        }
        let v = F::<T>::deserialize(d)?;
        Self::new(v.value, v.context).map_err(serde::de::Error::custom)
    }
}
