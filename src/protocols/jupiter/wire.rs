// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

mod bounded;
mod types;
use crate::{
    domain::{
        ExactDecimal,
        jupiter::{
            BuildRequest, FetchedAt, Quote, QuoteData, QuoteFees, QuoteRequest, Router,
            SourceValidity, SwapBuild, SwapBuildData,
        },
        solana::{Hash, Pubkey},
    },
    error::{Error, ProviderError},
};
use bounded::{Bounded, document};
use serde::{Deserialize, de::IgnoredAny};
use types::{InstructionWire, Leg, Tables, Units, null_only};
pub(super) const MAX_DOCUMENT: usize = bounded::MAX_DOCUMENT;
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
macro_rules! response {
    ($name:ident { $($fields:tt)* }) => {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct $name {
            input_mint: Pubkey, output_mint: Pubkey, in_amount: Units,
            out_amount: Units, other_amount_threshold: Units, swap_mode: String,
            slippage_bps: u16, price_impact_pct: String, route_plan: Bounded<Leg,128>,
            $($fields)*
        }
        impl $name {
            fn matches(&self, input: Pubkey, output: Pubkey, amount:u64, slippage:u16)->Result<(),Error>{
                if self.input_mint != input || self.output_mint != output || self.in_amount.0 != amount
                    || self.slippage_bps != slippage || self.swap_mode != "ExactIn" { return Err(invalid()); }
                Ok(())
            }
        }
    };
}
response!(Order {
    router: Router,
    #[serde(deserialize_with = "null_only")]
    transaction: (),
    #[serde(deserialize_with = "null_only")]
    taker: (),
    fee_bps: Option<u16>,
    fee_mint: Option<Pubkey>,
    platform_fee: Option<PlatformFee>,
    signature_fee_lamports: Option<u64>,
    prioritization_fee_lamports: Option<u64>,
    rent_fee_lamports: Option<u64>,
    last_valid_block_height: Option<Units>,
    expire_at: Option<String>,
    error_code: Option<i32>,
    #[serde(default)]
    error_message: IgnoredAny,
});
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlatformFee {
    amount: Option<Units>,
    fee_bps: Option<u16>,
    fee_mint: Option<Pubkey>,
}
pub(super) fn quote(bytes: &[u8], request: QuoteRequest) -> Result<Quote, Error> {
    let raw = document(bytes)?;
    let o: Order = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    let _ = (o.transaction, o.taker, o.error_message);
    if o.error_code.is_some() {
        return Err(invalid());
    }
    let r = request.data();
    o.matches(r.input_mint, r.output_mint, r.amount, r.slippage.value())?;
    let platform = o.platform_fee;
    Quote::new(QuoteData {
        request,
        router: o.router,
        out_amount: o.out_amount.0,
        other_amount_threshold: o.other_amount_threshold.0,
        price_impact: ExactDecimal::parse(&o.price_impact_pct).map_err(|_| invalid())?,
        routes: o
            .route_plan
            .0
            .into_iter()
            .map(Leg::typed)
            .collect::<Result<_, _>>()?,
        fees: QuoteFees {
            total_fee_bps: o.fee_bps,
            platform_fee_bps: platform.as_ref().and_then(|p| p.fee_bps),
            fee_mint: o.fee_mint,
            platform_fee_mint: platform.as_ref().and_then(|p| p.fee_mint),
            platform_fee_amount: platform.and_then(|p| p.amount.map(|v| v.0)),
            signature_fee_lamports: o.signature_fee_lamports,
            priority_fee_lamports: o.prioritization_fee_lamports,
            rent_fee_lamports: o.rent_fee_lamports,
        },
        validity: SourceValidity {
            expires_at_literal: o
                .expire_at
                .map(|s| crate::domain::jupiter::ExpiryLiteral::new(&s))
                .transpose()
                .map_err(|_| invalid())?,
            last_valid_block_height: o.last_valid_block_height.map(|v| v.0),
        },
    })
    .map_err(|_| invalid())
}
response!(Build {
    compute_budget_instructions: Bounded<InstructionWire, 64>,
    setup_instructions: Bounded<InstructionWire, 64>,
    swap_instruction: InstructionWire,
    #[serde(deserialize_with = "optional")]
    cleanup_instruction: Option<InstructionWire>,
    other_instructions: Bounded<InstructionWire, 64>,
    #[serde(deserialize_with = "null_only")]
    tip_instruction: (),
    #[serde(deserialize_with="optional")]
    addresses_by_lookup_table_address: Option<Tables>,
    blockhash_with_metadata: BlockhashFacts,
    transaction_version: u8,
    compute_unit_price: Option<Units>,
});
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockhashFacts {
    blockhash: [u8; 32],
    last_valid_block_height: u64,
    fetched_at: Time,
}
#[derive(Deserialize)]
struct Time {
    secs_since_epoch: u64,
    nanos_since_epoch: u32,
}
pub(super) fn build(bytes: &[u8], request: BuildRequest) -> Result<SwapBuild, Error> {
    let raw = document(bytes)?;
    let b: Build = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    let () = b.tip_instruction;
    if b.transaction_version != 0 || b.compute_unit_price.is_some() {
        return Err(invalid());
    }
    let r = request.data();
    b.matches(r.input_mint, r.output_mint, r.amount, r.slippage.value())?;
    SwapBuild::new(SwapBuildData {
        request,
        out_amount: b.out_amount.0,
        other_amount_threshold: b.other_amount_threshold.0,
        price_impact: ExactDecimal::parse(&b.price_impact_pct).map_err(|_| invalid())?,
        routes: b
            .route_plan
            .0
            .into_iter()
            .map(Leg::typed)
            .collect::<Result<_, _>>()?,
        compute_budget: b
            .compute_budget_instructions
            .0
            .into_iter()
            .map(InstructionWire::typed)
            .collect::<Result<_, _>>()?,
        setup: b
            .setup_instructions
            .0
            .into_iter()
            .map(InstructionWire::typed)
            .collect::<Result<_, _>>()?,
        swap: b.swap_instruction.typed()?,
        cleanup: b
            .cleanup_instruction
            .map(InstructionWire::typed)
            .transpose()?,
        other: b
            .other_instructions
            .0
            .into_iter()
            .map(InstructionWire::typed)
            .collect::<Result<_, _>>()?,
        lookup_tables: b
            .addresses_by_lookup_table_address
            .map_or_else(Vec::new, |t| t.0),
        blockhash: Hash::from_bytes(b.blockhash_with_metadata.blockhash),
        last_valid_block_height: b.blockhash_with_metadata.last_valid_block_height,
        blockhash_fetched_at: FetchedAt::new(
            b.blockhash_with_metadata.fetched_at.secs_since_epoch,
            b.blockhash_with_metadata.fetched_at.nanos_since_epoch,
        )
        .map_err(|_| invalid())?,
    })
    .map_err(|_| invalid())
}

fn optional<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
