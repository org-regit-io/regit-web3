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

## Development

Use [rustup](https://rustup.rs/) and [just](https://just.systems/), then run:

```sh
rustup show
just tools
```

| Command | Verifies |
| --- | --- |
| `just test` | Deterministic domain, configuration and loopback RPC behavior |
| `just doctest` | Compiled public Rust examples |
| `just gate` | Required precommit checks: formatting, strict Clippy, tests, documentation, unused dependencies and dependency policy |

Default tests run without external providers or credentials. Live provider qualification has not been performed. Nextest retains `--no-tests fail`.

`just sbom` generates a CycloneDX bill of materials in `sbom/`. GitHub workflows run only when manually dispatched.

## License and attribution

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution and [AUTHORS.md](AUTHORS.md) for authorship.

Citation metadata is available in [CITATION.cff](CITATION.cff).
