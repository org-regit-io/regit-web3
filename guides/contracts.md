# Operation contracts

[Overview](../README.md) · [Catalogue](catalogue.md) · [Contracts](contracts.md) · [Features](features.md) · [Qualification](qualification.md) · [Development](development.md)

Exact values and typed observations retain each family’s actual network, ledger and source semantics. Preparation, signed-content verification and submission remain separate operations.

## Core values and wallet contracts

| Area | Contract |
| --- | --- |
| Exact values | `Amount` retains unsigned 256-bit base units and explicit optional decimals; `ExactDecimal` retains bounded signed decimals. Exact numeric values serialize as strings; no floating-point transaction amounts |
| Validation | Constructors and deserialization enforce matching identity, precision, schema and observation invariants. Typed errors use fixed diagnostics |
| Wallet handoff | Immutable family-specific network/intent/unsigned snapshots, bounded caller-generated IDs and read-only review. Correlation is checked before trusted signed-content verification; confirmed output has no unchecked constructor or deserialization path. Custom snapshots and verifiers must honor their documented contracts |

## Chain records and operations

| Area | Contract |
| --- | --- |
| EVM identity/context | Canonical hexadecimal identities and exact chain IDs; native/ERC-20 state retains the captured canonical block hash. Transactions/receipts retain actual nullable inclusion, top-level execution outcomes and retrieval/source facts; JSON identity/signatures remain source-reported |
| Litecoin/Dogecoin identity/context | Family-qualified addresses and full main/test/regression genesis identities, exact litoshis/koinu and signed mempool deltas. Indexed history retains complete boundary blocks and explicit continuation uncertainty; transaction bytes remain opaque source data |
| Bitcoin Cash identity/context | Explicit address namespace, full genesis and fork checkpoint; exact satoshis, signed unconfirmed delta and CashToken quantities/NFT metadata. Source height zero/null remain distinct; opaque raw bytes have a computed ID, with verbose facts retained separately |
| Solana identity/context | Canonical base58 identities and full genesis network identity; maintained legacy/v0/v1 byte codecs. RPC controls, evaluation slots, inclusion slots and last-valid block heights remain distinct; structural validation does not verify signatures or finality |
| Bitcoin identity/context | Network-qualified addresses and full standard genesis hashes; exact confirmed satoshis and a separate signed mempool delta; bounded history retains its cursor and completeness limits. Canonical transaction bytes retain computed txid/wtxid, exact inputs/outputs/scripts/witness and weight; indexed previous outputs, nullable fee and inclusion remain separate source facts |
| Cardano identity/context | Payment/stake addresses, validated Byron CRC and nested encoding, explicit network tag/magic, and policy/raw-name asset identities; exact ADA and native-asset quantities retain declared precision. UTxO pages retain page/count/order and output origin separately from evaluation context |
| XRPL identity/context | Classic/X-address validation, network/tag/currency identities, exact XRP drops and issued decimals; actual ledger attribution, signed trustline balances and source-reported fee suggestions. Binary transaction IDs are recomputed; indexed inclusion and JSON execution outcomes remain separate |
| TON identity/context | CRC-checked friendly/raw addresses, full zero-state identity, exact nanotons and extra-currency IDs. Bounded maintained BOC hashes/transaction decoding, linked account history and actual masterchain evaluation; decoded transaction fees and reported aggregates remain separate |
| THORChain identity/context | Exact Cosmos chain ID and separate account prefix; full chain/symbol and distinct layer-one/synthetic/trade/secured representations. Protocol quantities use 1e8 units; native dust and gas retain their own units. Source stages and per-chain heights do not independently prove external inclusion |
| THORChain quotes | Requested input identity/amount remain supplied query facts. The source can expand asset identifiers and does not report its resolved input; the HTTP backend retains typed unreported resolution. Full output fee identity, fee totals, expiry and optional settings are checked |
| EVM HTTP state reads | Verify `eth_chainId` at establishment and before each read; resolve once; use EIP-1898 `blockHash` with `requireCanonical: true`; retries retain exact address/calldata/hash. ERC-20 name/symbol/decimals remain individually optional, without an assumed precision |
| EVM execution and preparation | Explicit sender/nonce/gas/value/fees/access lists; canonical nonce/call reads and Geth-compatible hash gas-estimation extension with no height fallback. Fees are separately sourced. Canonical legacy/type1/type2 unsigned signing bytes and digest preserve exact intent; no signer or inferred defaults |
| EVM signed submission | Canonical replay-protected legacy/type1/type2 envelope and computed hash; structural checks do not verify signatures, recovered sender or intent. One explicit write after chain preflight, no retry; matching node hash is acknowledgment only, unresolved post-dispatch outcomes remain potentially submitted |
| EVM HTTP transaction reads | Typed legacy/type1–4 source fields and bounded receipts/logs; explicit absence, pending/included state and failed/succeeded/unknown execution stay distinct. Sequential status reads validate matching facts without claiming an atomic snapshot |
| Litecoin/Dogecoin HTTP reads | Explicit documented mainnet BlockCypher bases; verify full genesis/chain name at establishment and before every operation. Complete bounded indexed transaction arrays, source-height history cursors and fee preferences per 1000 serialized bytes; no computed raw identity, consensus/signature proof or vbyte guarantee |
| Bitcoin Cash Electrum reads | Certified TLS with caller-supplied DER roots and explicit host/server name; protocol 1.6, token support, full genesis and fork headers checked before every read. Balance/filter, complete height-interval history, exact BCH/1000-byte fees, transaction/status and bounded UTXOs; one total budget without HTTP selection |
| Solana HTTP operations | Verify the full genesis hash at establishment and before each read; return exact SOL lamports, decoded present/absent accounts, or SPL raw units with mint/program/owner/state and reported decimals. Preserve requested commitment/minimum slot separately from the actual slot; ignore scaled UI amounts. Transaction/status, hash/height, fee and simulation use method-specific controls and one deadline; one-shot submission sets node retries to zero and preserves ambiguous post-dispatch outcomes |
| Bitcoin Esplora reads | Verify full genesis at establishment and before each read; retain exact balances, bounded history, fee estimates and status. Full retrieval compares canonical raw bytes with indexed immutable fields under one deadline; structural/money checks do not execute scripts, validate signatures or prove inclusion |
| Cardano Blockfrost reads | Verify expected genesis network magic before every operation; retain exact indexed balances, bounded address/asset/reward/UTxO pages, network/epoch/staking facts and original CBOR transaction/status data. Original body hashes, declared fees, failed-script collateral and source paid fees remain distinct; missing data is not a zero balance |
| Cardano preparation and submission | Pure Conway key-spend ADA/native-asset payments retain selected inputs, every output/change, fee, absolute validity slots and key-witness count. Exact serialized-size fee/minimum-ADA checks use explicit protocol parameters. Separate externally signed, body/key/size-bound raw CBOR is dispatched once; structural checks do not verify signatures and acknowledgment does not establish execution |
| XRPL HTTP reads | Verify the expected network; resolve one validated ledger hash for account/trustline reads. History uses an explicit bounded ledger range; transaction bytes retain their computed identity and source metadata. Fees retain open-ledger context; pending/range records do not invent a ledger hash |
| XRPL signed submission | Explicit caller-supplied bytes and network; one write attempt after network preflight. Echoed bytes and computed identity are checked; preliminary handling never establishes validated execution. Unresolved failures after dispatch retain a potentially submitted outcome |
| TON HTTP operations | Full zero-state checked before every operation; account sequence frozen across retries with full block correlation. Explicit bounded history/message scans, fee-body estimates and client-local request spacing share one deadline. One-shot submission acknowledgment is not execution; unresolved post-dispatch failures remain potentially submitted |
| THORChain HTTP reads | Explicit Cosmos network/account-prefix/API configuration; network verification before each operation under one total deadline. Complete bounded collections fail without truncation; quotes expire without implicit refresh and separate reads have no common snapshot claim |

## Protocol records and preparation

| Area | Contract |
| --- | --- |
| 1inch Classic Swap | Exact EVM assets/units and v6.1 graphs; reported protocol IDs match caller allowlists/exclusions. Separate spender and fresh swap reads retain the original selection and every explicit review setting. Strict ordinary envelopes retain source gas suggestions; calldata intent and estimation overrides remain unverified, with no invented expiry/block/genesis or atomic snapshot |
| Rubic direct routes | Caller-qualified families/aliases, exact gross/source/leg amounts, fees and bounded alternatives. Original selection and caller constraints remain beside fresh estimates, optional token metadata and unsigned EVM/Solana payloads; source-expanded filters stay source facts. Structural encoding does not prove swap intent, source progress does not prove finality, and expiry remains unreported |
| LI.FI source contracts | Explicit chain-family catalogue, exact assets/units/slippage/costs and bounded alternatives. Preparation retains the old selection beside fresh source estimates/payload; encoding checks do not prove signed intent. Status distinguishes chain hashes, provider transfer IDs and refunds; quote expiry remains unreported |
| Jupiter V2 | Quote-only `/order` and fresh `/build` retain independent source-selected routes. Maintained V0 compilation uses exact source instructions/lookup tables and explicit caller CU/signer/other-instruction placement; these facts do not verify swap semantics. Separate fee/simulation observations match the exact unsigned bytes, retain actual slots and share one total deadline |
| Uniswap V3 operations | Explicit deployment/path/input/call settings; QuoterV2 quotes and at most 16 supplied paths retain one canonical hash. Exact raw output, per-hop prices/ticks and source gas estimates; reverted candidates remain distinct. Unsigned Universal Router 2.1.2 preparation retains literal recipient, floor-rounded slippage, explicit deadline and separate ERC-20/Permit2 allowance requirements |

## Provider and indexed data

| Area | Contract |
| --- | --- |
| Market data | Provider listing IDs and explicit currencies/units; exact nullable prices, TVL/volume/fee/revenue values, signed APY percentages and supplied series timestamps; source and retrieval facts without invented ledger anchors |
| Bitcoin mempool data | Exact satoshi totals and sat/vB suggestions; individual histogram bins, at most ten recent arrivals and explicitly bounded full ID lists. Fractional weight/4 sizes retain quarter-byte increments; separate reads do not establish an atomic snapshot |
| Provider HTTP reads | Explicit CoinGecko anonymous/Demo/Pro configuration; independent DefiLlama TVL/yields/stablecoin endpoints. Exact decimal decoding, explicit collection/page limits, no silent truncation or source fallback |
| Helius HTTP reads | Full Solana genesis verified before every read under one deadline. DAS index progress, parsed inclusion, parser outcomes and execution remain separate source facts. Ordered parse batches retain duplicate inputs/item failures; owner/history continuations preserve every original predicate and reject another client’s handle before dispatch. Source interpretations and cached valuations do not prove canonical transaction bytes or finality |
| mempool.space HTTP reads | Explicit expected full Bitcoin genesis and API base; one total deadline for setup/reads/retries. Canonical transaction retrieval/status reuse compatible Esplora contracts and retain the supplied provider label |

## Shared transport

| Area | Contract |
| --- | --- |
| Optional transport | Explicit configuration, verified TLS, bounded bodies/retries and one total operation deadline; endpoint credentials and raw provider messages excluded from diagnostics |

EVM HTTP native reads retain configured precision, finality `unknown` and confirmations `null`; requested tags do not establish either. Bitcoin and Cardano indexed reads do not promise a hash-selected snapshot or lasting finality. Each family retains its own ledger and source semantics. Pure observation construction validates supplied records and does not independently verify a remote source.
