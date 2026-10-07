# Development and verification

[Overview](../README.md) · [Catalogue](catalogue.md) · [Contracts](contracts.md) · [Features](features.md) · [Qualification](qualification.md) · [Development](development.md)

Use [rustup](https://rustup.rs/) and [just](https://just.systems/), then run:

```sh
rustup show
just tools
```

| Command | Verifies |
| --- | --- |
| `just test` | Meaningful domain/capability and deterministic HTTP/RPC fixture behavior, including the executable example |
| `just doctest` | Compiled public Rust examples |
| `just gate` | Native formatting, strict Clippy, tests, documentation, unused dependencies and dependency policy |
| `just wasm-tools` | Install the WASM target, LLVM tools and pinned matching wasm-bindgen runner |
| `just wasm-test-node` | Execute the existing pure catalogue/value/encoding/preparation assertions as WASM in Node |
| `just wasm-test-browser` | Execute browser HTTP/RPC fixture behavior in isolated headless Chrome |
| `just wasm-gate` | Strict WASM library/test lint plus actual Node and browser execution |

WASM tests require Node 19+ and Chrome with a compatible ChromeDriver on `PATH` or selected by `CHROMEDRIVER`. The runner can be selected with `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER`; its version must match the locked wasm-bindgen crate. The Node recipe selects the toolchain's `llvm-ar` dynamically: native macOS archive tools cannot index WASM C objects used by Bitcoin. Browser fixtures replace Fetch with deterministic responses while exercising real browser requests, streams, timers and abort signals; they use fake credentials and make no provider calls.

Ordinary tests require no external provider credentials. Live read qualification is explicit and opt-in. Nextest retains `--no-tests fail`. `just sbom` generates an all-feature CycloneDX bill of materials in `sbom/`. GitHub workflows run only when manually dispatched.

## API documentation and package review

```sh
cargo doc --locked --all-features --no-deps --open
cargo package --locked --list
```

The package includes the public guides, examples, fixtures and attribution. Local project notes and credential files are excluded. Package review does not publish the crate. See the [Cargo manifest reference](https://doc.rust-lang.org/cargo/reference/manifest.html#the-readme-field) and [package reference](https://doc.rust-lang.org/cargo/commands/cargo-package.html).

Live read harnesses and their explicit inputs are documented in [examples/README.md](../examples/README.md). See [qualification](qualification.md) for actual proof and pending checks.

Contribution and pull-request guidance is in [CONTRIBUTING.md](../CONTRIBUTING.md).
