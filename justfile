# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Regit

deny_version := "0.20.2"
shear_version := "1.13.4"
taplo_version := "0.10.0"
cyclonedx_version := "0.5.9"
nextest_version := "0.9.144"

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

sbom:
    mkdir -p sbom
    cargo cyclonedx --all --all-features --format json --spec-version 1.5 --license-strict
    mv regit-web3.cdx.json sbom/regit-web3.cdx.json
