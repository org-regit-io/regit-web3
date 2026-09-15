// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Feature-gated protocols module layout.

#[cfg(feature = "jupiter")]
pub mod jupiter;

#[cfg(feature = "uniswap")]
pub mod uniswap;

#[cfg(feature = "oneinch")]
pub mod oneinch;

#[cfg(feature = "lifi")]
pub mod lifi;

#[cfg(feature = "rubic")]
pub mod rubic;
