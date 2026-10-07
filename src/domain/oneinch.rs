// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact EVM Classic Swap v6.1 requests, reported route graphs and unsigned review.
//!
//! Native sentinel encoding is provider-specific; it is not a token contract.
//! Source quotes have no reported block, TTL or signed-execution proof. Fresh swap
//! responses remain distinct from original selections. Calldata is opaque source
//! evidence; structural review does not establish its ABI intent or approval.

mod preparation;
mod records;
mod request;

pub use preparation::{PreparedSwap, SourceTransaction, SourceTransactionData};
pub use records::{
    Context, Expiry, Hop, LiquiditySource, LiquiditySources, Method, ProtocolShare, Quote,
    QuoteData, RouteGraph, Spender, StateOverrides, Token, TokenSwaps,
};
pub use request::{
    Asset, AssetKind, Limits, ProtocolId, QuoteRequest, QuoteSettings, ReturnPolicy, SlippageBps,
    SourceText, SwapRequest, SwapSettings,
};

use crate::error::{Error, ValidationError};
fn invalid_request() -> Error {
    ValidationError::InvalidOneinchRequest.into()
}
fn invalid_record() -> Error {
    ValidationError::InvalidOneinchRecord.into()
}
fn invalid_payload() -> Error {
    ValidationError::InvalidOneinchPayload.into()
}

fn bounded<'de, D, T, const MAX: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    struct Visitor<T, const MAX: usize>(std::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>, const MAX: usize> serde::de::Visitor<'de>
        for Visitor<T, MAX>
    {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bounded 1inch collection")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut a: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(v) = a.next_element()? {
                if values.len() == MAX {
                    return Err(serde::de::Error::custom("1inch collection bound"));
                }
                values.push(v);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Visitor::<T, MAX>(std::marker::PhantomData))
}
