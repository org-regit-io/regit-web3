// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use regit_web3::{
    domain::{
        ExactDecimal, Source, Timestamp,
        jupiter::{
            self, BuildRequest, BuildRequestData, Context, Destination, DexFilter, FetchedAt,
            Instruction, InstructionData, InstructionFields, Label, Observation, Operation,
            PreparationSettings, PreparedSwap, QuoteRequest, QuoteRequestData, RouteStep,
            RouteStepData, SlippageBps, SwapBuild, SwapBuildData, SwapIntent, SwapIntentData,
        },
        solana::{Hash, Pubkey},
    },
    error::Error,
};
pub(super) fn input() -> Result<Pubkey, Error> {
    Pubkey::parse("So11111111111111111111111111111111111111112")
}
pub(super) fn output() -> Result<Pubkey, Error> {
    Pubkey::parse("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
}
pub(super) fn taker() -> Result<Pubkey, Error> {
    Pubkey::parse("GkwFnmMDvn3HGMpJpWBg8tgJxr3NxNvg3AXxvXVPbRGJ")
}
pub(super) fn quote_request() -> Result<QuoteRequest, Error> {
    QuoteRequest::new(QuoteRequestData {
        network: jupiter::mainnet("mainnet")?,
        input_mint: input()?,
        output_mint: output()?,
        amount: 1_000_000,
        slippage: SlippageBps::new(100)?,
        excluded_routers: vec![jupiter::Router::JupiterZ],
        excluded_dexes: vec![],
        max_route_steps: 128,
    })
}
pub(super) fn build_request() -> Result<BuildRequest, Error> {
    BuildRequest::new(BuildRequestData {
        network: jupiter::mainnet("mainnet")?,
        input_mint: input()?,
        output_mint: output()?,
        amount: 1_000_000,
        taker: taker()?,
        payer: taker()?,
        slippage: SlippageBps::new(100)?,
        minimum_output: 100,
        wrap_and_unwrap_sol: true,
        destination: Destination::Taker,
        dex_filter: DexFilter::All,
        max_accounts: 64,
        blockhash_slots_to_expiry: 150,
        compute_unit_price_percentile: 5000,
        max_route_steps: 128,
    })
}
pub(super) fn route(input: Pubkey, output: Pubkey) -> Result<RouteStep, Error> {
    RouteStep::new(RouteStepData {
        amm: Pubkey::from_bytes([7; 32]),
        label: Label::new("fixture")?,
        input_mint: input,
        output_mint: output,
        in_amount: 1_000_000,
        out_amount: 10_001,
        percent: ExactDecimal::parse("100")?,
        bps: 10_000,
    })
}
pub(super) fn instruction(program: Pubkey, data: Vec<u8>) -> Result<Instruction, Error> {
    Instruction::new(InstructionFields {
        program_id: program,
        accounts: vec![],
        data: InstructionData::new(data)?,
    })
}
pub(super) fn build_data() -> Result<SwapBuildData, Error> {
    let mut price = vec![3];
    price.extend(1_484u64.to_le_bytes());
    Ok(SwapBuildData {
        request: build_request()?,
        out_amount: 10_001,
        other_amount_threshold: 9_901,
        price_impact: ExactDecimal::parse("-0.000000000000000001")?,
        routes: vec![route(input()?, output()?)?],
        compute_budget: vec![instruction(
            Pubkey::parse("ComputeBudget111111111111111111111111111111")?,
            price,
        )?],
        setup: vec![],
        swap: instruction(
            Pubkey::parse("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4")?,
            vec![1, 2, 3],
        )?,
        cleanup: None,
        other: vec![],
        lookup_tables: vec![],
        blockhash: Hash::from_bytes([9; 32]),
        last_valid_block_height: 400,
        blockhash_fetched_at: FetchedAt::new(300, 123)?,
    })
}
pub(super) fn observation(build: SwapBuild) -> Result<Observation<SwapBuild>, Error> {
    Observation::new(
        build,
        Context::new(
            Operation::Build,
            jupiter::mainnet("mainnet")?,
            Source::new("fixture", "swap_v2_build", env!("CARGO_PKG_VERSION"))?,
            Timestamp::from_unix_seconds(310),
        )?,
    )
}
pub(super) fn prepared() -> Result<PreparedSwap, Error> {
    PreparedSwap::new(SwapIntent::new(SwapIntentData {
        build: observation(SwapBuild::new(build_data()?)?)?,
        settings: settings(),
    })?)
}
pub(super) fn settings() -> PreparationSettings {
    PreparationSettings {
        compute_unit_limit: 400_000,
        maximum_compute_unit_price: 2_000,
        other_instruction_placement: jupiter::OtherInstructionPlacement::AfterCleanup,
        additional_signers: vec![],
    }
}
