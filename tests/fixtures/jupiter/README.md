# Jupiter Swap API V2 fixtures

| File | Provenance and use |
| --- | --- |
| `order_v2.json` | Public keyless `/swap/v2/order` discovery on 2026-10-07, wrapped SOL→USDC, exact input 1,000,000 raw units, slippage 100 bps, `excludeRouters=jupiterz`, taker omitted. `transaction` and `taker` are explicitly null. Source Dflow serial percentages are preserved rather than summed. |
| `build_v2.json` | Public keyless `/swap/v2/build` discovery on 2026-10-07, the same pair/input with the documented public example taker, explicit V0, 64 accounts, 150 expiry slots and CU-price percentile 5000. Retains full lookup-table order and legitimate duplicate entries, source threshold rounding and actual hash/last-valid height/fetch time. |
| `live_inputs.json` | Explicit inputs for `jupiter_live`, including public API/RPC endpoints and non-secret source labels. Settings include caller CU limit/price ceiling, additional signer list and explicit source-other-instruction placement. No key, signer or submission input exists. |

| Primary reference | Contract checked |
| --- | --- |
| [V2 OpenAPI](https://github.com/jup-ag/docs/blob/main/openapi-spec/swap/v2/swap.yaml) | `/order` quote-only null transaction; `/build` fresh Metis quote/instructions, V0/V1 distinction, exact source amount fields, source blockhash/height/time and optional fee facts. |
| [Current developer index](https://github.com/jup-ag/docs/blob/main/llms.txt) | Documented keyless access at 0.5 requests/second and optional caller-supplied `x-api-key`. |
| [Build workflow](https://github.com/jup-ag/docs/blob/main/swap/build/index.mdx) | V0 instruction compilation with caller compute-unit limit; source compute-budget instructions supply CU price. |
| [Common instruction order](https://github.com/jup-ag/docs/blob/main/swap/build/common-instructions.mdx) | Compute→setup→swap→cleanup group order. Source `otherInstructions` placement is caller-selected and retained in review without a source-order claim. |
| [V1 migration](https://github.com/jup-ag/docs/blob/main/swap/migration/metis-to-build.mdx) | Fresh V2 build and canonical route basis-point shares, alongside exact compatibility percentages. |
| [Solana SDK message source](https://docs.rs/solana-message/5.1.0/solana_message/v0/struct.Message.html) | Maintained V0 `try_compile`, ordered lookup-table accounts and canonical family message/transaction validation. |
| [Compute-budget source](https://docs.rs/solana-compute-budget-interface/3.1.0/src/solana_compute_budget_interface/lib.rs.html) | Fixed tag-2/u32-LE CU limit and tag-3/u64-LE micro-lamport price encodings. Independent test vectors check these two encodings. |

The live harness paces quote/build calls by 2.1 seconds. It exercises the Rust public quote/build APIs, local unsigned preparation/handoff and exact-message Solana fee/simulation operations, then checks typed serde roundtrips and source/version/time attribution. RPC genesis verification is separate from Jupiter's declared mainnet identity. Quote and build are independent source selections. The documented public taker can produce source `AccountNotFound`; that remains a qualified simulation response, without execution-success proof. No funded write is performed.
