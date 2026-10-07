// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};

use super::{AddressBalance, Network, NetworkId, UtxoPage};

/// The typed indexed operation represented by an observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Current aggregate indexed asset balances.
    Balance,
    /// One current indexed unspent-output page.
    Utxos,
}

/// Attribution for a current indexed read without a fabricated snapshot anchor.
///
/// The indexer may change between pages or requests. A `UTxO` creation block does
/// not establish the evaluation block of its present unspent state. No finality
/// or confirmation assessment is invented by this context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    operation: Operation,
    network: Network,
    source: Source,
    retrieved_at: Timestamp,
}
impl Context {
    /// Records explicit caller-supplied network, attribution and retrieval time.
    #[must_use]
    pub const fn new(
        operation: Operation,
        network: Network,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Self {
        Self {
            operation,
            network,
            source,
            retrieved_at,
        }
    }
    /// Returns the declared indexed operation.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }
    /// Returns the technical network and display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns explicit non-secret provider/method/version labels.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns the supplied retrieval timestamp, independently of ledger time.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}

/// A strict typed indexed result with attributable family-specific context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation<T> {
    schema_version: u16,
    context: Context,
    value: T,
}

impl<T> Observation<T> {
    /// Returns the wire schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns indexed provenance without a universal block anchor.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns the typed value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
    fn validated(
        value: T,
        context: Context,
        operation: Operation,
        network: NetworkId,
    ) -> Result<Self, Error> {
        if context.operation() != operation {
            return Err(ValidationError::ObservationOperationMismatch.into());
        }
        if context.network().identity() != network {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(Self {
            schema_version: 1,
            context,
            value,
        })
    }
}
impl Observation<AddressBalance> {
    /// Attributes a balance to matching operation and technical network.
    ///
    /// # Errors
    /// Rejects operation or network mismatch; aliases remain independent.
    pub fn balance(value: AddressBalance, context: Context) -> Result<Self, Error> {
        let network = value.network().identity();
        Self::validated(value, context, Operation::Balance, network)
    }
}
impl Observation<UtxoPage> {
    /// Attributes an output page to matching operation and technical network.
    ///
    /// # Errors
    /// Rejects operation or network mismatch.
    pub fn utxos(value: UtxoPage, context: Context) -> Result<Self, Error> {
        let network = value.network().identity();
        Self::validated(value, context, Operation::Utxos, network)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields<T> {
    schema_version: u16,
    context: Context,
    value: T,
}
macro_rules! observation_deserialize {
    ($value:ty, $constructor:ident) => {
        impl<'de> Deserialize<'de> for Observation<$value> {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let fields = Fields::<$value>::deserialize(deserializer)?;
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
observation_deserialize!(AddressBalance, balance);
observation_deserialize!(UtxoPage, utxos);
