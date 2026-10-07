// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Modular Web3 primitives and typed operations for Rust.
//!
//! The catalogue comprises ten chain families: EVM, Solana, Cardano, Bitcoin,
//! Litecoin, Dogecoin, Bitcoin Cash, XRPL, TON, and `THORChain`; five protocols:
//! Jupiter, Uniswap, 1inch, LI.FI, and Rubic; and five providers: `CoinGecko`,
//! `DefiLlama`, Helius, Blockfrost, and mempool.space. All twenty modules provide
//! functional operations within their documented supported profiles. Authenticated
//! 1inch live data and final platform qualification remain separate pending checks.
//!
//! The [`domain`] module provides exact integer amounts and signed decimals,
//! validated family identities, account values, and attributable observations.
//! Constructors and deserialization enforce matching invariants. EVM, Solana,
//! Bitcoin, and Cardano retain distinct identity and context contracts. Bitcoin
//! records include exact satoshis, signed mempool deltas, bounded history, fees,
//! and inclusion status; Cardano records include ADA/native assets and bounded
//! `UTxO` pages. The [`error`] module provides typed failures with fixed diagnostics.
//!
//! Default features are empty. Pure family capabilities use standard futures
//! and allow independently supplied implementations. Their [`future::MaybeSend`]
//! bound preserves `Send` on native targets and permits host-local JavaScript
//! futures on `wasm32-unknown-unknown`. Optional concrete
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
//! Maintained legacy/v0/v1 codecs validate canonical structure without verifying
//! signatures. Ordinary recent-blockhash native/classic SPL preparation retains
//! last-valid block height in wallet review. Transaction/status, blockhash/height,
//! message-fee and unsigned-simulation capabilities preserve method-specific
//! evaluation and inclusion facts. Separate one-shot signed submission sets node
//! retries to zero and retains unknown post-dispatch outcomes. Seven added methods
//! have representative mainnet proof: real v1 retrieval and successful unsigned
//! native simulation; legacy/v0/SPL preparation and submission are fixture-qualified.
//! Its optional `solana-http` backend verifies the full genesis hash and reads
//! exact SOL balances, SPL token-account balances, and present/absent accounts.
//! Read results retain actual slot context. Bitcoin's optional `bitcoin-esplora`
//! backend verifies the full genesis hash and reads address balances, bounded
//! history, exact fee estimates, transaction status, and canonical raw/indexed
//! transaction retrieval. Cardano provides exact indexed records, original-CBOR
//! body hashing, typed readers and explicit Conway key-spend payment preparation.
//! The optional `blockfrost-http` backend verifies network magic and implements
//! seventeen indexed read/estimate methods plus separate one-shot raw-CBOR
//! submission. Declared fees and failed-script collateral stay separate from
//! indexed paid fees. Structural witness checks do not verify signatures; source
//! acknowledgement does not establish execution. Indexed reads do not establish
//! a hash-selected snapshot or lasting finality. `CoinGecko` provides pure search,
//! ID/currency price, markets-page and
//! historical-chart contracts; `coingecko-http` supplies explicit anonymous or
//! Demo/Pro-header reads with exact prices and independently timed series.
//! `DefiLlama` provides pure TVL, yield, stablecoin and USD analytics contracts;
//! `defillama-http` supplies independently configured TVL/analytics, yield and
//! stablecoin sources. Peg-denominated circulation, USD valuations and percent
//! APYs remain distinct; large datasets fail at explicit limits without truncation.
//! Helius provides pure DAS asset and current Parsed Events transaction/history
//! records with exact raw token units and bounded structured metadata. The
//! `helius-http` backend verifies full Solana genesis before every read using one
//! explicit RPC/REST base. Parser outcomes, execution, inclusion slots and DAS
//! index progress stay distinct; cached prices remain source valuations. Owner
//! and history continuations retain every original query control, and concrete
//! handles reject a different client before dispatch. All four methods have
//! representative authenticated mainnet qualification, including duplicate parsed
//! batches and successful history records; no signing or submission is provided.
//! `mempool-space-http` supplies genesis-checked mempool summaries, bounded recent
//! and full-ID lists, exact sat/vB recommendations and compatible canonical
//! Bitcoin transaction/status retrieval. Moving observations are independent;
//! complete lists fail at explicit limits without truncation or invented paging.
//! TON supplies CRC-checked standard addresses, full zero-state identity, exact
//! nanotons and bounded maintained BOC/transaction/message representation hashes.
//! `ton-http` supplies network/account/history/transaction/status/message-scan
//! reads and explicit wallet-body fee estimates. Account retries freeze the
//! selected masterchain sequence and check full returned block identity; history
//! linkage does not prove inclusion. Decoded fees and reported aggregate/outgoing
//! components remain separate. Optional caller-configured request spacing and
//! every retry share one deadline. Internal-message preparation supports wallet
//! review; outer sender/expiry remain unencoded wallet policy. Separate one-shot
//! submission retains acknowledgment or ambiguous post-dispatch outcome without
//! retry. Seven reads/estimates and an outgoing-fee transaction have live proof;
//! submission is fixture-qualified without funded writes or signature verification.
//! Jupiter V2 supplies quote-only source selection, fresh Metis V0 instructions,
//! immutable unsigned compilation and external-wallet handoff. Caller CU ceilings,
//! signer allow-lists and source-other-instruction placement are explicit. Source
//! instruction/lookup facts do not prove swap semantics. Composed exact-message
//! Solana fee and unsigned simulation share one deadline and retain separate slots.
//! Current mainnet quote/build/handoff/estimate have representative live proof;
//! the public taker returned `AccountNotFound`, without swap-success or submission.
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
//! source evidence. Cardano/Blockfrost all seventeen indexed reads/estimate and
//! explicit unsigned review have representative authenticated mainnet proof;
//! preparation supports protocol majors 9–11. No funded submission was performed.
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
//! Uniswap supplies V3 `QuoterV2` exact-input quotes and bounded comparison of
//! caller-supplied paths, with one captured canonical EVM hash and total deadline.
//! Raw output and per-hop prices/ticks remain exact source facts. Pure Universal
//! Router 2.1.2 preparation retains literal recipient, explicit deadline, exact
//! slippage floor and separate ERC-20/Permit2 allowance requirements. It supplies
//! unsigned transaction fields and typed wallet handoff without signing or
//! submission. V3 quote/comparison have representative live proof; deployment
//! declarations are caller-verified and no global route discovery is implied.
//! 1inch supplies Classic Swap v6.1 exact-input EVM quotes, bounded liquidity
//! catalogues, source spender reads and independent fresh unsigned preparation.
//! Pure `oneinch` contracts keep exact assets/units, actual graph/filter facts and
//! immutable original review; `oneinch-http` takes explicit caller-held Bearer
//! authorization and a replaceable API prefix. Separate spender/fresh reads do
//! not share a snapshot. Strict ordinary transaction fields retain source gas
//! suggestions; opaque calldata and estimation overrides do not prove reviewed
//! intent or execution. Expiry, block and genesis proof are unreported. The four
//! methods and local handoff have 31 deterministic/loopback tests; authenticated
//! live qualification remains pending.
//! Rubic supplies the direct API-v2 chain catalogue, all/best quotes, fresh unsigned
//! preparation and extended source status. Pure contracts retain caller-qualified
//! families, exact amounts/fees and bounded JSON source metadata; `rubic-http`
//! selects a separate outgoing backend. Original selections and explicit caller
//! constraints remain beside recalculated estimates and EVM/Solana payloads.
//! Structural checks do not establish calldata/instruction intent, genesis,
//! expiry or finality. Five operations and EVM review/handoff have representative
//! native Mac live proof; the supplied unrelated public hash returned `NotFound`.
//! Solana preparation has fixture proof. Direct preparation does not execute.
//! The [`wallets`] module supplies generic typed preparation, read-only review and
//! external handoff. Caller-generated IDs and exact snapshots are correlated
//! before a trusted caller-supplied verifier checks actual signed-content binding.
//! Confirmed output has no unchecked constructor or deserialization path. The
//! `evm` feature supplies canonical transaction preparation; the `xrpl` feature
//! supplies an ordinary Payment JSON adapter; `uniswap` supplies an unsigned
//! router-field adapter; `solana` supplies a canonical-message adapter; `ton`
//! supplies an internal-message adapter; `rubic` composes direct EVM/Solana
//! preparation with generic review/handoff; `oneinch` retains an unsigned source
//! envelope and explicit original swap review. Concrete cryptographic
//! verification, signing, custody and connectors remain separate extensions.
//! Preparation and handoff do not submit. Authenticated 1inch data and final
//! platform qualification remain pending. Representative
//! proofs apply to their recorded operations, networks and query variants.
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
pub mod future;
pub mod protocols;
pub mod providers;
// Compile the private transport when a concrete backend or its tests use it.
// Extend this predicate as additional integrations are implemented.
#[cfg(any(
    all(
        feature = "bitcoin-cash-electrum",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ),
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
            feature = "ton-http",
            feature = "helius-http",
            feature = "rubic-http",
            feature = "oneinch-http",
            test
        )
    )
))]
mod transport;
pub mod wallets;
