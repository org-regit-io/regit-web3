# THORChain fixtures

Public source samples retrieved on 2026-10-07 from the mainnet THORNode base documented at [Connecting to THORChain](https://dev.thorchain.org/concepts/connecting-to-thorchain.html): `https://gateway.liquify.com/chain/thorchain_api`. These responses are point-in-time source facts, without a historical hash anchor.

| Fixture | Source route / qualification detail |
| --- | --- |
| `nodeinfo.json` | `/cosmos/base/tendermint/v1beta1/node_info`, narrowed to expected network and source version |
| `balance.json` | `/cosmos/bank/v1beta1/balances/thor1dheycdevq39qlkxs2a6wuuzyn4aqxhve4qxtxt/by_denom?denom=rune` |
| `pool.json`, `pools.json` | `/thorchain/pools`, BTC.BTC record; ownership units exceed 2^53 |
| `network.json` | `/thorchain/network` |
| `inbounds.json`, `lastblocks.json` | `/thorchain/inbound_addresses`, `/thorchain/lastblock`, BTC and ETH records |
| `quote.json` | `/thorchain/quote/swap?from_asset=BTC.BTC&to_asset=ETH.ETH&amount=100000000`, expiry is historical; tests supply explicit observation time or a future local fixture expiry |
| `internal_status.json` | `/thorchain/tx/status/A487E167659E0FC3681B60D1AEABFAB180D6C6917F5200AD48246D313F3469AC`, THOR-held trade assets, null gas, internal blank outbound ID |
| `external_status.json` | `/thorchain/tx/status/A3F81568387CD3880AED812780799E8F6D3F970E071F7B1861B581B20399F21F`, ETH.USDT to BTC.BTC with separate observed and planned outbound facts |

Status inputs were publicly sourced from the documented [Midgard actions API](https://gateway.liquify.com/chain/thorchain_midgard/v2/doc), using explicit bounded action pages; the crate does not discover or substitute transaction inputs. A 100-item discovery request returned HTTP 500; a documented 10-item BTC.BTC-filtered request succeeded.

Wire contracts and units were checked against current primary [THORNode OpenAPI](https://gitlab.com/thorchain/thornode/-/blob/develop/openapi/openapi.yaml), [query source](https://gitlab.com/thorchain/thornode/-/blob/develop/x/thorchain/querier.go), [quote source](https://gitlab.com/thorchain/thornode/-/blob/develop/x/thorchain/querier_quotes.go), [asset source](https://gitlab.com/thorchain/thornode/-/blob/develop/common/asset.go) and [transaction ID source](https://gitlab.com/thorchain/thornode/-/blob/develop/common/tx.go). Foreign addresses and signatures remain source-qualified; they are not cryptographically or consensually verified by these fixtures.
