# Regit Web3

An open-source Rust library for reusable Web3 primitives and typed operations, developed by [Regit](https://www.regit.io).

The full library scope comprises ten chain families, five protocols and five data providers. Local exact values, family-specific identities, encoding and transaction preparation are separated from typed operation contracts and replaceable RPC/provider backends. Concrete backends are selected through Cargo features.

## Catalogue and implementation status

All modules below are part of the required library scope. Current implementation is partial; pending operations remain required.

| Chain family | Scope | Current implementation |
| --- | --- | --- |
| EVM | Native/ERC-20 balances, transactions/receipts, gas/fees, transfers and approvals | Native/ERC-20 reads, transaction/receipt/status, independent fees, canonical nonce/call/gas estimates, unsigned native/ERC-20 transfer/approval preparation and explicit one-shot signed submission implemented |
| Solana | SOL/SPL balances, accounts, transactions and transfers | SOL/account/SPL reads, canonical legacy/v0/v1 transactions and status, blockhash/height, exact message fees, unsigned simulation, ordinary native/classic SPL preparation and separate one-shot signed submission implemented |
| Cardano | Balances, UTxOs, assets, transactions, epoch/staking and preparation | Exact indexed reads, original-body-hash transaction/status records, ordinary Conway ADA/native-asset preparation, fresh-parameter fee estimation and separate one-shot signed submission implemented |
| Bitcoin | Validation, balance/history, fees and transactions/status | Pure family contracts and optional Esplora balances, bounded history, exact fee estimates, status and canonical full transaction retrieval implemented |
| Litecoin | Validation, balance/history, fees and supporting transaction data | Pure family identities and five optional BlockCypher mainnet reads implemented |
| Dogecoin | Validation, balance/history, fees and supporting transaction data | Pure family identities and five optional BlockCypher mainnet reads implemented |
| Bitcoin Cash | Validation, balance/history, fees and supporting transaction data | CashAddr/legacy validation, genesis/fork-qualified identities and six optional certificate-verified Electrum-Cash TLS reads implemented |
| XRPL | XRP/issued balances, history, trustlines and transfers | Address/amount identities, XRP balances, ledger-bound trustline pages, fees, bounded account history, binary transactions/status, ordinary sequence-based XRP/issued Payment JSON preparation and explicit one-shot signed submission implemented |
| TON | Validation, balance/history, network data and transfers | Standard address/zero-state/BOC validation, seven TON Center read/estimate methods, unsigned internal-message preparation and separate one-shot external-message submission implemented |
| THORChain | RUNE balances, pools, network data, quotes and cross-chain state | RUNE balances, individual/complete layer-one pool reads, network values, swap quotes, inbound vaults, chain heights and transaction progress implemented |

| Protocol | Scope | Current implementation |
| --- | --- | --- |
| Jupiter | Solana aggregation quotes/routes and swap preparation | Swap API V2 quote-only source selection, fresh Metis V0 instruction builds, immutable unsigned preparation/handoff and exact-message Solana fee/simulation implemented |
| Uniswap | Direct EVM DEX quotes and swap preparation | V3 QuoterV2 exact-input quotes, bounded supplied-path comparison and unsigned Universal Router 2.1.2 swap preparation implemented |
| 1inch | Aggregated EVM quotes/routes and swap preparation | Pending |
| LI.FI | Cross-chain quotes/routes, execution preparation and status | Exact-input quotes, bounded route alternatives, immutable selected-step preparation, and transaction/provider-transfer status implemented |
| Rubic | Cross-chain quotes/routes, unsigned preparation and progress | API-v2 chain catalogue, direct all/best quotes, fresh unsigned EVM/Solana preparation/handoff and extended status implemented |

| Provider | Scope | Current implementation |
| --- | --- | --- |
| CoinGecko | Asset search, prices, markets and historical market data | Search, exact ID/currency prices, explicit markets pages and historical price/capitalization/volume charts implemented |
| DefiLlama | Protocol TVL, yields, stablecoins and DeFi analytics | Protocol/chain TVL and history, yields/history, stablecoins/history and DEX/fees/revenue/holders-revenue analytics implemented |
| Helius | Solana assets, parsed transactions and address history | Four DAS/current Parsed Events reads with exact source records, bounded pages and immutable query continuations implemented; authenticated live qualification pending |
| Blockfrost | Indexed Cardano network, asset and supporting account data | Seventeen read/estimate methods covering balances, UTxOs, address/asset/reward pages, metadata, network/epoch/parameters/staking and transactions; separate raw-CBOR submission implemented |
| mempool.space | Bitcoin mempool, fees and transaction data | Backlog summaries, recent arrivals, bounded full transaction-ID lists, exact recommended fees, canonical transaction retrieval and inclusion status implemented |

Generic typed wallet preparation, review and external signing-handoff contracts are implemented, with EVM canonical transaction, Solana canonical message, Jupiter compiled V0, XRPL Payment JSON, Uniswap router-field, Cardano ordinary-payment, TON internal-message and Rubic direct-swap preparations. Returned signed content requires a caller-supplied verifier against the original preparation; matching echoed metadata alone does not verify it. Concrete cryptographic verification, signing, custody and connectors remain separate extensions. Callers own approval and replay policy; preparation and handoff do not submit.

## Implemented contracts

| Area | Contract |
| --- | --- |
| Exact values | `Amount` retains unsigned 256-bit base units and explicit optional decimals; `ExactDecimal` retains bounded signed decimals. Exact numeric values serialize as strings; no floating-point transaction amounts |
| Validation | Constructors and deserialization enforce matching identity, precision, schema and observation invariants. Typed errors use fixed diagnostics |
| Wallet handoff | Immutable family-specific network/intent/unsigned snapshots, bounded caller-generated IDs and read-only review. Correlation is checked before trusted signed-content verification; confirmed output has no unchecked constructor or deserialization path. Custom snapshots and verifiers must honor their documented contracts |
| EVM identity/context | Canonical hexadecimal identities and exact chain IDs; native/ERC-20 state retains the captured canonical block hash. Transactions/receipts retain actual nullable inclusion, top-level execution outcomes and retrieval/source facts; JSON identity/signatures remain source-reported |
| Litecoin/Dogecoin identity/context | Family-qualified addresses and full main/test/regression genesis identities, exact litoshis/koinu and signed mempool deltas. Indexed history retains complete boundary blocks and explicit continuation uncertainty; transaction bytes remain opaque source data |
| Bitcoin Cash identity/context | Explicit address namespace, full genesis and fork checkpoint; exact satoshis, signed unconfirmed delta and CashToken quantities/NFT metadata. Source height zero/null remain distinct; opaque raw bytes have a computed ID, with verbose facts retained separately |
| Solana identity/context | Canonical base58 identities and full genesis network identity; maintained legacy/v0/v1 byte codecs. RPC controls, evaluation slots, inclusion slots and last-valid block heights remain distinct; structural validation does not verify signatures or finality |
| Bitcoin identity/context | Network-qualified addresses and full standard genesis hashes; exact confirmed satoshis and a separate signed mempool delta; bounded history retains its cursor and completeness limits. Canonical transaction bytes retain computed txid/wtxid, exact inputs/outputs/scripts/witness and weight; indexed previous outputs, nullable fee and inclusion remain separate source facts |
| Cardano identity/context | Payment/stake addresses, validated Byron CRC and nested encoding, explicit network tag/magic, and policy/raw-name asset identities; exact ADA and native-asset quantities retain declared precision. UTxO pages retain page/count/order and output origin separately from evaluation context |
| XRPL identity/context | Classic/X-address validation, network/tag/currency identities, exact XRP drops and issued decimals; actual ledger attribution, signed trustline balances and source-reported fee suggestions. Binary transaction IDs are recomputed; indexed inclusion and JSON execution outcomes remain separate |
| TON identity/context | CRC-checked friendly/raw addresses, full zero-state identity, exact nanotons and extra-currency IDs. Bounded maintained BOC hashes/transaction decoding, linked account history and actual masterchain evaluation; decoded transaction fees and reported aggregates remain separate |
| THORChain identity/context | Exact Cosmos chain ID and separate account prefix; full chain/symbol and distinct layer-one/synthetic/trade/secured representations. Protocol quantities use 1e8 units; native dust and gas retain their own units. Source stages and per-chain heights do not independently prove external inclusion |
| THORChain quotes | Requested input identity/amount remain supplied query facts. The source can expand asset identifiers and does not report its resolved input; the HTTP backend retains typed unreported resolution. Full output fee identity, fee totals, expiry and optional settings are checked |
| Market data | Provider listing IDs and explicit currencies/units; exact nullable prices, TVL/volume/fee/revenue values, signed APY percentages and supplied series timestamps; source and retrieval facts without invented ledger anchors |
| Bitcoin mempool data | Exact satoshi totals and sat/vB suggestions; individual histogram bins, at most ten recent arrivals and explicitly bounded full ID lists. Fractional weight/4 sizes retain quarter-byte increments; separate reads do not establish an atomic snapshot |
| EVM HTTP state reads | Verify `eth_chainId` at establishment and before each read; resolve once; use EIP-1898 `blockHash` with `requireCanonical: true`; retries retain exact address/calldata/hash. ERC-20 name/symbol/decimals remain individually optional, without an assumed precision |
| EVM execution and preparation | Explicit sender/nonce/gas/value/fees/access lists; canonical nonce/call reads and Geth-compatible hash gas-estimation extension with no height fallback. Fees are separately sourced. Canonical legacy/type1/type2 unsigned signing bytes and digest preserve exact intent; no signer or inferred defaults |
| EVM signed submission | Canonical replay-protected legacy/type1/type2 envelope and computed hash; structural checks do not verify signatures, recovered sender or intent. One explicit write after chain preflight, no retry; matching node hash is acknowledgment only, unresolved post-dispatch outcomes remain potentially submitted |
| EVM HTTP transaction reads | Typed legacy/type1–4 source fields and bounded receipts/logs; explicit absence, pending/included state and failed/succeeded/unknown execution stay distinct. Sequential status reads validate matching facts without claiming an atomic snapshot |
| Litecoin/Dogecoin HTTP reads | Explicit documented mainnet BlockCypher bases; verify full genesis/chain name at establishment and before every operation. Complete bounded indexed transaction arrays, source-height history cursors and fee preferences per 1000 serialized bytes; no computed raw identity, consensus/signature proof or vbyte guarantee |
| Bitcoin Cash Electrum reads | Certified TLS with caller-supplied DER roots and explicit host/server name; protocol 1.6, token support, full genesis and fork headers checked before every read. Balance/filter, complete height-interval history, exact BCH/1000-byte fees, transaction/status and bounded UTXOs; one total budget without HTTP selection |
| Solana HTTP operations | Verify the full genesis hash at establishment and before each read; return exact SOL lamports, decoded present/absent accounts, or SPL raw units with mint/program/owner/state and reported decimals. Preserve requested commitment/minimum slot separately from the actual slot; ignore scaled UI amounts. Transaction/status, hash/height, fee and simulation use method-specific controls and one deadline; one-shot submission sets node retries to zero and preserves ambiguous post-dispatch outcomes |
| Bitcoin Esplora reads | Verify full genesis at establishment and before each read; retain exact balances, bounded history, fee estimates and status. Full retrieval compares canonical raw bytes with indexed immutable fields under one deadline; structural/money checks do not execute scripts, validate signatures or prove inclusion |
| Cardano Blockfrost reads | Verify expected genesis network magic before every operation; retain exact indexed balances, bounded address/asset/reward/UTxO pages, network/epoch/staking facts and original CBOR transaction/status data. Original body hashes, declared fees, failed-script collateral and source paid fees remain distinct; missing data is not a zero balance |
| Cardano preparation and submission | Pure Conway key-spend ADA/native-asset payments retain selected inputs, every output/change, fee, absolute validity slots and key-witness count. Exact serialized-size fee/minimum-ADA checks use explicit protocol parameters. Separate externally signed, body/key/size-bound raw CBOR is dispatched once; structural checks do not verify signatures and acknowledgment does not establish execution |
| XRPL HTTP reads | Verify the expected network; resolve one validated ledger hash for account/trustline reads. History uses an explicit bounded ledger range; transaction bytes retain their computed identity and source metadata. Fees retain open-ledger context; pending/range records do not invent a ledger hash |
| XRPL signed submission | Explicit caller-supplied bytes and network; one write attempt after network preflight. Echoed bytes and computed identity are checked; preliminary handling never establishes validated execution. Unresolved failures after dispatch retain a potentially submitted outcome |
| TON HTTP operations | Full zero-state checked before every operation; account sequence frozen across retries with full block correlation. Explicit bounded history/message scans, fee-body estimates and client-local request spacing share one deadline. One-shot submission acknowledgment is not execution; unresolved post-dispatch failures remain potentially submitted |
| Provider HTTP reads | Explicit CoinGecko anonymous/Demo/Pro configuration; independent DefiLlama TVL/yields/stablecoin endpoints. Exact decimal decoding, explicit collection/page limits, no silent truncation or source fallback |
| Helius HTTP reads | Full Solana genesis verified before every read under one deadline. DAS index progress, parsed inclusion, parser outcomes and execution remain separate source facts. Ordered parse batches retain duplicate inputs/item failures; owner/history continuations preserve every original predicate and reject another client’s handle before dispatch. Source interpretations and cached valuations do not prove canonical transaction bytes or finality |
| mempool.space HTTP reads | Explicit expected full Bitcoin genesis and API base; one total deadline for setup/reads/retries. Canonical transaction retrieval/status reuse compatible Esplora contracts and retain the supplied provider label |
| THORChain HTTP reads | Explicit Cosmos network/account-prefix/API configuration; network verification before each operation under one total deadline. Complete bounded collections fail without truncation; quotes expire without implicit refresh and separate reads have no common snapshot claim |
| Rubic direct routes | Caller-qualified families/aliases, exact gross/source/leg amounts, fees and bounded alternatives. Original selection and caller constraints remain beside fresh estimates, optional token metadata and unsigned EVM/Solana payloads; source-expanded filters stay source facts. Structural encoding does not prove swap intent, source progress does not prove finality, and expiry remains unreported |
| LI.FI source contracts | Explicit chain-family catalogue, exact assets/units/slippage/costs and bounded alternatives. Preparation retains the old selection beside fresh source estimates/payload; encoding checks do not prove signed intent. Status distinguishes chain hashes, provider transfer IDs and refunds; quote expiry remains unreported |
| Jupiter V2 | Quote-only `/order` and fresh `/build` retain independent source-selected routes. Maintained V0 compilation uses exact source instructions/lookup tables and explicit caller CU/signer/other-instruction placement; these facts do not verify swap semantics. Separate fee/simulation observations match the exact unsigned bytes, retain actual slots and share one total deadline |
| Uniswap V3 operations | Explicit deployment/path/input/call settings; QuoterV2 quotes and at most 16 supplied paths retain one canonical hash. Exact raw output, per-hop prices/ticks and source gas estimates; reverted candidates remain distinct. Unsigned Universal Router 2.1.2 preparation retains literal recipient, floor-rounded slippage, explicit deadline and separate ERC-20/Permit2 allowance requirements |
| Optional transport | Explicit configuration, verified TLS, bounded bodies/retries and one total operation deadline; endpoint credentials and raw provider messages excluded from diagnostics |

EVM HTTP native reads retain configured precision, finality `unknown` and confirmations `null`; requested tags do not establish either. Bitcoin and Cardano indexed reads do not promise a hash-selected snapshot or lasting finality. Each family retains its own ledger and source semantics. Pure observation construction validates supplied records and does not independently verify a remote source.

EVM ERC-20 balance/allowance/metadata, transaction/receipt/status, fee/nonce/call/estimate reads, Litecoin/Dogecoin five reads with history continuation, Bitcoin Cash six Electrum-TLS reads, Solana native/account/SPL reads, Bitcoin indexed and full transaction reads, XRPL's six read methods, CoinGecko's four operations, DefiLlama's eight reader methods, mempool.space's six reads and THORChain's eight read/quote methods have representative opt-in live qualification. Wallet extension contracts and EVM/XRPL preparation adapters have deterministic fixture qualification. EVM and XRPL explicit submission paths have deterministic and loopback fixture qualification; no funded live submission was performed. LI.FI’s four methods and both transaction/provider-transfer status selectors have representative EVM live qualification; other family payload encodings have fixture proof. Uniswap’s quote and supplied-path comparison have representative V3 live qualification; unsigned Universal Router 2.1.2 preparation and wallet handoff have fixture proof. TON’s seven read/estimate methods, an outgoing-fee transaction and local unsigned review have representative live qualification. TON submission has fixture proof; no funded write was performed. Solana’s seven added read/fee/simulation operations have representative mainnet qualification, including real v1 retrieval and a successful unsigned native simulation. Legacy/v0 codecs, classic SPL preparation and submission have fixture proof; no signed live submission was performed. Jupiter V2 quote/build, unsigned handoff and exact-message fee/simulation have representative mainnet qualification; the public example taker returned `AccountNotFound`, without swap-success proof. Cardano/Blockfrost operations, payment preparation/handoff and one-shot submission have deterministic and loopback fixture proof. Authenticated Blockfrost live qualification remains pending; no funded live submission was performed. Helius’s four reads and pagination contracts have deterministic and loopback fixture proof; authenticated data qualification remains pending. Rubic’s five direct API-v2 methods and unsigned EVM handoff have representative native Mac live qualification; the supplied unrelated public hash returned `NotFound`, without executed-transfer proof. Solana preparation has fixture proof. These operation-specific observations do not complete the full catalogue or qualify every network, provider plan or query variant.

## Features

Default features are empty. Pure capabilities use standard Rust futures and do not select an HTTP client or asynchronous runtime. Concrete implementations enable their own dependencies explicitly. Feature names alone do not imply implemented operations.

| Feature | API / composition |
| --- | --- |
| No features | Shared exact values, typed errors and generic wallet preparation/review/handoff; no networking dependencies |
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
| `rubic` | Pure cross-family direct-route contracts, exact bounded JSON metadata codec, `RubicReader` and unsigned EVM/Solana preparation/handoff |
| `http` | Shared `HttpConfig` and `RpcEndpoint`; `RpcLimits` is also available with `bitcoin-cash-electrum` |
| `evm-http` | Bounded `EvmClient` implementing read/simulation capabilities and separate explicit signed submission; Reqwest/rustls and caller-owned Tokio runtime |
| `litecoin-http`, `dogecoin-http` | Family-specific `BlockCypherClient` and explicit `BlockCypherConfig`; documented mainnet sources only |
| `bitcoin-cash-electrum` | Certificate-verified Electrum-Cash 1.6 TLS `ElectrumClient`, explicit trust roots/network/source/limits and caller-owned Tokio runtime; no HTTP backend |
| `solana-http` | Bounded `SolanaClient` implementing native/token/account/transaction/status/blockhash/fee/simulation capabilities and separate submission with explicit `SolanaHttpConfig`; Reqwest/rustls and caller-owned Tokio runtime |
| `bitcoin-esplora` | Bounded `EsploraClient` implementing Bitcoin read and transaction capabilities with explicit `EsploraConfig` |
| `blockfrost-http` | Bounded `BlockfrostClient` for all seventeen indexed read/estimate methods and separate one-shot raw-CBOR submission; explicit `BlockfrostHttpConfig`, caller-owned project credentials and Tokio runtime |
| `xrpl-http` | Bounded `XrplClient` with explicit expected network and `XrplHttpConfig` |
| `coingecko-http` | Bounded `CoinGeckoClient` with explicit `CoinGeckoHttpConfig`, optional caller-supplied credential tier and item bounds |
| `defillama-http` | Bounded `DefiLlamaClient` with independent source configurations and explicit item bounds |
| `helius-http` | Bounded `HeliusClient` with explicit full Solana genesis and one RPC/Parsed Events base; caller-owned authentication/runtime and opaque concrete-client-bound continuation handles |
| `mempool-space-http` | Bounded `MempoolSpaceClient` with explicit genesis/API configuration; composes the compatible Bitcoin Esplora backend |
| `ton-http` | `TonClient` with explicit `TonHttpConfig`, zero-state identity, replaceable API-v2 endpoint and optional caller-selected request spacing; caller-owned Tokio runtime |
| `thorchain-http` | Bounded `ThorchainClient` with explicit `ThorchainHttpConfig`, expected Cosmos chain ID and separate account prefix |
| `lifi-http` | Bounded `LifiClient`, explicit chain-family catalogue, optional caller-provided headers and private exact continuation bound to the configured authority |
| `jupiter-http` | `JupiterClient` for explicitly configured V2 quote/build access; composes a caller-selected Solana execution reader for exact-message fees/simulation |
| `uniswap-http` | `UniswapV3Client` composes the EVM backend; exact-input quotes and supplied-route comparison use frozen canonical hashes and one total deadline |
| `rubic-http` | Bounded direct API-v2 `RubicClient`, explicit `RubicHttpConfig`/family catalogue/headers and one total deadline; quote/build requests do not execute swaps |
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
