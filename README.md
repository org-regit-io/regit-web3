# Regit Web3

An open-source Rust library for Web3 primitives, developed by [Regit](https://www.regit.io).

## Status

The crate is an unreleased library scaffold. Its public modules are placeholders; no chain operations, protocol integrations, data providers, or wallet connectors are implemented.

Chain, protocol, and provider modules are feature-gated. The default feature is `evm`; enabling features currently adds no network behavior.

## Development

Use [rustup](https://rustup.rs/) and [just](https://just.systems/), then run:

```sh
rustup show
just tools
just gate
```

The gate checks formatting, Clippy, tests, doctests, documentation, unused dependencies, and dependency policy. Nextest uses `--no-tests fail`; the full gate currently fails because the scaffold contains no behavior tests. There are no doctests yet.

`just sbom` generates a CycloneDX bill of materials in `sbom/`. GitHub workflows run only when manually dispatched.

## License and attribution

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution and [AUTHORS.md](AUTHORS.md) for authorship.

Citation metadata is available in [CITATION.cff](CITATION.cff).
