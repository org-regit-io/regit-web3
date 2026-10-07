# Rust examples and live read qualification

Ordinary tests use deterministic fixtures and require no external provider access. Live tests are ignored by default; explicitly selecting one requires its caller-owned inputs and makes real read-only requests. The library accepts typed configuration and never loads these environment variables.

| Live test target | Backend feature | Qualified reads / explicit harness inputs |
| --- | --- | --- |
| [`evm_live`](../tests/evm_live.rs) | `evm-http` | Native balance; required endpoint/network/address/precision/source/selector inputs below |
| [`solana_live`](../tests/solana_live.rs) | `solana-http` | SOL balance, present decoded account and individual SPL token account; explicit URL/source/full genesis/network alias/commitment/account and token identity inputs |
| [`bitcoin_live`](../tests/bitcoin_live.rs) | `bitcoin-esplora` | Balance, recent history, fees, status and full transaction; explicit URL/source/network/network alias/address/transaction ID |
| [`xrpl_live`](../tests/xrpl_live.rs) | `xrpl-http` | XRP balance, trustline page, fees, bounded history, binary transaction and execution status; explicit endpoint/network/account/source/minimum-ledger inputs |
| [`coingecko_live`](../tests/coingecko_live.rs) | `coingecko-http` | Search, ID/currency prices, one markets page and history; explicit anonymous API base/source/item bound/listing/currency/search/time range |
| [`defillama_live`](../tests/defillama_live.rs) | `defillama-http` | TVL/history, yields/history, stablecoins/history and all four analytics metrics; three explicit bases/source labels plus item bound/protocol/pool/chain/stablecoin/analytics IDs |

After setting the linked test's required inputs, select its feature and target:

```sh
cargo test --locked --no-default-features --features solana-http --test solana_live -- --ignored --nocapture
cargo test --locked --no-default-features --features bitcoin-esplora --test bitcoin_live -- --ignored --nocapture
cargo test --locked --no-default-features --features xrpl-http --test xrpl_live -- --ignored --nocapture
cargo test --locked --no-default-features --features coingecko-http --test coingecko_live -- --ignored --nocapture
cargo test --locked --no-default-features --features defillama-http --test defillama_live -- --ignored --nocapture
```

Missing inputs or source errors fail an explicitly selected test. Read qualification applies to the recorded operation/provider/network/query; it does not establish full-library readiness or lasting finality. Bitcoin and provider data are separately retrieved indexed observations. Solana slot minimums are lower bounds; XRPL account/trustline reads retain a resolved validated ledger hash, while range/pending observations retain no invented hash.

## Bitcoin full transaction retrieval

This explicit public mainnet vector exercises raw transaction decoding and matching indexed fields through the Rust API. Select the single qualifier to avoid requiring unrelated address-history inputs:

```sh
env \
  REGIT_WEB3_BITCOIN_URL='https://blockstream.info/api' \
  REGIT_WEB3_BITCOIN_PROVIDER_ID='blockstream-mainnet' \
  REGIT_WEB3_BITCOIN_NETWORK='mainnet' \
  REGIT_WEB3_BITCOIN_NETWORK_ALIAS='bitcoin-mainnet' \
  REGIT_WEB3_BITCOIN_TXID='14396c6a212fce4a794501b5840c718a700daa9e462427fed22a7f38d5197b1e' \
  cargo test --locked --no-default-features --features bitcoin-esplora \
    --test bitcoin_live -- --ignored --exact bitcoin_transaction_live --nocapture
```

The public [SegWit record](https://blockstream.info/api/tx/14396c6a212fce4a794501b5840c718a700daa9e462427fed22a7f38d5197b1e) qualified on 2026-10-07: 303 bytes, 882 weight units, 221 virtual bytes, exact indexed fee 380 satoshis. The [genesis transaction](https://blockstream.info/api/tx/4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b) also qualified: 204 bytes, 816 weight units, computed txid equal to wtxid, legitimate null coinbase previous output. Canonical decoding and structural consistency do not verify scripts/signatures or prove inclusion; fee and previous outputs remain source-reported facts.

## EVM native-balance example

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

### EVM live qualification

Ordinary tests use deterministic fixtures. With the explicit inputs above, run the ignored public API test separately:

```sh
cargo test --locked --test evm_live --features evm-http -- --ignored --nocapture
```

Missing inputs fail when this test is explicitly selected. The recorded reads are historical, EVM-specific point-in-time evidence; they do not qualify other integrations or establish lasting finality.
