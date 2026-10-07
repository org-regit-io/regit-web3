# Regit Web3

An open-source Rust library for reusable Web3 primitives and typed operations, developed by [Regit](https://www.regit.io).

The full library scope comprises ten chain families, five protocols and five data providers. Local exact values, family-specific identities, encoding and transaction preparation are separated from typed operation contracts and replaceable RPC/provider backends. Concrete backends are selected through Cargo features.

## Catalogue and implementation status

All modules below are part of the required library scope. Current implementation is partial; pending operations remain required.

| Chain family | Scope | Current implementation |
| --- | --- | --- |
| EVM | Native/ERC-20 balances, transactions/receipts, gas/fees, transfers and approvals | Hash-pinned native-balance reader and optional HTTP backend implemented; wider operations pending |
| Solana | SOL/SPL balances, accounts, transactions and transfers | Pure family contracts and optional HTTP backend for SOL, SPL token-account balances and account reads implemented; transactions and transfers pending |
| Cardano | Balances, UTxOs, assets, transactions, epoch/staking and preparation | Pure family contracts and optional Blockfrost current ADA/native-asset balances and explicit UTxO pages implemented; transactions, fees, epoch/staking and preparation pending |
| Bitcoin | Validation, balance/history, fees and transactions/status | Pure family contracts and optional Esplora balances, bounded history, exact fee estimates, status and canonical full transaction retrieval implemented |
| Litecoin | Validation, balance/history and fees | Pending |
| Dogecoin | Validation, balance/history and fees | Pending |
| Bitcoin Cash | Validation, balance/history and fees | Pending |
| XRPL | XRP/issued balances, history, trustlines and transfers | Address/amount identities, XRP balances, ledger-bound trustline pages, fees, bounded account history, binary transactions/status, ordinary sequence-based XRP/issued Payment JSON preparation and explicit one-shot signed submission implemented |
| TON | Validation, balance/history, network data and transfers | Pending |
| THORChain | RUNE balances, pools, network data, quotes and cross-chain state | RUNE balances, individual/complete layer-one pool reads, network values, swap quotes, inbound vaults, chain heights and transaction progress implemented |

| Protocol | Scope | Current implementation |
| --- | --- | --- |
| Jupiter | Solana aggregation quotes/routes and swap preparation | Pending |
| Uniswap | Direct EVM DEX quotes and swap preparation | Pending |
| 1inch | Aggregated EVM quotes/routes and swap preparation | Pending |
| LI.FI | Cross-chain quotes/routes, execution preparation and status | Pending |
| Rubic | Cross-chain quotes/routes and supported execution preparation | Pending |

| Provider | Scope | Current implementation |
| --- | --- | --- |
| CoinGecko | Asset search, prices, markets and historical market data | Search, exact ID/currency prices, explicit markets pages and historical price/capitalization/volume charts implemented |
| DefiLlama | Protocol TVL, yields, stablecoins and DeFi analytics | Protocol/chain TVL and history, yields/history, stablecoins/history and DEX/fees/revenue/holders-revenue analytics implemented |
| Helius | Solana assets, parsed transactions and address history | Pending |
| Blockfrost | Indexed Cardano network, asset and supporting account data | Genesis verification, current full-asset address balances and explicit UTxO pages implemented; wider provider operations pending |
| mempool.space | Bitcoin mempool, fees and transaction data | Backlog summaries, recent arrivals, bounded full transaction-ID lists, exact recommended fees, canonical transaction retrieval and inclusion status implemented |

Generic typed wallet preparation, review and external signing-handoff contracts are implemented, with an XRPL Payment JSON adapter. Returned signed content requires a caller-supplied verifier against the original preparation; matching echoed metadata alone does not verify it. Concrete cryptographic verification, signing, custody and connectors remain separate extensions. Callers own approval and replay policy; preparation and handoff do not submit.

## Implemented contracts

| Area | Contract |
| --- | --- |
| Exact values | `Amount` retains unsigned 256-bit base units and explicit optional decimals; `ExactDecimal` retains bounded signed decimals. Exact numeric values serialize as strings; no floating-point transaction amounts |
| Validation | Constructors and deserialization enforce matching identity, precision, schema and observation invariants. Typed errors use fixed diagnostics |
| Wallet handoff | Immutable family-specific network/intent/unsigned snapshots, bounded caller-generated IDs and read-only review. Correlation is checked before trusted signed-content verification; confirmed output has no unchecked constructor or deserialization path. Custom snapshots and verifiers must honor their documented contracts |
| EVM identity/context | EVM addresses/hashes use canonical lowercase hexadecimal; network identity uses the exact chain ID; native observations retain block number/hash/time, retrieval time and source |
| Solana identity/context | Addresses, hashes and signatures use canonical base58; network identity uses the full genesis hash. Typed results retain commitment/minimum context slot separately from the actual slot, without inventing block hashes/timestamps or independently verified finality |
| Bitcoin identity/context | Network-qualified addresses and full standard genesis hashes; exact confirmed satoshis and a separate signed mempool delta; bounded history retains its cursor and completeness limits. Canonical transaction bytes retain computed txid/wtxid, exact inputs/outputs/scripts/witness and weight; indexed previous outputs, nullable fee and inclusion remain separate source facts |
| Cardano identity/context | Payment/stake addresses, validated Byron CRC and nested encoding, explicit network tag/magic, and policy/raw-name asset identities; exact ADA and native-asset quantities retain declared precision. UTxO pages retain page/count/order and output origin separately from evaluation context |
| XRPL identity/context | Classic/X-address validation, network/tag/currency identities, exact XRP drops and issued decimals; actual ledger attribution, signed trustline balances and source-reported fee suggestions. Binary transaction IDs are recomputed; indexed inclusion and JSON execution outcomes remain separate |
| THORChain identity/context | Exact Cosmos chain ID and separate account prefix; full chain/symbol and distinct layer-one/synthetic/trade/secured representations. Protocol quantities use 1e8 units; native dust and gas retain their own units. Source stages and per-chain heights do not independently prove external inclusion |
| THORChain quotes | Requested input identity/amount remain supplied query facts. The source can expand asset identifiers and does not report its resolved input; the HTTP backend retains typed unreported resolution. Full output fee identity, fee totals, expiry and optional settings are checked |
| Market data | Provider listing IDs and explicit currencies/units; exact nullable prices, TVL/volume/fee/revenue values, signed APY percentages and supplied series timestamps; source and retrieval facts without invented ledger anchors |
| Bitcoin mempool data | Exact satoshi totals and sat/vB suggestions; individual histogram bins, at most ten recent arrivals and explicitly bounded full ID lists. Fractional weight/4 sizes retain quarter-byte increments; separate reads do not establish an atomic snapshot |
| EVM HTTP native read | Verify `eth_chainId` at establishment and before each read; resolve the selector; use EIP-1898 `blockHash` with `requireCanonical: true`; retries retain the captured hash/address without a new head or height fallback |
| Solana HTTP reads | Verify the full genesis hash at establishment and before each read; return exact SOL lamports, decoded present/absent accounts, or SPL raw units with mint/program/owner/state and reported decimals. Preserve requested commitment/minimum slot separately from the actual slot; ignore scaled UI amounts |
| Bitcoin Esplora reads | Verify full genesis at establishment and before each read; retain exact balances, bounded history, fee estimates and status. Full retrieval compares canonical raw bytes with indexed immutable fields under one deadline; structural/money checks do not execute scripts, validate signatures or prove inclusion |
| Cardano Blockfrost reads | Verify expected genesis network magic at establishment and before each read; return current indexed ADA/native-asset balances or one explicit bounded UTxO page. Missing data is not a zero balance |
| XRPL HTTP reads | Verify the expected network; resolve one validated ledger hash for account/trustline reads. History uses an explicit bounded ledger range; transaction bytes retain their computed identity and source metadata. Fees retain open-ledger context; pending/range records do not invent a ledger hash |
| XRPL signed submission | Explicit caller-supplied bytes and network; one write attempt after network preflight. Echoed bytes and computed identity are checked; preliminary handling never establishes validated execution. Unresolved failures after dispatch retain a potentially submitted outcome |
| Provider HTTP reads | Explicit CoinGecko anonymous/Demo/Pro configuration; independent DefiLlama TVL/yields/stablecoin endpoints. Exact decimal decoding, explicit collection/page limits, no silent truncation or source fallback |
| mempool.space HTTP reads | Explicit expected full Bitcoin genesis and API base; one total deadline for setup/reads/retries. Canonical transaction retrieval/status reuse compatible Esplora contracts and retain the supplied provider label |
| THORChain HTTP reads | Explicit Cosmos network/account-prefix/API configuration; network verification before each operation under one total deadline. Complete bounded collections fail without truncation; quotes expire without implicit refresh and separate reads have no common snapshot claim |
| Optional transport | Explicit configuration, verified TLS, bounded bodies/retries and one total operation deadline; endpoint credentials and raw provider messages excluded from diagnostics |

EVM HTTP native reads retain configured precision, finality `unknown` and confirmations `null`; requested tags do not establish either. Bitcoin and Cardano indexed reads do not promise a hash-selected snapshot or lasting finality. Each family retains its own ledger and source semantics. Pure observation construction validates supplied records and does not independently verify a remote source.

Solana native/account/SPL reads, Bitcoin indexed and full transaction reads, XRPL's six read methods, CoinGecko's four operations, DefiLlama's eight reader methods, mempool.space's six reads and THORChain's eight read/quote methods have representative opt-in live qualification. Wallet extension contracts and the XRPL JSON adapter have deterministic fixture qualification. XRPL's submit-only path has deterministic and loopback fixture qualification; no funded live submission was performed. Blockfrost live qualification remains pending. These operation-specific observations do not complete the full catalogue or qualify every network, provider plan or query variant.

## Features

Default features are empty. Pure capabilities use standard Rust futures and do not select an HTTP client or asynchronous runtime. Concrete implementations enable their own dependencies explicitly. Feature names alone do not imply implemented operations.

| Feature | API / composition |
| --- | --- |
| No features | Shared exact values, typed errors and generic wallet preparation/review/handoff; no networking dependencies |
| `evm` | Runtime-independent `NativeBalanceReader` capability |
| `solana` | Pure family types and native/token/account reader capabilities |
| `bitcoin` | Pure family types/canonical transactions; `BitcoinReader` balance/history/fees/status and separate `TransactionReader` capabilities |
| `cardano` | Pure family types and balance/UTxO reader capabilities |
| `blockfrost` | Cardano family composition and provider module; HTTP backend selected separately |
| `xrpl` | Pure family identities/amounts, ledger/history/transaction/submission contracts, local unsigned Payment fields and the `XrplPaymentPreparation` wallet adapter |
| `coingecko` | Pure search/price/markets/history contracts and exact market records |
| `defillama` | Pure TVL/yields/stablecoins/analytics contracts and exact market records |
| `mempool-space` | Pure Bitcoin mempool/fees/transaction reader capability, exact source records and bounded collections |
| `thorchain` | Pure Cosmos/asset identities, exact source records, quote input-resolution metadata and the eight-method `ThorchainReader` capability |
| `http` | Shared `HttpConfig`, `RpcEndpoint` and `RpcLimits` configuration |
| `evm-http` | Bounded `EvmClient` implementing the EVM reader; Reqwest/rustls and caller-owned Tokio runtime |
| `solana-http` | Bounded `SolanaClient` implementing native/token/account readers with explicit `SolanaHttpConfig`; Reqwest/rustls and caller-owned Tokio runtime |
| `bitcoin-esplora` | Bounded `EsploraClient` implementing Bitcoin read and transaction capabilities with explicit `EsploraConfig` |
| `blockfrost-http` | Bounded `BlockfrostClient` implementing Cardano balance/UTxO readers with explicit `BlockfrostHttpConfig`; Reqwest/rustls and caller-owned Tokio runtime |
| `xrpl-http` | Bounded `XrplClient` with explicit expected network and `XrplHttpConfig` |
| `coingecko-http` | Bounded `CoinGeckoClient` with explicit `CoinGeckoHttpConfig`, optional caller-supplied credential tier and item bounds |
| `defillama-http` | Bounded `DefiLlamaClient` with independent source configurations and explicit item bounds |
| `mempool-space-http` | Bounded `MempoolSpaceClient` with explicit genesis/API configuration; composes the compatible Bitcoin Esplora backend |
| `thorchain-http` | Bounded `ThorchainClient` with explicit `ThorchainHttpConfig`, expected Cosmos chain ID and separate account prefix |
| `all` | All catalogue features and currently implemented backends |

Concrete HTTP backends use Reqwest/rustls and a caller-owned Tokio runtime with I/O/time drivers. The library loads no environment configuration. The [Rust example and live-test guide](examples/README.md) document explicit inputs, backend selection and read-specific qualification.

Private implementation roles remain separate:

| Location | Responsibility |
| --- | --- |
| `src/domain/` | Exact values, family identities and validated records |
| Chain/provider capability modules | Runtime-independent typed operation contracts |
| Backend modules, such as `src/chains/solana/http.rs` | Network calls, operation stages and source attribution |
| Integration `wire.rs` modules | Typed remote request/result fields and integration-specific validation |
| `src/transport/http.rs`, `src/transport/budget.rs`, `src/transport/rpc.rs` | HTTP sending/retries/body bounds; shared operation deadlines; pure JSON-RPC encoding/decoding |

## Development

Use [rustup](https://rustup.rs/) and [just](https://just.systems/), then run:

```sh
rustup show
just tools
```

| Command | Verifies |
| --- | --- |
| `just test` | Meaningful domain/capability and deterministic HTTP/RPC fixture behavior, including the executable example |
| `just doctest` | Compiled public Rust examples |
| `just gate` | Formatting, strict Clippy, tests, documentation, unused dependencies and dependency policy |

Ordinary tests require no external provider credentials. Live read qualification is explicit and opt-in. Nextest retains `--no-tests fail`. `just sbom` generates an all-feature CycloneDX bill of materials in `sbom/`. GitHub workflows run only when manually dispatched.

## License and attribution

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution and [AUTHORS.md](AUTHORS.md) for authorship. Citation metadata is available in [CITATION.cff](CITATION.cff).
