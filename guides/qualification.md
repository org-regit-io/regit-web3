# Qualification

[Overview](../README.md) · [Primitives](primitives.md) · [Catalogue](catalogue.md) · [Contracts](contracts.md) · [Features](features.md) · [Qualification](qualification.md) · [Development](development.md)

Qualification applies to the recorded operation, network, provider plan and query variant. Fixtures, feature compilation and live reads provide different evidence; none establish every variant or funded execution.

## Deterministic and target evidence

| Evidence | Qualified behavior / limits |
| --- | --- |
| Native macOS | Strict gate passed: 753 behavior tests across 85 test binaries, 24 opt-in live tests skipped, eight doctests, formatting, Clippy, documentation and dependency policy |
| Core composition | Checked exact arithmetic/rounding, qualified EVM/Solana mapping keys and ordered partial outcomes have deterministic fixture proof. Synthetic identities and observations do not qualify remote assets, listings or prices |
| JavaScript WASM in Node | 355 assertions across 33 pure integration targets execute exact values/arithmetic, explicit mappings, bounded collections, family identities, encodings, preparation and typed capabilities across the catalogue |
| Browser HTTP fixtures | Twelve headless Chrome cases execute actual Request/Response, streams, timers and abort signals with deterministic Fetch fixtures. EVM RPC and typed CoinGecko GET cover bounded bodies, explicit headers/query parameters, frozen-hash retries, shared deadlines, cancellation, timer cleanup and one-shot unknown submission outcomes. Fake credentials only; no provider browser live calls |
| Feature boundaries | All twenty pure catalogue features and nineteen HTTP backends compile for JavaScript WASM. Empty defaults and pure selections exclude HTTP/runtime dependencies. Electrum-Cash TCP/TLS remains native |
| Wallet contracts | Generic preparation/review/external handoff and EVM/XRPL adapters have deterministic fixture proof. Returned signed content requires trusted caller-supplied verification; metadata correlation does not verify signed content |
| Remaining checks | Authenticated 1inch live data and current Linux/Windows qualification remain pending. Browser shared-behavior fixtures do not qualify every provider’s browser live access |

## Representative live reads and preparation

| Integration | Recorded proof | Qualification limits |
| --- | --- | --- |
| EVM | Native/ERC-20 balances, allowance/metadata, transactions/receipts/status and fee/nonce/call/estimate reads | Explicit submission has deterministic/loopback fixture proof; no funded live submission |
| Solana | Native/account/SPL reads and seven added read/fee/simulation operations on mainnet, including real v1 retrieval and successful unsigned native simulation | Legacy/v0 codecs, classic SPL preparation, local preparation and submission have fixture proof; no signed live submission |
| Cardano / Blockfrost | All seventeen indexed read/estimate methods and explicit unsigned review authenticated on mainnet at epoch 660/protocol 11 | Operations, ordinary payment preparation/handoff and one-shot submission retain deterministic/loopback fixture proof; no funded live submission |
| Bitcoin | Indexed reads and canonical full transaction retrieval | Source inclusion remains separate from independent script, signature or finality proof |
| Litecoin / Dogecoin | Five reads per family, including history continuation | Documented BlockCypher mainnet profiles; source/raw transaction limits remain in the [contracts](contracts.md) |
| Bitcoin Cash | Six Electrum-TLS reads | Native certificate-verified backend; source/fork/token limits remain in the [contracts](contracts.md) |
| XRPL | Six read methods | Explicit submission has deterministic/loopback fixture proof; no funded live submission |
| TON | Seven read/estimate methods, an outgoing-fee transaction and local unsigned review | Submission has fixture proof; no funded write |
| THORChain | Eight read/quote methods | Source progress does not independently prove external inclusion |
| Jupiter | V2 quote/build, unsigned handoff and exact-message fee/simulation on mainnet | Public example taker returned `AccountNotFound`; no swap-success proof |
| Uniswap | V3 quote and supplied-path comparison | Unsigned Universal Router 2.1.2 preparation and wallet handoff have fixture proof |
| 1inch | Four operations and unsigned review/handoff have 31 deterministic/loopback tests | Authenticated live data remains pending |
| LI.FI | Four methods and both transaction/provider-transfer status selectors on EVM | Other family payload encodings have fixture proof |
| Rubic | Five direct API-v2 methods and unsigned EVM handoff on native macOS | Supplied unrelated public hash returned `NotFound`; no executed-transfer proof |
| CoinGecko | Four operations | Explicit recorded API configuration and query variants |
| DefiLlama | Eight reader methods | Recorded endpoints and metric variants |
| Helius | Four authenticated mainnet reads, including duplicate parsed batches and successful history records | Continuation variants retain deterministic/loopback fixture proof |
| mempool.space | Six reads | Separate moving indexed observations, without an atomic snapshot |

Preparation and handoff do not sign or submit. No funded live submission was performed. Source identity, execution and finality claims remain subject to the family-specific [contracts](contracts.md). The [live harness guide](../examples/README.md) documents explicit inputs and opt-in execution.
