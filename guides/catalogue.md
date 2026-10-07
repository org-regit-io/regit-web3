# Catalogue

[Overview](../README.md) · [Primitives](primitives.md) · [Catalogue](catalogue.md) · [Contracts](contracts.md) · [Features](features.md) · [Qualification](qualification.md) · [Development](development.md)

All twenty modules provide the operations described below. Qualification remains specific to recorded operations and providers; authenticated 1inch live data remains pending. See [qualification](qualification.md) for recorded platform checks and their source checkpoint.

## Shared primitives

These core APIs require no optional features and compose with the catalogue:

| API | Operations |
| --- | --- |
| `Amount`, `ExactDecimal` | Checked exact arithmetic, explicit rounding and decimal/base-unit conversion |
| `domain::asset_binding` | Bounded caller-declared full-asset-key mappings to provider-scoped market listings, with source/time and exact lookup |
| `domain::collections` | Bounded ordered typed outcomes, unique keys, exact lookup and supplied success/failure counts |

Examples and contracts are in the [primitive guide](primitives.md). Family-specific keys and observations select their own capability feature.

## Chains

| Chain family | Scope | Current implementation |
| --- | --- | --- |
| EVM | Native/ERC-20 balances, transactions/receipts, gas/fees, transfers and approvals | Native/ERC-20 reads, transaction/receipt/status, independent fees, canonical nonce/call/gas estimates, unsigned native/ERC-20 transfer/approval preparation and explicit one-shot signed submission implemented |
| Solana | SOL/SPL balances, accounts, transactions and transfers | SOL/account/SPL reads, canonical legacy/v0/v1 transactions and status, blockhash/height, exact message fees, unsigned simulation, ordinary native/classic SPL preparation and separate one-shot signed submission implemented |
| Cardano | Balances, UTxOs, assets, transactions, epoch/staking and preparation | Exact indexed reads, original-body-hash transaction/status records, ordinary Conway ADA/native-asset preparation, fresh-parameter fee estimation and separate one-shot signed submission implemented |
| Bitcoin | Validation, balance/history, fees and transactions/status | Pure family contracts and optional Esplora balances, bounded history, exact fee estimates, status and canonical full transaction retrieval implemented |
| Litecoin | Validation, balance/history, fees and supporting transaction data | Pure family identities and five optional BlockCypher mainnet reads implemented |
| Dogecoin | Validation, balance/history, fees and supporting transaction data | Pure family identities and five optional BlockCypher mainnet reads implemented |
| Bitcoin Cash | Validation, balance/history, fees and supporting transaction data | CashAddr/legacy validation, genesis/fork-qualified identities and six optional certificate-verified Electrum-Cash TLS reads implemented |
| XRPL | XRP/issued balances, history, trustlines and transfers | Address/amount identities, XRP balances, ledger-bound trustline pages, fees, bounded account history, binary transactions/status, ordinary sequence-based XRP/issued Payment JSON preparation and explicit one-shot signed submission implemented |
| TON | Validation, balance/history, network data and transfers | Standard address/zero-state/BOC validation, seven TON Center read/estimate methods, unsigned internal-message preparation and separate one-shot external-message submission implemented |
| THORChain | RUNE balances, pools, network data, quotes and cross-chain state | RUNE balances, individual/complete layer-one pool reads, network values, swap quotes, inbound vaults, chain heights and transaction progress implemented |

## Protocols

| Protocol | Scope | Current implementation |
| --- | --- | --- |
| Jupiter | Solana aggregation quotes/routes and swap preparation | Swap API V2 quote-only source selection, fresh Metis V0 instruction builds, immutable unsigned preparation/handoff and exact-message Solana fee/simulation implemented |
| Uniswap | Direct EVM DEX quotes and swap preparation | V3 QuoterV2 exact-input quotes, bounded supplied-path comparison and unsigned Universal Router 2.1.2 swap preparation implemented |
| 1inch | Aggregated EVM quotes/routes and swap preparation | Classic Swap v6.1 exact-input quotes, bounded liquidity catalogue, spender reads and fresh unsigned preparation/handoff implemented; authenticated live qualification pending |
| LI.FI | Cross-chain quotes/routes, execution preparation and status | Exact-input quotes, bounded route alternatives, immutable selected-step preparation, and transaction/provider-transfer status implemented |
| Rubic | Cross-chain quotes/routes, unsigned preparation and progress | API-v2 chain catalogue, direct all/best quotes, fresh unsigned EVM/Solana preparation/handoff and extended status implemented |

## Providers

| Provider | Scope | Current implementation |
| --- | --- | --- |
| CoinGecko | Asset search, prices, markets and historical market data | Search, exact ID/currency prices, explicit markets pages and historical price/capitalization/volume charts implemented |
| DefiLlama | Protocol TVL, yields, stablecoins and DeFi analytics | Protocol/chain TVL and history, yields/history, stablecoins/history and DEX/fees/revenue/holders-revenue analytics implemented |
| Helius | Solana assets, parsed transactions and address history | Four DAS/current Parsed Events reads with exact source records, bounded pages and immutable query continuations implemented; four methods have representative authenticated mainnet qualification |
| Blockfrost | Indexed Cardano network, asset and supporting account data | Seventeen read/estimate methods covering balances, UTxOs, address/asset/reward pages, metadata, network/epoch/parameters/staking and transactions; separate raw-CBOR submission implemented |
| mempool.space | Bitcoin mempool, fees and transaction data | Backlog summaries, recent arrivals, bounded full transaction-ID lists, exact recommended fees, canonical transaction retrieval and inclusion status implemented |

## Wallet extension contracts

Generic typed wallet preparation, review and external signing-handoff contracts are implemented, with EVM canonical transaction, Solana canonical message, Jupiter compiled V0, XRPL Payment JSON, Uniswap router-field, Cardano ordinary-payment, TON internal-message, 1inch source-envelope and Rubic direct-swap preparations. Returned signed content requires a caller-supplied verifier against the original preparation; matching echoed metadata alone does not verify it. Concrete cryptographic verification, signing, custody and connectors remain separate extensions. Callers own approval and replay policy; preparation and handoff do not submit.
