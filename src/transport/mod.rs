// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Private transport components with separate HTTP, deadline, and codec roles.

mod budget;
mod http;
#[cfg(any(feature = "evm-http", feature = "solana-http"))]
mod rpc;

pub(crate) use budget::OperationBudget;
pub(crate) use http::HttpClient;
#[cfg(any(feature = "xrpl-http", feature = "evm-http"))]
pub(crate) use http::submission_unknown;

#[cfg(feature = "evm-http")]
pub(crate) use rpc::{decode_response, encode_request};
