// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Narrow `THORNode`/Cosmos wire records. Extension fields are ignored; known
//! duplicates, absent required facts and invalid values fail with safe diagnostics.

use crate::{
    domain::{
        Timestamp,
        thorchain::{
            Address, Asset, Chain, ChainAddress, ChainTransaction, ChainTransactionData, Coin,
            CollectionLimit, CompletedStage, ConfirmationStage, InboundAddress, InboundAddresses,
            InboundData, LastBlock, LastBlocks, NetworkData, ObservedStage, OutboundDelayStage,
            OutboundSignedStage, PlannedOutbound, PlannedOutboundData, Pool, PoolData, PoolStatus,
            Pools, ProtocolAmount, QuoteFees, QuoteInputResolution, RawQuantity, RuneBalance,
            Stages, StreamingStatus, SwapQuoteData, SwapRequest, SwapStatus, Text, TransactionMemo,
            TransactionStatus, Txid, deserialize_optional_vec, deserialize_vec,
        },
    },
    error::{Error, ProviderError},
};
use serde::{Deserialize, de::DeserializeOwned};

pub(super) fn invalid_response() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| invalid_response())
}
fn checked<T>(value: Result<T, Error>) -> Result<T, Error> {
    value.map_err(|_| invalid_response())
}

#[derive(Deserialize)]
struct NodeInfo {
    default_node_info: DefaultNodeInfo,
}
#[derive(Deserialize)]
struct DefaultNodeInfo {
    network: String,
}
pub(super) fn chain_id(bytes: &[u8]) -> Result<String, Error> {
    let v: NodeInfo = decode(bytes)?;
    if v.default_node_info.network.len() > 50 {
        return Err(invalid_response());
    }
    Ok(v.default_node_info.network)
}
#[derive(Deserialize)]
struct BalanceResponse {
    balance: Option<BankCoin>,
}
#[derive(Deserialize)]
struct BankCoin {
    denom: String,
    amount: ProtocolAmount,
}
pub(super) fn balance(bytes: &[u8], address: Address) -> Result<RuneBalance, Error> {
    let v: BalanceResponse = decode(bytes)?;
    let coin = v.balance.ok_or(Error::UnavailableData)?;
    if coin.denom != "rune" {
        return Err(invalid_response());
    }
    Ok(RuneBalance::new(address, coin.amount))
}
#[derive(Deserialize)]
#[serde(transparent, bound(deserialize = "T: Deserialize<'de>"))]
struct SourceList<T>(#[serde(deserialize_with = "deserialize_vec")] Vec<T>);

#[derive(Deserialize)]
struct PoolDataFields {
    asset: Asset,
    status: PoolStatus,
    #[serde(default)]
    decimals: Option<u8>,
    pending_inbound_asset: ProtocolAmount,
    pending_inbound_rune: ProtocolAmount,
    balance_asset: ProtocolAmount,
    balance_rune: ProtocolAmount,
    asset_tor_price: ProtocolAmount,
    pool_units: RawQuantity,
    #[serde(rename = "LP_units")]
    lp_units: RawQuantity,
    synth_units: RawQuantity,
    synth_supply: ProtocolAmount,
    savers_depth: ProtocolAmount,
    savers_units: RawQuantity,
    savers_fill_bps: RawQuantity,
    savers_capacity_remaining: ProtocolAmount,
    synth_mint_paused: bool,
    synth_supply_remaining: ProtocolAmount,
    derived_depth_bps: RawQuantity,
    #[serde(default)]
    trading_halted: Option<bool>,
    #[serde(default)]
    volume_asset: Option<ProtocolAmount>,
    #[serde(default)]
    volume_rune: Option<ProtocolAmount>,
}
impl From<PoolDataFields> for PoolData {
    fn from(v: PoolDataFields) -> Self {
        Self {
            asset: v.asset,
            status: v.status,
            decimals: v.decimals,
            pending_inbound_asset: v.pending_inbound_asset,
            pending_inbound_rune: v.pending_inbound_rune,
            balance_asset: v.balance_asset,
            balance_rune: v.balance_rune,
            asset_tor_price: v.asset_tor_price,
            pool_units: v.pool_units,
            lp_units: v.lp_units,
            synth_units: v.synth_units,
            synth_supply: v.synth_supply,
            savers_depth: v.savers_depth,
            savers_units: v.savers_units,
            savers_fill_bps: v.savers_fill_bps,
            savers_capacity_remaining: v.savers_capacity_remaining,
            synth_mint_paused: v.synth_mint_paused,
            synth_supply_remaining: v.synth_supply_remaining,
            derived_depth_bps: v.derived_depth_bps,
            trading_halted: v.trading_halted,
            volume_asset: v.volume_asset,
            volume_rune: v.volume_rune,
        }
    }
}

#[derive(Deserialize)]
struct NetworkDataFields {
    bond_reward_rune: ProtocolAmount,
    total_bond_units: RawQuantity,
    available_pools_rune: ProtocolAmount,
    vaults_liquidity_rune: ProtocolAmount,
    effective_security_bond: ProtocolAmount,
    total_reserve: ProtocolAmount,
    vaults_migrating: bool,
    #[serde(default)]
    xmr_active_vault_ready: Option<bool>,
    gas_spent_rune: ProtocolAmount,
    gas_withheld_rune: ProtocolAmount,
    #[serde(default)]
    outbound_fee_multiplier: Option<RawQuantity>,
    native_outbound_fee_rune: ProtocolAmount,
    native_tx_fee_rune: ProtocolAmount,
    tns_register_fee_rune: ProtocolAmount,
    tns_fee_per_block_rune: ProtocolAmount,
    rune_price_in_tor: ProtocolAmount,
    tor_price_in_rune: ProtocolAmount,
    tor_price_halted: bool,
}
impl From<NetworkDataFields> for NetworkData {
    fn from(v: NetworkDataFields) -> Self {
        Self {
            bond_reward_rune: v.bond_reward_rune,
            total_bond_units: v.total_bond_units,
            available_pools_rune: v.available_pools_rune,
            vaults_liquidity_rune: v.vaults_liquidity_rune,
            effective_security_bond: v.effective_security_bond,
            total_reserve: v.total_reserve,
            vaults_migrating: v.vaults_migrating,
            xmr_active_vault_ready: v.xmr_active_vault_ready,
            gas_spent_rune: v.gas_spent_rune,
            gas_withheld_rune: v.gas_withheld_rune,
            outbound_fee_multiplier: v.outbound_fee_multiplier,
            native_outbound_fee_rune: v.native_outbound_fee_rune,
            native_tx_fee_rune: v.native_tx_fee_rune,
            tns_register_fee_rune: v.tns_register_fee_rune,
            tns_fee_per_block_rune: v.tns_fee_per_block_rune,
            rune_price_in_tor: v.rune_price_in_tor,
            tor_price_in_rune: v.tor_price_in_rune,
            tor_price_halted: v.tor_price_halted,
        }
    }
}

#[derive(Deserialize)]
struct QuoteFeesFields {
    asset: Asset,
    #[serde(default)]
    affiliate: Option<ProtocolAmount>,
    #[serde(default)]
    outbound: Option<ProtocolAmount>,
    liquidity: ProtocolAmount,
    total: ProtocolAmount,
    slippage_bps: u64,
    total_bps: u64,
}
impl From<QuoteFeesFields> for QuoteFees {
    fn from(v: QuoteFeesFields) -> Self {
        Self {
            asset: v.asset,
            affiliate: v.affiliate,
            outbound: v.outbound,
            liquidity: v.liquidity,
            total: v.total,
            slippage_bps: v.slippage_bps,
            total_bps: v.total_bps,
        }
    }
}

#[derive(Deserialize)]
struct ObservedStageFields {
    #[serde(default)]
    started: Option<bool>,
    #[serde(default)]
    pre_confirmation_count: Option<u64>,
    final_count: u64,
    completed: bool,
}
impl From<ObservedStageFields> for ObservedStage {
    fn from(v: ObservedStageFields) -> Self {
        Self {
            started: v.started,
            pre_confirmation_count: v.pre_confirmation_count,
            final_count: v.final_count,
            completed: v.completed,
        }
    }
}

#[derive(Deserialize)]
struct ConfirmationStageFields {
    #[serde(default)]
    counting_start_height: Option<u64>,
    #[serde(default)]
    chain: Option<Chain>,
    #[serde(default)]
    external_observed_height: Option<u64>,
    #[serde(default)]
    external_confirmation_delay_height: Option<u64>,
    #[serde(default)]
    remaining_confirmation_seconds: Option<u64>,
    completed: bool,
}
impl From<ConfirmationStageFields> for ConfirmationStage {
    fn from(v: ConfirmationStageFields) -> Self {
        Self {
            counting_start_height: v.counting_start_height,
            chain: v.chain,
            external_observed_height: v.external_observed_height,
            external_confirmation_delay_height: v.external_confirmation_delay_height,
            remaining_confirmation_seconds: v.remaining_confirmation_seconds,
            completed: v.completed,
        }
    }
}

#[derive(Deserialize)]
struct CompletedStageFields {
    completed: bool,
}
impl From<CompletedStageFields> for CompletedStage {
    fn from(v: CompletedStageFields) -> Self {
        Self {
            completed: v.completed,
        }
    }
}

#[derive(Deserialize)]
struct StreamingStatusFields {
    interval: u64,
    quantity: u64,
    count: u64,
}
impl From<StreamingStatusFields> for StreamingStatus {
    fn from(v: StreamingStatusFields) -> Self {
        Self {
            interval: v.interval,
            quantity: v.quantity,
            count: v.count,
        }
    }
}

#[derive(Deserialize)]
struct OutboundDelayStageFields {
    #[serde(default)]
    remaining_delay_blocks: Option<u64>,
    #[serde(default)]
    remaining_delay_seconds: Option<u64>,
    completed: bool,
}
impl From<OutboundDelayStageFields> for OutboundDelayStage {
    fn from(v: OutboundDelayStageFields) -> Self {
        Self {
            remaining_delay_blocks: v.remaining_delay_blocks,
            remaining_delay_seconds: v.remaining_delay_seconds,
            completed: v.completed,
        }
    }
}

#[derive(Deserialize)]
struct OutboundSignedStageFields {
    #[serde(default)]
    scheduled_outbound_height: Option<u64>,
    #[serde(default)]
    blocks_since_scheduled: Option<u64>,
    completed: bool,
}
impl From<OutboundSignedStageFields> for OutboundSignedStage {
    fn from(v: OutboundSignedStageFields) -> Self {
        Self {
            scheduled_outbound_height: v.scheduled_outbound_height,
            blocks_since_scheduled: v.blocks_since_scheduled,
            completed: v.completed,
        }
    }
}

#[derive(Deserialize)]
struct CoinFields {
    asset: Asset,
    amount: ProtocolAmount,
    #[serde(default)]
    decimals: Option<u8>,
}
impl From<CoinFields> for Coin {
    fn from(v: CoinFields) -> Self {
        Self {
            asset: v.asset,
            amount: v.amount,
            decimals: v.decimals,
        }
    }
}

pub(super) fn pool(bytes: &[u8], asset: &Asset) -> Result<Pool, Error> {
    let v: PoolDataFields = decode(bytes)?;
    if &v.asset != asset {
        return Err(invalid_response());
    }
    checked(Pool::new(v.into()))
}
pub(super) fn pools(bytes: &[u8], limit: CollectionLimit) -> Result<Pools, Error> {
    let v: SourceList<PoolDataFields> = decode(bytes)?;
    within(v.0.len(), limit)?;
    checked(Pools::new(
        v.0.into_iter()
            .map(|p| checked(Pool::new(p.into())))
            .collect::<Result<_, _>>()?,
    ))
}
pub(super) fn network(bytes: &[u8]) -> Result<NetworkData, Error> {
    decode::<NetworkDataFields>(bytes).map(Into::into)
}
fn within(count: usize, limit: CollectionLimit) -> Result<(), Error> {
    if count > limit.get() as usize {
        Err(invalid_response())
    } else {
        Ok(())
    }
}
fn settlement_chain(asset: &Asset) -> Result<Chain, Error> {
    if asset.is_thor_held() {
        Chain::parse("THOR")
    } else {
        Ok(asset.chain().clone())
    }
}
fn address(value: Option<String>, chain: &Chain) -> Result<Option<ChainAddress>, Error> {
    value
        .filter(|s| !s.is_empty())
        .map(|s| checked(ChainAddress::new(chain.clone(), &s)))
        .transpose()
}
fn text(value: Option<String>) -> Result<Option<Text>, Error> {
    value
        .filter(|s| !s.is_empty())
        .map(|s| checked(Text::new(&s)))
        .transpose()
}
#[derive(Deserialize)]
struct QuoteFields {
    expected_amount_out: ProtocolAmount,
    fees: QuoteFeesFields,
    expiry: u64,
    warning: Text,
    notes: Text,
    #[serde(default)]
    inbound_address: Option<String>,
    #[serde(default)]
    router: Option<String>,
    #[serde(default)]
    memo: Option<String>,
    #[serde(default)]
    inbound_confirmation_blocks: Option<u64>,
    #[serde(default)]
    inbound_confirmation_seconds: Option<u64>,
    outbound_delay_blocks: u64,
    outbound_delay_seconds: u64,
    #[serde(default)]
    dust_threshold: Option<RawQuantity>,
    #[serde(default)]
    recommended_min_amount_in: Option<ProtocolAmount>,
    #[serde(default)]
    recommended_gas_rate: Option<RawQuantity>,
    #[serde(default)]
    gas_rate_units: Option<Text>,
    #[serde(default)]
    max_streaming_quantity: Option<u64>,
    #[serde(default)]
    streaming_swap_blocks: Option<u64>,
    #[serde(default)]
    streaming_swap_seconds: Option<u64>,
    #[serde(default)]
    total_swap_seconds: Option<u64>,
}
pub(super) fn quote(bytes: &[u8], request: &SwapRequest) -> Result<SwapQuoteData, Error> {
    let v: QuoteFields = decode(bytes)?;
    let chain = settlement_chain(request.from_asset())?;
    let data = SwapQuoteData {
        input_resolution: QuoteInputResolution::Unreported,
        expected_amount_out: v.expected_amount_out,
        fees: v.fees.into(),
        expiry: Timestamp::from_unix_seconds(v.expiry),
        warning: v.warning,
        notes: v.notes,
        inbound_address: address(v.inbound_address, &chain)?,
        router: address(v.router, &chain)?,
        memo: text(v.memo)?,
        inbound_confirmation_blocks: v.inbound_confirmation_blocks,
        inbound_confirmation_seconds: v.inbound_confirmation_seconds,
        outbound_delay_blocks: v.outbound_delay_blocks,
        outbound_delay_seconds: v.outbound_delay_seconds,
        dust_threshold: v.dust_threshold,
        recommended_min_amount_in: v.recommended_min_amount_in,
        recommended_gas_rate: v.recommended_gas_rate,
        gas_rate_units: v.gas_rate_units,
        max_streaming_quantity: v.max_streaming_quantity,
        streaming_swap_blocks: v.streaming_swap_blocks,
        streaming_swap_seconds: v.streaming_swap_seconds,
        total_swap_seconds: v.total_swap_seconds,
    };
    Ok(data)
}
#[derive(Deserialize)]
struct InboundFields {
    chain: Chain,
    address: String,
    pub_key: Text,
    #[serde(default)]
    router: Option<String>,
    halted: EffectiveHalt,
    global_trading_paused: bool,
    chain_trading_paused: bool,
    chain_lp_actions_paused: bool,
    observed_fee_rate: RawQuantity,
    gas_rate: RawQuantity,
    gas_rate_units: Text,
    outbound_tx_size: RawQuantity,
    outbound_fee: ProtocolAmount,
    dust_threshold: RawQuantity,
}
#[derive(Deserialize)]
#[serde(transparent)]
struct EffectiveHalt(bool);
pub(super) fn inbounds(bytes: &[u8], limit: CollectionLimit) -> Result<InboundAddresses, Error> {
    let v: SourceList<InboundFields> = decode(bytes)?;
    within(v.0.len(), limit)?;
    let items =
        v.0.into_iter()
            .map(|v| {
                let vault_address = checked(ChainAddress::new(v.chain.clone(), &v.address))?;
                let router = address(v.router, &v.chain)?;
                checked(InboundAddress::new(InboundData {
                    chain: v.chain,
                    address: vault_address,
                    pub_key: v.pub_key,
                    router,
                    halted: v.halted.0,
                    global_trading_paused: v.global_trading_paused,
                    chain_trading_paused: v.chain_trading_paused,
                    chain_lp_actions_paused: v.chain_lp_actions_paused,
                    observed_fee_rate: v.observed_fee_rate,
                    gas_rate: v.gas_rate,
                    gas_rate_units: v.gas_rate_units,
                    outbound_tx_size: v.outbound_tx_size,
                    outbound_fee: v.outbound_fee,
                    dust_threshold: v.dust_threshold,
                }))
            })
            .collect::<Result<_, _>>()?;
    checked(InboundAddresses::new(items))
}
#[derive(Deserialize)]
struct LastBlockFields {
    chain: Chain,
    last_observed_in: u64,
    last_signed_out: u64,
    thorchain: u64,
}
pub(super) fn lastblocks(bytes: &[u8], limit: CollectionLimit) -> Result<LastBlocks, Error> {
    let v: SourceList<LastBlockFields> = decode(bytes)?;
    within(v.0.len(), limit)?;
    let items =
        v.0.into_iter()
            .map(|v| {
                checked(LastBlock::new(
                    v.chain,
                    v.last_observed_in,
                    v.last_signed_out,
                    v.thorchain,
                ))
            })
            .collect::<Result<_, _>>()?;
    checked(LastBlocks::new(items))
}
#[derive(Deserialize)]
struct SwapStatusFields {
    pending: bool,
    #[serde(default)]
    streaming: Option<StreamingStatusFields>,
}
#[derive(Deserialize)]
struct StagesFields {
    inbound_observed: ObservedStageFields,
    #[serde(default)]
    inbound_confirmation_counted: Option<ConfirmationStageFields>,
    #[serde(default)]
    inbound_finalised: Option<CompletedStageFields>,
    #[serde(default)]
    swap_status: Option<SwapStatusFields>,
    #[serde(default)]
    swap_finalised: Option<CompletedStageFields>,
    #[serde(default)]
    outbound_delay: Option<OutboundDelayStageFields>,
    #[serde(default)]
    outbound_signed: Option<OutboundSignedStageFields>,
}
impl From<StagesFields> for Stages {
    fn from(v: StagesFields) -> Self {
        Self {
            inbound_observed: v.inbound_observed.into(),
            inbound_confirmation_counted: v.inbound_confirmation_counted.map(Into::into),
            inbound_finalised: v.inbound_finalised.map(Into::into),
            swap_status: v.swap_status.map(|s| SwapStatus {
                pending: s.pending,
                streaming: s.streaming.map(Into::into),
            }),
            swap_finalised: v.swap_finalised.map(Into::into),
            outbound_delay: v.outbound_delay.map(Into::into),
            outbound_signed: v.outbound_signed.map(Into::into),
        }
    }
}
#[derive(Deserialize)]
struct TxFields {
    id: Txid,
    chain: Chain,
    from_address: String,
    to_address: String,
    #[serde(deserialize_with = "deserialize_optional_vec")]
    coins: Option<Vec<CoinFields>>,
    #[serde(deserialize_with = "deserialize_optional_vec")]
    gas: Option<Vec<CoinFields>>,
    #[serde(default)]
    memo: Option<TransactionMemo>,
}
impl TxFields {
    fn transaction(self, limit: CollectionLimit) -> Result<ChainTransaction, Error> {
        if let Some(v) = &self.coins {
            within(v.len(), limit)?;
        }
        if let Some(v) = &self.gas {
            within(v.len(), limit)?;
        }
        checked(ChainTransaction::new(ChainTransactionData {
            id: self.id,
            from_address: checked(ChainAddress::new(self.chain.clone(), &self.from_address))?,
            to_address: checked(ChainAddress::new(self.chain.clone(), &self.to_address))?,
            chain: self.chain,
            coins: self.coins.map(|v| v.into_iter().map(Into::into).collect()),
            gas: self.gas.map(|v| v.into_iter().map(Into::into).collect()),
            memo: self.memo,
        }))
    }
}
#[derive(Deserialize)]
struct PlannedFields {
    chain: Chain,
    to_address: String,
    coin: CoinFields,
    refund: bool,
}
impl PlannedFields {
    fn planned(self) -> Result<PlannedOutbound, Error> {
        checked(PlannedOutbound::new(PlannedOutboundData {
            to_address: checked(ChainAddress::new(self.chain.clone(), &self.to_address))?,
            chain: self.chain,
            coin: self.coin.into(),
            refund: self.refund,
        }))
    }
}
#[derive(Deserialize)]
struct StatusFields {
    #[serde(default)]
    tx: Option<TxFields>,
    #[serde(default, deserialize_with = "deserialize_optional_vec")]
    planned_out_txs: Option<Vec<PlannedFields>>,
    #[serde(default, deserialize_with = "deserialize_optional_vec")]
    out_txs: Option<Vec<TxFields>>,
    stages: StagesFields,
}
pub(super) fn status(
    bytes: &[u8],
    txid: Txid,
    limit: CollectionLimit,
) -> Result<TransactionStatus, Error> {
    let v: StatusFields = decode(bytes)?;
    if let Some(items) = &v.planned_out_txs {
        within(items.len(), limit)?;
    }
    if let Some(items) = &v.out_txs {
        within(items.len(), limit)?;
    }
    let tx = v.tx.map(|t| t.transaction(limit)).transpose()?;
    let planned = v
        .planned_out_txs
        .map(|items| {
            items
                .into_iter()
                .map(PlannedFields::planned)
                .collect::<Result<_, _>>()
        })
        .transpose()?;
    let out = v
        .out_txs
        .map(|items| {
            items
                .into_iter()
                .map(|t| t.transaction(limit))
                .collect::<Result<_, _>>()
        })
        .transpose()?;
    checked(TransactionStatus::new(
        txid,
        tx,
        planned,
        out,
        v.stages.into(),
    ))
}
