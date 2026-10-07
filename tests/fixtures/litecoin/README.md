# Litecoin indexed source fixtures

Public, keyless BlockCypher mainnet reads, captured for deterministic tests on 2026-10-07. The exact reported heights, confirmations, quantities and times are historical fixture facts, not current network state or cryptographic proof.

- Primary provider contract: https://www.blockcypher.com/dev/bitcoin/ (supported chains, atomic units, TXRef boundary-block pagination, transaction input/output pagination).
- Primary family network/address parameters: https://github.com/litecoin-project/litecoin/blob/master/src/chainparams.cpp
- Transaction monetary widths/range: the same maintained Core repository's `src/amount.h`.
- Provider base: https://api.blockcypher.com/v1/ltc/main
- Genesis: `/blocks/0?limit=1`, independently matched to the full Core genesis hash.
- Public block-one transaction: `fa3906a4219078364372d0e2715f93e822edd0b47ce146c71ba7ba57179b50f6`, discovered through `/blocks/1?limit=1`.
- Transaction payload: `/txs/fa3906a4219078364372d0e2715f93e822edd0b47ce146c71ba7ba57179b50f6?includeHex=true&limit=2000`. Raw bytes are retained as opaque source data; no Bitcoin decoder, computed-ID, consensus, signature, MWEB or AuxPoW proof is claimed.
- Address: `LSdTvMHRm8sScqwCi6x9wzYQae8JeZhx6y`, supplied by that public transaction's actual output.
- Balance: `/addrs/LSdTvMHRm8sScqwCi6x9wzYQae8JeZhx6y/balance`.
- Nonempty history: `/addrs/LSdTvMHRm8sScqwCi6x9wzYQae8JeZhx6y?limit=10&includeScript=true`; `hasMore=true` is preserved. This page is not a complete address history.
- Fees: root chain resource, native atomic units per 1000 bytes. Bucket labels describe provider preferences, not a promised confirmation time.

Tests may mutate specific fields to exercise malformed data, complete boundary blocks, missing values and resource/deadline rules. Those synthetic variants are not live observations.
