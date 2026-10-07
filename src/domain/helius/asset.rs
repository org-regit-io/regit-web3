// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Cursor, MetadataValue, OwnerRequest, SourceText};
use crate::{
    domain::{
        ExactDecimal,
        solana::{Hash, Pubkey},
    },
    error::Error,
};
use serde::{Deserialize, Serialize};

/// Source asset content. URIs and display metadata are retained, never fetched.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetContent {
    /// Source metadata schema URI, when supplied.
    pub schema: Option<SourceText>,
    /// Source off-chain metadata URI.
    pub json_uri: Option<SourceText>,
    /// Bounded source media records.
    #[serde(deserialize_with = "super::bounded::optional_list")]
    pub files: Option<Vec<AssetFile>>,
    /// Named metadata, preserving heterogeneous attribute values exactly.
    pub metadata: Option<MetadataValue>,
    /// Named source links; these are not network operations.
    pub links: Option<MetadataValue>,
}
/// A provider-reported media reference, without URI safety or availability proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetFile {
    /// Source URI.
    pub uri: Option<SourceText>,
    /// Source CDN URI, retained separately.
    pub cdn_uri: Option<SourceText>,
    /// Source MIME label.
    pub mime: Option<SourceText>,
}
/// Source authority and scopes, without proof that it can authorize an operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAuthority {
    /// Encoding-valid authority account.
    pub address: Pubkey,
    /// Source scope labels.
    #[serde(deserialize_with = "super::bounded::list")]
    pub scopes: Vec<SourceText>,
}
/// Source compression facts; hashes are not a verified Merkle proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compression {
    /// Source compression eligibility.
    pub eligible: bool,
    /// Source current compression state.
    pub compressed: bool,
    /// Optional source data hash; empty uncompressed wire sentinels become absent.
    pub data_hash: Option<Hash>,
    /// Optional creator hash.
    pub creator_hash: Option<Hash>,
    /// Optional asset hash.
    pub asset_hash: Option<Hash>,
    /// Optional compression tree account.
    pub tree: Option<Pubkey>,
    /// Optional reported sequence, including zero.
    pub sequence: Option<u64>,
    /// Optional reported leaf index, including zero.
    pub leaf_id: Option<u64>,
}
/// Provider grouping. Values are not universally public keys or verified collections.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grouping {
    /// Source grouping key.
    pub key: SourceText,
    /// Source grouping value.
    pub value: SourceText,
    /// Optional actual source verification label.
    pub verified: Option<bool>,
    /// Optional source collection metadata.
    pub metadata: Option<MetadataValue>,
}
/// Exact source royalty facts. These do not enforce payment policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Royalty {
    /// Source royalty model label.
    pub model: SourceText,
    /// Source target account when supplied.
    pub target: Option<Pubkey>,
    /// Exact source fraction, retained separately from basis points.
    pub percent: ExactDecimal,
    /// Source basis points.
    pub basis_points: u16,
    /// Source primary-sale flag.
    pub primary_sale_happened: bool,
    /// Source royalty lock flag.
    pub locked: bool,
}
/// A provider-reported creator share and verification flag.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Creator {
    /// Creator account.
    pub address: Pubkey,
    /// Source percentage share, bounded to 100 without inventing absent creators.
    pub share: u8,
    /// Source verification flag, not independent signature proof.
    pub verified: bool,
}
/// Source ownership interpretation. Token ownership can have multiple holders.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ownership {
    /// Source frozen flag.
    pub frozen: bool,
    /// Source delegated flag.
    pub delegated: bool,
    /// Optional source delegate account.
    pub delegate: Option<Pubkey>,
    /// Source ownership model, commonly `single` or `token`.
    pub model: SourceText,
    /// Optional actual reported owner; absence stays unknown. The DAS empty
    /// owner sentinel for `token` ownership is represented as absent.
    pub owner: Option<Pubkey>,
}
/// Source NFT print supply; this is distinct from fungible raw token supply.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintSupply {
    /// Maximum source print supply; zero remains the actual source convention.
    pub maximum: u64,
    /// Current reported print supply.
    pub current: u64,
    /// Optional reported edition nonce.
    pub edition_nonce: Option<u8>,
}
/// Cached provider valuation, separate from chain quantities and retrieval time.
/// DAS price data may be cached and unavailable for assets outside its coverage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedPrice {
    /// Optional exact price per whole token; no transaction denomination is inferred.
    pub price_per_token: Option<ExactDecimal>,
    /// Optional exact reported total valuation.
    pub total_price: Option<ExactDecimal>,
    /// Source currency label, not a Solana asset identity.
    pub currency: SourceText,
}
/// Exact fungible token facts. Scaled display balances are not used as raw units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenInfo {
    /// Raw mint supply, when reported.
    pub supply: Option<u64>,
    /// Raw requested-owner token balance, when reported.
    pub balance: Option<u64>,
    /// Mint decimals, when reported.
    pub decimals: Option<u8>,
    /// Optional actual token program identity.
    pub token_program: Option<Pubkey>,
    /// Actual source-associated token account, without local ATA derivation.
    pub associated_token_address: Option<Pubkey>,
    /// Optional actual mint authority.
    pub mint_authority: Option<Pubkey>,
    /// Optional actual freeze authority.
    pub freeze_authority: Option<Pubkey>,
    /// Optional source display symbol.
    pub symbol: Option<SourceText>,
    /// Optional cached valuation, without a data-age assertion.
    pub price: Option<CachedPrice>,
}
/// Supported source asset fields. Unmodeled extensions are identified separately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetData {
    /// Exact source DAS identity; compressed IDs need not be mint addresses.
    pub id: Pubkey,
    /// Optional actual source index progress attached to this particular asset.
    pub last_indexed_slot: Option<u64>,
    /// Source token/interface label, with unknown future labels preserved.
    pub interface: SourceText,
    /// Optional source content, never fetched by the library.
    pub content: Option<AssetContent>,
    /// Bounded source authorities.
    #[serde(deserialize_with = "super::bounded::optional_list")]
    pub authorities: Option<Vec<AssetAuthority>>,
    /// Optional source compression facts.
    pub compression: Option<Compression>,
    /// Bounded source grouping facts.
    #[serde(deserialize_with = "super::bounded::optional_list")]
    pub grouping: Option<Vec<Grouping>>,
    /// Optional source royalties.
    pub royalty: Option<Royalty>,
    /// Bounded source creator facts.
    #[serde(deserialize_with = "super::bounded::optional_list")]
    pub creators: Option<Vec<Creator>>,
    /// Optional actual source ownership facts.
    pub ownership: Option<Ownership>,
    /// Optional source NFT print-supply facts.
    pub print_supply: Option<PrintSupply>,
    /// Optional source mutability flag.
    pub mutable: Option<bool>,
    /// Optional source burn flag.
    pub burnt: Option<bool>,
    /// Optional exact fungible token facts.
    pub token_info: Option<TokenInfo>,
    /// Optional source agent flag.
    pub is_agent: Option<bool>,
    /// Optional source agent-token identity.
    pub agent_token: Option<Pubkey>,
    /// Optional source asset-signer account.
    pub asset_signer: Option<Pubkey>,
    /// Source extensions whose semantics this profile deliberately does not decode.
    #[serde(deserialize_with = "super::bounded::list")]
    pub unsupported_extensions: Vec<SourceText>,
}
/// Validated bounded asset record; validation does not prove chain existence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AssetData", into = "AssetData")]
pub struct Asset(AssetData);
impl Asset {
    /// Checks source value widths, percentage bounds and collection capacity.
    /// # Errors
    /// Rejects malformed or excessive source records without diagnostic content.
    pub fn new(data: AssetData) -> Result<Self, Error> {
        let one = ExactDecimal::parse("1")?;
        if data.authorities.as_ref().is_some_and(|v| v.len() > 256)
            || data.grouping.as_ref().is_some_and(|v| v.len() > 256)
            || data.creators.as_ref().is_some_and(|v| v.len() > 256)
            || data.unsupported_extensions.len() > 128
            || data
                .authorities
                .as_ref()
                .is_some_and(|v| v.iter().any(|a| a.scopes.len() > 64))
            || data
                .content
                .as_ref()
                .is_some_and(|c| c.files.as_ref().is_some_and(|v| v.len() > 256))
            || data
                .creators
                .as_ref()
                .is_some_and(|v| v.iter().any(|c| c.share > 100))
            || data.royalty.as_ref().is_some_and(|r| {
                r.basis_points > 10000 || r.percent.is_negative() || r.percent > one
            })
            || data
                .token_info
                .as_ref()
                .and_then(|t| t.price.as_ref())
                .is_some_and(|p| {
                    p.price_per_token
                        .as_ref()
                        .is_some_and(ExactDecimal::is_negative)
                        || p.total_price
                            .as_ref()
                            .is_some_and(ExactDecimal::is_negative)
                })
        {
            return Err(super::bounded::invalid_asset());
        }
        if let Some(r) = &data.royalty {
            let expected = ExactDecimal::parse(&format!(
                "{}.{:04}",
                r.basis_points / 10000,
                r.basis_points % 10000
            ))?;
            if r.percent != expected {
                return Err(super::bounded::invalid_asset());
            }
        }
        if data
            .print_supply
            .as_ref()
            .is_some_and(|s| s.maximum != 0 && s.current > s.maximum)
        {
            return Err(super::bounded::invalid_asset());
        }
        Ok(Self(data))
    }
    /// Returns exact source fields and explicitly unsupported extensions.
    #[must_use]
    pub const fn data(&self) -> &AssetData {
        &self.0
    }
    /// Returns the source asset identity.
    #[must_use]
    pub const fn id(&self) -> Pubkey {
        self.0.id
    }
}
impl TryFrom<AssetData> for Asset {
    type Error = Error;
    fn try_from(d: AssetData) -> Result<Self, Error> {
        Self::new(d)
    }
}
impl From<Asset> for AssetData {
    fn from(a: Asset) -> Self {
        a.0
    }
}

/// Optional DAS-reported native balance and cached valuation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBalance {
    /// Exact raw SOL lamports.
    pub lamports: u64,
    /// Optional exact cached price per SOL.
    pub price_per_sol: Option<ExactDecimal>,
    /// Optional exact cached total value.
    pub total_price: Option<ExactDecimal>,
}
/// One explicit owner-assets page; source totals and cursor are not snapshot proof.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OwnerPageFields")]
pub struct OwnerPage {
    request: OwnerRequest,
    items: Vec<Asset>,
    reported_total: u64,
    reported_grand_total: Option<u64>,
    reported_page: Option<u32>,
    cursor: Option<Cursor>,
    native_balance: Option<NativeBalance>,
}
impl OwnerPage {
    /// Validates one page against immutable owner/query controls.
    /// # Errors
    /// Rejects excessive items, duplicate identities, contradictory single ownership,
    /// unexpected optional data or negative cached valuation.
    pub fn new(
        request: OwnerRequest,
        items: Vec<Asset>,
        reported_total: u64,
        reported_grand_total: Option<u64>,
        reported_page: Option<u32>,
        cursor: Option<Cursor>,
        native_balance: Option<NativeBalance>,
    ) -> Result<Self, Error> {
        if let super::AssetPosition::Page { page } = request.position()
            && reported_page.is_some_and(|reported| reported != *page)
        {
            return Err(super::bounded::invalid_asset());
        }
        let mut ids = std::collections::BTreeSet::new();
        if items.len() > usize::from(request.limit())
            || items.iter().any(|a| {
                !ids.insert(a.id())
                    || a.data().ownership.as_ref().is_some_and(|o| {
                        o.model.as_str() == "single"
                            && o.owner.is_some_and(|owner| owner != request.owner())
                    })
            })
            || native_balance.is_some() && !request.show_native_balance()
            || reported_grand_total.is_some() && !request.show_grand_total()
            || native_balance.as_ref().is_some_and(|b| {
                b.price_per_sol
                    .as_ref()
                    .is_some_and(ExactDecimal::is_negative)
                    || b.total_price
                        .as_ref()
                        .is_some_and(ExactDecimal::is_negative)
            })
            || cursor.is_some()
                && !matches!(request.position(), super::AssetPosition::Cursor { .. })
        {
            return Err(super::bounded::invalid_asset());
        }
        if reported_total < items.len() as u64
            || reported_grand_total.is_some_and(|g| g < reported_total)
        {
            return Err(super::bounded::invalid_asset());
        }
        let mut prior: Option<[u8; 32]> = None;
        for item in &items {
            let id = item.id().bytes();
            if let super::AssetPosition::Range { before, after } = request.position()
                && (before.is_some_and(|v| id >= v.bytes())
                    || after.is_some_and(|v| id <= v.bytes()))
            {
                return Err(super::bounded::invalid_asset());
            }
            if request.sort() == super::AssetSort::Id
                && prior.is_some_and(|p| match request.direction() {
                    super::SortDirection::Asc => p >= id,
                    super::SortDirection::Desc => p <= id,
                })
            {
                return Err(super::bounded::invalid_asset());
            }
            prior = Some(id);
        }
        Ok(Self {
            request,
            items,
            reported_total,
            reported_grand_total,
            reported_page,
            cursor,
            native_balance,
        })
    }
    /// Returns exact requested page controls.
    #[must_use]
    pub const fn request(&self) -> &OwnerRequest {
        &self.request
    }
    /// Returns every source item, without truncation.
    #[must_use]
    pub fn items(&self) -> &[Asset] {
        &self.items
    }
    /// Returns the actual `total`, without promoting it to a grand total.
    #[must_use]
    pub const fn reported_total(&self) -> u64 {
        self.reported_total
    }
    /// Returns actual optional `grand_total`, separately from `total`.
    #[must_use]
    pub const fn reported_grand_total(&self) -> Option<u64> {
        self.reported_grand_total
    }
    /// Returns the actual optional source page label, without inventing one for keysets.
    #[must_use]
    pub const fn reported_page(&self) -> Option<u32> {
        self.reported_page
    }
    /// Returns optional exact source cursor; it does not assert another page exists.
    #[must_use]
    pub const fn cursor(&self) -> Option<&Cursor> {
        self.cursor.as_ref()
    }
    /// Returns source native balance only when actually supplied.
    #[must_use]
    pub const fn native_balance(&self) -> Option<&NativeBalance> {
        self.native_balance.as_ref()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerPageFields {
    request: OwnerRequest,
    #[serde(deserialize_with = "super::bounded::list")]
    items: Vec<Asset>,
    reported_total: u64,
    reported_grand_total: Option<u64>,
    reported_page: Option<u32>,
    cursor: Option<Cursor>,
    native_balance: Option<NativeBalance>,
}
impl TryFrom<OwnerPageFields> for OwnerPage {
    type Error = Error;
    fn try_from(f: OwnerPageFields) -> Result<Self, Error> {
        Self::new(
            f.request,
            f.items,
            f.reported_total,
            f.reported_grand_total,
            f.reported_page,
            f.cursor,
            f.native_balance,
        )
    }
}
