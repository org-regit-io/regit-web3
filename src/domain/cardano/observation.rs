// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};

use super::{
    AddressBalance, AddressDetails, AssetDetails, Epoch, IndexPage, Network, NetworkData,
    NetworkId, PageItem, PageTarget, PaymentEstimate, ProtocolParameters, StakeAccount,
    SubmissionResult, Transaction, TransactionStatus, TransactionUtxos, UtxoPage,
};

/// The typed indexed operation represented by an observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Current aggregate indexed asset balances.
    Balance,
    /// One current indexed unspent-output page.
    Utxos,
    /// Supporting address classification and stake data.
    AddressDetails,
    /// One explicit address transaction page.
    AddressTransactions,
    /// Current network supply and stake.
    NetworkData,
    /// Exact or latest selected epoch.
    Epoch,
    /// Exact source protocol parameters.
    ProtocolParameters,
    /// Stake account registration/delegation/reward facts.
    StakeAccount,
    /// One explicit reward page.
    Rewards,
    /// One explicit native-asset catalogue page.
    Assets,
    /// Requested native-asset identity/supply/metadata.
    AssetDetails,
    /// One explicit token transaction page.
    AssetTransactions,
    /// One explicit token holder page.
    AssetHolders,
    /// Correlated original transaction CBOR and source inclusion.
    Transaction,
    /// Complete bounded transaction input/output facts.
    TransactionUtxos,
    /// Current source transaction indexing/validity status.
    TransactionStatus,
    /// Local exact ordinary-payment calculation using fresh indexed parameters.
    PaymentEstimate,
    /// One-shot source submission acknowledgement, not execution/finality.
    Submission,
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
/// Trusted extension binding typed results to their actual operation/network.
/// Custom backends must preserve source semantics and exact family identities.
pub trait ObservationValue {
    /// Returns the actual represented operation.
    fn operation(&self) -> Operation;
    /// Returns the actual technical network identity.
    fn network_identity(&self) -> NetworkId;
}
macro_rules! observed {
    ($type:ty,$operation:ident) => {
        impl ObservationValue for $type {
            fn operation(&self) -> Operation {
                Operation::$operation
            }
            fn network_identity(&self) -> NetworkId {
                self.network().identity()
            }
        }
    };
}
observed!(AddressBalance, Balance);
observed!(UtxoPage, Utxos);
observed!(AddressDetails, AddressDetails);
observed!(NetworkData, NetworkData);
observed!(Epoch, Epoch);
observed!(ProtocolParameters, ProtocolParameters);
observed!(StakeAccount, StakeAccount);
observed!(AssetDetails, AssetDetails);
observed!(Transaction, Transaction);
observed!(TransactionUtxos, TransactionUtxos);
observed!(TransactionStatus, TransactionStatus);
observed!(PaymentEstimate, PaymentEstimate);
observed!(SubmissionResult, Submission);
impl<T: PageItem> ObservationValue for IndexPage<T> {
    fn operation(&self) -> Operation {
        match self.target() {
            PageTarget::Assets => Operation::Assets,
            PageTarget::AddressTransactions { .. } => Operation::AddressTransactions,
            PageTarget::AssetTransactions { .. } => Operation::AssetTransactions,
            PageTarget::AssetHolders { .. } => Operation::AssetHolders,
            PageTarget::Rewards { .. } => Operation::Rewards,
        }
    }
    fn network_identity(&self) -> NetworkId {
        self.network().identity()
    }
}
impl<T: ObservationValue> Observation<T> {
    /// Attributes a typed result without inventing indexed snapshot/finality facts.
    ///
    /// # Errors
    /// Rejects operation or technical network mismatch.
    pub fn new(value: T, context: Context) -> Result<Self, Error> {
        let operation = value.operation();
        let network = value.network_identity();
        Self::validated(value, context, operation, network)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields<T> {
    schema_version: u16,
    context: Context,
    value: T,
}
impl<'de, T: ObservationValue + Deserialize<'de>> Deserialize<'de> for Observation<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Fields::<T>::deserialize(d)?;
        if v.schema_version != 1 {
            return Err(serde::de::Error::custom(
                ValidationError::UnsupportedSchemaVersion,
            ));
        }
        Self::new(v.value, v.context).map_err(serde::de::Error::custom)
    }
}
