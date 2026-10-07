# LI.FI fixtures

The three JSON files are anonymous public responses captured from `https://li.quest/v1` on 2026-10-07. They contain public example accounts and historical transaction identifiers. Tests modify copies to exercise malformed numbers, duplicate keys, source identity rewrites, missing facts, refunds, bounded collections and exact continuation replay.

| File | Operation | Inputs |
| --- | --- | --- |
| `quote.json` | `GET /quote` | Arbitrum 42161 → Base 8453, native ETH, 10000000000000000 base units, slippage 0.005 |
| `routes.json` | `POST /advanced/routes` | Same transfer; transaction execution, gasless disabled, destination calls disabled, chain switching disabled |
| `status.json` | `GET /status` | Historical Arbitrum → Taiko 167000 transfer; sending hash `0xe1ffdcf09d5aa92a2d89b1b39db3f8cadf09428a296cce0d5e387595ac83d08f` |

Primary references checked after Context7's LI.FI documentation and maintained SDK lookup:

- [Current API specification](https://docs.li.fi/openapi.yaml): quote, route alternatives, step transaction preparation and status request/response fields.
- [Maintained types](https://github.com/lifinance/types/tree/main/src): exact chain numbers, step/action/estimate fields and current unavailable-route diagnostics.
- [Maintained SDK](https://github.com/lifinance/sdk/tree/main/packages): actual family payload forms and transaction-status polling inputs.
- [Public API access and limits](https://docs.li.fi/api-reference/rate-limits): anonymous access and caller-provided API keys. Limits can change; this backend makes no access or quota guarantee.

Current compatibility observations:

- A quote/route step selection UUID such as `bf1da5e9-ba03-4cc2-abe0-e0ceaf6ee132:0` is rejected by the status endpoint with HTTP 400 (`Not a valid txHash`), despite older specification prose mentioning a step ID. The maintained SDK supplies an actual transaction hash and an optional distinct `step.transactionId`.
- The observed provider transfer ID `0x5e9bd1e1232bcfb28e660ce116fe910aa058345604334e5f560034f51ef5327c` works as the status endpoint's `txHash` lookup and is echoed as `transactionId`. Its type remains separate from step UUIDs and family-qualified transaction hashes.
- The historical response reports Taiko 167000. Supplying a current different destination chain hint does not establish a different actual chain. The backend sends no optional chain hints and checks the caller's expected pair against all actual reported records.
- Current `unavailableRoutes` is an object containing `filteredOut` and `failed`, rather than the outdated array shape in specification prose. Typed results retain paths and machine error codes; free-form provider error messages are omitted.
- Universal quote expiry is unreported. Encoded payloads can carry their own bounds or signatures; source metadata and encoding checks do not establish decoded transfer intent or consensus finality.

Backend responses and private step continuations are limited to 2 MiB. Caller limits allow up to 64 route alternatives, 256 total steps per route and 128 costs per list; nested step depth is at most eight. Unavailable diagnostics have a 4096-entry limit per retained list. Source payload text is limited to 256 KiB. Every exceeded bound fails the operation without truncation. All actual referenced chains, including fee/gas tokens and intermediate steps, require explicit caller qualification.

The ordinary pure/loopback tests are offline. The opt-in harness uses 20 seconds total per operation, 5 seconds connect, at most two additional safe read retries and a 2 MiB body limit. Quote and preparation POSTs do not submit transactions. Reproduce its actual read/quote/preparation proof with these explicit public inputs:

```sh
export REGIT_WEB3_LIFI_URL=https://li.quest/v1
export REGIT_WEB3_LIFI_PROVIDER_ID=lifi-public
export REGIT_WEB3_LIFI_CHAINS='[{"id":42161,"family":"evm"},{"id":8453,"family":"evm"},{"id":167000,"family":"evm"}]'
export REGIT_WEB3_LIFI_FROM_CHAIN_ID=42161
export REGIT_WEB3_LIFI_TO_CHAIN_ID=8453
export REGIT_WEB3_LIFI_FROM_TOKEN=0x0000000000000000000000000000000000000000
export REGIT_WEB3_LIFI_TO_TOKEN=0x0000000000000000000000000000000000000000
export REGIT_WEB3_LIFI_AMOUNT=10000000000000000
export REGIT_WEB3_LIFI_FROM_ACCOUNT=0x204dedcf79dbbb02359205f4f64ce2cbdd483906
export REGIT_WEB3_LIFI_TO_ACCOUNT=0x204dedcf79dbbb02359205f4f64ce2cbdd483906
export REGIT_WEB3_LIFI_SLIPPAGE=0.005
export REGIT_WEB3_LIFI_STATUS_FROM_CHAIN_ID=42161
export REGIT_WEB3_LIFI_STATUS_TO_CHAIN_ID=167000
export REGIT_WEB3_LIFI_STATUS_TX_HASH=0xe1ffdcf09d5aa92a2d89b1b39db3f8cadf09428a296cce0d5e387595ac83d08f
export REGIT_WEB3_LIFI_STATUS_TRANSFER_ID=0x5e9bd1e1232bcfb28e660ce116fe910aa058345604334e5f560034f51ef5327c
cargo test --offline --locked --no-default-features --features lifi-http --test lifi_live -- --ignored --nocapture
```

The fixture and representative live proof cover explicit supplied inputs. Other supported family encodings are exercised locally; the EVM live sample does not qualify every LI.FI chain, tool or payload family.
