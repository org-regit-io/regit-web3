// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{collections::HashSet, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    domain::{Amount, U256},
    error::{Error, ValidationError},
};

use super::{AssetId, AssetKind, Hash, Network, NetworkId, PaymentAddress, ScriptHash};

/// An exact asset quantity and independently supplied optional precision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AssetAmountFields")]
pub struct AssetAmount {
    asset_id: AssetId,
    amount: Amount,
}

impl AssetAmount {
    /// Records exact base units; native ADA requires six decimal places.
    ///
    /// # Errors
    /// Rejects native precision other than six. Token precision remains optional.
    pub fn new(asset_id: AssetId, raw: U256, decimals: Option<u8>) -> Result<Self, Error> {
        if matches!(asset_id.asset(), AssetKind::Native) && decimals != Some(6) {
            return Err(ValidationError::DecimalMismatch.into());
        }
        Ok(Self {
            asset_id,
            amount: Amount::new(raw, decimals),
        })
    }
    /// Returns technical identity without display aliases or precision.
    #[must_use]
    pub const fn asset_id(&self) -> &AssetId {
        &self.asset_id
    }
    /// Returns exact units and optional precision.
    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.amount
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetAmountFields {
    asset_id: AssetId,
    amount: Amount,
}

impl TryFrom<AssetAmountFields> for AssetAmount {
    type Error = Error;
    fn try_from(fields: AssetAmountFields) -> Result<Self, Error> {
        Self::new(
            fields.asset_id,
            fields.amount.raw(),
            fields.amount.decimals(),
        )
    }
}

fn validate_assets(values: &[AssetAmount], network: NetworkId, output: bool) -> Result<(), Error> {
    if values.len() > 10_000 {
        return Err(ValidationError::InvalidCardanoRecord.into());
    }
    let mut identities = HashSet::with_capacity(values.len());
    for value in values {
        if value.asset_id().network() != network {
            return Err(ValidationError::NetworkMismatch.into());
        }
        if !identities.insert(value.asset_id()) {
            return Err(ValidationError::InvalidCardanoRecord.into());
        }
        if output {
            if value.amount().raw() > U256::from(u64::MAX) {
                return Err(ValidationError::CardanoAmountOverflow.into());
            }
            if matches!(value.asset_id().asset(), AssetKind::Token { .. })
                && value.amount().raw().is_zero()
            {
                return Err(ValidationError::InvalidCardanoRecord.into());
            }
        }
    }
    if output
        && !values
            .iter()
            .any(|value| matches!(value.asset_id().asset(), AssetKind::Native))
    {
        return Err(ValidationError::InvalidCardanoRecord.into());
    }
    Ok(())
}

fn bounded_values<'de, D, T, const MAX: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct ValuesVisitor<T, const MAX: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const MAX: usize> de::Visitor<'de> for ValuesVisitor<T, MAX> {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded collection of Cardano records")
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Vec<T>, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = sequence.next_element::<T>()? {
                if values.len() == MAX {
                    return Err(de::Error::custom(ValidationError::InvalidCardanoRecord));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(ValuesVisitor::<T, MAX>(std::marker::PhantomData))
}

fn deserialize_assets<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<AssetAmount>, D::Error> {
    bounded_values::<D, AssetAmount, 10_000>(deserializer)
}

/// Current indexed asset balances for one payment address.
///
/// An explicitly empty indexed result remains empty; omitted native units are
/// not replaced with a fabricated zero. Aggregate quantities retain 256 bits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AddressBalanceFields")]
pub struct AddressBalance {
    network: Network,
    address: PaymentAddress,
    assets: Vec<AssetAmount>,
}

impl AddressBalance {
    /// Records exact unique assets for a compatible address and technical network.
    ///
    /// # Errors
    /// Rejects network mismatches, duplicate identities or more than 10,000 assets.
    pub fn new(
        network: Network,
        address: PaymentAddress,
        assets: Vec<AssetAmount>,
    ) -> Result<Self, Error> {
        if !address.is_compatible_with(network.identity()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        validate_assets(&assets, network.identity(), false)?;
        Ok(Self {
            network,
            address,
            assets,
        })
    }
    /// Returns the technical network and retained alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns the indexed address.
    #[must_use]
    pub const fn address(&self) -> &PaymentAddress {
        &self.address
    }
    /// Returns exactly the supplied asset entries.
    #[must_use]
    pub fn assets(&self) -> &[AssetAmount] {
        &self.assets
    }
    /// Returns explicitly reported ADA units, or absence of a native entry.
    #[must_use]
    pub fn native_amount(&self) -> Option<&Amount> {
        self.assets
            .iter()
            .find(|value| matches!(value.asset_id().asset(), AssetKind::Native))
            .map(AssetAmount::amount)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddressBalanceFields {
    network: Network,
    address: PaymentAddress,
    #[serde(deserialize_with = "deserialize_assets")]
    assets: Vec<AssetAmount>,
}
impl TryFrom<AddressBalanceFields> for AddressBalance {
    type Error = Error;
    fn try_from(fields: AddressBalanceFields) -> Result<Self, Error> {
        Self::new(fields.network, fields.address, fields.assets)
    }
}

/// Bounded exact hexadecimal data such as a supplied inline datum.
///
/// The application bound is one MiB, independently of era-specific ledger
/// limits. Encoding validity does not verify a datum's CBOR or script semantics.
#[derive(Clone, Eq, PartialEq)]
pub struct HexData(Vec<u8>);

impl HexData {
    /// Maximum retained bytes for one opaque value.
    pub const MAX_BYTES: usize = 1024 * 1024;
    /// Records bounded exact bytes.
    ///
    /// # Errors
    /// Rejects values exceeding the declared application bound.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(ValidationError::InvalidCardanoRecord.into());
        }
        Ok(Self(bytes))
    }
    /// Parses bounded unprefixed hexadecimal, including an empty value.
    ///
    /// # Errors
    /// Rejects malformed, prefixed or excessive data.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() > Self::MAX_BYTES * 2
            || !value.len().is_multiple_of(2)
            || value.starts_with("0x")
            || value.starts_with("0X")
        {
            return Err(ValidationError::InvalidCardanoRecord.into());
        }
        Self::from_bytes(
            const_hex::decode(value)
                .map_err(|_| Error::from(ValidationError::InvalidCardanoRecord))?,
        )
    }
    /// Returns exact opaque bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for HexData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HexData")
            .field("bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}
impl Serialize for HexData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&const_hex::encode(&self.0))
    }
}
impl<'de> Deserialize<'de> for HexData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Explicit optional output datum and reference-script fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputData {
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    datum_hash: Option<Hash>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    inline_datum: Option<HexData>,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    reference_script_hash: Option<ScriptHash>,
}
impl OutputData {
    /// Records independently supplied opaque fields, including explicit absence.
    #[must_use]
    pub const fn new(
        datum_hash: Option<Hash>,
        inline_datum: Option<HexData>,
        reference_script_hash: Option<ScriptHash>,
    ) -> Self {
        Self {
            datum_hash,
            inline_datum,
            reference_script_hash,
        }
    }
    /// Returns a supplied datum hash without verifying a preimage.
    #[must_use]
    pub const fn datum_hash(&self) -> Option<Hash> {
        self.datum_hash
    }
    /// Returns supplied opaque inline bytes.
    #[must_use]
    pub const fn inline_datum(&self) -> Option<&HexData> {
        self.inline_datum.as_ref()
    }
    /// Returns the supplied reference-script hash.
    #[must_use]
    pub const fn reference_script_hash(&self) -> Option<ScriptHash> {
        self.reference_script_hash
    }
}

/// A current indexed unspent transaction output with its creation block.
///
/// The creation block is not the block at which the indexer's unspent state
/// was evaluated. Inline datum bytes are opaque and retained separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "UtxoFields")]
pub struct Utxo {
    network: Network,
    address: PaymentAddress,
    transaction_id: Hash,
    output_index: u16,
    assets: Vec<AssetAmount>,
    creation_block: Hash,
    data: OutputData,
}

impl Utxo {
    /// Validates explicit output identity, exact values and optional datum fields.
    ///
    /// # Errors
    /// Rejects address/network mismatches, duplicate assets, missing ADA,
    /// unsigned 64-bit overflow or zero quantities for output token entries.
    pub fn new(
        network: Network,
        address: PaymentAddress,
        transaction_id: Hash,
        output_index: u16,
        assets: Vec<AssetAmount>,
        creation_block: Hash,
        data: OutputData,
    ) -> Result<Self, Error> {
        if !address.is_compatible_with(network.identity()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        validate_assets(&assets, network.identity(), true)?;
        Ok(Self {
            network,
            address,
            transaction_id,
            output_index,
            assets,
            creation_block,
            data,
        })
    }
    /// Returns technical network and the supplied display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns this output's payment address.
    #[must_use]
    pub const fn address(&self) -> &PaymentAddress {
        &self.address
    }
    /// Returns the exact creating transaction identity.
    #[must_use]
    pub const fn transaction_id(&self) -> Hash {
        self.transaction_id
    }
    /// Returns the output index within its transaction.
    #[must_use]
    pub const fn output_index(&self) -> u16 {
        self.output_index
    }
    /// Returns exact per-output values.
    #[must_use]
    pub fn assets(&self) -> &[AssetAmount] {
        &self.assets
    }
    /// Returns the creating block, without claiming an evaluation snapshot.
    #[must_use]
    pub const fn creation_block(&self) -> Hash {
        self.creation_block
    }
    /// Returns an explicitly supplied datum hash, or its absence.
    #[must_use]
    pub const fn datum_hash(&self) -> Option<Hash> {
        self.data.datum_hash()
    }
    /// Returns opaque supplied inline datum bytes, independently of its hash.
    #[must_use]
    pub const fn inline_datum(&self) -> Option<&HexData> {
        self.data.inline_datum()
    }
    /// Returns the supplied 28-byte reference-script hash, or absence.
    #[must_use]
    pub const fn reference_script_hash(&self) -> Option<ScriptHash> {
        self.data.reference_script_hash()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UtxoFields {
    network: Network,
    address: PaymentAddress,
    transaction_id: Hash,
    output_index: u16,
    #[serde(deserialize_with = "deserialize_assets")]
    assets: Vec<AssetAmount>,
    creation_block: Hash,
    data: OutputData,
}
impl TryFrom<UtxoFields> for Utxo {
    type Error = Error;
    fn try_from(fields: UtxoFields) -> Result<Self, Error> {
        Self::new(
            fields.network,
            fields.address,
            fields.transaction_id,
            fields.output_index,
            fields.assets,
            fields.creation_block,
            fields.data,
        )
    }
}

/// Explicit indexed listing order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    /// Oldest outputs first.
    Asc,
    /// Newest outputs first.
    Desc,
}

/// A bounded page request, with no implicit unbounded page gathering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PageRequestFields")]
pub struct PageRequest {
    page: u32,
    count: u8,
    order: Order,
}
impl PageRequest {
    /// Records a supported page and count.
    ///
    /// # Errors
    /// Requires page 1–21,474,836 and count 1–100.
    pub fn new(page: u32, count: u8, order: Order) -> Result<Self, Error> {
        if !(1..=21_474_836).contains(&page) || !(1..=100).contains(&count) {
            return Err(ValidationError::InvalidPageRequest.into());
        }
        Ok(Self { page, count, order })
    }
    /// Returns the explicitly requested page.
    #[must_use]
    pub const fn page(self) -> u32 {
        self.page
    }
    /// Returns the maximum items in this page.
    #[must_use]
    pub const fn count(self) -> u8 {
        self.count
    }
    /// Returns the explicit listing order.
    #[must_use]
    pub const fn order(self) -> Order {
        self.order
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageRequestFields {
    page: u32,
    count: u8,
    order: Order,
}
impl TryFrom<PageRequestFields> for PageRequest {
    type Error = Error;
    fn try_from(fields: PageRequestFields) -> Result<Self, Error> {
        Self::new(fields.page, fields.count, fields.order)
    }
}

/// Completeness limited to one response from a changing index.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    /// A full page may have further results; no total is supplied.
    MayHaveMore,
    /// This response has fewer items than requested; subsequent index state may change.
    ShortPage,
}

/// One indexed output page with its explicit request and honest completeness.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "UtxoPageFields")]
pub struct UtxoPage {
    network: Network,
    address: PaymentAddress,
    requested_page: PageRequest,
    status: PageStatus,
    outputs: Vec<Utxo>,
}
impl UtxoPage {
    /// Records one page, preserving address/network identity and unique outpoints.
    ///
    /// # Errors
    /// Rejects excessive items, duplicate outpoints or mismatched output identities.
    pub fn new(
        network: Network,
        address: PaymentAddress,
        requested_page: PageRequest,
        outputs: Vec<Utxo>,
    ) -> Result<Self, Error> {
        if !address.is_compatible_with(network.identity()) {
            return Err(ValidationError::NetworkMismatch.into());
        }
        if outputs.len() > usize::from(requested_page.count()) {
            return Err(ValidationError::InvalidCardanoRecord.into());
        }
        let mut outpoints = HashSet::with_capacity(outputs.len());
        for output in &outputs {
            if output.network().identity() != network.identity() {
                return Err(ValidationError::NetworkMismatch.into());
            }
            if output.address() != &address
                || !outpoints.insert((output.transaction_id(), output.output_index()))
            {
                return Err(ValidationError::InvalidCardanoRecord.into());
            }
        }
        let status = if outputs.len() == usize::from(requested_page.count()) {
            PageStatus::MayHaveMore
        } else {
            PageStatus::ShortPage
        };
        Ok(Self {
            network,
            address,
            requested_page,
            status,
            outputs,
        })
    }
    /// Returns the requested network with retained alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns the requested payment address.
    #[must_use]
    pub const fn address(&self) -> &PaymentAddress {
        &self.address
    }
    /// Returns the exact page/count/order request.
    #[must_use]
    pub const fn requested_page(&self) -> PageRequest {
        self.requested_page
    }
    /// Returns honest per-response completeness.
    #[must_use]
    pub const fn status(&self) -> PageStatus {
        self.status
    }
    /// Returns the retained output page.
    #[must_use]
    pub fn outputs(&self) -> &[Utxo] {
        &self.outputs
    }
}
fn deserialize_outputs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Utxo>, D::Error> {
    bounded_values::<D, Utxo, 100>(deserializer)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UtxoPageFields {
    network: Network,
    address: PaymentAddress,
    requested_page: PageRequest,
    status: PageStatus,
    #[serde(deserialize_with = "deserialize_outputs")]
    outputs: Vec<Utxo>,
}
impl TryFrom<UtxoPageFields> for UtxoPage {
    type Error = Error;
    fn try_from(fields: UtxoPageFields) -> Result<Self, Error> {
        let value = Self::new(
            fields.network,
            fields.address,
            fields.requested_page,
            fields.outputs,
        )?;
        if value.status() != fields.status {
            return Err(ValidationError::InvalidCardanoRecord.into());
        }
        Ok(value)
    }
}
