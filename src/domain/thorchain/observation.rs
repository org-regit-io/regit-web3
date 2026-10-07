// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Serialize};

use super::{
    Address, Asset, CollectionLimit, InboundAddresses, LastBlocks, Network, NetworkData, Pool,
    Pools, RuneBalance, SwapQuote, SwapRequest, TransactionStatus, Txid,
};
use crate::{
    domain::{Source, Timestamp},
    error::{Error, ValidationError},
};

/// Exact `THORNode` operation and caller-supplied query inputs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Native RUNE balance lookup.
    RuneBalance {
        /// Exact queried THOR account.
        address: Address,
    },
    /// One full layer-one pool identity.
    Pool {
        /// Exact queried pool asset.
        asset: Asset,
    },
    /// Complete bounded source pool catalogue.
    Pools {
        /// Explicit caller collection ceiling; no truncation.
        limit: CollectionLimit,
    },
    /// Source network data and actual fee/price suggestions.
    Network,
    /// Exact swap quote request; preparation/signing/submission are separate.
    SwapQuote {
        /// Full asset/amount/optional setting inputs.
        request: Box<SwapRequest>,
    },
    /// Complete bounded inbound vault/halt/fee-unit source state.
    InboundAddresses {
        /// Explicit caller collection ceiling.
        limit: CollectionLimit,
    },
    /// Actual external observation and `THORChain` signing/evaluation heights.
    LastBlocks {
        /// Explicit caller collection ceiling.
        limit: CollectionLimit,
    },
    /// Request-bound observed/planned/outbound transaction progress.
    TransactionStatus {
        /// Exact queried inbound identifier.
        txid: Txid,
        /// Explicit per-collection ceiling, including nested coins.
        limit: CollectionLimit,
    },
}

/// Schema-versioned expected Cosmos identity, exact request and source retrieval facts.
///
/// No common block hash is invented for separately queried source state.
/// External/THOR heights remain supplied inside their family records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContextFields")]
pub struct Context {
    schema_version: u16,
    network: Network,
    operation: Operation,
    source: Source,
    retrieved_at: Timestamp,
}
impl Context {
    /// Records explicit query/source/time and validates supplied THOR address prefixes.
    ///
    /// # Errors
    /// Rejects an address whose prefix differs from the caller-qualified network.
    pub fn new(
        network: Network,
        operation: Operation,
        source: Source,
        retrieved_at: Timestamp,
    ) -> Result<Self, Error> {
        match &operation {
            Operation::RuneBalance { address } => check_address(address, &network)?,
            Operation::SwapQuote { request } => {
                for address in [
                    request.parameters().destination.as_ref(),
                    request.parameters().refund_address.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    if address.chain().as_str() == "THOR" {
                        check_address(&Address::parse(address.as_str())?, &network)?;
                    }
                }
            }
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
    /// Returns the supported schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    /// Returns expected exact Cosmos chain ID and separately qualified account prefix.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns the exact typed source operation and query inputs.
    #[must_use]
    pub const fn operation(&self) -> &Operation {
        &self.operation
    }
    /// Returns bounded source attribution with no endpoint/credentials.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
    /// Returns explicitly supplied whole Unix seconds after retrieval.
    #[must_use]
    pub const fn retrieved_at(&self) -> Timestamp {
        self.retrieved_at
    }
}
fn check_address(address: &Address, network: &Network) -> Result<(), Error> {
    if address.prefix() == network.account_prefix() {
        Ok(())
    } else {
        Err(ValidationError::NetworkMismatch.into())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFields {
    schema_version: u16,
    network: Network,
    operation: Operation,
    source: Source,
    retrieved_at: Timestamp,
}
impl TryFrom<ContextFields> for Context {
    type Error = Error;
    fn try_from(v: ContextFields) -> Result<Self, Error> {
        if v.schema_version == 1 {
            Self::new(v.network, v.operation, v.source, v.retrieved_at)
        } else {
            Err(super::invalid_record())
        }
    }
}

/// Validated query-bound `THORNode` source data; it is not independent consensus proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation<T> {
    context: Context,
    value: T,
}
impl<T> Observation<T> {
    /// Returns validated request/network/source attribution.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }
    /// Returns the exact typed source value.
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

macro_rules! observed {
    ($ty:ty, $method:ident, $check:expr, $doc:literal) => {
        impl Observation<$ty> {
            #[doc = $doc]
            ///
            /// # Errors
            /// Rejects differing query identities, collection bounds or observation time.
            pub fn $method(value: $ty, context: Context) -> Result<Self, Error> {
                let check: fn(&$ty, &Context) -> bool = $check;
                if !check(&value, &context) {
                    return Err(ValidationError::ObservationOperationMismatch.into());
                }
                Ok(Self { context, value })
            }
        }
        impl<'de> Deserialize<'de> for Observation<$ty> {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = ObservationFields::<$ty>::deserialize(d)?;
                Self::$method(v.value, v.context).map_err(serde::de::Error::custom)
            }
        }
    };
}
observed!(
    RuneBalance,
    rune_balance,
    |v, c| matches!(&c.operation, Operation::RuneBalance { address } if address == v.address()),
    "Binds exact native RUNE to its queried account."
);
observed!(
    Pool,
    pool,
    |v, c| matches!(&c.operation, Operation::Pool { asset } if asset == v.asset()),
    "Binds a pool to its exact requested full asset identity."
);
observed!(
    Pools,
    pools,
    |v, c| matches!(&c.operation, Operation::Pools { limit } if v.items().len() <= limit.get() as usize),
    "Binds a complete pool catalogue to its explicit caller ceiling."
);
observed!(
    NetworkData,
    network,
    |_v, c| matches!(c.operation, Operation::Network),
    "Records exact network data with its source operation."
);
observed!(
    SwapQuote,
    swap_quote,
    |v, c| matches!(&c.operation, Operation::SwapQuote { request } if request.as_ref() == v.request() && v.observed_at() == c.retrieved_at && quote_prefixes(v, &c.network)),
    "Associates a nonexpired quote with the exact request/time, separately retaining source input resolution."
);
observed!(
    InboundAddresses,
    inbound_addresses,
    |v, c| matches!(&c.operation, Operation::InboundAddresses { limit } if v.items().len() <= limit.get() as usize && v.items().iter().all(|v| address_matches(&v.data().address, &c.network) && v.data().router.as_ref().is_none_or(|v| address_matches(v, &c.network)))),
    "Binds complete inbound states to the caller's collection ceiling."
);
observed!(
    LastBlocks,
    last_blocks,
    |v, c| matches!(&c.operation, Operation::LastBlocks { limit } if v.items().len() <= limit.get() as usize),
    "Records separately supplied external and `THORChain` heights."
);
observed!(
    TransactionStatus,
    transaction_status,
    |v, c| matches!(&c.operation, Operation::TransactionStatus { txid, limit } if txid == v.query_id() && status_within(v, *limit) && status_prefixes(v, &c.network)),
    "Binds source cross-chain state to its queried inbound ID and exact per-collection limits."
);
fn status_within(value: &TransactionStatus, limit: CollectionLimit) -> bool {
    let limit = limit.get() as usize;
    value.planned_outbounds().is_none_or(|v| v.len() <= limit)
        && value.outbounds().is_none_or(|v| v.len() <= limit)
        && value
            .transaction()
            .into_iter()
            .chain(value.outbounds().into_iter().flatten())
            .all(|v| {
                v.data().coins.as_ref().is_none_or(|v| v.len() <= limit)
                    && v.data().gas.as_ref().is_none_or(|v| v.len() <= limit)
            })
}
fn status_prefixes(value: &TransactionStatus, network: &Network) -> bool {
    let matches = |address: &super::ChainAddress| {
        address.chain().as_str() != "THOR"
            || Address::parse(address.as_str())
                .is_ok_and(|v| v.prefix() == network.account_prefix())
    };
    value
        .transaction()
        .into_iter()
        .chain(value.outbounds().into_iter().flatten())
        .all(|v| matches(&v.data().from_address) && matches(&v.data().to_address))
        && value
            .planned_outbounds()
            .into_iter()
            .flatten()
            .all(|v| matches(&v.data().to_address))
}

fn quote_prefixes(value: &SwapQuote, network: &Network) -> bool {
    [
        value.data().inbound_address.as_ref(),
        value.data().router.as_ref(),
    ]
    .into_iter()
    .flatten()
    .all(|address| {
        address.chain().as_str() != "THOR"
            || Address::parse(address.as_str())
                .is_ok_and(|v| v.prefix() == network.account_prefix())
    })
}

fn address_matches(address: &super::ChainAddress, network: &Network) -> bool {
    address.chain().as_str() != "THOR"
        || Address::parse(address.as_str()).is_ok_and(|v| v.prefix() == network.account_prefix())
}
