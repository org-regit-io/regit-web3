Describe the concrete behavior or tooling change and its motivation.

- [ ] Run `just gate`; include commands/results and explain any failed or unexecuted check.
- [ ] Run relevant WASM checks when portable capabilities, encodings or browser backends change.
- [ ] Keep new integration code in its own chain, protocol or provider modules.
- [ ] Keep the full catalogue scope intact; distinguish implemented operations from pending work.
- [ ] Keep local primitives and typed capabilities separate from replaceable RPC/provider backends.
- [ ] Keep signing backend selection, secret provisioning and approval policy caller-owned.
- [ ] Add meaningful fixtures and match examples/documentation to actual behavior and qualification limits; include no secrets or provider credentials.

See [CONTRIBUTING.md](https://github.com/org-regit-io/regit-web3/blob/main/CONTRIBUTING.md) for setup and review guidance.
