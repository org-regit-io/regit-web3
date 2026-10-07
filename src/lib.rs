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
//! EVM native/ERC-20 balance, allowance, optional metadata and transaction/receipt
//! status reads use pure capabilities and the optional `evm-http` backend.
//! State reads verify chain identity, resolve one block, and read with EIP-1898 `blockHash` and
//! `requireCanonical: true`. Retries retain the captured hash and address without
//! re-resolving a head or falling back to height. ERC-20 precision is never assumed.
//! Typed transaction/receipt records retain actual inclusion and distinct execution
//! outcomes; source JSON identities/signatures are not independently verified.
//! Nonce and local calls retain canonical state; gas estimates require the
//! Geth-compatible hash-selector extension without height fallback. Fee facts are
//! independently sourced. Exact native/ERC-20 preparation retains caller choices
//! and canonical legacy/type1/type2 signing bytes. Explicit one-shot submission
//! structurally checks supported signed envelopes and matches the computed hash;
//! it does not verify signatures/sender/intent or prove acceptance/execution.
//! Unresolved post-dispatch failures retain possible submission outcome.
//! Litecoin/Dogecoin provide family-qualified addresses, genesis identities and
//! exact litoshi/koinu balances with separate signed mempool deltas. Optional
//! `BlockCypher` mainnet backends read balances, height-cursor history, fee
//! preferences, complete indexed transactions and status after genesis checks.
//! Fees are per 1000 serialized bytes; opaque raw bytes and source inclusion do
//! not establish computed identity, consensus or signature proof.
//! Bitcoin Cash provides distinct `CashAddr`/legacy and `CashToken` contracts,
//! full genesis plus explicit fork-checkpoint identity, and six source reads
//! through optional certificate-verified Electrum-Cash 1.6 TLS. Explicit DER
//! trust roots, server name and limits select no HTTP dependency. Source history
//! intervals and zero/null status facts retain actual semantics. Raw bytes have
//! a computed identity; verbose fields remain separate, without consensus,
//! signature, raw-field agreement or independently verified inclusion claims.
//! Solana provides pure identities,
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
//! EVM added reads, Litecoin/Dogecoin, Bitcoin Cash, Solana, Bitcoin, XRPL and these provider
//! backends have representative read-live
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
//! LI.FI supplies exact-input quotes, bounded route alternatives, selected-step
//! source preparation and transaction/provider-transfer status. An explicit chain
//! catalogue prevents family inference from numbers. Immutable selections and
//! private authority-bound continuations stay separate from fresh estimates and
//! payloads; encoding does not establish verified signed intent. The four methods
//! have representative EVM live proof; other family encodings have fixture proof.
//! The [`wallets`] module supplies generic typed preparation, read-only review and
//! external handoff. Caller-generated IDs and exact snapshots are correlated
//! before a trusted caller-supplied verifier checks actual signed-content binding.
//! Confirmed output has no unchecked constructor or deserialization path. The
//! `evm` feature supplies canonical transaction preparation; the `xrpl` feature
//! supplies an ordinary Payment JSON adapter. Concrete cryptographic
//! verification, signing, custody and connectors remain separate extensions.
//! Preparation and handoff do not submit. Wider chain and provider operations and
//! other protocol operations remain pending.
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
#[cfg(any(feature = "http", feature = "bitcoin-cash-electrum"))]
pub mod config;
pub mod domain;
pub mod error;
pub mod protocols;
pub mod providers;
// Compile the private transport when a concrete backend or its tests use it.
// Extend this predicate as additional integrations are implemented.
#[cfg(any(
    feature = "bitcoin-cash-electrum",
    all(
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
            test
        )
    )
))]
mod transport;
pub mod wallets;
