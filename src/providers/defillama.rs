// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Typed `DefiLlama` TVL, yields, stablecoin circulation and USD analytics reads.
//! Pure contracts require no HTTP/runtime. `defillama-http` composes explicit
//! replaceable TVL, yield and stablecoin endpoints with bounded outgoing reads.
use crate::{
    domain::{
        defillama::{
            Analytics, AnalyticsRequest, ProtocolHistory, ProtocolTvl, ProviderId,
            StablecoinHistory, StablecoinScope, Stablecoins, TvlHistory, TvlScope, YieldHistory,
            YieldPools,
        },
        market::Observation,
    },
    error::Error,
};
use std::future::Future;
#[cfg(feature = "defillama-http")]
mod http;
#[cfg(feature = "defillama-http")]
mod wire;
#[cfg(feature = "defillama-http")]
pub use http::{DefiLlamaClient, DefiLlamaHttpConfig};
/// Replaceable provider reader with explicit units, request identities and time data.
pub trait DefiLlamaReader {
    /// Returns current exact USD TVL for an explicitly selected protocol.
    fn protocol_tvl(
        &self,
        protocol: ProviderId,
    ) -> impl Future<Output = Result<Observation<ProtocolTvl>, Error>> + crate::future::MaybeSend;
    /// Returns protocol USD TVL history and current provider-labelled breakdowns.
    fn protocol_history(
        &self,
        protocol: ProviderId,
    ) -> impl Future<Output = Result<Observation<ProtocolHistory>, Error>> + crate::future::MaybeSend;
    /// Returns aggregate or chain-labelled provider TVL history in USD.
    fn tvl_history(
        &self,
        scope: TvlScope,
    ) -> impl Future<Output = Result<Observation<TvlHistory>, Error>> + crate::future::MaybeSend;
    /// Returns current exact USD pool TVLs and signed APYs in percent.
    fn yield_pools(
        &self,
    ) -> impl Future<Output = Result<Observation<YieldPools>, Error>> + crate::future::MaybeSend;
    /// Returns one explicitly selected pool's UTC-dated TVL/APY history.
    fn yield_history(
        &self,
        pool: ProviderId,
    ) -> impl Future<Output = Result<Observation<YieldHistory>, Error>> + crate::future::MaybeSend;
    /// Returns circulating peg-unit amounts and separately reported USD prices.
    fn stablecoins(
        &self,
    ) -> impl Future<Output = Result<Observation<Stablecoins>, Error>> + crate::future::MaybeSend;
    /// Returns explicit stablecoin aggregate/chain/asset history with separate unit maps.
    fn stablecoin_history(
        &self,
        scope: StablecoinScope,
    ) -> impl Future<Output = Result<Observation<StablecoinHistory>, Error>> + crate::future::MaybeSend;
    /// Returns one protocol's documented DEX-volume or fee/revenue USD analytics.
    fn analytics(
        &self,
        request: AnalyticsRequest,
    ) -> impl Future<Output = Result<Observation<Analytics>, Error>> + crate::future::MaybeSend;
}
