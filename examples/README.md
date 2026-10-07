# Rust examples and live read qualification

Ordinary tests use deterministic fixtures and require no external provider access. Live tests are ignored by default; explicitly selecting one requires its caller-owned inputs and makes real read-only requests. The library accepts typed configuration and never loads these environment variables.

| Live test target | Backend feature | Qualified reads / explicit harness inputs |
| --- | --- | --- |
| [`evm_live`](../tests/evm_live.rs) | `evm-http` | Native/ERC-20/transaction/receipt/status reads or fee/nonce/call/gas estimation; select the exact qualifier and its explicit inputs below |
| [`litecoin_live`](../tests/litecoin_live.rs) | `litecoin-http` | Five documented mainnet BlockCypher reads and history continuation; explicit URL/source/genesis/alias/address/txid and history/transaction capacities |
| [`dogecoin_live`](../tests/dogecoin_live.rs) | `dogecoin-http` | Five documented mainnet BlockCypher reads and history continuation; same family-qualified inputs |
| [`bitcoin_cash_live`](../tests/bitcoin_cash_live.rs) | `bitcoin-cash-electrum` | Six Electrum-Cash TLS source reads; explicit host/port/server name/DER trust root/full genesis/fork checkpoint/address/txid/history interval/capacity/fee target |
| [`solana_live`](../tests/solana_live.rs) | `solana-http` | SOL/account/SPL reads plus canonical transaction/status, recent hash/validity/height, exact message fee and unsigned simulation; explicit URL/source/full genesis/network alias/commitment/account and token identity inputs |
| [`blockfrost_live`](../tests/blockfrost_live.rs) | `blockfrost-http` | Awaiting authenticated live qualification: seventeen indexed read/estimate methods and local unsigned review; explicit project credential/network/address/stake/asset/transaction/epoch/payment inputs in the fixture recipe |
| [`bitcoin_live`](../tests/bitcoin_live.rs) | `bitcoin-esplora` | Balance, recent history, fees, status and full transaction; explicit URL/source/network/network alias/address/transaction ID |
| [`xrpl_live`](../tests/xrpl_live.rs) | `xrpl-http` | XRP balance, trustline page, fees, bounded history, binary transaction and execution status; explicit endpoint/network/account/source/minimum-ledger inputs |
| [`coingecko_live`](../tests/coingecko_live.rs) | `coingecko-http` | Search, ID/currency prices, one markets page and history; explicit anonymous API base/source/item bound/listing/currency/search/time range |
| [`defillama_live`](../tests/defillama_live.rs) | `defillama-http` | TVL/history, yields/history, stablecoins/history and all four analytics metrics; three explicit bases/source labels plus item bound/protocol/pool/chain/stablecoin/analytics IDs |
| [`mempool_space_live`](../tests/mempool_space_live.rs) | `mempool-space-http` | Backlog, recent arrivals, full bounded IDs, recommended fees, canonical transaction and status; explicit API base/source/network/alias/confirmed transaction/full-list capacity |
| [`ton_live`](../tests/ton_live.rs) | `ton-http` | Seven read/estimate methods, additional outgoing-fee transaction and local unsigned review; explicit URL/source/zero-state/account/cursors/message/fee-body/transfer/pacing inputs |
| [`thorchain_live`](../tests/thorchain_live.rs) | `thorchain-http` | RUNE balance, individual/complete layer-one pool reads, network values, swap quote, inbound vaults, chain heights and transaction progress; explicit API base/source/Cosmos chain ID/account prefix/alias/account/assets/amount/destination/transaction/item bound |
| [`jupiter_live`](../tests/jupiter_live.rs) | `jupiter-http` | V2 quote-only selection, fresh Metis build, local canonical V0 preparation/handoff and exact-message Solana fee/simulation; explicit API/RPC/config/request/settings inputs |
| [`uniswap_live`](../tests/uniswap_live.rs) | `uniswap-http` | V3 exact-input quotes, supplied-path comparison and local unsigned Universal Router 2.1.2 preparation; explicit EVM/deployment/path/input/call/recipient/slippage/deadline/handoff inputs |
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

## EVM execution reads and unsigned preparation

The `fees_nonce_call_and_estimate_live` qualifier uses the native example inputs
plus `REGIT_WEB3_EVM_CALL_JSON`, a serialized `TransactionCallData` with explicit
chain, sender, destination, nonce, gas cap, value, calldata and fee terms. The
harness queries fees separately, then reuses the returned nonce observation's
captured canonical block hash for call and estimate. It does not use the queried
nonce as a signing policy or perform a write.

```sh
cargo test --locked --no-default-features --features evm-http \
  --test evm_live -- --ignored --exact fees_nonce_call_and_estimate_live --nocapture
cargo test --locked --no-default-features --features evm --test evm_preparation
cargo test --locked --no-default-features --features evm-http --test evm_submission
```

`TransactionRequest` prepares native value transfers or standard ERC-20 transfer
and approval calldata from explicit nonce/gas/fee choices. `PreparedTransaction`
implements the generic wallet preparation contract and retains canonical
legacy/type1/type2 signing bytes and digest. These bytes do not recover or bind
an expected sender without external signature verification. `SignedSubmission`
checks canonical supported envelope structure, ranges, chain and computed ID;
it does not prove signature validity or reviewed intent. `submit_signed` is a
separate explicit one-shot write. Its loopback tests cover ambiguous outcomes;
there is no live funded-submission qualifier.

## Bitcoin Cash TLS reads

The `bitcoin_cash_six_source_reads_live` qualifier uses the explicit harness
variables `REGIT_WEB3_BITCOIN_CASH_{HOST,PORT,SERVER_NAME,ROOT_DER_PATH,
NETWORK_ALIAS,GENESIS_HASH,FORK_HEIGHT,FORK_HASH,PROVIDER_ID,ADDRESS,TXID,
CAPACITY,HISTORY_FROM,HISTORY_TO,FEE_TARGET}`. `ROOT_DER_PATH` is a caller-owned
certificate trust anchor; certificate and server-name validation stay enabled.
The harness qualifies a positive public mainnet balance, finite nonempty history,
exact BCH/1000-byte fee suggestion, confirmed transaction/status and nonempty
UTXOs. Token-bearing metadata has fixture proof; the recorded public address's
outputs were token-free. It does not sign or submit.

```sh
cargo test --locked --no-default-features --features bitcoin-cash-electrum \
  --test bitcoin_cash_live -- --ignored --exact bitcoin_cash_six_source_reads_live --nocapture
cargo test --locked --no-default-features --features bitcoin-cash-electrum \
  --test bitcoin_cash --test bitcoin_cash_electrum
```

The backend negotiates Electrum-Cash 1.6 and checks token support, full genesis
and the explicit fork checkpoint at establishment and before every operation.
A namespace alone cannot distinguish chains sharing genesis or test prefixes.
History intervals retain their inclusive lower/exclusive upper bounds; an open
tip includes source mempool entries without establishing a common snapshot.
Raw transaction bytes retain a computed identity beside source verbose fields,
without consensus decoding, script/signature validation or inclusion proof.

## Uniswap V3 quotes and unsigned preparation

The qualifier uses the EVM example inputs plus the following caller-owned values.
[`live_inputs.json`](../tests/fixtures/uniswap/live_inputs.json) retains the public
qualification inputs and exact supported deployment profile.

| Variable | Value |
| --- | --- |
| `REGIT_WEB3_UNISWAP_QUOTE_JSON` | Serialized `V3QuoteRequest`: declared contracts, forward token/fee path, raw input and explicit sender/nonce/gas/fees |
| `REGIT_WEB3_UNISWAP_ROUTES_JSON` | Serialized `V3RouteRequest` with at most 16 supplied paths and the same input/call settings |
| `REGIT_WEB3_UNISWAP_RECIPIENT` | Literal output-token recipient |
| `REGIT_WEB3_UNISWAP_SLIPPAGE_BPS` | Explicit tolerance, 0 through 10000 |
| `REGIT_WEB3_UNISWAP_DEADLINE` | Explicit Unix-second router deadline |
| `REGIT_WEB3_UNISWAP_HANDOFF_ID` | Bounded caller-generated handoff identifier |

```sh
cargo test --locked --no-default-features --features uniswap-http \
  --test uniswap_live -- --ignored --exact \
  v3_quotes_supplied_routes_and_router_2_1_2_unsigned_preparation_live --nocapture
```

Both reads qualified through the Rust API on 2026-10-07. Every compared path
used the same captured canonical hash; the harness constructed an immutable
unsigned Universal Router 2.1.2 intent and wallet handoff. No approval, signer
or submission was invoked. This profile supports V3 pools/QuoterV2 and the
six-field router 2.1.2 command; supplied deployment declarations require caller
verification. Preparation checks the deadline against quoted block time only;
current-time expiry and future liquidity remain caller policy.

## TON reads, fee estimates and unsigned review

[`TON fixture documentation`](../tests/fixtures/ton/README.md) retains the exact
public inputs for the seven-method qualifier and an additional outgoing-fee
transaction. It supplies the expected full zero-state, account/cursors, bounded
history, incoming-message hash, an explicit unsigned no-action Wallet-V4 fee
body and local internal-message review choices. Caller-selected request spacing
1100 ms and separate harness operation pacing 2100 ms qualified the anonymous
endpoint after initial unpaced attempts returned `RateLimited`. Spacing is
client-local, consumes the total deadline and does not guarantee provider quotas.

```sh
cargo test --locked --no-default-features --features ton-http \
  --test ton_live -- --ignored --exact \
  current_ton_reads_fee_estimate_and_internal_review --nocapture
```

The Rust API qualified these reads/estimates on 2026-10-07. The fee body uses
`ignore_chksig=true`; its stored sequence is an explicit input, never refreshed
implicitly. Decoded transaction fees remain separate from the reported aggregate
and outgoing fee components. Account evaluation retains a full source block;
history/status do not invent a common block or finality proof. Internal-message
preparation encodes destination/value/bounce/body; outer-wallet sender/expiry are
review policy for an external verifier. Submission has one-shot loopback proof
only, with no funded write or signature-verification claim.

## Solana transaction reads and unsigned simulation

The added qualifier calls seven RPC methods through the public Rust API. Set
these explicit public inputs, then select only the execution qualifier:

```sh
export REGIT_WEB3_SOLANA_URL=https://api.mainnet.solana.com
export REGIT_WEB3_SOLANA_GENESIS_HASH=5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
export REGIT_WEB3_SOLANA_NETWORK_ALIAS=mainnet
export REGIT_WEB3_SOLANA_PROVIDER_ID=solana-mainnet
export REGIT_WEB3_SOLANA_COMMITMENT=confirmed
export REGIT_WEB3_SOLANA_TRANSACTION_SIGNATURE=4ofUzrGeDGsRvg7oTzC133nSACkXMP9vagDw3RMyswa2AsAB9Vp9Vyh7EAY2WudZDjf43NJkN1KJLyo2mVYDvg5x
export REGIT_WEB3_SOLANA_SIMULATION_FEE_PAYER=AXmnRBrNtYYyyo82cLBBhnWJ7o1iqNLZbuEVpDB3V666
export REGIT_WEB3_SOLANA_SIMULATION_RECIPIENT=AXmnRBrNtYYyyo82cLBBhnWJ7o1iqNLZbuEVpDB3V666
export REGIT_WEB3_SOLANA_SIMULATION_LAMPORTS=1
export REGIT_WEB3_SOLANA_SIMULATION_EXPECTED_OUTCOME=success
cargo test --locked --no-default-features --features solana-http \
  --test solana_live solana_execution_reads_live -- --ignored --exact --nocapture
```

On 2026-10-07 this qualified a real 1956-byte v1 transaction, independent
status, a fresh hash/last-valid block height, exact 5000-lamport message fee and
a successful unsigned legacy self-transfer simulation consuming 150 compute
units. Future account state can change the simulation result. The minimum slot
is a lower bound, last-valid height is separate unencoded review policy, and
separate responses do not share an atomic snapshot. Maintained legacy/v0 codecs,
classic SPL TransferChecked preparation and one-shot submission have substantive
fixture proof. No signed submission, signing or funded transfer was performed.
[Fixture provenance](../tests/fixtures/solana_execution/README.md) retains the
source vector and primary contracts.

## Jupiter V2 quotes, fresh builds and unsigned estimates

[Live inputs](../tests/fixtures/jupiter/live_inputs.json) retain the explicit
public request/configuration/settings objects. Export `api_url` and
`api_provider_id` as `REGIT_WEB3_JUPITER_URL` and
`REGIT_WEB3_JUPITER_PROVIDER_ID`; export `rpc_url`, `rpc_provider_id` and
`handoff_id` as their `REGIT_WEB3_JUPITER_RPC_URL`,
`REGIT_WEB3_JUPITER_RPC_PROVIDER_ID` and `REGIT_WEB3_JUPITER_HANDOFF_ID` values.
Serialize the `quote`, `build` and `settings` objects into
`REGIT_WEB3_JUPITER_QUOTE_JSON`, `REGIT_WEB3_JUPITER_BUILD_JSON` and
`REGIT_WEB3_JUPITER_SETTINGS_JSON`. Then select the qualifier:

```sh
cargo test --locked --no-default-features --features jupiter-http \
  --test jupiter_live v2_quote_build_unsigned_handoff_fee_and_simulation_live \
  -- --ignored --exact --nocapture
```

The harness uses explicit keyless mode and spaces the two API requests by
2100 ms; quotas remain provider-dependent. On 2026-10-07 the public Rust API
qualified quote/build, local unsigned review/handoff and source fee/simulation.
The source-selected quote and fresh build had different routes/outputs. The
public example taker returned `AccountNotFound` in simulation, without proof of
swap success. Source instructions/ALT state require trusted external semantic
review; the explicit `otherInstructions` placement is caller policy. API mainnet
identity is declared; the composed Solana RPC separately verifies full genesis.
No signing, provider-managed execution or submission was performed.
[Fixture provenance](../tests/fixtures/jupiter/README.md) records primary contracts.

## Cardano indexed operations and payment preparation

`cardano` provides exact family identities, indexed records, original transaction
CBOR/body hashes and explicit Conway key-spend ADA/native-asset preparation.
Callers select every input, output/change, fee, absolute validity slot and witness
count; explicit protocol parameters determine minimum ADA and serialized-size fees.
Preparation and generic wallet handoff perform no signing or submission.

`blockfrost-http` implements seventeen indexed read/estimate methods with fresh
network-magic verification, bounded explicit pages and one operation deadline.
The separate signed submission method sends exact raw CBOR once after preflight.
It checks body/key/size correlation without verifying cryptographic signatures;
provider acknowledgment does not establish execution or finality.

The [fixture and input recipe](../tests/fixtures/blockfrost/README.md) documents
the supported payment profile and every live input. Ordinary pure/loopback tests
pass; authenticated indexed-data qualification remains pending. The ignored
harness calls every read/estimate and local review, and never submits:

```sh
cargo test --locked --no-default-features --features blockfrost-http \
  --test blockfrost_live -- --ignored --nocapture
```
