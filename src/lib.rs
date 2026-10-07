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
//! Constructors and deserialization enforce matching invariants. EVM, Solana,
//! Bitcoin, and Cardano retain distinct identity and context contracts. Bitcoin
//! records include exact satoshis, signed mempool deltas, bounded history, fees,
//! and inclusion status; Cardano records include ADA/native assets and bounded
//! `UTxO` pages. The [`error`] module provides typed failures with fixed diagnostics.
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
//! account/balance/observation types, and native/token/account reader contracts.
//! Its optional `solana-http` backend verifies the full genesis hash and reads
//! exact SOL balances, SPL token-account balances, and present/absent accounts.
//! Read results retain actual slot context. Bitcoin's optional `bitcoin-esplora`
//! backend verifies the full genesis hash and reads address balances, bounded
//! history, exact fee estimates, transaction status, and canonical raw/indexed
//! transaction retrieval. Cardano provides pure
//! balance/UTxO reader contracts; the optional `blockfrost-http` backend verifies
//! network magic and reads current indexed ADA/native-asset balances and explicit
//! `UTxO` pages. Indexed reads do not establish a hash-selected snapshot or lasting
//! finality. `CoinGecko` provides pure search, ID/currency price, markets-page and
//! historical-chart contracts; `coingecko-http` supplies explicit anonymous or
//! Demo/Pro-header reads with exact prices and independently timed series.
//! `DefiLlama` provides pure TVL, yield, stablecoin and USD analytics contracts;
//! `defillama-http` supplies independently configured TVL/analytics, yield and
//! stablecoin sources. Peg-denominated circulation, USD valuations and percent
//! APYs remain distinct; large datasets fail at explicit limits without truncation.
//! Solana, Bitcoin, XRPL and these market-provider backends have representative live
//! qualification alongside deterministic fixtures. This is point-in-time source
//! evidence; Cardano's backend has fixture coverage with live proof pending.
//! XRPL provides classic/X-address identities, exact drops and issued values,
//! source-attributed ledger observations, and ordinary unsigned Payment JSON
//! preparation. Its `xrpl-http` backend supplies ledger-anchored account/trustline
//! reads, explicit fee estimates, computed-ID opaque transaction retrieval,
//! separate execution/inclusion status, and bounded explicit-range account
//! history. Binary history retains omitted ledger hashes without inventing a
//! common anchor. Signed submission and advanced preparation remain pending.
//! Wider chain and provider operations, protocol operations, and wallet handoff
//! contracts remain pending.
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
#[cfg(all(
    feature = "http",
    any(
        feature = "evm-http",
        feature = "solana-http",
        feature = "bitcoin-esplora",
        feature = "blockfrost-http",
        feature = "xrpl-http",
        feature = "coingecko-http",
        feature = "defillama-http",
        test
    )
))]
mod transport;
pub mod wallets;
