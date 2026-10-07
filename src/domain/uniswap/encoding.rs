// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Bounded explicit ABI for `QuoterV2` and Universal Router 2.1.2 only.

use super::{SwapIntent, V3Path};
#[cfg(feature = "uniswap-http")]
use super::{V3Quote, V3QuoteData, V3QuoteRequest};
use crate::{
    domain::evm::{Data, Quantity, U256},
    error::Error,
};
use sha3::{Digest, Keccak256};

fn selector(signature: &[u8]) -> Vec<u8> {
    Keccak256::digest(signature)[..4].to_vec()
}
fn word(out: &mut Vec<u8>, value: U256) {
    out.extend(value.to_be_bytes::<32>());
}
fn dynamic_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + bytes.len().div_ceil(32) * 32);
    word(&mut out, U256::from(bytes.len()));
    out.extend(bytes);
    out.resize(32 + bytes.len().div_ceil(32) * 32, 0);
    out
}
pub(super) fn quote_call(path: &V3Path, amount: Quantity) -> Result<Data, Error> {
    let mut out = selector(b"quoteExactInput(bytes,uint256)");
    word(&mut out, U256::from(64));
    word(&mut out, amount.value());
    out.extend(dynamic_bytes(path.encoded()?.bytes()));
    Data::new(out)
}
pub(super) fn swap_call(intent: &SwapIntent) -> Result<Data, Error> {
    let data = intent.data();
    let quote = data.quote.value();
    let path_tail = dynamic_bytes(quote.data().request.data().path.encoded()?.bytes());
    let mut command = vec![0; 12];
    command.extend(data.recipient.bytes());
    word(&mut command, quote.data().request.data().amount_in.value());
    word(&mut command, intent.minimum_output().value());
    word(&mut command, U256::from(192));
    word(&mut command, U256::from(1)); // payerIsUser; Permit2 authorization is caller-owned.
    word(&mut command, U256::from(192 + path_tail.len()));
    command.extend(path_tail);
    word(&mut command, U256::ZERO); // Empty minHopPriceX36, aggregate minimum only.

    let commands = dynamic_bytes(&[0x00]); // V3_SWAP_EXACT_IN, allow-revert bit absent.
    let mut out = selector(b"execute(bytes,bytes[],uint256)");
    word(&mut out, U256::from(96));
    word(&mut out, U256::from(96 + commands.len()));
    word(&mut out, U256::from(data.deadline.unix_seconds()));
    out.extend(commands);
    word(&mut out, U256::from(1)); // inputs.length
    word(&mut out, U256::from(32)); // First bytes offset relative to array elements head.
    out.extend(dynamic_bytes(&command));
    Data::new(out)
}

/// Decodes the exact canonical four-field `QuoterV2` output for this bounded path.
/// Offsets, array lengths, widths, padding and trailing bytes cannot be normalized.
#[cfg(feature = "uniswap-http")]
pub(crate) fn decode_quote(request: V3QuoteRequest, bytes: &[u8]) -> Result<V3Quote, Error> {
    let hops = request.data().path.hops();
    if bytes.len() != 192 + 64 * hops {
        return Err(invalid());
    }
    let read = |offset: usize| U256::from_be_slice(&bytes[offset..offset + 32]);
    if read(32) != U256::from(128)
        || read(64) != U256::from(160 + 32 * hops)
        || read(128) != U256::from(hops)
        || read(160 + 32 * hops) != U256::from(hops)
    {
        return Err(invalid());
    }
    let mut prices = Vec::with_capacity(hops);
    let mut ticks = Vec::with_capacity(hops);
    for index in 0..hops {
        let price = read(160 + 32 * index);
        let crossed = read(192 + 32 * hops + 32 * index);
        if price.bit_len() > 160 || crossed.bit_len() > 32 {
            return Err(invalid());
        }
        prices.push(Quantity::new(price));
        ticks.push(crossed.to::<u32>());
    }
    V3Quote::new(V3QuoteData {
        request,
        amount_out: Quantity::new(read(0)),
        sqrt_price_x96_after: prices,
        initialized_ticks_crossed: ticks,
        gas_estimate: Quantity::new(read(96)),
    })
}
#[cfg(feature = "uniswap-http")]
fn invalid() -> Error {
    crate::error::ValidationError::InvalidUniswapQuote.into()
}
