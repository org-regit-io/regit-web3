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
//! `mempool-space-http` supplies genesis-checked mempool summaries, bounded recent
//! and full-ID lists, exact sat/vB recommendations and compatible canonical
//! Bitcoin transaction/status retrieval. Moving observations are independent;
//! complete lists fail at explicit limits without truncation or invented paging.
//! `THORChain` supplies pure Cosmos/asset identities and exact protocol records;
//! `thorchain-http` supplies RUNE balances, individual/complete layer-one pool
//! reads, network values, swap quotes, inbound vaults, chain heights and
//! transaction progress. Cosmos chain ID and account prefix remain separate.
//! Quotes preserve requested input identity and quantity; the source can expand
//! identifiers without reporting its resolved input, which remains explicitly
//! unreported. Full output fee identity
//! and expiry are checked. Per-chain heights and source stages do not establish
//! a common snapshot, independent external inclusion or signed execution.
//! Solana, Bitcoin, XRPL and these provider backends have representative read-live
//! qualification; `THORChain` has representative read/quote qualification.
//! Deterministic fixtures cover each implemented backend. This is point-in-time
//! source evidence; Cardano's live proof remains pending.
//! XRPL provides classic/X-address identities, exact drops and issued values,
//! source-attributed ledger observations, and ordinary unsigned Payment JSON
//! preparation. Its `xrpl-http` backend supplies ledger-anchored account/trustline
//! reads, explicit fee estimates, computed-ID opaque transaction retrieval,
//! separate execution/inclusion status, and bounded explicit-range account
//! history. Binary history retains omitted ledger hashes without inventing a
//! common anchor. Explicit signed-payload submission uses one write attempt and
//! preserves unresolved outcomes after dispatch; this submit-only path has fixture
//! qualification, without a funded live submission. Payment preparation covers
//! ordinary sequence-based XRP/issued transfers; concrete signing and additional
//! transaction families remain separate extensions.
//! The [`wallets`] module supplies generic typed preparation, read-only review and
//! external handoff. Caller-generated IDs and exact snapshots are correlated
//! before a trusted caller-supplied verifier checks actual signed-content binding.
//! Confirmed output has no unchecked constructor or deserialization path. The
//! `xrpl` feature supplies an ordinary Payment JSON adapter; concrete cryptographic
//! verification, signing, custody and connectors remain separate extensions.
//! Preparation and handoff do not submit. Wider chain and provider operations and
//! protocol operations remain pending.
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
// Compile the private transport when a concrete backend or its tests use it.
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
        feature = "mempool-space-http",
        feature = "thorchain-http",
        test
    )
))]
mod transport;
pub mod wallets;
