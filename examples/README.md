# Rust examples and live read qualification

Ordinary tests use deterministic fixtures and require no external provider access. Live tests are ignored by default; explicitly selecting one requires its caller-owned inputs and makes real read-only requests. The library accepts typed configuration and never loads these environment variables.

| Live test target | Backend feature | Qualified reads / explicit harness inputs |
| --- | --- | --- |
| [`evm_live`](../tests/evm_live.rs) | `evm-http` | Native balance or ERC-20 balance/allowance/optional metadata and transaction/receipt/status; select the exact qualifier and its explicit inputs below |
| [`litecoin_live`](../tests/litecoin_live.rs) | `litecoin-http` | Five documented mainnet BlockCypher reads and history continuation; explicit URL/source/genesis/alias/address/txid and history/transaction capacities |
| [`dogecoin_live`](../tests/dogecoin_live.rs) | `dogecoin-http` | Five documented mainnet BlockCypher reads and history continuation; same family-qualified inputs |
| [`solana_live`](../tests/solana_live.rs) | `solana-http` | SOL balance, present decoded account and individual SPL token account; explicit URL/source/full genesis/network alias/commitment/account and token identity inputs |
| [`bitcoin_live`](../tests/bitcoin_live.rs) | `bitcoin-esplora` | Balance, recent history, fees, status and full transaction; explicit URL/source/network/network alias/address/transaction ID |
| [`xrpl_live`](../tests/xrpl_live.rs) | `xrpl-http` | XRP balance, trustline page, fees, bounded history, binary transaction and execution status; explicit endpoint/network/account/source/minimum-ledger inputs |
| [`coingecko_live`](../tests/coingecko_live.rs) | `coingecko-http` | Search, ID/currency prices, one markets page and history; explicit anonymous API base/source/item bound/listing/currency/search/time range |
| [`defillama_live`](../tests/defillama_live.rs) | `defillama-http` | TVL/history, yields/history, stablecoins/history and all four analytics metrics; three explicit bases/source labels plus item bound/protocol/pool/chain/stablecoin/analytics IDs |
| [`mempool_space_live`](../tests/mempool_space_live.rs) | `mempool-space-http` | Backlog, recent arrivals, full bounded IDs, recommended fees, canonical transaction and status; explicit API base/source/network/alias/confirmed transaction/full-list capacity |
| [`thorchain_live`](../tests/thorchain_live.rs) | `thorchain-http` | RUNE balance, individual/complete layer-one pool reads, network values, swap quote, inbound vaults, chain heights and transaction progress; explicit API base/source/Cosmos chain ID/account prefix/alias/account/assets/amount/destination/transaction/item bound |
| [`lifi_live`](../tests/lifi_live.rs) | `lifi-http` | Quote, routes, preparation and transaction/provider-transfer status; explicit URL/source/chain-family catalogue/assets/accounts/raw amount/slippage/status inputs |

After setting the linked test's required inputs, select its feature and target:

```sh
cargo test --locked --no-default-features --features solana-http --test solana_live -- --ignored --nocapture
cargo test --locked --no-default-features --features bitcoin-esplora --test bitcoin_live -- --ignored --nocapture
cargo test --locked --no-default-features --features xrpl-http --test xrpl_live -- --ignored --nocapture
cargo test --locked --no-default-features --features coingecko-http --test coingecko_live -- --ignored --nocapture
cargo test --locked --no-default-features --features defillama-http --test defillama_live -- --ignored --nocapture
cargo test --locked --no-default-features --features mempool-space-http --test mempool_space_live -- --ignored --nocapture
cargo test --locked --no-default-features --features thorchain-http --test thorchain_live -- --ignored --nocapture
```

Missing inputs or source errors fail an explicitly selected test. Read qualification applies to the recorded operation/provider/network/query; it does not establish full-library readiness or lasting finality. Bitcoin and provider data are separately retrieved indexed observations. Solana slot minimums are lower bounds; XRPL account/trustline reads retain a resolved validated ledger hash, while range/pending observations retain no invented hash.

## Wallet preparation and handoff

The default library includes typed `Preparation`, `PreparedRequest`, read-only
`Review`, caller-generated `HandoffId` and request/response records. A response
remains unverified until `verify_handoff` first correlates its ID and snapshot,
then invokes a trusted caller-supplied `SignedPayloadVerifier` against the actual
signed content. `VerifiedSignedPayload` retains the original preparation and
cannot be restored by deserialization. Custom snapshot and verifier contracts
are part of this trust boundary.

With `xrpl`, `XrplPaymentPreparation` derives ordinary unsigned Payment JSON fields
from validated intent. These are reviewable fields, not binary signing bytes.
The library supplies no concrete cryptographic verifier, signer or connector;
these wallet operations do not sign or submit. See the [generic contract fixtures](../tests/wallets.rs)
and [XRPL adapter fixtures](../tests/wallets_xrpl.rs), which require no runtime or
external wallet:

```sh
cargo test --locked --no-default-features --test wallets
cargo test --locked --no-default-features --features xrpl --test wallets_xrpl
```

## mempool.space reads

This explicit mainnet input selects all six read methods. The harness uses a
16 MiB body ceiling and a 100000-ID capacity for the full, unpaged mempool list;
exceeding either bound fails the operation. It never returns a truncated list.
Recent arrivals are capped at ten by the provider. Summary and list counts can
change between requests and are not required to agree.

```sh
env \
  REGIT_WEB3_MEMPOOL_SPACE_URL='https://mempool.space/api' \
  REGIT_WEB3_MEMPOOL_SPACE_PROVIDER_ID='mempool-public' \
  REGIT_WEB3_MEMPOOL_SPACE_NETWORK='mainnet' \
  REGIT_WEB3_MEMPOOL_SPACE_NETWORK_ALIAS='bitcoin' \
  REGIT_WEB3_MEMPOOL_SPACE_MAX_TXIDS='100000' \
  REGIT_WEB3_MEMPOOL_SPACE_TXID='14396c6a212fce4a794501b5840c718a700daa9e462427fed22a7f38d5197b1e' \
  cargo test --locked --no-default-features --features mempool-space-http \
    --test mempool_space_live -- --ignored --nocapture --test-threads=1
```

## THORChain reads and quotes

This explicit mainnet input selects all eight read/quote methods. The harness
supplies the Cosmos chain ID and account prefix independently, a 2 MiB body
ceiling and a 1000-item collection capacity. Quotes retain requested input and
amount in protocol 1e8 units; the source can expand asset identifiers without
reporting the resolved input. The backend records unreported input resolution
and checks the full output fee identity and current expiry. Source cross-chain
progress does not independently establish external inclusion or signed execution.

```sh
env \
  REGIT_WEB3_THORCHAIN_URL='https://gateway.liquify.com/chain/thorchain_api' \
  REGIT_WEB3_THORCHAIN_PROVIDER_ID='liquify-thornode' \
  REGIT_WEB3_THORCHAIN_CHAIN_ID='thorchain-1' \
  REGIT_WEB3_THORCHAIN_ACCOUNT_PREFIX='thor' \
  REGIT_WEB3_THORCHAIN_NETWORK_ALIAS='mainnet' \
  REGIT_WEB3_THORCHAIN_ACCOUNT='thor1dheycdevq39qlkxs2a6wuuzyn4aqxhve4qxtxt' \
  REGIT_WEB3_THORCHAIN_FROM_ASSET='BTC.BTC' \
  REGIT_WEB3_THORCHAIN_TO_ASSET='ETH.ETH' \
  REGIT_WEB3_THORCHAIN_AMOUNT='100000000' \
  REGIT_WEB3_THORCHAIN_DESTINATION='0x1c7b17362c84287bd1184447e6dfeaf920c31bbe' \
  REGIT_WEB3_THORCHAIN_MAX_ITEMS='1000' \
  REGIT_WEB3_THORCHAIN_TXID='A3F81568387CD3880AED812780799E8F6D3F970E071F7B1861B581B20399F21F' \
  cargo test --locked --no-default-features --features thorchain-http \
    --test thorchain_live -- --ignored --nocapture
```

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
cargo test --locked --test evm_live --features evm-http -- --ignored --exact native_balance_live --nocapture
```

Missing inputs fail when this test is explicitly selected. The recorded reads are historical, EVM-specific point-in-time evidence; they do not qualify other integrations or establish lasting finality.

## EVM ERC-20 and transaction reads

The second EVM qualifier uses the native example inputs plus
`REGIT_WEB3_ERC20_CONTRACT`, `REGIT_WEB3_ERC20_SPENDER` and
`REGIT_WEB3_TRANSACTION_ID`. It reads raw token units without assuming precision,
records each optional metadata field, and separately checks transaction, receipt
and status. Source inclusion and failed execution remain distinct.

```sh
cargo test --locked --no-default-features --features evm-http \
  --test evm_live -- --ignored --exact erc20_and_transactions_live --nocapture
```

A representative Ethereum-mainnet USDC read qualified on 2026-10-07 through
PublicNode: decimals 6, name USD Coin and symbol USDC. The selected included
legacy transaction had receipt status 0 (failed); inclusion did not become a
success or finality claim. Typed forms 1–4 have fixture coverage; this live
transaction does not qualify every transaction form or query.

## Litecoin and Dogecoin indexed reads

Each harness requires its family prefix (`REGIT_WEB3_LITECOIN_` or
`REGIT_WEB3_DOGECOIN_`) followed by `URL`, `PROVIDER_ID`, `NETWORK_ALIAS`,
`GENESIS_HASH`, `ADDRESS`, `TXID`, `HISTORY_MINIMUM`, `HISTORY_CAPACITY` and
`TRANSACTION_CAPACITY`. The backend supports the documented mainnet endpoints;
pure address/network validation also covers the supported test/regression forms.
The harness paces requests for the anonymous provider tier.

```sh
cargo test --locked --no-default-features --features litecoin-http \
  --test litecoin_live -- --ignored --nocapture
cargo test --locked --no-default-features --features dogecoin-http \
  --test dogecoin_live -- --ignored --nocapture
```

Both five-method qualifiers and separate history continuations passed through
the Rust API on 2026-10-07. Fees retain native atomic units per 1000 serialized
bytes. Indexed raw transaction bytes remain opaque; source IDs, inclusion and
coinbase classification do not establish computed identity or consensus/signature
verification. A missing continuation flag is uncertainty, not proof of exhaustion.

## LI.FI quotes, routes and preparation

The [fixture notes](../tests/fixtures/lifi/README.md) contain the explicit public
replay inputs and source compatibility observations. Preparation sends the exact
selected private step to its configured authority and returns fresh source
estimates/payload for review. It submits nothing. Chain hashes and provider
transfer IDs are distinct status inputs; quote/route selection UUIDs are not
accepted status hashes. The representative EVM proof covers four methods and
both supported status selectors. Other family encodings have local fixture proof.

```sh
cargo test --locked --no-default-features --features lifi-http \
  --test lifi_live -- --ignored --nocapture
```
