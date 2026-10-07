// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Family-specific chain capabilities and replaceable implementations.

#[cfg(any(feature = "litecoin-http", feature = "dogecoin-http"))]
mod blockcypher;

#[cfg(feature = "evm")]
pub mod evm;

#[cfg(feature = "solana")]
pub mod solana;

#[cfg(feature = "cardano")]
pub mod cardano;

#[cfg(feature = "bitcoin")]
pub mod bitcoin;

#[cfg(feature = "litecoin")]
pub mod litecoin;

#[cfg(feature = "dogecoin")]
pub mod dogecoin;

#[cfg(feature = "bitcoin-cash")]
pub mod bitcoin_cash;

#[cfg(feature = "xrpl")]
pub mod xrpl;

#[cfg(feature = "ton")]
pub mod ton;

#[cfg(feature = "thorchain")]
pub mod thorchain;
