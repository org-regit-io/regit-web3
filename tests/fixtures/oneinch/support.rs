// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Synthetic exact Classic Swap v6.1 values, never live execution evidence.
use regit_web3::{
    domain::{
        Address, ChainId, ExactDecimal, NetworkId, Source, Timestamp,
        evm::{Data, Quantity},
        oneinch::{
            Asset, AssetKind, Context, Expiry, Hop, Method, PreparedSwap, ProtocolId,
            ProtocolShare, Quote, QuoteData, QuoteRequest, QuoteSettings, ReturnPolicy, RouteGraph,
            SourceText, SourceTransaction, SourceTransactionData, Spender, StateOverrides,
            SwapRequest, SwapSettings, Token, TokenSwaps,
        },
    },
    error::Error,
};
pub(crate) const SRC: &str = "0x1111111111111111111111111111111111111111";
pub(crate) const DST: &str = "0x2222222222222222222222222222222222222222";
pub(crate) const FROM: &str = "0x3333333333333333333333333333333333333333";
pub(crate) const ROUTER: &str = "0x4444444444444444444444444444444444444444";
pub(crate) const SPENDER: &str = "0x5555555555555555555555555555555555555555";
pub(crate) fn network() -> Result<NetworkId, Error> {
    NetworkId::new(ChainId::from(1_u64), "fixture-ethereum")
}
pub(crate) fn asset(text: &str) -> Result<Asset, Error> {
    Asset::new(
        ChainId::from(1_u64),
        AssetKind::Erc20(Address::parse(text)?),
    )
}
pub(crate) fn request() -> Result<QuoteRequest, Error> {
    QuoteRequest::new(
        network()?,
        asset(SRC)?,
        asset(DST)?,
        Quantity::from_decimal("9007199254740993")?,
        QuoteSettings::provider_defaults(),
    )
}
pub(crate) fn context(method: Method) -> Result<Context, Error> {
    Ok(Context {
        network: network()?,
        method,
        source: Source::new("fixture", "classic-v6.1", env!("CARGO_PKG_VERSION"))?,
        retrieved_at: Timestamp::from_unix_seconds(1_791_340_000),
    })
}
pub(crate) fn graph() -> Result<RouteGraph, Error> {
    RouteGraph::new(
        network()?,
        vec![TokenSwaps {
            token: asset(SRC)?,
            hops: vec![Hop {
                destination: asset(DST)?,
                from_token_id: 0,
                to_token_id: 77,
                percent: ExactDecimal::parse("100")?,
                protocols: vec![ProtocolShare {
                    name: ProtocolId::new("UNISWAP_V3")?,
                    percent: ExactDecimal::parse("33.000000000000000001")?,
                }],
            }],
        }],
    )
}
pub(crate) fn quote(method: Method, amount: u64) -> Result<Quote, Error> {
    let r = request()?;
    let token = |asset, name| -> Result<Token, Error> {
        Ok(Token {
            asset,
            decimals: 18,
            symbol: SourceText::new(name)?,
            name: SourceText::new(name)?,
            fee_on_transfer: None,
        })
    };
    Quote::new(
        r.clone(),
        QuoteData {
            source_token: token(r.source(), "SRC")?,
            destination_token: token(r.destination(), "DST")?,
            destination_amount: amount.into(),
            routes: graph()?,
            estimated_gas: Some(180_000_u64.into()),
            expiry: Expiry::Unreported,
            state_overrides: StateOverrides::Unreported,
        },
        context(method)?,
    )
}
pub(crate) fn settings() -> Result<SwapSettings, Error> {
    Ok(SwapSettings {
        from: Address::parse(FROM)?,
        origin: Address::from_bytes([0x66; 20]),
        recipient: Address::from_bytes([0x77; 20]),
        output_floor: 900_u64.into(),
        return_policy: ReturnPolicy::Minimum(900_u64.into()),
        router: Address::parse(ROUTER)?,
        spender: Address::parse(SPENDER)?,
        native_value: 0_u64.into(),
        disable_estimate: true,
    })
}
pub(crate) fn swap_request() -> Result<SwapRequest, Error> {
    SwapRequest::new(quote(Method::Quote, 1000)?, settings()?)
}
pub(crate) fn transaction() -> Result<SourceTransaction, Error> {
    SourceTransaction::new(
        network()?,
        SourceTransactionData {
            from: Address::parse(FROM)?,
            to: Address::parse(ROUTER)?,
            data: Data::parse("0x12345678deadbeef")?,
            value: 0_u64.into(),
            gas_price: Quantity::from_decimal("9007199254740993")?,
            gas: 200_000_u64.into(),
            gas_used: None,
        },
    )
}
pub(crate) fn spender() -> Result<Spender, Error> {
    Spender::new(context(Method::Spender)?, Address::parse(SPENDER)?)
}
pub(crate) fn prepared() -> Result<PreparedSwap, Error> {
    PreparedSwap::new(
        swap_request()?,
        quote(Method::Swap, 950)?,
        transaction()?,
        spender()?,
    )
}
