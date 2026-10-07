# TON fixtures and read qualification

| File | Evidence |
| --- | --- |
| `masterchain.json` | Public TON Center API-v2 `getMasterchainInfo` response, mainnet, 2026-10-07. |
| `account.json` | Public `getAddressInformation` response for the documented account below. |
| `transactions.json` | Two public account transactions, exact full BOCs and source hashes. Both actually aborted with skipped `no_gas` compute; they are not successful transfers. |
| `outgoing.json` | Public account transaction `107309865000001`, hash `m7Z0ppQFUkwvBfnub3EboWbphq2PCkxC+J/GOxdldLs=`, with one outgoing message and nonzero forwarding fee. |

The loopback account builder substitutes the fixture masterchain block for its
later account block to test full response/query correlation. This synthetic
combination is a fixture, not an independently verified account inclusion proof.
BOC, address CRC, representation hashes and internal-message encoding use
`tycho-types` 0.3.5. Unsupported descriptions/header formats retain explicit
uninterpreted evidence. Small historical IHR-field values can overlap modern
extra flags; no historical IHR interpretation is inferred. Signed submission is
qualified only through fixtures.

Decoded TL-B `total_fees` remain separate from the API aggregate: the outgoing
fixture decodes 442944 nanotons while the provider reports 507479, including
64535 outgoing forwarding nanotons. Provider storage/other components sum to
that aggregate. Ordered outgoing hashes and supported forwarding fields are
correlated with BOC evidence; API-labelled IHR fees remain source facts,
including when header interpretation is unavailable. The signed `int32`
extra-currency API ID preserves its exact 32-bit protocol identity as `u32`.

Primary contracts checked through Context7 and current primary sources:

- [Address formats](https://docs.ton.org/foundations/addresses/formats)
- [TON Center API-v2 OpenAPI](https://toncenter.com/api/v2/openapi.json)
- [Transaction retrieval](https://docs.ton.org/api/v2/transactions/get-transactions)
- [Wallet V4](https://docs.ton.org/contracts/standard/wallets/v4) and its maintained contract source
- [TON library transaction/fee mapping](https://github.com/ton-blockchain/ton/blob/master/tonlib/tonlib/TonlibClient.cpp)
- [TON library API schema](https://github.com/ton-blockchain/ton/blob/master/tl/generate/scheme/tonlib_api.tl)
- [Maintained codec source](https://github.com/broxus/tycho-types/tree/v0.3.5)
- [Mainnet zero-state](https://ton-blockchain.github.io/global.config.json)

Actual Rust read qualification uses explicit public inputs, no API key, no
submission. Initial unpaced and between-operation-only attempts returned
`RateLimited`. The final backend explicitly spaces each scheduled attempt,
including retries, within the same operation budget. This client-local setting
is not a provider-wide rate-limit guarantee. The harness separately paces whole
operations. Library configuration never discovers these values.

```sh
env \
  REGIT_WEB3_TON_URL=https://toncenter.com/api/v2 \
  REGIT_WEB3_TON_PROVIDER_ID=toncenter-mainnet \
  REGIT_WEB3_TON_ZERO_ROOT='F6OpKZKqvqeFp6CQmFomXNMfMj2EnaUSOXN+Mh+wVWk=' \
  REGIT_WEB3_TON_ZERO_FILE='XplPz01CXAps5qeSWUtxcyBfdAo5zVb1N979KLSKD24=' \
  REGIT_WEB3_TON_ACCOUNT=EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrHF \
  REGIT_WEB3_TON_TX_LT=107688363000020 \
  REGIT_WEB3_TON_TX_HASH='HiUo0gLKMkSkCgEsSPGcwRsKjHjcUKyLfzJY+YynwAQ=' \
  REGIT_WEB3_TON_MESSAGE_HASH='CtQlGYxP5PwzMlInD6NU3cpjPk5nDYw+rzftKBfHaMI=' \
  REGIT_WEB3_TON_OUTGOING_TX_LT=107309865000001 \
  REGIT_WEB3_TON_OUTGOING_TX_HASH='m7Z0ppQFUkwvBfnub3EboWbphq2PCkxC+J/GOxdldLs=' \
  REGIT_WEB3_TON_PAGE_LIMIT=2 \
  REGIT_WEB3_TON_FEE_BODY_BOC=te6ccgEBAQEATwAAmgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAApqaMXdzWUAAAAA9MA \
  REGIT_WEB3_TON_PREPARE_DESTINATION=EQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPrHF \
  REGIT_WEB3_TON_PREPARE_NANOTONS=1000000 \
  REGIT_WEB3_TON_WALLET_VALID_UNTIL=2000000000 \
  REGIT_WEB3_TON_REQUEST_SPACING_MILLIS=1100 \
  REGIT_WEB3_TON_OPERATION_INTERVAL_MILLIS=2100 \
  cargo test --offline --locked --no-default-features --features ton-http \
    --test ton_live -- --ignored --nocapture
```

The explicit fee body is an unsigned **no-action** Wallet-V4 body assembled with
the maintained cell builder: zero 512-bit signature placeholder, wallet ID
698983191, caller-selected validity 2000000000, stored account sequence 979 and
operation 0. `ignore_chksig=true` estimates only. This is neither a signed
transaction nor a library wallet encoder. Refresh this caller-supplied body if
the account sequence changes; do not silently replace it after a source error.

The harness uses 5-second connection / 20-second total operation limits,
2-MiB response ceilings and two additional safe-read retries. Every BOC is
bounded to 64 KiB, one root, 8192 cells and 256 levels; one history page retains
at most 100 transactions and 256 outgoing messages per transaction. Fees retain
at most 256 destination components. No list is silently truncated.

Current native macOS qualification: 18 pure and 20 loopback tests passed,
strict pure/backend Clippy passed, and the opt-in live test passed in 25.16s on
2026-10-07 (retrieval Unix seconds 1791347398–1791347421). It exercised all seven
outgoing read/estimate methods, an additional nonzero-outgoing transaction
lookup and pure internal-message review. The two-row account history,
full transaction/execution lookup, outgoing-fee transaction and incoming-message scan are exact selected
vectors; this does not prove every account, transaction description or wallet
version. Returned account full-block identity is checked against the requested
masterchain evaluation; history linkage does not establish block inclusion.
Outer-wallet sender/expiry are immutable review policy, not encoded by the
internal-message preparation BOC. No funded transaction was submitted.
