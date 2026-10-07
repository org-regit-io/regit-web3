// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{assets, invalid_response, required_option};
use crate::{
    domain::{
        Amount, ExactDecimal,
        cardano::{
            AddressBalance, AddressData, AddressDetails, AssetData, AssetDetails, AssetEntry,
            AssetHolder, AssetId, AssetKind, AssetName, DrepId, Epoch, EpochData, EpochSelector,
            Hash, IndexPage, IndexedAddressKind, Lovelace, MetadataAvailability, Network,
            NetworkData, NetworkSupply, Order, PageRequest, PageTarget, PaymentAddress,
            PaymentParameters, PolicyId, PoolId, ProtocolParameters, Reward, RewardKind,
            SourceText, StakeAccount, StakeAccountData, StakeAddress, TokenMetadata,
            TransactionReference, asset_fingerprint,
        },
    },
    error::Error,
};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|_| invalid_response())
}
fn checked<T>(value: Result<T, Error>) -> Result<T, Error> {
    value.map_err(|_| invalid_response())
}
fn selected(epoch: u64, selector: EpochSelector) -> Result<(), Error> {
    if matches!(selector,EpochSelector::Number(n)if n!=epoch) {
        Err(invalid_response())
    } else {
        Ok(())
    }
}

#[derive(Deserialize)]
struct Supply {
    max: Lovelace,
    total: Lovelace,
    circulating: Lovelace,
    locked: Lovelace,
    treasury: Lovelace,
    reserves: Lovelace,
}
#[derive(Deserialize)]
struct Stake {
    live: Lovelace,
    active: Lovelace,
}
#[derive(Deserialize)]
struct NetworkWire {
    supply: Supply,
    stake: Stake,
}
pub(in crate::providers::blockfrost) fn network(
    bytes: &[u8],
    network: &Network,
) -> Result<NetworkData, Error> {
    let v: NetworkWire = decode(bytes)?;
    checked(NetworkData::new(
        network.clone(),
        NetworkSupply {
            maximum: v.supply.max,
            total: v.supply.total,
            circulating: v.supply.circulating,
            locked: v.supply.locked,
            treasury: v.supply.treasury,
            reserves: v.supply.reserves,
            live_stake: v.stake.live,
            active_stake: v.stake.active,
        },
    ))
}
#[derive(Deserialize)]
struct EpochWire {
    epoch: u64,
    start_time: u64,
    end_time: u64,
    first_block_time: u64,
    last_block_time: u64,
    block_count: u64,
    tx_count: u64,
    output: Lovelace,
    fees: Lovelace,
    #[serde(deserialize_with = "required_option")]
    active_stake: Option<Lovelace>,
}
pub(in crate::providers::blockfrost) fn epoch(
    bytes: &[u8],
    network: &Network,
    selector: EpochSelector,
) -> Result<Epoch, Error> {
    let v: EpochWire = decode(bytes)?;
    selected(v.epoch, selector)?;
    checked(Epoch::new(
        network.clone(),
        EpochData {
            epoch: v.epoch,
            start_unix_seconds: v.start_time,
            end_unix_seconds: v.end_time,
            first_block_unix_seconds: v.first_block_time,
            last_block_unix_seconds: v.last_block_time,
            block_count: v.block_count,
            transaction_count: v.tx_count,
            output: v.output,
            fees: v.fees,
            active_stake: v.active_stake,
        },
    ))
}
struct Decimal(ExactDecimal);
impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Box::<RawValue>::deserialize(d)?;
        let raw = v.get();
        let text = if raw.starts_with('"') {
            serde_json::from_str::<String>(raw).map_err(serde::de::Error::custom)?
        } else {
            raw.to_owned()
        };
        ExactDecimal::parse(&text)
            .map(Self)
            .map_err(|_| serde::de::Error::custom("invalid provider decimal"))
    }
}
#[derive(Deserialize)]
struct ParametersWire {
    epoch: u64,
    min_fee_a: u64,
    min_fee_b: u64,
    max_tx_size: u32,
    protocol_major_ver: u64,
    protocol_minor_ver: u64,
    key_deposit: Lovelace,
    pool_deposit: Lovelace,
    #[serde(deserialize_with = "required_option")]
    max_val_size: Option<String>,
    #[serde(deserialize_with = "required_option")]
    coins_per_utxo_size: Option<String>,
    #[serde(deserialize_with = "required_option")]
    price_mem: Option<Decimal>,
    #[serde(deserialize_with = "required_option")]
    price_step: Option<Decimal>,
}
pub(in crate::providers::blockfrost) fn parameters(
    bytes: &[u8],
    network: &Network,
    selector: EpochSelector,
) -> Result<ProtocolParameters, Error> {
    let v: ParametersWire = decode(bytes)?;
    selected(v.epoch, selector)?;
    let max_value_bytes = v
        .max_val_size
        .map(|s| {
            checked(Lovelace::parse(&s))
                .and_then(|v| checked(v.payment_coin()))
                .and_then(|v| u32::try_from(v).map_err(|_| invalid_response()))
        })
        .transpose()?;
    let coins_per_utxo_size = v
        .coins_per_utxo_size
        .map(|s| checked(Lovelace::parse(&s)).and_then(|v| checked(v.payment_coin())))
        .transpose()?;
    checked(ProtocolParameters::new(
        network.clone(),
        PaymentParameters {
            epoch: v.epoch,
            min_fee_coefficient: v.min_fee_a,
            min_fee_constant: v.min_fee_b,
            max_transaction_bytes: v.max_tx_size,
            max_value_bytes,
            coins_per_utxo_size,
            protocol_major: v.protocol_major_ver,
            protocol_minor: v.protocol_minor_ver,
            key_deposit: v.key_deposit,
            pool_deposit: v.pool_deposit,
            script_memory_price: v.price_mem.map(|d| d.0),
            script_step_price: v.price_step.map(|d| d.0),
        },
    ))
}
#[derive(Deserialize)]
struct AddressWire {
    address: String,
    amount: Vec<super::UnitQuantity>,
    #[serde(deserialize_with = "required_option")]
    stake_address: Option<String>,
    #[serde(rename = "type")]
    kind: IndexedAddressKind,
    script: bool,
}
pub(in crate::providers::blockfrost) fn address(
    bytes: &[u8],
    network: &Network,
    requested: &PaymentAddress,
) -> Result<AddressDetails, Error> {
    let v: AddressWire = decode(bytes)?;
    let returned = checked(PaymentAddress::parse(&v.address))?;
    if &returned != requested {
        return Err(invalid_response());
    }
    let stake_address = v
        .stake_address
        .map(|s| checked(StakeAddress::parse(&s)))
        .transpose()?;
    let balance = checked(AddressBalance::new(
        network.clone(),
        returned,
        assets(v.amount, network.identity())?,
    ))?;
    checked(AddressDetails::new(
        network.clone(),
        AddressData {
            balance,
            stake_address,
            address_kind: v.kind,
            script: v.script,
        },
    ))
}
#[derive(Deserialize)]
struct AccountWire {
    stake_address: String,
    active: bool,
    registered: bool,
    #[serde(deserialize_with = "required_option")]
    active_epoch: Option<u64>,
    controlled_amount: Lovelace,
    rewards_sum: Lovelace,
    withdrawals_sum: Lovelace,
    reserves_sum: Lovelace,
    treasury_sum: Lovelace,
    withdrawable_amount: Lovelace,
    #[serde(deserialize_with = "required_option")]
    pool_id: Option<String>,
    #[serde(deserialize_with = "required_option")]
    drep_id: Option<String>,
}
pub(in crate::providers::blockfrost) fn account(
    bytes: &[u8],
    network: &Network,
    requested: &StakeAddress,
) -> Result<StakeAccount, Error> {
    let v: AccountWire = decode(bytes)?;
    let address = checked(StakeAddress::parse(&v.stake_address))?;
    if &address != requested {
        return Err(invalid_response());
    }
    checked(StakeAccount::new(
        network.clone(),
        StakeAccountData {
            address,
            active: v.active,
            registered: v.registered,
            active_epoch: v.active_epoch,
            controlled: v.controlled_amount,
            rewards: v.rewards_sum,
            withdrawals: v.withdrawals_sum,
            reserves: v.reserves_sum,
            treasury: v.treasury_sum,
            withdrawable: v.withdrawable_amount,
            pool: v.pool_id.map(|s| checked(PoolId::parse(&s))).transpose()?,
            drep: v.drep_id.map(|s| checked(DrepId::parse(&s))).transpose()?,
        },
    ))
}
#[derive(Deserialize)]
struct Offchain {
    name: SourceText,
    description: SourceText,
    #[serde(deserialize_with = "required_option")]
    ticker: Option<SourceText>,
    #[serde(deserialize_with = "required_option")]
    url: Option<SourceText>,
    #[serde(deserialize_with = "required_option")]
    decimals: Option<u8>,
}
struct Presence(MetadataAvailability);
impl Default for Presence {
    fn default() -> Self {
        Self(MetadataAvailability::Unreported)
    }
}
impl<'de> Deserialize<'de> for Presence {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Box::<RawValue>::deserialize(d)?;
        if v.get() == "null" {
            Ok(Self(MetadataAvailability::Null))
        } else if v.get().starts_with('{') {
            Ok(Self(MetadataAvailability::PresentUninterpreted))
        } else {
            Err(serde::de::Error::custom("invalid provider metadata"))
        }
    }
}
#[derive(Deserialize)]
struct AssetWire {
    asset: String,
    policy_id: String,
    #[serde(deserialize_with = "required_option")]
    asset_name: Option<String>,
    fingerprint: String,
    quantity: String,
    initial_mint_tx_hash: String,
    mint_or_burn_count: u64,
    #[serde(deserialize_with = "required_option")]
    metadata: Option<Offchain>,
    #[serde(default)]
    onchain_metadata: Presence,
}
pub(in crate::providers::blockfrost) fn asset(
    bytes: &[u8],
    network: &Network,
    requested: &AssetId,
) -> Result<AssetDetails, Error> {
    let v: AssetWire = decode(bytes)?;
    let asset = checked(AssetId::from_unit(network.identity(), &v.asset))?;
    let AssetKind::Token {
        policy_id,
        asset_name,
    } = asset.asset()
    else {
        return Err(invalid_response());
    };
    if &asset != requested
        || checked(PolicyId::parse(&v.policy_id))? != *policy_id
        || !v.asset_name.map_or_else(
            || asset_name.bytes().is_empty(),
            |s| AssetName::parse(&s).is_ok_and(|n| &n == asset_name),
        )
        || checked(asset_fingerprint(&asset))? != v.fingerprint
    {
        return Err(invalid_response());
    }
    checked(AssetDetails::new(
        network.clone(),
        AssetData {
            asset,
            quantity: checked(Amount::from_decimal(&v.quantity, None))?,
            initial_mint: checked(Hash::parse(&v.initial_mint_tx_hash))?,
            mint_or_burn_count: v.mint_or_burn_count,
            metadata: v.metadata.map(|m| TokenMetadata {
                name: m.name,
                description: m.description,
                ticker: m.ticker,
                url: m.url,
                decimals: m.decimals,
            }),
            onchain_metadata: v.onchain_metadata.0,
        },
    ))
}
#[derive(Deserialize)]
struct AssetEntryWire {
    asset: String,
    quantity: String,
}
pub(in crate::providers::blockfrost) fn asset_page(
    bytes: &[u8],
    network: &Network,
    page: PageRequest,
) -> Result<IndexPage<AssetEntry>, Error> {
    let rows: Vec<AssetEntryWire> = decode(bytes)?;
    if rows.len() > usize::from(page.count()) {
        return Err(invalid_response());
    }
    let items = rows
        .into_iter()
        .map(|v| {
            Ok(AssetEntry {
                asset: checked(AssetId::from_unit(network.identity(), &v.asset))?,
                quantity: checked(Amount::from_decimal(&v.quantity, None))?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    checked(IndexPage::new(
        network.clone(),
        PageTarget::Assets,
        page,
        items,
    ))
}
#[derive(Deserialize)]
struct ReferenceWire {
    tx_hash: String,
    tx_index: u32,
    block_height: u64,
    block_time: u64,
}
pub(in crate::providers::blockfrost) fn transaction_page(
    bytes: &[u8],
    network: &Network,
    target: PageTarget,
    page: PageRequest,
) -> Result<IndexPage<TransactionReference>, Error> {
    let rows: Vec<ReferenceWire> = decode(bytes)?;
    if rows.len() > usize::from(page.count()) {
        return Err(invalid_response());
    }
    let items = rows
        .into_iter()
        .map(|v| {
            Ok(TransactionReference {
                transaction_id: checked(Hash::parse(&v.tx_hash))?,
                index: v.tx_index,
                block_height: v.block_height,
                block_unix_seconds: v.block_time,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    for pair in items.windows(2) {
        let left = (pair[0].block_height, pair[0].index);
        let right = (pair[1].block_height, pair[1].index);
        if match page.order() {
            Order::Asc => left >= right,
            Order::Desc => left <= right,
        } {
            return Err(invalid_response());
        }
    }
    checked(IndexPage::new(network.clone(), target, page, items))
}
#[derive(Deserialize)]
struct HolderWire {
    address: String,
    quantity: String,
}
pub(in crate::providers::blockfrost) fn holder_page(
    bytes: &[u8],
    network: &Network,
    asset: AssetId,
    page: PageRequest,
) -> Result<IndexPage<AssetHolder>, Error> {
    let rows: Vec<HolderWire> = decode(bytes)?;
    if rows.len() > usize::from(page.count()) {
        return Err(invalid_response());
    }
    let items = rows
        .into_iter()
        .map(|v| {
            Ok(AssetHolder {
                address: checked(PaymentAddress::parse(&v.address))?,
                quantity: checked(Amount::from_decimal(&v.quantity, None))?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    checked(IndexPage::new(
        network.clone(),
        PageTarget::AssetHolders { asset },
        page,
        items,
    ))
}
#[derive(Deserialize)]
struct RewardWire {
    epoch: u64,
    amount: Lovelace,
    pool_id: String,
    #[serde(rename = "type")]
    kind: RewardKind,
}
pub(in crate::providers::blockfrost) fn reward_page(
    bytes: &[u8],
    network: &Network,
    address: StakeAddress,
    page: PageRequest,
) -> Result<IndexPage<Reward>, Error> {
    let rows: Vec<RewardWire> = decode(bytes)?;
    if rows.len() > usize::from(page.count()) {
        return Err(invalid_response());
    }
    let items = rows
        .into_iter()
        .map(|v| {
            Ok(Reward {
                epoch: v.epoch,
                amount: v.amount,
                pool: checked(PoolId::parse(&v.pool_id))?,
                kind: v.kind,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    checked(IndexPage::new(
        network.clone(),
        PageTarget::Rewards { address },
        page,
        items,
    ))
}
