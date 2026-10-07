# Contributing

Thanks for helping improve Regit Web3. Focused fixes, tests and documentation are welcome.

## Start a change

1. Check existing issues. Use the [bug report](https://github.com/org-regit-io/regit-web3/issues/new?template=bug_report.yml) or [feature request](https://github.com/org-regit-io/regit-web3/issues/new?template=feature_request.yml) form when useful.
2. Fork the repository and create a branch for one coherent change.
3. Keep changes focused on reusable Web3 library APIs. Document additions or changes to supported profiles in the [catalogue](guides/catalogue.md) and [contracts](guides/contracts.md).
4. Add meaningful fixtures and update affected examples or public documentation.
5. Open a pull request describing the behavior, compatibility impact and check results.

## Preserve the library contracts

- Keep exact values, family identities and typed capabilities independent of transport/runtime dependencies. Default features remain empty; backends are selected explicitly.
- Retain family-specific network and observation semantics, including EVM canonical hash reads and frozen-hash retries.
- Keep preparation, signed-content verification and explicit submission separate. Callers own credentials, approval and replay policy.
- Preserve strict Rust/Clippy lints, public API documentation and the prohibition on unsafe code. Check current primary documentation before changing a dependency or provider integration.
- Test actual behavior, malformed inputs, identity mismatches, bounds and relevant retry/cancellation cases. Record qualification limits accurately; fixtures do not replace live proof.

## Check your change

Rust 1.98 or newer is required; the repository pins its development toolchain. Install [rustup](https://rustup.rs/) and [just](https://just.systems/), then run:

```sh
just tools
just gate
```

Run `just wasm-gate` when changing portable capabilities, encodings or browser backends. The [development guide](guides/development.md) documents Node, Chrome/ChromeDriver, LLVM tools and matching WASM runner requirements. Include the commands and results in the pull request; explain any failed or unexecuted check.

Ordinary tests use fixtures and need no provider credentials. Live tests are explicitly opt-in and require their documented inputs. GitHub workflows run only by manual dispatch; a pull request does not trigger automatic checks or publication.

## Reports and review

Include the crate version or Git revision, Rust version, target, enabled features and a minimal reproducer. Describe expected and actual behavior. Remove secrets, private keys, provider credentials and sensitive transaction data from code, URLs and logs.

For new capabilities, describe the typed operation, feature boundary, supported profile and qualification plan. Keep pull requests focused so the behavior and its evidence are easy to review.
