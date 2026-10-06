# Regit Web3

An open-source Rust library for reusable Web3 primitives, developed by [Regit](https://www.regit.io).

The library is organized around chain primitives and integrations, protocols, data providers, and wallet contracts. EVM is the first implemented chain path.

| Module | Scope | Current status |
| --- | --- | --- |
| `domain` | Exact values, identities, assets and observations | Exact amounts and EVM native-balance records implemented |
| `config` | Caller-supplied network and transport configuration | Validated typed EVM configuration implemented |
| `chains` | Chain-specific operations | EVM client and hash-pinned native balance reads implemented; other chains scaffolded |
| `protocols` | Protocol integrations | Scaffold |
| `providers` | Data-provider integrations | Scaffold |
| `wallets` | Wallet contracts and connectors | Scaffold |

## Status

The library implements exact unsigned 256-bit amounts, validated EVM identities, native asset metadata, block selectors, and native-balance observation records. Observations retain explicit source labels, separate block and retrieval timestamps, finality context, and schema version. Typed errors use fixed diagnostics.

Constructors and deserialization enforce the same domain invariants. Amounts and chain identifiers serialize as decimal strings; addresses and hashes use canonical lowercase hexadecimal.

The EVM client verifies `eth_chainId` at establishment and before each native balance read. `get_native_balance` resolves the requested block, pins the balance call to its hash with EIP-1898 `requireCanonical: true`, and returns an observation. RPC requests enforce a total deadline, response-size bounds and limited retries.

Native balances retain configured decimal precision and explicit source/block context. Reads leave finality `unknown` and confirmations `null`; requested block tags do not infer either.

The default feature is `evm`. Feature names identify integration boundaries; they do not imply implemented integration support.

## Native balance example

The [Rust example](examples/native_balance.rs) constructs typed configuration and reads through the public API. Set these example-owned environment inputs before running it:

| Variable | Value |
| --- | --- |
| `REGIT_WEB3_RPC_URL` | HTTP(S) endpoint supporting chain/block lookup and EIP-1898 canonical hash reads |
| `REGIT_WEB3_CHAIN_ID` | Expected chain ID as a canonical decimal integer |
| `REGIT_WEB3_NETWORK_ALIAS` | Non-secret network label |
| `REGIT_WEB3_ADDRESS` | `0x`-prefixed EVM address |
| `REGIT_WEB3_NATIVE_DECIMALS` | Explicit native precision, `0`–`255` |
| `REGIT_WEB3_NATIVE_SYMBOL` | Optional display symbol; omit when unavailable |
| `REGIT_WEB3_PROVIDER_ID` | Non-secret source label |
| `REGIT_WEB3_BLOCK_SELECTOR` | `latest`, `safe`, `finalized`, `number:<decimal height>` or `hash:<0x-prefixed hash>` |

```sh
cargo run --locked --example native_balance --features evm
```

Success writes one observation JSON line to stdout. Failure writes a fixed typed error to stderr and exits with status 1. Endpoint credentials and provider diagnostic text are excluded. The example sets a 5-second connection timeout, a 15-second total operation budget, a 1 MiB response limit and two additional attempts per RPC stage. Environment parsing belongs to the example; the library accepts typed configuration.

## Development

Use [rustup](https://rustup.rs/) and [just](https://just.systems/), then run:

```sh
rustup show
just tools
```

| Command | Verifies |
| --- | --- |
| `just test` | Deterministic domain, configuration and loopback RPC behavior, including the executable example |
| `just doctest` | Compiled public Rust examples |
| `just gate` | Required precommit checks: formatting, strict Clippy, tests, documentation, unused dependencies and dependency policy |

Default tests run without external providers or credentials. Live provider qualification has not been performed. Nextest retains `--no-tests fail`.

`just sbom` generates a CycloneDX bill of materials in `sbom/`. GitHub workflows run only when manually dispatched.

## License and attribution

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution and [AUTHORS.md](AUTHORS.md) for authorship.

Citation metadata is available in [CITATION.cff](CITATION.cff).
