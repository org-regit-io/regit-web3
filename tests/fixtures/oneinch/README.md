# Classic Swap v6.1 fixtures

These are synthetic protocol fixtures, not observed transactions or live execution proof. Current primary schemas were checked on 2026-10-07. Context7 resolved the 1inch documentation mirror, but its documentation query returned `Could not fetch documentation snippets from the library`; the primary endpoint documentation was used directly.

| Operation | Current primary reference | Library behavior |
| --- | --- | --- |
| Quote | [GET quote](https://business.1inch.com/portal/documentation/apis/swap/classic-swap/methods/v6.1/chainId/quote/method/get) | Exact input units, requested token/graph/gas fields and actual v6.1 `TokenSwaps` / hops / liquidity-share graph |
| Sources | [GET liquidity-sources](https://business.1inch.com/portal/documentation/apis/swap/classic-swap/methods/v6.1/chainId/liquidity-sources/method/get) | Complete bounded catalogue; duplicate IDs or excess entries fail whole |
| Spender | [GET approve/spender](https://business.1inch.com/portal/documentation/apis/swap/classic-swap/methods/v6.1/chainId/approve/spender/method/get) | Source-reported spender identity; no approval payload or write |
| Preparation | [GET swap](https://business.1inch.com/portal/documentation/apis/swap/classic-swap/methods/v6.1/chainId/swap/method/get) | Separate spender read and fresh unsigned source result, retaining original selection and explicit review policy |
| Authentication | [Authentication](https://business.1inch.com/portal/documentation/apis/authentication) | Explicit caller-owned Bearer API key or existing OAuth access token; no acquisition, refresh, discovery or environment loading in the library |

The Classic Swap endpoints describe EVM support. A broader product list including other families does not qualify these endpoints for them. The literal backend path is `v6.1/{chainId}` relative to a caller-supplied swap API prefix. Configured URL queries are rejected so hidden source parameters cannot override reviewed typed values.

The route graph is the current v6.1 object shape, not v6.0 triple arrays. Percentages and JSON integer gas suggestions are decoded from lexical JSON numbers into exact decimals/integers. String-number and private-number-object substitutions are invalid. Source token IDs need not index the outer group array, repeated token IDs must agree on actual addresses, and no percentage conservation or path topology is inferred. Every reported liquidity-share ID must match the exact caller allowlist when nonempty and must not match its exclusions; an empty graph remains absence of route evidence, without completeness or execution inference. The fractional fixture deliberately retains `33.000000000000000001` without requiring its shares to sum to 100.

Native asset identity is distinct from the provider's `0xeeee…eeee` encoding. Native-input call value must exactly match the reviewed input; the ordinary ERC20-input profile requires zero call value. Fee-on-transfer tokens known from source metadata are outside this preparation profile. Preparation sends exactly one of `minReturn` or `slippage`, explicit `from`, `origin`, `receiver` and `disableEstimate`, and fixes partial fill, compatibility, patching, Permit2, force-approval and access-list creation to false. It supplies no permit or partner-fee/referrer parameters. Complexity 0–3 and split counts 1–100 are this bounded ordinary routing profile; they are not asserted to exhaust future API options.

The separately retrieved spender and fresh transaction do not share an atomic snapshot. The output amount, sender, router, spender and native value are checked; recipient, origin, return semantics and token transfer semantics in calldata remain unverified. Opaque calldata is retained for independent review. No nonce, block/genesis proof, expiry, signature, execution, approval or finality is invented. Source gas/price/optional gas-used suggestions remain distinct from caller signing policy. Nonempty returned access lists conflict with the disabled ordinary profile. Unknown transaction envelope fields are rejected, including chain/type/authorization/blob/signature/nonce properties; descriptive token/quote metadata remains forward-compatible. The ordinary positive vector omits gas-used/access-list fields. A separately labeled supported optional-metadata vector preserves the exact source suggestion without inferring its simulation or additional equality rules. Estimation override presence is represented as missing, null, empty or present-uninterpreted; override JSON is not exported or injected into RPC simulation.

| Capacity | Bound |
| --- | --- |
| HTTP document | Caller-selected, at most 2 MiB |
| Route graph | At most 64 groups, 256 hops/group, 256 shares/hop and 4096 total group/hop/share records; caller can select a lower total |
| Complete liquidity catalogue | At most 512 entries; caller can select a lower total |
| Protocol/connector filters | At most 64 per collection |
| Source protocol ID | 128 ASCII bytes using letters, digits, underscore, hyphen or dot |
| Source display text | 4096 UTF-8 bytes, preserving JSON-escaped controls |
| Unsigned calldata | 4 through 65536 bytes; bounded opaque data |

One operation deadline covers all reads, safe retries, body transfer and typed decoding. Quote/swap retries retain identical encoded requests. The cumulative fixture gives each of two response bodies a two-second delay under a three-second total budget: each phase fits individually but their combined preparation times out. Oversize results fail rather than truncate. Malformed successful responses are terminal; diagnostics and Debug do not disclose endpoint headers, response bodies or calldata bytes.

Focused native checks:

```sh
cargo test --offline --locked --no-default-features --features oneinch-http --test oneinch --test oneinch_http --test oneinch_live
cargo clippy --offline --locked --no-default-features --features oneinch-http --lib --test oneinch --test oneinch_http --test oneinch_live -- -D warnings
cargo clippy --offline --locked --no-default-features --features oneinch --lib --test oneinch -- -D warnings
```

Current fixture result: 12 pure and 19 actual-loopback tests passed; one live test is ignored ordinarily. Pure dependency qualification excludes HTTP, runtime, URL and TLS dependencies. No dependencies were added for this integration. Authenticated Rust live data is still unqualified while a caller-owned credential is unavailable; discovery, documentation and these fixtures do not substitute for it.

To run the actual no-write harness, supply all these test-owned environment inputs:

| Input prefix `REGIT_WEB3_ONEINCH_` | Meaning |
| --- | --- |
| `URL` | Explicit swap API prefix, normally `https://api.1inch.com/swap/` |
| `PROVIDER_ID`, `BEARER_TOKEN` | Explicit source label and caller-owned held credential; do not print the token |
| `CHAIN_ID`, `NETWORK_ALIAS` | Caller-qualified EVM chain number and label |
| `SOURCE_ASSET`, `DESTINATION_ASSET` | Exact ERC20 address or the literal `native` for an explicit native identity |
| `AMOUNT` | Exact source raw units |
| `FROM`, `ORIGIN`, `RECIPIENT` | Explicit review addresses |
| `ROUTER`, `SPENDER` | Independently qualified expected contract identities |
| `NATIVE_VALUE`, `OUTPUT_FLOOR` | Exact reviewed native wei value and destination raw-unit floor |
| `SLIPPAGE_BPS`, `DISABLE_ESTIMATE` | Explicit percentage choice and `true` / `false` estimation policy |

```sh
cargo test --offline --locked --no-default-features --features oneinch-http --test oneinch_live -- --ignored --nocapture
```

The harness selects no secrets or actors, makes all four actual Rust API calls and typed roundtrips, and constructs only an unsigned immutable handoff review. Its HTTP limits are two MiB, five-second connect and twenty-second total deadlines with two safe-read retries. It neither approves nor signs nor submits.
