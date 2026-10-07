// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    AssetId, AssetKind, Hash, Network, NetworkId, PageRequest, PageStatus, PaymentAddress,
    StakeAddress,
};
use crate::{
    domain::{Amount, ExactDecimal, U256},
    error::{Error, ValidationError},
};

pub(super) fn invalid() -> Error {
    ValidationError::InvalidCardanoRecord.into()
}

/// Exact unsigned lovelace base units, retaining up to 256 bits for indexed aggregates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Lovelace(U256);
impl Lovelace {
    /// Records exact indexed base units without conversion to floating point.
    #[must_use]
    pub const fn new(raw: U256) -> Self {
        Self(raw)
    }
    /// Parses canonical unsigned decimal base units.
    ///
    /// # Errors
    /// Rejects signs, fractions, exponents, leading zeros and values exceeding 256 bits.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.is_empty()
            || text.len() > 78
            || !text.bytes().all(|b| b.is_ascii_digit())
            || (text.len() > 1 && text.starts_with('0'))
        {
            return Err(invalid());
        }
        Ok(Self(Amount::from_decimal(text, None)?.raw()))
    }
    /// Returns exact base units without changing the amount's denomination.
    #[must_use]
    pub const fn raw(self) -> U256 {
        self.0
    }
    /// Returns an ADA amount with six decimal places.
    #[must_use]
    pub const fn amount(self) -> Amount {
        Amount::new(self.0, Some(6))
    }
    /// Returns supported ordinary-payment width in lovelaces.
    ///
    /// # Errors
    /// Rejects indexed aggregates that cannot fit a transaction's unsigned 64-bit coin.
    pub fn payment_coin(self) -> Result<u64, Error> {
        u64::try_from(self.0).map_err(|_| ValidationError::CardanoAmountOverflow.into())
    }
}
impl Serialize for Lovelace {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for Lovelace {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Bounded exact provider display text; it does not identify an address or asset.
/// UTF-8/control bytes are preserved and escaped by JSON and `Debug` formatting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceText(String);
impl SourceText {
    /// Retains at most 4096 UTF-8 bytes without stripping source data.
    ///
    /// # Errors
    /// Rejects text above the explicit byte ceiling.
    pub fn new(text: impl Into<String>) -> Result<Self, Error> {
        let text = text.into();
        if text.len() > 4096 {
            return Err(invalid());
        }
        Ok(Self(text))
    }
    /// Returns the exact source text, independently of a display interpretation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for SourceText {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<SourceText> for String {
    fn from(v: SourceText) -> Self {
        v.0
    }
}

/// Explicit epoch selection; latest remains a changing provider index.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "epoch",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EpochSelector {
    /// Provider's current epoch at the time of this one request.
    Latest,
    /// The exact requested epoch number, including genesis epoch zero.
    Number(u64),
}

/// Source supply/stake facts in exact lovelaces; no cross-request snapshot is asserted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkSupply {
    /// Source maximum supply.
    pub maximum: Lovelace,
    /// Source current total supply.
    pub total: Lovelace,
    /// Source circulating supply.
    pub circulating: Lovelace,
    /// Source script-locked supply.
    pub locked: Lovelace,
    /// Source treasury supply.
    pub treasury: Lovelace,
    /// Source reserve supply.
    pub reserves: Lovelace,
    /// Source live delegated stake.
    pub live_stake: Lovelace,
    /// Source active stake.
    pub active_stake: Lovelace,
}

/// Source facts about one selected epoch, with Unix-second units kept explicit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpochData {
    /// Exact source epoch number.
    pub epoch: u64,
    /// Source start in Unix seconds.
    pub start_unix_seconds: u64,
    /// Source end in Unix seconds.
    pub end_unix_seconds: u64,
    /// Source first block time in Unix seconds.
    pub first_block_unix_seconds: u64,
    /// Source last block time in Unix seconds.
    pub last_block_unix_seconds: u64,
    /// Indexed block count, which can change for a current epoch.
    pub block_count: u64,
    /// Indexed transaction count.
    pub transaction_count: u64,
    /// Aggregate output base units, not a balance or supply.
    pub output: Lovelace,
    /// Aggregate source transaction fees.
    pub fees: Lovelace,
    /// Actual source nullable active stake; absence is not zero.
    pub active_stake: Option<Lovelace>,
}

/// Protocol parameters required by ordinary payment fee/output checks.
/// No Plutus cost model, reference-script fee or remote simulation is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentParameters {
    /// Source epoch of these parameters.
    pub epoch: u64,
    /// Exact linear minimum fee factor in lovelaces per serialized byte.
    pub min_fee_coefficient: u64,
    /// Exact minimum fee constant in lovelaces.
    pub min_fee_constant: u64,
    /// Source maximum encoded transaction size in bytes.
    pub max_transaction_bytes: u32,
    /// Nullable source maximum encoded value size in bytes.
    pub max_value_bytes: Option<u32>,
    /// Nullable source `UTxO` byte cost (Conway/Babbage) or word cost (Alonzo).
    /// The selected preparation era determines the supported interpretation.
    pub coins_per_utxo_size: Option<u64>,
    /// Exact source protocol major version, separate from the node release.
    pub protocol_major: u64,
    /// Exact source protocol minor version.
    pub protocol_minor: u64,
    /// Source key registration deposit, independent of this payment's fee.
    pub key_deposit: Lovelace,
    /// Source pool registration deposit.
    pub pool_deposit: Lovelace,
    /// Optional exact script-memory price, never converted through binary float.
    pub script_memory_price: Option<ExactDecimal>,
    /// Optional exact script-step price.
    pub script_step_price: Option<ExactDecimal>,
}

macro_rules! record {
    ($name:ident,$fields:ident,$data:ty,$doc:literal,$check:expr) => {
        #[doc=$doc]
        #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
        pub struct $name {
            network: Network,
            data: $data,
        }
        impl $name {
            /// Validates explicitly supplied source facts and retains their network.
            ///
            /// # Errors
            /// Rejects invalid source structure or incompatible family identities.
            pub fn new(network: Network, data: $data) -> Result<Self, Error> {
                let checked: Result<(), Error> = ($check)(&network, &data);
                checked?;
                Ok(Self { network, data })
            }
            /// Returns expected family network, without claiming an independent snapshot.
            #[must_use]
            pub const fn network(&self) -> &Network {
                &self.network
            }
            /// Returns immutable exact source facts.
            #[must_use]
            pub const fn data(&self) -> &$data {
                &self.data
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = $fields::deserialize(d)?;
                Self::new(v.network, v.data).map_err(serde::de::Error::custom)
            }
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $fields {
            network: Network,
            data: $data,
        }
        impl TryFrom<$fields> for $name {
            type Error = Error;
            fn try_from(v: $fields) -> Result<Self, Error> {
                Self::new(v.network, v.data)
            }
        }
    };
}
record!(
    NetworkData,
    NetworkDataFields,
    NetworkSupply,
    "Current indexed Cardano supply and stake with expected network attribution.",
    |_: &Network, _: &NetworkSupply| Ok(())
);
record!(
    Epoch,
    EpochFields,
    EpochData,
    "Indexed facts for a source-selected epoch.",
    |_: &Network, d: &EpochData| {
        if d.start_unix_seconds >= d.end_unix_seconds {
            Err(invalid())
        } else {
            Ok(())
        }
    }
);
record!(
    ProtocolParameters,
    ProtocolParametersFields,
    PaymentParameters,
    "Indexed source protocol parameters used by explicit pure payment profiles.",
    |_: &Network, d: &PaymentParameters| {
        if d.max_transaction_bytes == 0
            || d.script_memory_price
                .as_ref()
                .is_some_and(ExactDecimal::is_negative)
            || d.script_step_price
                .as_ref()
                .is_some_and(ExactDecimal::is_negative)
        {
            Err(invalid())
        } else {
            Ok(())
        }
    }
);

/// Extra source address facts, separate from an inferred account snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddressData {
    /// Full exact balance for the requested address.
    pub balance: super::AddressBalance,
    /// Source stake credential address, nullable for enterprise/Byron addresses.
    pub stake_address: Option<StakeAddress>,
    /// Source address classification.
    pub address_kind: IndexedAddressKind,
    /// Source script-payment flag.
    pub script: bool,
}
/// Provider address era classification, not an inferred network.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexedAddressKind {
    /// Bootstrap-era source address.
    Byron,
    /// Shelley-family source address.
    Shelley,
}
record!(
    AddressDetails,
    AddressDetailsFields,
    AddressData,
    "Indexed address facts and balances retained for the exact requested address.",
    |n: &Network, d: &AddressData| {
        if d.balance.network().identity() != n.identity()
            || d.stake_address
                .as_ref()
                .is_some_and(|a| !a.is_compatible_with(n.identity()))
        {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let address = d.balance.address().bytes();
        let kind = address[0] >> 4;
        if d.script != matches!(kind, 1 | 3 | 5 | 7)
            || (d.address_kind == IndexedAddressKind::Byron) != (kind == 8)
        {
            return Err(invalid());
        }
        if d.stake_address.is_some() && kind >= 6 {
            return Err(invalid());
        }
        if let Some(stake) = &d.stake_address
            && kind <= 3
        {
            let expected_header =
                if matches!(kind, 2 | 3) { 0xf0 } else { 0xe0 } | (address[0] & 15);
            if stake.bytes()[0] != expected_header || stake.bytes()[1..] != address[29..] {
                return Err(invalid());
            }
        }
        Ok(())
    }
);

/// A validated Bech32 pool operator identity (28 bytes, `pool` prefix).
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PoolId(String);
impl PoolId {
    /// Parses canonical lowercase pool identity using the maintained checksum codec.
    ///
    /// # Errors
    /// Rejects malformed checksum, prefix, width or noncanonical casing.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.len() > 100 {
            return Err(invalid());
        }
        let (hrp, bytes) = bech32::decode(text).map_err(|_| invalid())?;
        let canonical = bech32::encode::<bech32::Bech32>(hrp, &bytes).map_err(|_| invalid())?;
        if hrp.as_str() != "pool" || bytes.len() != 28 || canonical != text {
            return Err(invalid());
        }
        Ok(Self(canonical))
    }
    /// Returns the checksum-valid exact pool identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for PoolId {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<PoolId> for String {
    fn from(v: PoolId) -> Self {
        v.0
    }
}

/// Source governance delegation identifier, without claiming operator ownership.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DrepId(String);
impl DrepId {
    /// Retains an explicit always-choice or supported checksum-valid `DRep` syntax.
    ///
    /// # Errors
    /// Rejects invalid checksum/prefix/width/casing; accepts legacy 28-byte and
    /// CIP-129 29-byte forms without conflating their bytes.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.len() > 100 {
            return Err(invalid());
        }
        if matches!(text, "drep_always_abstain" | "drep_always_no_confidence") {
            return Ok(Self(text.into()));
        }
        let (hrp, bytes) = bech32::decode(text).map_err(|_| invalid())?;
        let canonical = bech32::encode::<bech32::Bech32>(hrp, &bytes).map_err(|_| invalid())?;
        let valid = (bytes.len() == 28 && matches!(hrp.as_str(), "drep" | "drep_script"))
            || (bytes.len() == 29 && hrp.as_str() == "drep" && matches!(bytes[0], 0x22 | 0x23));
        if !valid || canonical != text {
            return Err(invalid());
        }
        Ok(Self(canonical))
    }
    /// Returns exact source identifier syntax.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for DrepId {
    type Error = Error;
    fn try_from(v: String) -> Result<Self, Error> {
        Self::parse(&v)
    }
}
impl From<DrepId> for String {
    fn from(v: DrepId) -> Self {
        v.0
    }
}

/// Indexed stake account data; registration and delegation remain separate flags.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StakeAccountData {
    /// Exact requested reward address.
    pub address: StakeAddress,
    /// Source delegation activity, not registration.
    pub active: bool,
    /// Source registration state.
    pub registered: bool,
    /// Optional epoch of the most recent registration/deregistration action.
    pub active_epoch: Option<u64>,
    /// Source controlled lovelaces.
    pub controlled: Lovelace,
    /// Source aggregate rewards.
    pub rewards: Lovelace,
    /// Source aggregate withdrawals.
    pub withdrawals: Lovelace,
    /// Source aggregate reserve movements.
    pub reserves: Lovelace,
    /// Source aggregate treasury movements.
    pub treasury: Lovelace,
    /// Source currently withdrawable rewards.
    pub withdrawable: Lovelace,
    /// Source nullable delegated pool.
    pub pool: Option<PoolId>,
    /// Source nullable governance delegation.
    pub drep: Option<DrepId>,
}
record!(
    StakeAccount,
    StakeAccountFields,
    StakeAccountData,
    "Exact indexed stake/account facts without manufacturing zero rewards or delegation.",
    |n: &Network, d: &StakeAccountData| {
        if !d.address.is_compatible_with(n.identity()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(())
    }
);

/// Source off-chain token metadata; this does not replace policy/name identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenMetadata {
    /// Source name.
    pub name: SourceText,
    /// Source description.
    pub description: SourceText,
    /// Actual nullable source display ticker.
    pub ticker: Option<SourceText>,
    /// Actual nullable source URL text; no URL is fetched.
    pub url: Option<SourceText>,
    /// Actual nullable provider-reported precision, independent of base-unit identity.
    pub decimals: Option<u8>,
}
/// Explicit source arbitrary-metadata availability, not parsed standard compliance.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataAvailability {
    /// The provider did not supply this optional field.
    Unreported,
    /// The provider explicitly supplied null.
    Null,
    /// Source arbitrary metadata exists but is not interpreted as identity or code.
    PresentUninterpreted,
}
/// Typed indexed native-asset data with immutable full policy/name identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetData {
    /// Full exact requested asset identity.
    pub asset: AssetId,
    /// Source current supply, without inferred decimal precision.
    pub quantity: Amount,
    /// Source initial mint transaction.
    pub initial_mint: Hash,
    /// Source mint/burn event count.
    pub mint_or_burn_count: u64,
    /// Actual nullable source off-chain metadata.
    pub metadata: Option<TokenMetadata>,
    /// Arbitrary on-chain metadata is explicitly uninterpreted.
    pub onchain_metadata: MetadataAvailability,
}
record!(
    AssetDetails,
    AssetDetailsFields,
    AssetData,
    "Indexed asset quantity/provenance/display data correlated with its requested identity.",
    |n: &Network, d: &AssetData| {
        if d.asset.network() != n.identity() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        if matches!(d.asset.asset(), AssetKind::Native) || d.quantity.decimals().is_some() {
            return Err(invalid());
        }
        Ok(())
    }
);

/// Computes the CIP-14 fingerprint from exact policy/name bytes.
///
/// # Errors
/// Rejects ADA, which has no native-token policy/name fingerprint.
pub fn asset_fingerprint(asset: &AssetId) -> Result<String, Error> {
    let AssetKind::Token {
        policy_id,
        asset_name,
    } = asset.asset()
    else {
        return Err(invalid());
    };
    let mut bytes = policy_id.bytes().to_vec();
    bytes.extend_from_slice(asset_name.bytes());
    let digest = pallas_crypto::hash::Hasher::<160>::hash(&bytes);
    bech32::encode::<bech32::Bech32>(
        bech32::Hrp::parse("asset").map_err(|_| invalid())?,
        digest.as_ref(),
    )
    .map_err(|_| invalid())
}

/// Full transaction identity and source inclusion facts in one indexed listing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionReference {
    /// Exact transaction hash.
    pub transaction_id: Hash,
    /// Source index within the block.
    pub index: u32,
    /// Source block height.
    pub block_height: u64,
    /// Source block creation time in Unix seconds.
    pub block_unix_seconds: u64,
}
/// One indexed native-asset listing item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetEntry {
    /// Exact full token policy/name/network identity.
    pub asset: AssetId,
    /// Exact source indexed supply, with unreported precision.
    pub quantity: Amount,
}
/// One source holder entry for the explicitly requested asset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetHolder {
    /// Exact holder payment address.
    pub address: PaymentAddress,
    /// Exact token base units, with precision unreported.
    pub quantity: Amount,
}
/// Actual source reward category.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardKind {
    /// Source pool leader reward.
    Leader,
    /// Source delegation reward.
    Member,
    /// Source pool deposit refund.
    PoolDepositRefund,
}
/// A source stake reward; it does not mean the reward remains withdrawable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reward {
    /// Exact associated reward epoch.
    pub epoch: u64,
    /// Exact source reward lovelaces.
    pub amount: Lovelace,
    /// Exact source pool identity.
    pub pool: PoolId,
    /// Actual source reward category.
    pub kind: RewardKind,
}

/// Original typed target retained by one explicit bounded indexed page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PageTarget {
    /// Current native-asset catalogue page.
    Assets,
    /// Exact payment address's transaction page.
    AddressTransactions {
        /// Original requested payment address.
        address: PaymentAddress,
    },
    /// Exact token's transaction page.
    AssetTransactions {
        /// Original requested token identity.
        asset: AssetId,
    },
    /// Exact token's indexed holder page.
    AssetHolders {
        /// Original requested token identity.
        asset: AssetId,
    },
    /// Exact reward account's rewards page.
    Rewards {
        /// Original requested reward address.
        address: StakeAddress,
    },
}
/// Trusted extension for concrete typed indexed page items.
pub trait PageItem {
    /// Validates target/network correlation and returns a per-page uniqueness key.
    ///
    /// # Errors
    /// Rejects an inappropriate target, family identity or item structure.
    fn validate(&self, target: &PageTarget, network: NetworkId) -> Result<String, Error>;
}
impl PageItem for AssetEntry {
    fn validate(&self, target: &PageTarget, network: NetworkId) -> Result<String, Error> {
        if !matches!(target, PageTarget::Assets)
            || self.asset.network() != network
            || matches!(self.asset.asset(), AssetKind::Native)
            || self.quantity.decimals().is_some()
        {
            return Err(invalid());
        }
        Ok(asset_unit(&self.asset))
    }
}
impl PageItem for TransactionReference {
    fn validate(&self, target: &PageTarget, _: NetworkId) -> Result<String, Error> {
        if !matches!(
            target,
            PageTarget::AddressTransactions { .. } | PageTarget::AssetTransactions { .. }
        ) {
            return Err(invalid());
        }
        Ok(self.transaction_id.to_string())
    }
}
impl PageItem for AssetHolder {
    fn validate(&self, target: &PageTarget, network: NetworkId) -> Result<String, Error> {
        if !matches!(target, PageTarget::AssetHolders { .. })
            || !self.address.is_compatible_with(network)
            || self.quantity.decimals().is_some()
        {
            return Err(invalid());
        }
        Ok(self.address.to_string())
    }
}
impl PageItem for Reward {
    fn validate(&self, target: &PageTarget, _: NetworkId) -> Result<String, Error> {
        if !matches!(target, PageTarget::Rewards { .. }) {
            return Err(invalid());
        }
        Ok(format!(
            "{}:{}:{:?}",
            self.epoch,
            self.pool.as_str(),
            self.kind
        ))
    }
}
pub(crate) fn asset_unit(asset: &AssetId) -> String {
    match asset.asset() {
        AssetKind::Native => "lovelace".into(),
        AssetKind::Token {
            policy_id,
            asset_name,
        } => format!("{policy_id}{asset_name}"),
    }
}

/// One bounded source-index page; a short page is not permanent exhaustiveness.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IndexPage<T> {
    network: Network,
    target: PageTarget,
    request: PageRequest,
    status: PageStatus,
    items: Vec<T>,
}
impl<T: PageItem> IndexPage<T> {
    /// Validates at most the requested count, target/network identity and duplicates.
    ///
    /// # Errors
    /// Rejects mismatched targets, duplicate entries or excess items without truncation.
    pub fn new(
        network: Network,
        target: PageTarget,
        request: PageRequest,
        items: Vec<T>,
    ) -> Result<Self, Error> {
        match &target {
            PageTarget::AddressTransactions { address }
                if !address.is_compatible_with(network.identity()) =>
            {
                return Err(ValidationError::NetworkMismatch.into());
            }
            PageTarget::Rewards { address } if !address.is_compatible_with(network.identity()) => {
                return Err(ValidationError::NetworkMismatch.into());
            }
            PageTarget::AssetTransactions { asset } | PageTarget::AssetHolders { asset }
                if asset.network() != network.identity()
                    || matches!(asset.asset(), AssetKind::Native) =>
            {
                return Err(invalid());
            }
            _ => {}
        }
        if items.len() > usize::from(request.count()) {
            return Err(invalid());
        }
        let mut keys = HashSet::new();
        for item in &items {
            if !keys.insert(item.validate(&target, network.identity())?) {
                return Err(invalid());
            }
        }
        let status = if items.len() == usize::from(request.count()) {
            PageStatus::MayHaveMore
        } else {
            PageStatus::ShortPage
        };
        Ok(Self {
            network,
            target,
            request,
            status,
            items,
        })
    }
    /// Returns original expected network attribution.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns immutable original typed query target.
    #[must_use]
    pub const fn target(&self) -> &PageTarget {
        &self.target
    }
    /// Returns original explicit count/page/order.
    #[must_use]
    pub const fn request(&self) -> PageRequest {
        self.request
    }
    /// Returns honest per-response completeness.
    #[must_use]
    pub const fn status(&self) -> PageStatus {
        self.status
    }
    /// Returns every retained item in source response order.
    #[must_use]
    pub fn items(&self) -> &[T] {
        &self.items
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct IndexPageFields<T> {
    network: Network,
    target: PageTarget,
    request: PageRequest,
    status: PageStatus,
    #[serde(deserialize_with = "page_items")]
    items: Vec<T>,
}
fn page_items<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    super::value::bounded_values::<D, T, 100>(d)
}
impl<'de, T: PageItem + Deserialize<'de>> Deserialize<'de> for IndexPage<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = IndexPageFields::<T>::deserialize(d)?;
        let page =
            Self::new(v.network, v.target, v.request, v.items).map_err(serde::de::Error::custom)?;
        if page.status != v.status {
            return Err(serde::de::Error::custom(invalid()));
        }
        Ok(page)
    }
}
