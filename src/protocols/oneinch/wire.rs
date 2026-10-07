// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::{
        Address, ExactDecimal, NetworkId,
        evm::{Data, Quantity},
        oneinch::{
            Asset, AssetKind, Context, Expiry, Hop, Limits, LiquiditySource, LiquiditySources,
            ProtocolId, ProtocolShare, Quote, QuoteData, QuoteRequest, RouteGraph, SourceText,
            SourceTransaction, SourceTransactionData, Spender, StateOverrides, Token, TokenSwaps,
        },
    },
    error::{Error, ProviderError},
};
use serde::{
    Deserialize,
    de::{Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{fmt, marker::PhantomData};

pub(super) const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
pub(super) fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::Provider(ProviderError::ResponseTooLarge));
    }
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
struct List<T, const MAX: usize>(Vec<T>);
impl<'de, T: Deserialize<'de>, const MAX: usize> Deserialize<'de> for List<T, MAX> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct B<T, const MAX: usize>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const MAX: usize> Visitor<'de> for B<T, MAX> {
            type Value = List<T, MAX>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded provider collection")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut v = Vec::new();
                while let Some(next) = a.next_element()? {
                    if v.len() == MAX {
                        return Err(serde::de::Error::custom("provider collection bound"));
                    }
                    v.push(next);
                }
                Ok(List(v))
            }
        }
        d.deserialize_seq(B::<T, MAX>(PhantomData))
    }
}
#[derive(Deserialize)]
struct Number(Box<RawValue>);
impl Number {
    fn decimal(self) -> Result<ExactDecimal, Error> {
        ExactDecimal::parse(self.0.get()).map_err(|_| invalid())
    }
    fn quantity(self) -> Result<Quantity, Error> {
        let n = self.decimal()?;
        if n.is_negative() || !n.is_integer() {
            return Err(invalid());
        }
        Quantity::from_decimal(&n.canonical()).map_err(|_| invalid())
    }
}
#[derive(Default)]
struct Overrides(Option<StateOverrides>);
impl<'de> Deserialize<'de> for Overrides {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> Visitor<'de> for Object {
            type Value = bool;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an override object or null")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<bool, M::Error> {
                let mut nonempty = false;
                while m.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {
                    nonempty = true;
                }
                Ok(nonempty)
            }
        }
        let raw = Box::<RawValue>::deserialize(d)?;
        if raw.get() == "null" {
            return Ok(Self(Some(StateOverrides::Null)));
        }
        let mut parser = serde_json::Deserializer::from_str(raw.get());
        let present = serde::Deserializer::deserialize_map(&mut parser, Object)
            .map_err(serde::de::Error::custom)?;
        Ok(Self(Some(if present {
            StateOverrides::PresentUninterpreted
        } else {
            StateOverrides::Empty
        })))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenFields {
    address: String,
    symbol: String,
    name: String,
    decimals: u8,
    #[serde(default, rename = "isFoT")]
    fee_on_transfer: Option<bool>,
}
fn asset(value: &str, network: &NetworkId) -> Result<Asset, Error> {
    let a = Address::parse(value).map_err(|_| invalid())?;
    Asset::new(
        network.chain_id(),
        if a.bytes() == [0xee; 20] {
            AssetKind::Native
        } else {
            AssetKind::Erc20(a)
        },
    )
    .map_err(|_| invalid())
}
impl TokenFields {
    fn typed(self, network: &NetworkId) -> Result<Token, Error> {
        Ok(Token {
            asset: asset(&self.address, network)?,
            decimals: self.decimals,
            symbol: SourceText::new(&self.symbol).map_err(|_| invalid())?,
            name: SourceText::new(&self.name).map_err(|_| invalid())?,
            fee_on_transfer: self.fee_on_transfer,
        })
    }
}
#[derive(Deserialize)]
struct ShareFields {
    name: String,
    part: Number,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HopFields {
    part: Number,
    dst: String,
    from_token_id: u32,
    to_token_id: u32,
    protocols: List<ShareFields, 256>,
}
#[derive(Deserialize)]
struct GroupFields {
    token: String,
    hops: List<HopFields, 256>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteFields {
    src_token: TokenFields,
    dst_token: TokenFields,
    dst_amount: String,
    protocols: List<GroupFields, 64>,
    #[serde(default)]
    gas: Option<Number>,
    #[serde(default)]
    state_overrides: Overrides,
}
impl QuoteFields {
    fn typed(
        self,
        request: QuoteRequest,
        context: Context,
        limits: Limits,
    ) -> Result<Quote, Error> {
        let network = request.network();
        let mut groups = Vec::new();
        for g in self.protocols.0 {
            let mut hops = Vec::new();
            for h in g.hops.0 {
                let mut protocols = Vec::new();
                for p in h.protocols.0 {
                    protocols.push(ProtocolShare {
                        name: ProtocolId::new(&p.name).map_err(|_| invalid())?,
                        percent: p.part.decimal()?,
                    });
                }
                hops.push(Hop {
                    destination: asset(&h.dst, network)?,
                    from_token_id: h.from_token_id,
                    to_token_id: h.to_token_id,
                    percent: h.part.decimal()?,
                    protocols,
                });
            }
            groups.push(TokenSwaps {
                token: asset(&g.token, network)?,
                hops,
            });
        }
        let routes = RouteGraph::new(network.clone(), groups).map_err(|_| invalid())?;
        routes.check_limits(limits).map_err(|_| invalid())?;
        let data = QuoteData {
            source_token: self.src_token.typed(network)?,
            destination_token: self.dst_token.typed(network)?,
            destination_amount: Quantity::from_decimal(&self.dst_amount).map_err(|_| invalid())?,
            routes,
            estimated_gas: self.gas.map(Number::quantity).transpose()?,
            expiry: Expiry::Unreported,
            state_overrides: self.state_overrides.0.unwrap_or(StateOverrides::Unreported),
        };
        Quote::new(request, data, context).map_err(|_| invalid())
    }
}
pub(super) fn quote(
    bytes: &[u8],
    request: QuoteRequest,
    context: Context,
    limits: Limits,
) -> Result<Quote, Error> {
    decode::<QuoteFields>(bytes)?.typed(request, context, limits)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransactionFields {
    from: String,
    to: String,
    data: String,
    value: String,
    gas_price: String,
    gas: Number,
    #[serde(default)]
    gas_used: Option<Number>,
    #[serde(default)]
    access_list: Option<List<IgnoredAny, 256>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwapFields {
    src_token: TokenFields,
    dst_token: TokenFields,
    dst_amount: String,
    protocols: List<GroupFields, 64>,
    #[serde(default)]
    gas: Option<Number>,
    #[serde(default)]
    state_overrides: Overrides,
    tx: TransactionFields,
}
pub(super) fn swap(
    bytes: &[u8],
    request: QuoteRequest,
    context: Context,
    limits: Limits,
) -> Result<(Quote, SourceTransaction), Error> {
    let v: SwapFields = decode(bytes)?;
    if v.tx
        .access_list
        .is_some_and(|entries| !entries.0.is_empty())
    {
        return Err(invalid());
    }
    let network = request.network().clone();
    let quote = QuoteFields {
        src_token: v.src_token,
        dst_token: v.dst_token,
        dst_amount: v.dst_amount,
        protocols: v.protocols,
        gas: v.gas,
        state_overrides: v.state_overrides,
    }
    .typed(request, context, limits)?;
    let data = SourceTransactionData {
        from: Address::parse(&v.tx.from).map_err(|_| invalid())?,
        to: Address::parse(&v.tx.to).map_err(|_| invalid())?,
        data: Data::parse(&v.tx.data).map_err(|_| invalid())?,
        value: Quantity::from_decimal(&v.tx.value).map_err(|_| invalid())?,
        gas_price: Quantity::from_decimal(&v.tx.gas_price).map_err(|_| invalid())?,
        gas: v.tx.gas.quantity()?,
        gas_used: v.tx.gas_used.map(Number::quantity).transpose()?,
    };
    Ok((
        quote,
        SourceTransaction::new(network, data).map_err(|_| invalid())?,
    ))
}
#[derive(Deserialize)]
struct SourcesFields {
    protocols: List<LiquidityFields, 512>,
}
#[derive(Deserialize)]
struct LiquidityFields {
    id: String,
    title: String,
    img: String,
    img_color: String,
}
pub(super) fn sources(
    bytes: &[u8],
    context: Context,
    limits: Limits,
) -> Result<LiquiditySources, Error> {
    let v: SourcesFields = decode(bytes)?;
    if v.protocols.0.len() > usize::from(limits.liquidity_sources()) {
        return Err(invalid());
    }
    let items = v
        .protocols
        .0
        .into_iter()
        .map(|v| {
            Ok(LiquiditySource {
                id: ProtocolId::new(&v.id).map_err(|_| invalid())?,
                title: SourceText::new(&v.title).map_err(|_| invalid())?,
                image: SourceText::new(&v.img).map_err(|_| invalid())?,
                image_color: SourceText::new(&v.img_color).map_err(|_| invalid())?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    LiquiditySources::new(context, items).map_err(|_| invalid())
}
#[derive(Deserialize)]
struct SpenderFields {
    address: String,
}
pub(super) fn spender(bytes: &[u8], context: Context) -> Result<Spender, Error> {
    let v: SpenderFields = decode(bytes)?;
    Spender::new(context, Address::parse(&v.address).map_err(|_| invalid())?).map_err(|_| invalid())
}
