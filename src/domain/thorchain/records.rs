// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{collections::BTreeSet, fmt, marker::PhantomData};

use serde::{Deserialize, Serialize};

use super::{
    Address, Asset, AssetKind, Chain, ChainAddress, CollectionLimit, ProtocolAmount, RawQuantity,
    Text, TransactionMemo, Txid, invalid_record,
};
use crate::{
    domain::{Timestamp, U256},
    error::Error,
};

macro_rules! data_record {
    ($name:ident, $doc:literal, {$($(#[$attribute:meta])* $field:ident: $ty:ty => $description:literal),* $(,)?}) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($(#[$attribute])* #[doc = $description] pub $field: $ty),*
        }
    };
}

/// Exact native RUNE balance for one caller-qualified THOR account.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuneBalance {
    address: Address,
    amount: ProtocolAmount,
}
impl RuneBalance {
    /// Records an explicit source-returned RUNE amount; missing data is not zero.
    #[must_use]
    pub const fn new(address: Address, amount: ProtocolAmount) -> Self {
        Self { address, amount }
    }
    /// Returns the exact queried account.
    #[must_use]
    pub const fn address(&self) -> &Address {
        &self.address
    }
    /// Returns the exact RUNE amount in protocol 1e8 units.
    #[must_use]
    pub const fn amount(&self) -> ProtocolAmount {
        self.amount
    }
}

/// Source-reported pool availability; it does not establish future executability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PoolStatus {
    /// Available at the reported query time.
    Available,
    /// Staged pool with limited protocol operations.
    Staged,
    /// Suspended pool.
    Suspended,
}
data_record!(PoolData, "Explicit pool fields; construct a validated immutable Pool from these parts.", {
    asset: Asset => "Full layer-one pool asset identity.",
    status: PoolStatus => "Actual source-reported pool status.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    decimals: Option<u8> => "Optional source-reported external asset precision; protocol balances remain 1e8.",
    pending_inbound_asset: ProtocolAmount => "Pending underlying asset in protocol 1e8 units.",
    pending_inbound_rune: ProtocolAmount => "Pending RUNE in protocol 1e8 units.",
    balance_asset: ProtocolAmount => "Exact pool asset balance in protocol 1e8 units.",
    balance_rune: ProtocolAmount => "Exact pool RUNE balance in protocol 1e8 units.",
    asset_tor_price: ProtocolAmount => "TOR per underlying asset, scaled by 1e8.",
    pool_units: RawQuantity => "Total ownership units, distinct from token quantities.",
    lp_units: RawQuantity => "Liquidity-provider ownership units.",
    synth_units: RawQuantity => "Synthetic ownership units.",
    synth_supply: ProtocolAmount => "Synthetic supply in protocol 1e8 units.",
    savers_depth: ProtocolAmount => "Underlying asset held in savers, in protocol 1e8 units.",
    savers_units: RawQuantity => "Savers ownership units.",
    savers_fill_bps: RawQuantity => "Source-reported saver capacity fill in basis points.",
    savers_capacity_remaining: ProtocolAmount => "Remaining saver capacity in protocol asset units.",
    synth_mint_paused: bool => "Source-reported synthetic mint pause.",
    synth_supply_remaining: ProtocolAmount => "Remaining synthetic supply capacity in protocol units.",
    derived_depth_bps: RawQuantity => "Source-reported derived pool depth ratio in basis points.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    trading_halted: Option<bool> => "Explicit source-reported halt, or unavailable when absent.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    volume_asset: Option<ProtocolAmount> => "Optional 24-hour source asset volume in protocol units.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    volume_rune: Option<ProtocolAmount> => "Optional 24-hour RUNE volume in protocol units."
});

/// Immutable pool record with a full asset identity and checked ownership-unit sum.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PoolData", into = "PoolData")]
pub struct Pool(PoolData);
impl Pool {
    /// Validates full layer-one pool identity and total LP/synthetic ownership units.
    ///
    /// # Errors
    /// Rejects a wrapped pool identity, overflow or an inconsistent unit total.
    pub fn new(data: PoolData) -> Result<Self, Error> {
        if data.asset.kind() != AssetKind::LayerOne
            || data.lp_units.raw().checked_add(data.synth_units.raw())
                != Some(data.pool_units.raw())
        {
            return Err(invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns validated source fields without mutable access to the record.
    #[must_use]
    pub const fn data(&self) -> &PoolData {
        &self.0
    }
    /// Returns the canonical underlying asset.
    #[must_use]
    pub const fn asset(&self) -> &Asset {
        &self.0.asset
    }
}
impl TryFrom<PoolData> for Pool {
    type Error = Error;
    fn try_from(v: PoolData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Pool> for PoolData {
    fn from(v: Pool) -> Self {
        v.0
    }
}

macro_rules! collection {
    ($name:ident, $ty:ty, $key:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(Vec<$ty>);
        impl $name {
            /// Constructs a complete bounded collection without duplicate identities.
            ///
            /// # Errors
            /// Rejects duplicates or more than the supported maximum items.
            pub fn new(items: Vec<$ty>) -> Result<Self, Error> {
                if items.len() > CollectionLimit::MAXIMUM as usize {
                    return Err(invalid_record());
                }
                let key: fn(&$ty) -> String = $key;
                let mut identities = BTreeSet::new();
                if items.iter().any(|item| !identities.insert(key(item))) {
                    return Err(invalid_record());
                }
                Ok(Self(items))
            }
            /// Returns all supplied items; no source records were truncated.
            #[must_use]
            pub fn items(&self) -> &[$ty] {
                &self.0
            }
        }
        impl TryFrom<Vec<$ty>> for $name {
            type Error = Error;
            fn try_from(v: Vec<$ty>) -> Result<Self, Error> {
                Self::new(v)
            }
        }
        impl From<$name> for Vec<$ty> {
            fn from(v: $name) -> Self {
                v.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Self::new(deserialize_vec(d)?).map_err(serde::de::Error::custom)
            }
        }
    };
}
collection!(
    Pools,
    Pool,
    |v| v.asset().to_string(),
    "A complete source pool catalogue with unique full asset identities."
);

data_record!(NetworkData, "Exact source-reported network values; no inferred snapshot, price or fee policy.", {
    bond_reward_rune: ProtocolAmount => "Total RUNE awarded to node operators in 1e8 units.",
    total_bond_units: RawQuantity => "Total bond ownership units, distinct from RUNE amounts.",
    available_pools_rune: ProtocolAmount => "RUNE held in available pools.",
    vaults_liquidity_rune: ProtocolAmount => "RUNE valuation of vault layer-one assets.",
    effective_security_bond: ProtocolAmount => "Effective RUNE security bond.",
    total_reserve: ProtocolAmount => "Exact RUNE reserve balance.",
    vaults_migrating: bool => "Reported presence of unfinished retiring-vault migration.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    xmr_active_vault_ready: Option<bool> => "Reported XMR vault readiness, unavailable if absent.",
    gas_spent_rune: ProtocolAmount => "Total outbound gas spent, valued in RUNE.",
    gas_withheld_rune: ProtocolAmount => "Total gas fees withheld, valued in RUNE.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    outbound_fee_multiplier: Option<RawQuantity> => "Reported outbound multiplier in basis points, unavailable if absent.",
    native_outbound_fee_rune: ProtocolAmount => "Suggested native outbound fee in RUNE 1e8 units.",
    native_tx_fee_rune: ProtocolAmount => "Suggested native transaction fee in RUNE 1e8 units.",
    tns_register_fee_rune: ProtocolAmount => "Suggested `THORName` registration fee in RUNE.",
    tns_fee_per_block_rune: ProtocolAmount => "Suggested `THORName` per-block fee in RUNE.",
    rune_price_in_tor: ProtocolAmount => "TOR per RUNE scaled by 1e8; distinct from a RUNE amount.",
    tor_price_in_rune: ProtocolAmount => "RUNE per TOR scaled by 1e8; source valuation, not a reciprocal guarantee.",
    tor_price_halted: bool => "Source-reported anchor-price halt."
});

/// Explicit tolerance in basis points, independent of provider fee basis points.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct BasisPoints(u16);
impl BasisPoints {
    /// Validates a caller-selected tolerance in 0..=10000.
    ///
    /// # Errors
    /// Rejects more than 100 percent; no default is selected.
    pub fn new(value: u16) -> Result<Self, Error> {
        if value > 10_000 {
            Err(invalid_record())
        } else {
            Ok(Self(value))
        }
    }
    /// Returns the exact selected tolerance.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}
impl TryFrom<u16> for BasisPoints {
    type Error = Error;
    fn try_from(v: u16) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<BasisPoints> for u16 {
    fn from(v: BasisPoints) -> Self {
        v.0
    }
}
data_record!(StreamingParameters, "Explicit quote streaming settings; zero quantity requests provider-selected quantity.", {
    interval: u64 => "Explicit interval in `THORChain` blocks; zero is retained without inference.",
    quantity: u64 => "Explicit number of swaps; zero permits source auto-quantity selection."
});

/// Optional caller-selected quote inputs; omission delegates those settings to the source.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwapParameters {
    /// Optional destination qualified by the actual settlement chain.
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    pub destination: Option<ChainAddress>,
    /// Optional refund address qualified by the actual source settlement chain.
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    pub refund_address: Option<ChainAddress>,
    /// Optional explicit streaming interval/quantity, not an inferred execution policy.
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    pub streaming: Option<StreamingParameters>,
    /// Optional caller-selected price tolerance in 0..=10000 basis points.
    /// Mutually exclusive with liquidity tolerance when constructing a request.
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    pub tolerance_bps: Option<BasisPoints>,
    /// Optional caller-selected liquidity tolerance in 0..10000 basis points.
    /// Mutually exclusive with ordinary tolerance when constructing a request.
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    pub liquidity_tolerance_bps: Option<BasisPoints>,
}

/// An exact positive amount and caller-supplied input/output asset syntax for a quote.
///
/// Local validation does not establish catalogue existence or source resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SwapRequestFields")]
pub struct SwapRequest {
    from_asset: Asset,
    to_asset: Asset,
    amount: ProtocolAmount,
    parameters: SwapParameters,
}
impl SwapRequest {
    /// Validates positive protocol quantity and chain-qualified optional address inputs.
    ///
    /// # Errors
    /// Rejects zero, identical assets, an address for a different settlement chain,
    /// both tolerance modes together or liquidity tolerance of 10000 basis points.
    pub fn new(
        from_asset: Asset,
        to_asset: Asset,
        amount: ProtocolAmount,
        parameters: SwapParameters,
    ) -> Result<Self, Error> {
        if amount.raw() == U256::ZERO
            || from_asset == to_asset
            || (parameters.tolerance_bps.is_some() && parameters.liquidity_tolerance_bps.is_some())
            || parameters
                .liquidity_tolerance_bps
                .is_some_and(|v| v.get() >= 10_000)
            || parameters
                .destination
                .as_ref()
                .is_some_and(|v| !settles_on(&to_asset, v.chain()))
            || parameters
                .refund_address
                .as_ref()
                .is_some_and(|v| !settles_on(&from_asset, v.chain()))
        {
            return Err(invalid_record());
        }
        Ok(Self {
            from_asset,
            to_asset,
            amount,
            parameters,
        })
    }
    /// Returns the caller-supplied input asset syntax.
    #[must_use]
    pub const fn from_asset(&self) -> &Asset {
        &self.from_asset
    }
    /// Returns the caller-supplied output asset syntax.
    #[must_use]
    pub const fn to_asset(&self) -> &Asset {
        &self.to_asset
    }
    /// Returns the exact input amount in protocol 1e8 units.
    #[must_use]
    pub const fn amount(&self) -> ProtocolAmount {
        self.amount
    }
    /// Returns the exact optional caller inputs.
    #[must_use]
    pub const fn parameters(&self) -> &SwapParameters {
        &self.parameters
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwapRequestFields {
    from_asset: Asset,
    to_asset: Asset,
    amount: ProtocolAmount,
    parameters: SwapParameters,
}
impl TryFrom<SwapRequestFields> for SwapRequest {
    type Error = Error;
    fn try_from(v: SwapRequestFields) -> Result<Self, Error> {
        Self::new(v.from_asset, v.to_asset, v.amount, v.parameters)
    }
}
fn settles_on(asset: &Asset, chain: &Chain) -> bool {
    if asset.is_thor_held() {
        chain.as_str() == "THOR"
    } else {
        asset.chain() == chain
    }
}

data_record!(QuoteFees, "Source-reported quote fees in the explicitly supplied output asset and protocol units.", {
    asset: Asset => "The source-reported full fee asset identity.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    affiliate: Option<ProtocolAmount> => "Affiliate fee, unavailable if absent rather than assumed zero.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    outbound: Option<ProtocolAmount> => "Outbound fee in output-asset protocol units, unavailable if absent.",
    liquidity: ProtocolAmount => "Liquidity fee in output-asset protocol units.",
    total: ProtocolAmount => "Reported total fee in output-asset protocol units.",
    slippage_bps: u64 => "Source-reported slippage in basis points.",
    total_bps: u64 => "Source-reported total fee basis points, independent of caller tolerance."
});
/// Source-reported resolution of the requested quote input asset.
///
/// An associated request alone cannot establish which full asset a provider's
/// fuzzy matching resolved. Unreported resolution remains explicitly unavailable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuoteInputResolution {
    /// The source did not echo its resolved input identity; no verification is inferred.
    Unreported,
    /// The source explicitly reported its resolved full input identity.
    SourceReported {
        /// Actual source-reported input, checked against the requested identity.
        asset: Asset,
    },
}
data_record!(SwapQuoteData, "Source quote facts; construct an immutable quote with its exact request and observation time.", {
    input_resolution: QuoteInputResolution => "Source input-identity availability, independent of the associated request.",
    expected_amount_out: ProtocolAmount => "Expected output after fees in output-asset protocol 1e8 units.",
    fees: QuoteFees => "Reported fees explicitly denominated in the output asset.",
    expiry: Timestamp => "Supplied expiry in Unix seconds; never refreshed implicitly.",
    warning: Text => "Bounded source warning, with content redacted from Debug.",
    notes: Text => "Bounded source execution notes, with content redacted from Debug.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    inbound_address: Option<ChainAddress> => "Optional source inbound address, qualified by the source settlement chain.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    router: Option<ChainAddress> => "Optional source router, separately qualified without implying foreign checksum validation.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    memo: Option<Text> => "Optional source memo; absent when no usable memo is supplied.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    inbound_confirmation_blocks: Option<u64> => "Approximate source-chain confirmations, not `THORChain` confirmations.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    inbound_confirmation_seconds: Option<u64> => "Source-reported estimated inbound confirmation seconds.",
    outbound_delay_blocks: u64 => "Source-reported outbound delay in `THORChain` blocks.",
    outbound_delay_seconds: u64 => "Source-reported approximate outbound delay seconds.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    dust_threshold: Option<RawQuantity> => "Inbound-chain native base-unit threshold; not protocol 1e8 units.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    recommended_min_amount_in: Option<ProtocolAmount> => "Source input minimum advisory in protocol units; actual input resolution remains separate.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    recommended_gas_rate: Option<RawQuantity> => "Optional inbound gas rate, interpreted only with its supplied unit label.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    gas_rate_units: Option<Text> => "Supplied gas rate unit label; no normalization into protocol units.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    max_streaming_quantity: Option<u64> => "Source-reported maximum streaming quantity.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    streaming_swap_blocks: Option<u64> => "Approximate streaming duration in `THORChain` blocks.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    streaming_swap_seconds: Option<u64> => "Approximate streaming duration in seconds.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    total_swap_seconds: Option<u64> => "Approximate total duration; not a completion guarantee."
});

/// A request-bound, fee-denominated quote checked against explicit observation time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SwapQuoteFields")]
pub struct SwapQuote {
    request: SwapRequest,
    data: SwapQuoteData,
    observed_at: Timestamp,
}
impl SwapQuote {
    /// Validates reported input identity, fees, address chains, gas units and expiry.
    /// Unreported input resolution remains unavailable; the exact request is retained.
    ///
    /// # Errors
    /// Rejects incompatible source facts or returns unavailable for expired quotes.
    /// This function never reads a clock or refreshes the source.
    pub fn new(
        request: SwapRequest,
        data: SwapQuoteData,
        observed_at: Timestamp,
    ) -> Result<Self, Error> {
        if data.expiry.unix_seconds() <= observed_at.unix_seconds() {
            return Err(Error::UnavailableData);
        }
        let known_fees = [
            Some(data.fees.liquidity),
            data.fees.affiliate,
            data.fees.outbound,
        ]
        .into_iter()
        .flatten()
        .try_fold(U256::ZERO, |sum, fee| sum.checked_add(fee.raw()))
        .ok_or_else(invalid_record)?;
        if data.fees.asset != *request.to_asset()
            || matches!(&data.input_resolution, QuoteInputResolution::SourceReported { asset } if asset != request.from_asset())
            || known_fees > data.fees.total.raw()
            || (data.fees.affiliate.is_some()
                && data.fees.outbound.is_some()
                && known_fees != data.fees.total.raw())
            || data
                .inbound_address
                .as_ref()
                .is_some_and(|v| !settles_on(request.from_asset(), v.chain()))
            || data
                .router
                .as_ref()
                .is_some_and(|v| !settles_on(request.from_asset(), v.chain()))
            || data.recommended_gas_rate.is_some() != data.gas_rate_units.is_some()
            || data
                .gas_rate_units
                .as_ref()
                .is_some_and(|v| v.as_str().trim().is_empty())
        {
            return Err(invalid_record());
        }
        Ok(Self {
            request,
            data,
            observed_at,
        })
    }
    /// Returns the exact supplied request; actual input resolution is separate source metadata.
    #[must_use]
    pub const fn request(&self) -> &SwapRequest {
        &self.request
    }
    /// Returns validated source quote facts and protocol-unit minima.
    /// Requested input and actual source input resolution remain separate facts.
    #[must_use]
    pub const fn data(&self) -> &SwapQuoteData {
        &self.data
    }
    /// Returns explicitly supplied observation time, independently of source expiry.
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwapQuoteFields {
    request: SwapRequest,
    data: SwapQuoteData,
    observed_at: Timestamp,
}
impl TryFrom<SwapQuoteFields> for SwapQuote {
    type Error = Error;
    fn try_from(v: SwapQuoteFields) -> Result<Self, Error> {
        Self::new(v.request, v.data, v.observed_at)
    }
}

data_record!(InboundData, "Source inbound-vault, halt and external fee-unit facts; validate through `InboundAddress`.", {
    chain: Chain => "Source external chain code, not a Cosmos chain ID.",
    address: ChainAddress => "Source vault address qualified by its external chain.",
    pub_key: Text => "Bounded source vault public-key text, not a locally authenticated key.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    router: Option<ChainAddress> => "Optional chain-qualified router, absent if the source supplied none.",
    halted: bool => "Source-reported effective global/chain trading halt.",
    global_trading_paused: bool => "Source-reported global trading pause.",
    chain_trading_paused: bool => "Source-reported chain trading pause.",
    chain_lp_actions_paused: bool => "Source-reported chain liquidity-action pause.",
    observed_fee_rate: RawQuantity => "Source observed external fee rate; not assumed to share `gas_rate` units.",
    gas_rate: RawQuantity => "Suggested external gas rate with its supplied unit label.",
    gas_rate_units: Text => "Supplied external gas rate unit label.",
    outbound_tx_size: RawQuantity => "Source average outbound transaction size in the chain's fee-size units.",
    outbound_fee: ProtocolAmount => "Source outbound fee in the chain gas asset, normalized to THOR protocol 1e8 units.",
    dust_threshold: RawQuantity => "External-chain native base-unit dust threshold; no inferred decimals."
});
/// Immutable chain-qualified inbound state; halt and foreign-address facts remain source reported.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "InboundData", into = "InboundData")]
pub struct InboundAddress(InboundData);
impl InboundAddress {
    /// Validates matching chain attribution and actual source halt consistency.
    ///
    /// # Errors
    /// Rejects differing address/router chains or an inconsistent effective halt.
    pub fn new(data: InboundData) -> Result<Self, Error> {
        if data.address.chain() != &data.chain
            || data
                .router
                .as_ref()
                .is_some_and(|v| v.chain() != &data.chain)
            || data.halted != (data.global_trading_paused || data.chain_trading_paused)
            || data.pub_key.as_str().trim().is_empty()
            || data.gas_rate_units.as_str().trim().is_empty()
        {
            return Err(invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns the validated source record without mutable access.
    #[must_use]
    pub const fn data(&self) -> &InboundData {
        &self.0
    }
}
impl TryFrom<InboundData> for InboundAddress {
    type Error = Error;
    fn try_from(v: InboundData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<InboundAddress> for InboundData {
    fn from(v: InboundAddress) -> Self {
        v.0
    }
}
collection!(
    InboundAddresses,
    InboundAddress,
    |v| v.data().chain.as_str().to_owned(),
    "Complete source inbound states with unique chain identities."
);

/// External observation height and distinct `THORChain` signing/evaluation heights.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LastBlockFields")]
pub struct LastBlock {
    chain: Chain,
    last_observed_in: u64,
    last_signed_out: u64,
    thorchain: u64,
}
impl LastBlock {
    /// Validates source heights without comparing external height to `THORChain` height.
    ///
    /// # Errors
    /// Rejects a reported signing height beyond the reported `THORChain` height.
    pub fn new(
        chain: Chain,
        last_observed_in: u64,
        last_signed_out: u64,
        thorchain: u64,
    ) -> Result<Self, Error> {
        if last_signed_out > thorchain {
            return Err(invalid_record());
        }
        Ok(Self {
            chain,
            last_observed_in,
            last_signed_out,
            thorchain,
        })
    }
    /// Returns the external chain whose observation height is reported.
    #[must_use]
    pub const fn chain(&self) -> &Chain {
        &self.chain
    }
    /// Returns the last inbound observation height on the external chain.
    #[must_use]
    pub const fn last_observed_in(&self) -> u64 {
        self.last_observed_in
    }
    /// Returns the source-reported last outbound signing height on `THORChain`.
    #[must_use]
    pub const fn last_signed_out(&self) -> u64 {
        self.last_signed_out
    }
    /// Returns the source-reported `THORChain` evaluation height, without a block hash.
    #[must_use]
    pub const fn thorchain(&self) -> u64 {
        self.thorchain
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LastBlockFields {
    chain: Chain,
    last_observed_in: u64,
    last_signed_out: u64,
    thorchain: u64,
}
impl TryFrom<LastBlockFields> for LastBlock {
    type Error = Error;
    fn try_from(v: LastBlockFields) -> Result<Self, Error> {
        Self::new(v.chain, v.last_observed_in, v.last_signed_out, v.thorchain)
    }
}
collection!(
    LastBlocks,
    LastBlock,
    |v| v.chain().as_str().to_owned(),
    "Complete source external/`THORChain` height reports with unique chain identities."
);

data_record!(Coin, "An exact THOR asset identity and normalized protocol quantity.", {
    asset: Asset => "The full source asset identity.",
    amount: ProtocolAmount => "Exact amount normalized to THOR protocol 1e8 units.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    decimals: Option<u8> => "Optional source-reported native asset precision; amount remains protocol-normalized."
});
data_record!(ObservedStage, "Source observation counts/completion, without an inferred consensus threshold.", {
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    started: Option<bool> => "Legacy source started flag, unavailable if absent.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    pre_confirmation_count: Option<u64> => "Source pre-confirmation observation count.",
    final_count: u64 => "Source final observation count.",
    completed: bool => "Source-reported observation completion, not independent finality."
});
data_record!(ConfirmationStage, "Source external-chain confirmation counting and distinct `THORChain` start height.", {
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    counting_start_height: Option<u64> => "`THORChain` height when counting began.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    chain: Option<Chain> => "External source chain whose confirmations are counted.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    external_observed_height: Option<u64> => "Observed height on the external chain.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    external_confirmation_delay_height: Option<u64> => "External-chain height required to complete counting.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    remaining_confirmation_seconds: Option<u64> => "Source approximate remaining confirmation seconds.",
    completed: bool => "Source-reported confirmation counting completion."
});
data_record!(CompletedStage, "A supplied source stage-completion fact; omission is distinct from false.", {
    completed: bool => "Source-reported completion of this specific stage."
});
data_record!(StreamingStatus, "Actual source streaming settings and attempt count; attempts can exceed successful quantity.", {
    interval: u64 => "Reported interval in `THORChain` blocks.",
    quantity: u64 => "Reported total target number of sub-swaps.",
    count: u64 => "Reported attempts, not silently clamped to quantity."
});
data_record!(SwapStatus, "Source pending flag and separately supplied streaming state.", {
    pending: bool => "Whether the source reports a swap still pending.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    streaming: Option<StreamingStatus> => "Optional actual streaming state, unavailable when absent."
});
data_record!(OutboundDelayStage, "Source-reported outbound scheduling delay.", {
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    remaining_delay_blocks: Option<u64> => "Approximate remaining `THORChain` delay blocks.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    remaining_delay_seconds: Option<u64> => "Approximate remaining delay seconds.",
    completed: bool => "Source-reported outbound delay completion."
});
data_record!(OutboundSignedStage, "Source-reported outbound signing/broadcast progress, not external inclusion proof.", {
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    scheduled_outbound_height: Option<u64> => "Scheduled `THORChain` outbound height.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    blocks_since_scheduled: Option<u64> => "Reported `THORChain` blocks since scheduling.",
    completed: bool => "Source-reported external signing/broadcast completion."
});
data_record!(Stages, "Separate actual transaction stages; absent stages remain unavailable, not successful.", {
    inbound_observed: ObservedStage => "Source inbound observation counts and completion.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    inbound_confirmation_counted: Option<ConfirmationStage> => "Optional external-chain confirmation counting state.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    inbound_finalised: Option<CompletedStage> => "Optional source finalised-inbound state.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    swap_status: Option<SwapStatus> => "Optional pending/streaming swap state.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    swap_finalised: Option<CompletedStage> => "Optional legacy swap-finalised state; success/refund remains separate.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    outbound_delay: Option<OutboundDelayStage> => "Optional scheduling delay state.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    outbound_signed: Option<OutboundSignedStage> => "Optional signing/broadcast state; not external finality."
});

data_record!(ChainTransactionData, "Source observed transaction fields; construct a chain-consistent immutable record.", {
    id: Txid => "Actual source transaction identity, not a locally recomputed hash.",
    chain: Chain => "Actual transaction settlement chain.",
    from_address: ChainAddress => "Source chain-qualified sender.",
    to_address: ChainAddress => "Source chain-qualified recipient.",
    #[serde(deserialize_with = "deserialize_optional_vec")]
    coins: Option<Vec<Coin>> => "Exact transferred protocol quantities; null remains unavailable.",
    #[serde(deserialize_with = "deserialize_optional_vec")]
    gas: Option<Vec<Coin>> => "Exact source-reported protocol gas coins; null remains unavailable.",
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    memo: Option<TransactionMemo> => "Raw source UTF-8 memo; null and explicit empty remain distinct, with opaque diagnostics."
});
/// Immutable source transaction with matching chain/address/asset attribution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ChainTransactionData", into = "ChainTransactionData")]
pub struct ChainTransaction(ChainTransactionData);
impl ChainTransaction {
    /// Validates chain attribution and bounded unique transferred/gas coin identities.
    ///
    /// # Errors
    /// Rejects mismatched chains, duplicate assets or oversized coin collections.
    pub fn new(data: ChainTransactionData) -> Result<Self, Error> {
        if data.from_address.chain() != &data.chain || data.to_address.chain() != &data.chain {
            return Err(invalid_record());
        }
        for coins in [&data.coins, &data.gas].into_iter().flatten() {
            if coins.len() > CollectionLimit::MAXIMUM as usize {
                return Err(invalid_record());
            }
            let mut ids = BTreeSet::new();
            if coins
                .iter()
                .any(|v| !settles_on(&v.asset, &data.chain) || !ids.insert(v.asset.to_string()))
            {
                return Err(invalid_record());
            }
        }
        Ok(Self(data))
    }
    /// Returns validated source transaction fields.
    #[must_use]
    pub const fn data(&self) -> &ChainTransactionData {
        &self.0
    }
}
impl TryFrom<ChainTransactionData> for ChainTransaction {
    type Error = Error;
    fn try_from(v: ChainTransactionData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<ChainTransaction> for ChainTransactionData {
    fn from(v: ChainTransaction) -> Self {
        v.0
    }
}
data_record!(PlannedOutboundData, "Exact source-planned outbound/refund fields; construct a validated record.", {
    chain: Chain => "Actual planned settlement chain.",
    to_address: ChainAddress => "Source chain-qualified destination.",
    coin: Coin => "Planned exact asset and protocol quantity.",
    refund: bool => "Actual source refund flag, independently retained from stage completion."
});
/// Immutable source-planned outbound with a distinct explicit refund flag.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PlannedOutboundData", into = "PlannedOutboundData")]
pub struct PlannedOutbound(PlannedOutboundData);
impl PlannedOutbound {
    /// Validates matching coin/address settlement-chain attribution.
    ///
    /// # Errors
    /// Rejects inconsistent address or asset settlement chains.
    pub fn new(data: PlannedOutboundData) -> Result<Self, Error> {
        if data.to_address.chain() != &data.chain || !settles_on(&data.coin.asset, &data.chain) {
            return Err(invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns validated source-planned fields and explicit refund state.
    #[must_use]
    pub const fn data(&self) -> &PlannedOutboundData {
        &self.0
    }
}
impl TryFrom<PlannedOutboundData> for PlannedOutbound {
    type Error = Error;
    fn try_from(v: PlannedOutboundData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<PlannedOutbound> for PlannedOutboundData {
    fn from(v: PlannedOutbound) -> Self {
        v.0
    }
}

/// Request-bound cross-chain state with separate observed, planned and actual outbound facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TransactionStatusFields")]
pub struct TransactionStatus {
    query_id: Txid,
    transaction: Option<ChainTransaction>,
    planned_outbounds: Option<Vec<PlannedOutbound>>,
    outbounds: Option<Vec<ChainTransaction>>,
    stages: Stages,
}
impl TransactionStatus {
    /// Validates the exact query identity, bounded collections and supplied stage fields.
    ///
    /// # Errors
    /// Rejects a differing observed inbound ID, duplicate outbound IDs, oversized
    /// collections or reversed external confirmation heights. Absent inbound
    /// data remains absent; no stage/finality is inferred from other fields.
    pub fn new(
        query_id: Txid,
        transaction: Option<ChainTransaction>,
        planned_outbounds: Option<Vec<PlannedOutbound>>,
        outbounds: Option<Vec<ChainTransaction>>,
        stages: Stages,
    ) -> Result<Self, Error> {
        if transaction
            .as_ref()
            .is_some_and(|v| !v.data().id.same_transaction(&query_id))
            || planned_outbounds
                .as_ref()
                .is_some_and(|v| v.len() > CollectionLimit::MAXIMUM as usize)
            || outbounds
                .as_ref()
                .is_some_and(|v| v.len() > CollectionLimit::MAXIMUM as usize)
        {
            return Err(invalid_record());
        }
        let mut ids = BTreeSet::new();
        if outbounds
            .iter()
            .flatten()
            .any(|v| !v.data().id.is_blank() && !ids.insert(v.data().id.identity_key()))
        {
            return Err(invalid_record());
        }
        if stages.inbound_confirmation_counted.as_ref().is_some_and(|v| matches!((v.external_observed_height, v.external_confirmation_delay_height), (Some(observed), Some(delay)) if delay < observed)) { return Err(invalid_record()); }
        Ok(Self {
            query_id,
            transaction,
            planned_outbounds,
            outbounds,
            stages,
        })
    }
    /// Returns the exact requested inbound identifier.
    #[must_use]
    pub const fn query_id(&self) -> &Txid {
        &self.query_id
    }
    /// Returns the optional source-observed inbound, not an inferred missing transaction.
    #[must_use]
    pub const fn transaction(&self) -> Option<&ChainTransaction> {
        self.transaction.as_ref()
    }
    /// Returns exact source-planned outbounds and refund flags.
    #[must_use]
    pub fn planned_outbounds(&self) -> Option<&[PlannedOutbound]> {
        self.planned_outbounds.as_deref()
    }
    /// Returns actual source-observed outbounds, independently of planned outbounds.
    #[must_use]
    pub fn outbounds(&self) -> Option<&[ChainTransaction]> {
        self.outbounds.as_deref()
    }
    /// Returns actual stage progress without inferred finality or success.
    #[must_use]
    pub const fn stages(&self) -> &Stages {
        &self.stages
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionStatusFields {
    query_id: Txid,
    #[serde(deserialize_with = "super::super::deserialize_optional")]
    transaction: Option<ChainTransaction>,
    #[serde(deserialize_with = "deserialize_optional_vec")]
    planned_outbounds: Option<Vec<PlannedOutbound>>,
    #[serde(deserialize_with = "deserialize_optional_vec")]
    outbounds: Option<Vec<ChainTransaction>>,
    stages: Stages,
}
impl TryFrom<TransactionStatusFields> for TransactionStatus {
    type Error = Error;
    fn try_from(v: TransactionStatusFields) -> Result<Self, Error> {
        Self::new(
            v.query_id,
            v.transaction,
            v.planned_outbounds,
            v.outbounds,
            v.stages,
        )
    }
}

pub(crate) fn deserialize_vec<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    struct Bounded<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Bounded<T> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a bounded `THORChain` collection")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut a: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = a.next_element()? {
                if values.len() == CollectionLimit::MAXIMUM as usize {
                    return Err(serde::de::Error::custom(
                        "`THORChain` collection exceeds bound",
                    ));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Bounded(PhantomData))
}

pub(crate) fn deserialize_optional_vec<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Vec<T>>, D::Error> {
    struct OptionalBounded<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for OptionalBounded<T> {
        type Value = Option<Vec<T>>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("null or a bounded `THORChain` collection")
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            deserialize_vec(d).map(Some)
        }
    }
    d.deserialize_option(OptionalBounded(PhantomData))
}
