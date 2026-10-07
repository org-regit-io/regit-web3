// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{List, Number, invalid, metadata::Metadata};
use crate::{
    domain::{
        helius::{
            Asset, AssetAuthority, AssetContent, AssetData, AssetFile, AssetOptions, AssetPosition,
            AssetRequest, AssetSort, CachedPrice, Compression, Creator, Cursor, Grouping,
            NativeBalance, OwnerPage, OwnerRequest, Ownership, PrintSupply, Royalty, SortDirection,
            SourceText, TokenInfo,
        },
        solana::{Hash, Pubkey},
    },
    error::Error,
};
use serde::{Deserialize, Serialize, de::IgnoredAny};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
// These field names match the published DAS JSON schema.
#[allow(clippy::struct_field_names)]
struct Options {
    show_unverified_collections: bool,
    show_collection_metadata: bool,
    show_fungible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_grand_total: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_native_balance: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_zero_balance: Option<bool>,
}
impl Options {
    fn asset(q: AssetOptions) -> Self {
        Self {
            show_unverified_collections: q.show_unverified_collections,
            show_collection_metadata: q.show_collection_metadata,
            show_fungible: q.show_fungible,
            show_grand_total: None,
            show_native_balance: None,
            show_zero_balance: None,
        }
    }
}
#[derive(Serialize)]
pub(in super::super) struct AssetParams {
    id: Pubkey,
    options: Options,
}
pub(in super::super) fn asset_params(q: &AssetRequest) -> AssetParams {
    AssetParams {
        id: q.id,
        options: Options::asset(q.options),
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Sort {
    sort_by: AssetSort,
    sort_direction: SortDirection,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct OwnerParams {
    owner_address: Pubkey,
    limit: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<Pubkey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<Pubkey>,
    sort_by: Sort,
    options: Options,
}
pub(in super::super) fn owner_params(q: &OwnerRequest) -> OwnerParams {
    let (mut page, mut cursor, mut before, mut after) = (None, None, None, None);
    match q.position() {
        AssetPosition::Page { page: p } => page = Some(*p),
        AssetPosition::Cursor { cursor: c } => cursor.clone_from(c),
        AssetPosition::Range {
            before: b,
            after: a,
        } => {
            before = *b;
            after = *a;
        }
    }
    let mut options = Options::asset(q.options());
    options.show_grand_total = Some(q.show_grand_total());
    options.show_native_balance = Some(q.show_native_balance());
    options.show_zero_balance = Some(q.show_zero_balance());
    OwnerParams {
        owner_address: q.owner(),
        limit: q.limit(),
        page,
        cursor,
        before,
        after,
        sort_by: Sort {
            sort_by: q.sort(),
            sort_direction: q.direction(),
        },
        options,
    }
}
#[derive(Deserialize)]
struct ContentWire {
    #[serde(rename = "$schema")]
    schema: Option<SourceText>,
    json_uri: Option<SourceText>,
    files: Option<List<AssetFile>>,
    metadata: Option<Metadata>,
    links: Option<Metadata>,
}
#[derive(Deserialize)]
struct CompressionWire {
    eligible: bool,
    compressed: bool,
    data_hash: Option<String>,
    creator_hash: Option<String>,
    asset_hash: Option<String>,
    tree: Option<String>,
    seq: Option<u64>,
    leaf_id: Option<u64>,
}
fn optional_hash(v: Option<String>) -> Result<Option<Hash>, Error> {
    v.filter(|v| !v.is_empty())
        .map(|v| Hash::parse(&v).map_err(|_| invalid()))
        .transpose()
}
fn optional_key(v: Option<String>) -> Result<Option<Pubkey>, Error> {
    v.filter(|v| !v.is_empty())
        .map(|v| Pubkey::parse(&v).map_err(|_| invalid()))
        .transpose()
}
#[derive(Deserialize)]
struct GroupWire {
    group_key: SourceText,
    group_value: SourceText,
    verified: Option<bool>,
    collection_metadata: Option<Metadata>,
}
#[derive(Deserialize)]
struct RoyaltyWire {
    royalty_model: SourceText,
    target: Option<Pubkey>,
    percent: Number,
    basis_points: u16,
    primary_sale_happened: bool,
    locked: bool,
}
#[derive(Deserialize)]
struct OwnershipWire {
    frozen: bool,
    delegated: bool,
    delegate: Option<Pubkey>,
    ownership_model: SourceText,
    owner: Option<Pubkey>,
}
#[derive(Deserialize)]
struct SupplyWire {
    print_max_supply: u64,
    print_current_supply: u64,
    edition_nonce: Option<u8>,
}
#[derive(Deserialize)]
struct PriceWire {
    price_per_token: Option<Number>,
    total_price: Option<Number>,
    currency: SourceText,
}
#[derive(Deserialize)]
struct TokenWire {
    supply: Option<u64>,
    balance: Option<u64>,
    decimals: Option<u8>,
    token_program: Option<Pubkey>,
    associated_token_address: Option<Pubkey>,
    mint_authority: Option<Pubkey>,
    freeze_authority: Option<Pubkey>,
    symbol: Option<SourceText>,
    price_info: Option<PriceWire>,
}
#[derive(Deserialize)]
pub(in super::super) struct AssetWire {
    last_indexed_slot: Option<u64>,
    id: Pubkey,
    interface: SourceText,
    content: Option<ContentWire>,
    authorities: Option<List<AssetAuthority>>,
    compression: Option<CompressionWire>,
    grouping: Option<List<GroupWire>>,
    royalty: Option<RoyaltyWire>,
    creators: Option<List<Creator>>,
    ownership: Option<OwnershipWire>,
    supply: Option<SupplyWire>,
    mutable: Option<bool>,
    burnt: Option<bool>,
    token_info: Option<TokenWire>,
    is_agent: Option<bool>,
    agent_token: Option<Pubkey>,
    asset_signer: Option<Pubkey>,
    plugins: Option<IgnoredAny>,
    mint_extensions: Option<IgnoredAny>,
    uses: Option<IgnoredAny>,
    inscription: Option<IgnoredAny>,
    spl20: Option<IgnoredAny>,
}
impl AssetWire {
    pub(in super::super) fn into_asset(self) -> Result<(Asset, Option<u64>), Error> {
        let mut unsupported_extensions = Vec::new();
        for (name, present) in [
            ("plugins", self.plugins.is_some()),
            ("mint_extensions", self.mint_extensions.is_some()),
            ("uses", self.uses.is_some()),
            ("inscription", self.inscription.is_some()),
            ("spl20", self.spl20.is_some()),
        ] {
            if present {
                unsupported_extensions.push(SourceText::new(name).map_err(|_| invalid())?);
            }
        }
        let compression = self
            .compression
            .map(|c| {
                Ok::<_, Error>(Compression {
                    eligible: c.eligible,
                    compressed: c.compressed,
                    data_hash: optional_hash(c.data_hash)?,
                    creator_hash: optional_hash(c.creator_hash)?,
                    asset_hash: optional_hash(c.asset_hash)?,
                    tree: optional_key(c.tree)?,
                    sequence: c.seq,
                    leaf_id: c.leaf_id,
                })
            })
            .transpose()?;
        let data = AssetData {
            id: self.id,
            last_indexed_slot: self.last_indexed_slot,
            interface: self.interface,
            content: self.content.map(|c| AssetContent {
                schema: c.schema,
                json_uri: c.json_uri,
                files: c.files.map(|v| v.0),
                metadata: c.metadata.map(|m| m.0),
                links: c.links.map(|m| m.0),
            }),
            authorities: self.authorities.map(|v| v.0),
            compression,
            grouping: self.grouping.map(|v| {
                v.0.into_iter()
                    .map(|g| Grouping {
                        key: g.group_key,
                        value: g.group_value,
                        verified: g.verified,
                        metadata: g.collection_metadata.map(|m| m.0),
                    })
                    .collect()
            }),
            royalty: self.royalty.map(|r| Royalty {
                model: r.royalty_model,
                target: r.target,
                percent: r.percent.0,
                basis_points: r.basis_points,
                primary_sale_happened: r.primary_sale_happened,
                locked: r.locked,
            }),
            creators: self.creators.map(|v| v.0),
            ownership: self.ownership.map(|o| Ownership {
                frozen: o.frozen,
                delegated: o.delegated,
                delegate: o.delegate,
                model: o.ownership_model,
                owner: o.owner,
            }),
            print_supply: self.supply.map(|s| PrintSupply {
                maximum: s.print_max_supply,
                current: s.print_current_supply,
                edition_nonce: s.edition_nonce,
            }),
            mutable: self.mutable,
            burnt: self.burnt,
            token_info: self.token_info.map(|t| TokenInfo {
                supply: t.supply,
                balance: t.balance,
                decimals: t.decimals,
                token_program: t.token_program,
                associated_token_address: t.associated_token_address,
                mint_authority: t.mint_authority,
                freeze_authority: t.freeze_authority,
                symbol: t.symbol,
                price: t.price_info.map(|p| CachedPrice {
                    price_per_token: p.price_per_token.map(|n| n.0),
                    total_price: p.total_price.map(|n| n.0),
                    currency: p.currency,
                }),
            }),
            is_agent: self.is_agent,
            agent_token: self.agent_token,
            asset_signer: self.asset_signer,
            unsupported_extensions,
        };
        Ok((
            Asset::new(data).map_err(|_| invalid())?,
            self.last_indexed_slot,
        ))
    }
}
#[derive(Deserialize)]
struct NativeWire {
    lamports: u64,
    price_per_sol: Option<Number>,
    total_price: Option<Number>,
}
#[derive(Deserialize)]
pub(in super::super) struct OwnerWire {
    last_indexed_slot: Option<u64>,
    total: u64,
    grand_total: Option<u64>,
    limit: u16,
    page: Option<u32>,
    cursor: Option<Cursor>,
    items: List<AssetWire>,
    #[serde(rename = "nativeBalance")]
    native_balance: Option<NativeWire>,
}
impl OwnerWire {
    pub(in super::super) fn into_page(
        self,
        request: OwnerRequest,
    ) -> Result<(OwnerPage, Option<u64>), Error> {
        if self.limit != request.limit()
            || match request.position() {
                AssetPosition::Page { page } => self.page.is_some_and(|reported| reported != *page),
                _ => false,
            }
        {
            return Err(invalid());
        }
        let assets = self
            .items
            .0
            .into_iter()
            .map(|a| a.into_asset().map(|(a, _)| a))
            .collect::<Result<_, _>>()?;
        let balance = self.native_balance.map(|b| NativeBalance {
            lamports: b.lamports,
            price_per_sol: b.price_per_sol.map(|n| n.0),
            total_price: b.total_price.map(|n| n.0),
        });
        Ok((
            OwnerPage::new(
                request,
                assets,
                self.total,
                self.grand_total,
                self.page,
                self.cursor,
                balance,
            )
            .map_err(|_| invalid())?,
            self.last_indexed_slot,
        ))
    }
}
