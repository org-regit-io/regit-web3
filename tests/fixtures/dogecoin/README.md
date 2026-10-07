# Dogecoin indexed source fixtures

Public, keyless BlockCypher mainnet reads, captured for deterministic tests on 2026-10-07. The exact reported heights, confirmations, quantities and times are historical fixture facts, not current network state or cryptographic proof.

- Primary provider contract: https://www.blockcypher.com/dev/bitcoin/ (supported chains, atomic units, TXRef boundary-block pagination, transaction input/output pagination).
- Primary family network/address parameters: https://github.com/dogecoin/dogecoin/blob/master/src/chainparams.cpp
- Transaction monetary widths/range: the same maintained Core repository's `src/amount.h`.
- Provider base: https://api.blockcypher.com/v1/doge/main
- Genesis: `/blocks/0?limit=1`, independently matched to the full Core genesis hash. The provider supplies no Dogecoin genesis transaction; none is fabricated.
- Public block-one transaction: `5f7e779f7600f54e528686e91d5891f3ae226ee907f461692519e549105f521c`, discovered through `/blocks/1?limit=1`.
- Transaction payload: `/txs/5f7e779f7600f54e528686e91d5891f3ae226ee907f461692519e549105f521c?includeHex=true&limit=2000`. Raw bytes are retained as opaque source data; no Bitcoin decoder, computed-ID, consensus, signature, MWEB or AuxPoW proof is claimed.
- Address: `DLAznsPDLDRgsVcTFWRMYMG5uH6GddDtv8`, supplied by that public transaction's actual output.
- Balance: `/addrs/DLAznsPDLDRgsVcTFWRMYMG5uH6GddDtv8/balance`.
- Nonempty history: `/addrs/DLAznsPDLDRgsVcTFWRMYMG5uH6GddDtv8?limit=10&includeScript=true`; `hasMore=true` is preserved. This page is not a complete address history.
- Fees: root chain resource, native atomic units per 1000 bytes. Bucket labels describe provider preferences, not a promised confirmation time.

Tests may mutate specific fields to exercise malformed data, complete boundary blocks, missing values and resource/deadline rules. Those synthetic variants are not live observations.
