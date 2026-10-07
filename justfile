# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Regit

deny_version := "0.20.2"
shear_version := "1.13.4"
taplo_version := "0.10.0"
cyclonedx_version := "0.5.9"
nextest_version := "0.9.144"
wasm_bindgen_version := "0.2.129"
wasm_pure_features := "evm,solana,cardano,bitcoin,litecoin,dogecoin,bitcoin-cash,xrpl,ton,thorchain,jupiter,uniswap,oneinch,lifi,rubic,coingecko,defillama,helius,blockfrost,mempool-space"

default:
    @just --list

tools:
    cargo install cargo-deny --locked --version {{deny_version}}
    cargo install cargo-shear --locked --version {{shear_version}}
    cargo install taplo-cli --locked --version {{taplo_version}}
    cargo install cargo-cyclonedx --locked --version {{cyclonedx_version}}
    cargo install cargo-nextest --locked --version {{nextest_version}}

fmt:
    cargo fmt --all
    taplo fmt

fmt-check:
    cargo fmt --all --check

toml-fmt-check:
    taplo fmt --check --diff

lint:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

test:
    cargo nextest run --workspace --all-features --locked --no-tests fail

doctest:
    cargo test --workspace --all-features --locked --doc

docs:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked

shear:
    cargo shear

deny:
    cargo deny --workspace --all-features --locked check

gate: fmt-check toml-fmt-check lint test doctest docs shear deny

wasm-tools:
    rustup target add wasm32-unknown-unknown
    rustup component add llvm-tools
    cargo install wasm-bindgen-cli --locked --version {{wasm_bindgen_version}}

wasm-lint:
    cargo clippy --workspace --lib --tests --all-features --locked --target wasm32-unknown-unknown -- -D warnings

wasm-test-node:
    AR_wasm32_unknown_unknown="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-ar" CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER:-wasm-bindgen-test-runner}" cargo test --workspace --tests --no-default-features --features {{wasm_pure_features}} --locked --target wasm32-unknown-unknown

wasm-test-browser:
    WASM_BINDGEN_USE_BROWSER=1 CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER:-wasm-bindgen-test-runner}" cargo test --workspace --test wasm_http --no-default-features --features evm-http,coingecko-http --locked --target wasm32-unknown-unknown

wasm-gate: wasm-lint wasm-test-node wasm-test-browser

sbom:
    mkdir -p sbom
    cargo cyclonedx --all --all-features --format json --spec-version 1.5 --license-strict
    mv regit-web3.cdx.json sbom/regit-web3.cdx.json
