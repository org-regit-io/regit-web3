# Features and targets

[Overview](../README.md) · [Primitives](primitives.md) · [Catalogue](catalogue.md) · [Contracts](contracts.md) · [Features](features.md) · [Qualification](qualification.md) · [Development](development.md)

Default features are empty. Pure capabilities use standard Rust futures and do not select an HTTP client or asynchronous runtime. Concrete implementations enable their own dependencies explicitly.

## Pure capabilities

| Feature | API / composition |
| --- | --- |
| No features | Shared exact values and checked arithmetic, core market identifiers/records, explicit asset bindings, bounded observation collections, typed errors and generic wallet preparation/review/handoff; no networking dependencies |
| `evm` | Pure native/ERC-20/transaction, `FeeReader`, `ExecutionReader` and separate `EvmSubmitter` capabilities; exact preparation and bounded signed-envelope types |
| `litecoin`, `dogecoin` | Distinct family address/genesis/unit contracts and five-method reader capabilities |
| `bitcoin-cash` | Pure CashAddr/legacy address, network/fork/header identities, exact CashToken/source records and six-method `BitcoinCashReader` |
| `solana` | Pure identities, legacy/v0/v1 codecs, typed read/execution/submission capabilities and legacy native/classic SPL preparation/wallet review |
| `bitcoin` | Pure family types/canonical transactions; `BitcoinReader` balance/history/fees/status and separate `TransactionReader` capabilities |
| `cardano` | Pure indexed records, original CBOR/body hashing, `CardanoReader`/`CardanoSubmitter` capabilities and ordinary Conway payment preparation/review |
| `blockfrost` | Cardano family composition and provider module; HTTP backend selected separately |
| `xrpl` | Pure family identities/amounts, ledger/history/transaction/submission contracts, local unsigned Payment fields and the `XrplPaymentPreparation` wallet adapter |
| `coingecko` | Pure search/price/markets/history contracts and exact market records |
| `defillama` | Pure TVL/yields/stablecoins/analytics contracts and exact market records |
| `helius` | Pure DAS asset/source records, current parsed transaction/history records, exact metadata and four-method `HeliusReader` capability |
| `mempool-space` | Pure Bitcoin mempool/fees/transaction reader capability, exact source records and bounded collections |
| `ton` | Pure address/network/BOC/account/transaction/source-fee contracts, `TonReader`/`TonSubmitter` capabilities and unsigned internal-message preparation/wallet adapter |
| `thorchain` | Pure Cosmos/asset identities, exact source records, quote input-resolution metadata and the eight-method `ThorchainReader` capability |
| `lifi` | Pure cross-family quote/route/preparation/status contracts and `LifiReader` with independently supplied step handles |
| `jupiter` | Pure V2 quotes/routes/builds, explicit source/caller review and canonical V0 unsigned preparation/wallet adapter |
| `uniswap` | Pure V3 deployment/path/quote/comparison contracts, `V3QuoteReader` and unsigned Universal Router 2.1.2 preparation/wallet adapter |
| `oneinch` | Pure exact EVM assets, v6.1 graphs/filter checks, `ClassicSwapReader` and immutable unsigned source preparation/handoff; no credentials or networking |
| `rubic` | Pure cross-family direct-route contracts, exact bounded JSON metadata codec, `RubicReader` and unsigned EVM/Solana preparation/handoff |

## Backend configuration and implementations

| Feature | API / composition |
| --- | --- |
| `http` | Shared `HttpConfig` and `RpcEndpoint`; `RpcLimits` is also available with `bitcoin-cash-electrum` |
| `evm-http` | Bounded `EvmClient` implementing read/simulation capabilities and separate explicit signed submission; shared optional HTTP transport |
| `litecoin-http`, `dogecoin-http` | Family-specific `BlockCypherClient` and explicit `BlockCypherConfig`; documented mainnet sources only |
| `bitcoin-cash-electrum` | Native certificate-verified Electrum-Cash 1.6 TLS `ElectrumClient`, explicit trust roots/network/source/limits and caller-owned Tokio runtime; no HTTP backend |
| `solana-http` | Bounded `SolanaClient` implementing native/token/account/transaction/status/blockhash/fee/simulation capabilities and separate submission with explicit `SolanaHttpConfig`; shared optional HTTP transport |
| `bitcoin-esplora` | Bounded `EsploraClient` implementing Bitcoin read and transaction capabilities with explicit `EsploraConfig` |
| `blockfrost-http` | Bounded `BlockfrostClient` for all seventeen indexed read/estimate methods and separate one-shot raw-CBOR submission; explicit `BlockfrostHttpConfig` and caller-owned project credentials |
| `xrpl-http` | Bounded `XrplClient` with explicit expected network and `XrplHttpConfig` |
| `coingecko-http` | Bounded `CoinGeckoClient` with explicit `CoinGeckoHttpConfig`, optional caller-supplied credential tier and item bounds |
| `defillama-http` | Bounded `DefiLlamaClient` with independent source configurations and explicit item bounds |
| `helius-http` | Bounded `HeliusClient` with explicit full Solana genesis and one RPC/Parsed Events base; caller-owned authentication/runtime and opaque concrete-client-bound continuation handles |
| `mempool-space-http` | Bounded `MempoolSpaceClient` with explicit genesis/API configuration; composes the compatible Bitcoin Esplora backend |
| `ton-http` | `TonClient` with explicit `TonHttpConfig`, zero-state identity, replaceable API-v2 endpoint and optional caller-selected request spacing |
| `thorchain-http` | Bounded `ThorchainClient` with explicit `ThorchainHttpConfig`, expected Cosmos chain ID and separate account prefix |
| `lifi-http` | Bounded `LifiClient`, explicit chain-family catalogue, optional caller-provided headers and private exact continuation bound to the configured authority |
| `jupiter-http` | `JupiterClient` for explicitly configured V2 quote/build access; composes a caller-selected Solana execution reader for exact-message fees/simulation |
| `uniswap-http` | `UniswapV3Client` composes the EVM backend; exact-input quotes and supplied-route comparison use frozen canonical hashes and one total deadline |
| `oneinch-http` | `OneinchClient` with explicit v6.1 API prefix, EVM network, bounds and caller-held Bearer header; GET-only quote/catalogue/spender/unsigned preparation |
| `rubic-http` | Bounded direct API-v2 `RubicClient`, explicit `RubicHttpConfig`/family catalogue/headers and one total deadline; quote/build requests do not execute swaps |
| `all` | All catalogue features and currently implemented backends |

Concrete HTTP backends use Reqwest/rustls and a caller-owned Tokio runtime with I/O/time drivers on native targets. On `wasm32-unknown-unknown`, they use host Fetch, streams, abort signals and timers without a Tokio runtime. The host owns TLS and CORS policy; browser requests reject redirects and omit ambient cookies, cache and referrers. Explicit endpoint credentials remain caller-owned. Browser-controlled headers and URL user information fail before dispatch. The library loads no environment configuration.

## Native and JavaScript targets

| Target | Requirements and limits |
| --- | --- |
| Native Rust | Pure capabilities preserve `Send` futures and applicable `Sync` client/handle bounds; optional HTTP backends use the caller's Tokio runtime |
| JavaScript WASM | Pure identities, exact values, encodings and preparations; capability futures and shared clients stay on their originating host thread. TON hash-map initialization requires real `crypto.getRandomValues` (browser/worker or Node 19+) |
| Browser HTTP | Fetch/Request/Response, readable byte streams, AbortController, timers and performance clocks; provider CORS must permit explicit requests/headers. `connect_timeout` bounds response headers because Fetch does not expose a socket-connect phase; the total deadline spans all stages, retries and body consumption |
| `bitcoin-cash-electrum` | Native certificate-verified TCP/TLS backend; browsers cannot open raw Electrum sockets. The pure `bitcoin-cash` feature remains available on WASM |

The [Rust example and live-test guide](../examples/README.md) document explicit inputs, backend selection and read-specific qualification.

## Implementation layout

Private implementation roles remain separate:

| Location | Responsibility |
| --- | --- |
| `src/domain/mod.rs` and `src/domain/` | Public domain composition; exact values/arithmetic, asset bindings, observation collections and family identities/records |
| `src/config/mod.rs`, `src/config/http.rs`, `src/config/limits.rs`, `src/config/evm.rs` | Configuration composition; shared HTTP endpoints, transport limits and separate EVM configuration |
| Chain/provider capability modules | Runtime-independent typed operation contracts |
| Backend modules, such as `src/chains/solana/http.rs` | Network calls, operation stages and source attribution |
| Integration `wire.rs` modules | Typed remote request/result fields and integration-specific validation |
| `src/transport/http.rs`, `src/transport/http/{native,browser}.rs` | Shared HTTP policy and separate native/browser exchanges |
| `src/transport/{budget,clock,rpc}.rs` | Operation deadlines, target clocks and pure JSON-RPC encoding/decoding |
| `src/transport/http/{tests,write_tests}.rs`, `tests/wasm_http.rs` | Native shared-transport fixtures and actual browser HTTP/RPC behavior |

The [primitive guide](primitives.md) shows default-core arithmetic and explicitly selected Solana composition without a backend.
