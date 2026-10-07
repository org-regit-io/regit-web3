# Regit Web3

[![Rust 1.98](https://img.shields.io/badge/Rust-1.98-000000?logo=rust)](https://www.rust-lang.org/)
[![Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue)](https://github.com/org-regit-io/regit-web3/blob/main/LICENSE)
[![20 integrations](https://img.shields.io/badge/Catalogue-20_integrations-2563eb)](https://github.com/org-regit-io/regit-web3/blob/main/guides/catalogue.md)

**Web3 primitives and typed operations for Rust.** Exact values, validated identities, chain reads, protocol quotes, provider data and unsigned transaction preparation across ten chain families, five protocols and five providers.

An open-source library developed by [Regit](https://www.regit.io).

[Primitives](https://github.com/org-regit-io/regit-web3/blob/main/guides/primitives.md) · [Catalogue](https://github.com/org-regit-io/regit-web3/blob/main/guides/catalogue.md) · [Contracts](https://github.com/org-regit-io/regit-web3/blob/main/guides/contracts.md) · [Features](https://github.com/org-regit-io/regit-web3/blob/main/guides/features.md) · [Qualification](https://github.com/org-regit-io/regit-web3/blob/main/guides/qualification.md)

## Design

- Exact integer and decimal values; validation applies equally to constructors and deserialization.
- Checked arithmetic with explicit rounding, provider-scoped asset mappings and bounded collections of typed outcomes.
- Family-specific network and observation contracts retain actual block, slot, ledger and indexed-source semantics.
- Pure typed capabilities, modular preparation/review/handoff and optional replaceable RPC/provider backends.
- Empty default features. Explicit configuration, bounded responses and deadlines, fixed errors and credential-redacted diagnostics.

Preparation and handoff do not sign or submit. Supported submission operations are separate and explicit; signing, custody and connectors remain optional extensions.

## Install

Rust 1.98 or newer is required. Install [regit-web3 1.0.1 from crates.io](https://crates.io/crates/regit-web3/1.0.1):

```toml
[dependencies.regit-web3]
version = "1.0.1"
default-features = false
```

## Core primitives

The core API needs no optional features or runtime:

```rust
use regit_web3::domain::{Amount, ExactDecimal, RoundingMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let amount = Amount::from_decimal("9007199254740993", Some(6))?;
    assert_eq!(amount.formatted().as_deref(), Some("9007199254.740993"));

    let decimal = ExactDecimal::parse("-9007199254740993.12500")?;
    assert_eq!(decimal.canonical(), "-9007199254740993.125");

    let rounded = amount.to_exact_decimal()?.quantize(2, RoundingMode::HalfEven)?;
    assert_eq!(rounded.canonical(), "9007199254.74");
    Ok(())
}
```

Amounts retain unsigned 256-bit base units and explicit optional precision. Signed decimals preserve their exact value; numeric amount and decimal values serialize as strings.

| Primitive | Contract |
| --- | --- |
| Checked arithmetic | Exact addition, subtraction, multiplication and base-unit conversion; explicit precision and rounding |
| Asset-to-market bindings | Caller-supplied full asset keys mapped to exact provider/listing IDs, with attribution and bounded lookup |
| Observation collections | Ordered unique keys, typed successes and failures, and unchanged family observation contexts |

The [primitive guide](https://github.com/org-regit-io/regit-web3/blob/main/guides/primitives.md) provides examples and bounds. Identity validation, source attribution and [operation contracts](https://github.com/org-regit-io/regit-web3/blob/main/guides/contracts.md) remain explicit.

## Catalogue

All twenty modules provide functional operations within their documented profiles.

| Layer | Integrations | Operations |
| --- | --- | --- |
| Chains | EVM · Solana · Cardano · Bitcoin · Litecoin · Dogecoin · Bitcoin Cash · XRPL · TON · THORChain | Family identities, balances, history, transactions, fees and supported preparation/submission contracts |
| Protocols | Jupiter · Uniswap · 1inch · LI.FI · Rubic | Quotes, routes, unsigned preparation and supported progress/status reads |
| Providers | CoinGecko · DefiLlama · Helius · Blockfrost · mempool.space | Exact market, indexed chain and mempool records |

The [catalogue](https://github.com/org-regit-io/regit-web3/blob/main/guides/catalogue.md) lists each integration’s operations. The [contracts](https://github.com/org-regit-io/regit-web3/blob/main/guides/contracts.md) retain its network, source and verification limits.

## Select features

Add the required features to the dependency entry. Pure capability selections do not select HTTP or an asynchronous runtime:

```toml
features = ["solana", "bitcoin", "coingecko"]
```

Optional outgoing backends select their related capabilities and transport dependencies:

```toml
features = ["solana-http", "bitcoin-esplora", "coingecko-http"]
```

Native HTTP backends use a caller-owned Tokio runtime. JavaScript WASM uses host Fetch, streams, abort signals and timers; provider CORS policy applies. Electrum-Cash TCP/TLS remains native. See the [feature and target guide](https://github.com/org-regit-io/regit-web3/blob/main/guides/features.md) for the complete matrix and host requirements.

## Verification

All-feature library/example builds and offline fixtures pass on Linux, macOS and Windows in the [manual platform run](https://github.com/org-regit-io/regit-web3/actions/runs/37621428636) at source commit [`da1828f`](https://github.com/org-regit-io/regit-web3/commit/da1828f). The macOS strict gate and actual JavaScript WASM Node/browser fixture checks also pass. Representative live reads apply to their recorded networks, providers and query variants. Authenticated 1inch data remains pending; the [qualification guide](https://github.com/org-regit-io/regit-web3/blob/main/guides/qualification.md) distinguishes fixtures, runtime checks and live proof.

```sh
just gate
just wasm-gate
```

Ordinary tests require no provider credentials. Tool setup and target prerequisites are in the [development guide](https://github.com/org-regit-io/regit-web3/blob/main/guides/development.md); explicit live-read inputs are in the [Rust example guide](https://github.com/org-regit-io/regit-web3/blob/main/examples/README.md). GitHub workflows run only when manually dispatched.

## Contributing

Focused fixes, fixtures and documentation are welcome. See [CONTRIBUTING.md](https://github.com/org-regit-io/regit-web3/blob/main/CONTRIBUTING.md) for setup, checks and review guidance.

## License

Apache-2.0. See [LICENSE](https://github.com/org-regit-io/regit-web3/blob/main/LICENSE), [NOTICE](https://github.com/org-regit-io/regit-web3/blob/main/NOTICE), [AUTHORS.md](https://github.com/org-regit-io/regit-web3/blob/main/AUTHORS.md) and [CITATION.cff](https://github.com/org-regit-io/regit-web3/blob/main/CITATION.cff).
