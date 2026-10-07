// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Modular Web3 primitives and typed operations for Rust.
//!
//! The catalogue comprises ten chain families: EVM, Solana, Cardano, Bitcoin,
//! Litecoin, Dogecoin, Bitcoin Cash, XRPL, TON, and `THORChain`; five protocols:
//! Jupiter, Uniswap, 1inch, LI.FI, and Rubic; and five providers: `CoinGecko`,
//! `DefiLlama`, Helius, Blockfrost, and mempool.space. Implementation is partial;
//! the catalogue remains the full required library scope.
//!
//! The [`domain`] module provides exact integer amounts and signed decimals,
//! validated family identities, account values, and attributable observations.
//! Constructors and deserialization enforce matching invariants. EVM and Solana
//! have distinct identity and context contracts; one family's block semantics
//! are not applied to another. The [`error`] module provides typed failures with
//! fixed diagnostics.
//!
//! Default features are empty. Pure family capabilities use standard `Send`
//! futures and allow independently supplied implementations. Optional concrete
//! backends supply the necessary RPC/provider access and select their own
//! runtime and transport dependencies. The `http` feature exposes explicit,
//! redacted HTTP(S) configuration. Concrete backends use private bounded transport.
//!
//! Currently, EVM native-balance reads are implemented through the `evm`
//! capability and `evm-http` backend. The backend verifies chain identity,
//! resolves a block, and reads with EIP-1898 `blockHash` and
//! `requireCanonical: true`. Retries retain the captured hash and address without
//! re-resolving a head or falling back to height. Solana provides pure identities,
//! account/balance/observation types, and native/token/account reader contracts;
//! its concrete RPC backend is pending. Wider chain operations, protocol/provider
//! operations, and wallet preparation/handoff contracts remain pending.
//!
//! Read a native balance through an established EVM client:
//!
//! ```no_run
//! # #[cfg(feature = "evm-http")]
//! # fn main() {
//! use regit_web3::{
//!     chains::evm::EvmClient,
//!     domain::{Address, Balance, BlockSelector, Observation},
//!     error::Error,
//! };
//!
//! async fn read_balance(
//!     client: &EvmClient,
//!     address: Address,
//! ) -> Result<Observation<Balance>, Error> {
//!     client.get_native_balance(address, Some(BlockSelector::Safe)).await
//! }
//! # }
//! # #[cfg(not(feature = "evm-http"))]
//! # fn main() {}
//! ```

pub mod chains;
#[cfg(feature = "http")]
pub mod config;
pub mod domain;
pub mod error;
pub mod protocols;
pub mod providers;
// Compile the private transport when a concrete consumer or its tests use it.
// Extend this predicate as additional integrations are implemented.
#[cfg(all(feature = "http", any(feature = "evm-http", test)))]
mod transport;
pub mod wallets;
