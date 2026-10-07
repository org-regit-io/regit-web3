<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Regit -->

Solana execution fixtures
=========================

| File | Source and retained facts |
| --- | --- |
| `mainnet_v1_transaction.json` | Keyless certified HTTPS `getTransaction` at `https://api.mainnet.solana.com`, signature `4ofUzrGeDGsRvg7oTzC133nSACkXMP9vagDw3RMyswa2AsAB9Vp9Vyh7EAY2WudZDjf43NJkN1KJLyo2mVYDvg5x`, confirmed/base64/max-supported-version 1. Source core 4.3.0; version 1, inclusion slot 454111150, source block time 1791346138, transaction index 1064, fee 5410 lamports. The record was discovered with `getSignaturesForAddress` for the publicly documented wrapped-SOL mint `So11111111111111111111111111111111111111112`. |
| `mainnet_v1_transaction.hex` | The same 1956 canonical transaction bytes, decoded from the source's base64 representation; first static account `AXmnRBrNtYYyyo82cLBBhnWJ7o1iqNLZbuEVpDB3V666`. This vector exercises the current v1 inline-account format and exceeds the legacy/v0 1232-byte bound. |
| `mainnet_status.json` | Independent point-in-time `getSignatureStatuses` response for that signature, with history search enabled; source evaluation slot 454111151, inclusion slot 454111150, one confirmation and confirmed classification. Later status can legitimately differ. |

The native/classic SPL, v0 lookup, malformed and one-shot submission fixtures
are constructed locally in Rust. Dummy nonzero signature bytes are structural
test values, not cryptographically valid signatures. Submission fixtures use
only loopback listeners; live qualification performs reads and unsigned simulation.
No fixture establishes funding, approval, signature verification, consensus,
historical hash pinning or atomicity across RPC responses.

Primary contracts:

- [Solana RPC transaction retrieval](https://solana.com/docs/rpc/http/gettransaction), [status](https://solana.com/docs/rpc/http/getsignaturestatuses), [fees](https://solana.com/docs/rpc/http/getfeeformessage), [simulation](https://solana.com/docs/rpc/http/simulatetransaction), [submission](https://solana.com/docs/rpc/http/sendtransaction).
- [Latest blockhash](https://solana.com/docs/rpc/http/getlatestblockhash), [validity](https://solana.com/docs/rpc/http/isblockhashvalid), [actual block height](https://solana.com/docs/rpc/http/getblockheight), [official cluster endpoints](https://solana.com/docs/references/clusters).
- Maintained released `solana-message` and `solana-transaction` 5.1.0 with `wincode` 0.6.2 exact decoding, canonical re-encoding and explicit allocation/byte limits. Their structural validation does not perform cryptographic signature verification.
- [System transfer implementation](https://github.com/anza-xyz/solana-sdk/blob/master/system-interface/src/instruction.rs) and [classic SPL Token TransferChecked data/account contract](https://github.com/solana-program/token/blob/main/interface/src/instruction.rs).
