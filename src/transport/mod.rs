// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Private transport components with separate HTTP, deadline, and codec roles.

mod budget;
#[cfg(all(
    feature = "http",
    any(
        feature = "evm-http",
        feature = "solana-http",
        feature = "bitcoin-esplora",
        feature = "litecoin-http",
        feature = "dogecoin-http",
        feature = "blockfrost-http",
        feature = "xrpl-http",
        feature = "coingecko-http",
        feature = "defillama-http",
        feature = "mempool-space-http",
        feature = "thorchain-http",
        feature = "lifi-http",
        feature = "ton-http",
        feature = "helius-http",
        feature = "rubic-http",
        test
    )
))]
mod http;
#[cfg(any(feature = "evm-http", feature = "solana-http", feature = "helius-http"))]
mod rpc;

pub(crate) use budget::OperationBudget;
#[cfg(all(
    feature = "http",
    any(
        feature = "evm-http",
        feature = "solana-http",
        feature = "bitcoin-esplora",
        feature = "litecoin-http",
        feature = "dogecoin-http",
        feature = "blockfrost-http",
        feature = "xrpl-http",
        feature = "coingecko-http",
        feature = "defillama-http",
        feature = "mempool-space-http",
        feature = "thorchain-http",
        feature = "lifi-http",
        feature = "ton-http",
        feature = "helius-http",
        feature = "rubic-http",
        test
    )
))]
pub(crate) use http::HttpClient;
#[cfg(any(
    feature = "xrpl-http",
    feature = "evm-http",
    feature = "ton-http",
    feature = "blockfrost-http",
    feature = "solana-http"
))]
pub(crate) use http::submission_unknown;
#[cfg(any(feature = "evm-http", feature = "solana-http", feature = "helius-http"))]
pub(crate) use rpc::{decode_response, encode_request};
