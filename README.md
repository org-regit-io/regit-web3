# Regit Web3

An open-source Rust library for reusable Web3 primitives and typed operations, developed by [Regit](https://www.regit.io).

The full library scope comprises ten chain families, five protocols and five data providers. Local exact values, family-specific identities, encoding and transaction preparation are separated from typed operation contracts and replaceable RPC/provider backends. Concrete backends are selected through Cargo features.

## Catalogue and implementation status

All modules below are part of the required library scope. Current implementation is partial; pending operations remain required.

| Chain family | Scope | Current implementation |
| --- | --- | --- |
| EVM | Native/ERC-20 balances, transactions/receipts, gas/fees, transfers and approvals | Hash-pinned native-balance reader and optional HTTP backend implemented; wider operations pending |
| Solana | SOL/SPL balances, accounts, transactions and transfers | Pure identities, account/balance/observation types and native/token/account reader contracts implemented; RPC backend and wider operations pending |
| Cardano | Balances, UTxOs, assets, transactions, epoch/staking and preparation | Pending |
| Bitcoin | Validation, balance/history, fees and transaction status | Pending |
| Litecoin | Validation, balance/history and fees | Pending |
| Dogecoin | Validation, balance/history and fees | Pending |
| Bitcoin Cash | Validation, balance/history and fees | Pending |
| XRPL | XRP/issued balances, history, trustlines and transfers | Pending |
| TON | Validation, balance/history, network data and transfers | Pending |
| THORChain | RUNE balances, pools, network data, quotes and cross-chain state | Pending |

| Protocol | Scope | Current implementation |
| --- | --- | --- |
| Jupiter | Solana aggregation quotes/routes and swap preparation | Pending |
| Uniswap | Direct EVM DEX quotes and swap preparation | Pending |
| 1inch | Aggregated EVM quotes/routes and swap preparation | Pending |
| LI.FI | Cross-chain quotes/routes, execution preparation and status | Pending |
| Rubic | Cross-chain quotes/routes and supported execution preparation | Pending |

| Provider | Scope | Current implementation |
| --- | --- | --- |
| CoinGecko | Asset search, prices, markets and historical market data | Pending |
| DefiLlama | Protocol TVL, yields, stablecoins and DeFi analytics | Pending |
| Helius | Solana assets, parsed transactions and address history | Pending |
| Blockfrost | Indexed Cardano network, asset and supporting account data | Pending |
| mempool.space | Bitcoin mempool, fees and transaction data | Pending |

Wallet preparation, review and external signing-handoff contracts remain pending. Concrete signing/custody/connector backends are modular extensions; signing keys and approval policy are caller-owned. Preparation and submission are separate operations.

## Implemented contracts

| Area | Contract |
| --- | --- |
| Exact values | `Amount` retains unsigned 256-bit base units and explicit optional decimals; `ExactDecimal` retains bounded signed decimals. Exact numeric values serialize as strings; no floating-point transaction amounts |
| Validation | Constructors and deserialization enforce matching identity, precision, schema and observation invariants. Typed errors use fixed diagnostics |
| EVM identity/context | EVM addresses/hashes use canonical lowercase hexadecimal; network identity uses the exact chain ID; native observations retain block number/hash/time, retrieval time and source |
| Solana identity/context | Addresses, hashes and signatures use canonical base58; network identity uses the full genesis hash. Typed results retain commitment/minimum context slot separately from the actual slot, without inventing block hashes/timestamps or independently verified finality |
| EVM HTTP native read | Verify `eth_chainId` at establishment and before each read; resolve the selector; use EIP-1898 `blockHash` with `requireCanonical: true`; retries retain the captured hash/address without a new head or height fallback |
| Optional transport | Explicit configuration, verified TLS, bounded bodies/retries and one total operation deadline; endpoint credentials and raw provider messages excluded from diagnostics |

EVM HTTP native reads retain configured precision, finality `unknown` and confirmations `null`; requested tags do not establish either. Each family retains its own ledger and source semantics. Pure observation construction validates supplied records and does not independently verify a remote source.

## Features

Default features are empty. Pure capabilities use standard Rust futures and do not select an HTTP client or asynchronous runtime. Concrete implementations enable their own dependencies explicitly. Feature names alone do not imply implemented operations.

| Feature | API / composition |
| --- | --- |
| No features | Shared exact values, validated EVM domain records and typed errors; no networking dependencies |
| `evm` | Runtime-independent `NativeBalanceReader` capability |
| `solana` | Pure family types and native/token/account reader capabilities |
| `http` | Shared `HttpConfig`, `RpcEndpoint` and `RpcLimits` configuration |
| `evm-http` | Bounded `EvmClient` implementing the EVM reader; Reqwest/rustls and caller-owned Tokio runtime |
| `all` | All catalogue features and currently implemented backends |

The [EVM Rust example and recorded read qualification](examples/README.md) document explicit inputs and the `evm-http` feature. Its point-in-time evidence applies only to the recorded provider/network/operation.

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
