# Regit Web3

An open-source Rust library for Web3 primitives, developed by [Regit](https://www.regit.io).

## Status

The library implements exact unsigned 256-bit amounts, validated EVM identities, native asset metadata, block selectors, and native-balance observation records. Observations retain explicit source labels, separate block and retrieval timestamps, finality context, and schema version. Typed errors use fixed diagnostics.

Constructors and deserialization enforce the same domain invariants. Amounts and chain identifiers serialize as decimal strings; addresses and hashes use canonical lowercase hexadecimal.

Chain, protocol, provider, configuration, and wallet integration modules remain scaffolds. The default feature is `evm`; enabling integration features currently adds no network behavior.

## Development

Use [rustup](https://rustup.rs/) and [just](https://just.systems/), then run:

```sh
rustup show
just tools
just gate
```

The gate checks formatting, strict Clippy, behavior tests, doctests, documentation, unused dependencies, and dependency policy. Nextest retains `--no-tests fail`.

`just sbom` generates a CycloneDX bill of materials in `sbom/`. GitHub workflows run only when manually dispatched.

## License and attribution

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution and [AUTHORS.md](AUTHORS.md) for authorship.

Citation metadata is available in [CITATION.cff](CITATION.cff).
