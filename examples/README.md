# EVM native-balance example

The [Rust example](native_balance.rs) constructs typed configuration and reads through the public API. Set these example-owned environment inputs before running it:

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
export REGIT_WEB3_RPC_URL='https://ethereum-sepolia-rpc.publicnode.com'
export REGIT_WEB3_CHAIN_ID='11155111'
export REGIT_WEB3_NETWORK_ALIAS='sepolia'
export REGIT_WEB3_ADDRESS='0x0000000000000000000000000000000000000000'
export REGIT_WEB3_NATIVE_DECIMALS='18'
export REGIT_WEB3_NATIVE_SYMBOL='ETH'
export REGIT_WEB3_PROVIDER_ID='publicnode-sepolia'
export REGIT_WEB3_BLOCK_SELECTOR='finalized'
cargo run --locked --example native_balance --features evm-http
```

This setup reads [Sepolia](https://ethereum.org/en/developers/docs/networks/#sepolia) through [PublicNode's published Ethereum endpoint](https://ethereum.publicnode.com/). Both the example and the opt-in public API test completed a finalized read on chain `11155111`, source `publicnode-sepolia`:

| Read | Retrieval time (UTC) | Block | Hash |
| --- | --- | --- | --- |
| Rust example | 2026-10-06 23:48:31 | `11859104` | `0x96b8afc93add4ed3384e308547815f94af90a61c7dba6b55992ffadc08ece77a` |
| Opt-in public API test | 2026-10-06 23:51:31 | `11859136` | `0xdb58040708c22f79e11f1e7d86f2043f352b1c1414238b7d7e5f1217a3e0b6f9` |

This verifies those recorded recent reads; archive support and other selectors have not been qualified. Both observations retained finality `unknown` and confirmations `null`.

Success writes one observation JSON line to stdout. Failure writes a fixed typed error to stderr and exits with status 1. Endpoint credentials and provider diagnostic text are excluded. The example sets a 5-second connection timeout, a 15-second total operation budget, a 1 MiB response limit and two additional attempts per RPC stage. Environment parsing belongs to the example; the library accepts typed configuration.

## Opt-in live qualification

Ordinary tests use deterministic fixtures. With the explicit inputs above, run the ignored public API test separately:

```sh
cargo test --locked --test live --features evm-http -- --ignored --nocapture
```

Missing inputs fail when this test is explicitly selected. The recorded reads are historical, EVM-specific point-in-time evidence; they do not qualify other integrations or establish lasting finality.
