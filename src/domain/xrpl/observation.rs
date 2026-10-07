// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};

use super::{
    AccountBalance, FeeEstimate, Hash, HistoryPage, Network, Transaction, TransactionStatus,
    TrustLinePage,
};

/// An explicitly source-reported XRP Ledger evaluation identity.
///
/// A hash alone does not prove consensus. `validated` records the source's own
/// assessment. Binary history can report validated inclusion without returning
/// its ledger hash. Open-ledger fee context also has an index and no ledger hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LedgerFields")]
pub struct Ledger {
    index: u32,
    hash: Option<Hash>,
    validated: bool,
}
impl Ledger {
    /// Records actual evaluation facts without inventing finality or close time.
    ///
    /// # Errors
    /// Rejects a zero index. A missing hash remains explicit even if the source
    /// reports validated transaction inclusion.
    pub const fn new(index: u32, hash: Option<Hash>, validated: bool) -> Result<Self, Error> {
        if index == 0 {
            return Err(Error::Validation(ValidationError::InvalidXrplRecord));
        }
        Ok(Self {
            index,
            hash,
            validated,
        })
    }
    /// Returns the actual ledger sequence number.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
    /// Returns an actual closed-ledger hash, if reported.
    #[must_use]
    pub const fn hash(self) -> Option<Hash> {
        self.hash
    }
    /// Returns the source-reported validation assessment.
    #[must_use]
    pub const fn validated(self) -> bool {
        self.validated
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerFields {
    index: u32,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    hash: Option<Hash>,
    validated: bool,
}
impl TryFrom<LedgerFields> for Ledger {
    type Error = Error;
    fn try_from(v: LedgerFields) -> Result<Self, Error> {
        Self::new(v.index, v.hash, v.validated)
    }
}

/// The typed operation represented by an XRP Ledger observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Account-root XRP drops and sequencing facts.
    AccountBalance,
    /// One issued-balance/trustline page.
    TrustLines,
    /// Current fee suggestions for the open ledger.
    FeeEstimate,
    /// A transaction payload and source-reported inclusion lookup.
    Transaction,
    /// A transaction execution and source-reported inclusion lookup.
    TransactionStatus,
    /// One bounded account-history page over an explicit range.
    AccountHistory,
}

/// Explicit XRP Ledger network, evaluation and retrieval attribution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    operation: Operation,
    network: Network,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    ledger: Option<Ledger>,
    source: Source,
    retrieved_at: Timestamp,
}
impl Context {
    /// Records caller-supplied facts without network access or a clock.
    #[must_use]
    pub const fn new(
        operation: Operation,
        network: Network,
        ledger: Option<Ledger>,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Self {
        Self {
            operation,
            network,
            ledger,
            source,
            retrieved_at,
        }
    }
    /// Returns the typed represented operation.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }
    /// Returns the supplied technical network and independent alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns actual family ledger facts without an invented block time.
    #[must_use]
    pub const fn ledger(&self) -> Option<Ledger> {
        self.ledger
    }
    /// Returns explicit non-secret method and provider attribution.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns retrieval time, independently of the ledger close time.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}

/// A strict schema-versioned typed result with XRP Ledger attribution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation<T> {
    schema_version: u16,
    context: Context,
    value: T,
}
impl<T> Observation<T> {
    /// Returns the stable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns operation, network, ledger and retrieval attribution.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns the typed exact value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
    fn new(value: T, context: Context, expected: Operation) -> Result<Self, Error> {
        if context.operation() != expected {
            return Err(ValidationError::ObservationOperationMismatch.into());
        }
        Ok(Self {
            schema_version: 1,
            context,
            value,
        })
    }
}
impl Observation<AccountBalance> {
    /// Attributes account-root facts to a validated closed ledger.
    ///
    /// # Errors
    /// Rejects operation mismatch or a context that is not source-validated.
    pub fn account_balance(value: AccountBalance, context: Context) -> Result<Self, Error> {
        if !context
            .ledger()
            .is_some_and(|ledger| ledger.validated() && ledger.hash().is_some())
        {
            return Err(ValidationError::InvalidXrplRecord.into());
        }
        Self::new(value, context, Operation::AccountBalance)
    }
}
impl Observation<TrustLinePage> {
    /// Attributes trustline balances to their requested validated ledger.
    ///
    /// # Errors
    /// Rejects operation mismatch, unvalidated context or a differing page hash.
    pub fn trust_lines(value: TrustLinePage, context: Context) -> Result<Self, Error> {
        if !context
            .ledger()
            .is_some_and(|ledger| ledger.validated() && ledger.hash().is_some())
            || value
                .request()
                .ledger_hash()
                .is_some_and(|hash| context.ledger().and_then(Ledger::hash) != Some(hash))
        {
            return Err(ValidationError::InvalidXrplRecord.into());
        }
        Self::new(value, context, Operation::TrustLines)
    }
}
impl Observation<FeeEstimate> {
    /// Attributes open-ledger suggestions without claiming validated state.
    ///
    /// # Errors
    /// Rejects operation mismatch or a closed/validated fee context.
    pub fn fee_estimate(value: FeeEstimate, context: Context) -> Result<Self, Error> {
        if !context
            .ledger()
            .is_some_and(|ledger| !ledger.validated() && ledger.hash().is_none())
        {
            return Err(ValidationError::InvalidXrplRecord.into());
        }
        Self::new(value, context, Operation::FeeEstimate)
    }
}
impl Observation<Transaction> {
    /// Attributes a hash-checked payload to its actual reported inclusion.
    ///
    /// # Errors
    /// Rejects operation mismatch or differing payload/context inclusion.
    pub fn transaction(value: Transaction, context: Context) -> Result<Self, Error> {
        if value.ledger() != context.ledger() {
            return Err(ValidationError::InvalidXrplRecord.into());
        }
        Self::new(value, context, Operation::Transaction)
    }
}
impl Observation<TransactionStatus> {
    /// Attributes execution facts without interpreting missing data as failure.
    ///
    /// # Errors
    /// Rejects operation mismatch or differing status/context inclusion.
    pub fn transaction_status(value: TransactionStatus, context: Context) -> Result<Self, Error> {
        if value.ledger() != context.ledger() {
            return Err(ValidationError::InvalidXrplRecord.into());
        }
        Self::new(value, context, Operation::TransactionStatus)
    }
}
impl Observation<HistoryPage> {
    /// Attributes a searched account-history range without inventing a single anchor.
    ///
    /// # Errors
    /// Rejects operation mismatch or a fabricated history evaluation ledger.
    pub fn account_history(value: HistoryPage, context: Context) -> Result<Self, Error> {
        if context.ledger().is_some() {
            return Err(ValidationError::InvalidXrplRecord.into());
        }
        Self::new(value, context, Operation::AccountHistory)
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
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = Fields::<$value>::deserialize(d)?;
                if v.schema_version != 1 {
                    return Err(serde::de::Error::custom(
                        ValidationError::UnsupportedSchemaVersion,
                    ));
                }
                Self::$constructor(v.value, v.context).map_err(serde::de::Error::custom)
            }
        }
    };
}
observation_deserialize!(AccountBalance, account_balance);
observation_deserialize!(TrustLinePage, trust_lines);
observation_deserialize!(FeeEstimate, fee_estimate);

observation_deserialize!(Transaction, transaction);
observation_deserialize!(TransactionStatus, transaction_status);
observation_deserialize!(HistoryPage, account_history);
