// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure exact Helius identities, metadata, requests and observation invariants.
#![cfg(feature = "helius")]
use regit_web3::{
    domain::{
        ExactDecimal, Source, Timestamp,
        helius::*,
        solana::{Commitment, Hash, Network, Pubkey, Signature},
    },
    error::{Error, ValidationError},
};
use serde_json::json;
use std::collections::BTreeMap;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn text(s: &str) -> Result<SourceText, Error> {
    SourceText::new(s)
}
fn options() -> AssetOptions {
    AssetOptions {
        show_unverified_collections: false,
        show_collection_metadata: false,
        show_fungible: true,
    }
}
fn asset_data(id: Pubkey) -> Result<AssetData, Error> {
    Ok(AssetData {
        id,
        last_indexed_slot: None,
        interface: text("FungibleToken")?,
        content: None,
        authorities: None,
        compression: None,
        grouping: None,
        royalty: None,
        creators: None,
        ownership: None,
        print_supply: None,
        mutable: None,
        burnt: None,
        token_info: Some(TokenInfo {
            supply: Some(9_007_199_254_740_993),
            balance: Some(u64::MAX),
            decimals: Some(9),
            token_program: Some(Pubkey::from_bytes([3; 32])),
            associated_token_address: None,
            mint_authority: None,
            freeze_authority: None,
            symbol: None,
            price: Some(CachedPrice {
                price_per_token: Some(ExactDecimal::parse("0.0000000000000000000001")?),
                total_price: None,
                currency: text("USD")?,
            }),
        }),
        is_agent: None,
        agent_token: None,
        asset_signer: None,
        unsupported_extensions: Vec::new(),
    })
}
fn owner(position: AssetPosition, sort: AssetSort) -> Result<OwnerRequest, Error> {
    OwnerRequest::new(
        Pubkey::from_bytes([9; 32]),
        2,
        position,
        sort,
        SortDirection::Asc,
        options(),
        false,
        false,
        false,
    )
}
fn parsed() -> Result<ParsedTransaction, Error> {
    ParsedTransaction::new(ParsedTransactionData {
        slot: 100,
        block_time: Some(1_700_000_000),
        fee: u64::MAX,
        fee_payer: None,
        transaction_status: TransactionStatus::Succeeded,
        error: None,
        decoded_error: None,
        native_transfers: vec![NativeTransfer {
            from_user_account: None,
            to_user_account: None,
            amount: 9_007_199_254_740_993,
        }],
        token_transfers: vec![TokenTransfer {
            from_user_account: None,
            to_user_account: None,
            from_token_account: None,
            to_token_account: None,
            raw_token_amount: u64::MAX,
            decimals: 9,
            mint: Pubkey::from_bytes([5; 32]),
            token_standard: None,
        }],
        summary: None,
        instructions: vec![],
    })
}
fn result(signature: Signature) -> Result<TransactionResult, Error> {
    Ok(TransactionResult {
        signature,
        outcome: ParseOutcome::Ok {
            parsed: Box::new(parsed()?),
        },
    })
}
fn history() -> Result<HistoryRequest, Error> {
    HistoryRequest::new(
        Pubkey::from_bytes([9; 32]),
        2,
        Commitment::Confirmed,
        SortDirection::Desc,
        None,
        None,
        Some(Bounds::new(None, Some(90), None, Some(110))?),
        None,
        None,
    )
}
#[test]
fn exact_supply_lamports_tokens_and_cached_prices_roundtrip_without_floats() -> TestResult {
    let a = Asset::new(asset_data(Pubkey::from_bytes([1; 32]))?)?;
    let v = serde_json::to_string(&a)?;
    assert!(v.contains("9007199254740993"));
    assert!(v.contains("18446744073709551615"));
    assert_eq!(serde_json::from_str::<Asset>(&v)?, a);
    let p = parsed()?;
    assert_eq!(
        serde_json::from_str::<ParsedTransaction>(&serde_json::to_string(&p)?)?,
        p
    );
    Ok(())
}
#[test]
fn royalty_print_supply_and_creator_bounds_have_constructor_serde_parity() -> TestResult {
    let mut a = asset_data(Pubkey::from_bytes([1; 32]))?;
    a.royalty = Some(Royalty {
        model: text("creators")?,
        target: None,
        percent: ExactDecimal::parse("0.042")?,
        basis_points: 420,
        primary_sale_happened: false,
        locked: false,
    });
    assert!(Asset::new(a.clone()).is_ok());
    a.royalty.as_mut().unwrap().percent = ExactDecimal::parse("0.043")?;
    assert_eq!(
        Asset::new(a.clone()).unwrap_err(),
        Error::Validation(ValidationError::InvalidHeliusAsset)
    );
    assert!(serde_json::from_str::<Asset>(&serde_json::to_string(&a)?).is_err());
    a.royalty = None;
    a.print_supply = Some(PrintSupply {
        maximum: 1,
        current: 2,
        edition_nonce: None,
    });
    assert!(Asset::new(a.clone()).is_err());
    a.print_supply.as_mut().unwrap().maximum = 0;
    assert!(Asset::new(a.clone()).is_ok());
    a.creators = Some(vec![Creator {
        address: Pubkey::from_bytes([1; 32]),
        share: 101,
        verified: false,
    }]);
    assert!(Asset::new(a).is_err());
    Ok(())
}
#[test]
fn opaque_utf8_text_and_cursor_never_leak_through_debug_or_fixed_errors() -> TestResult {
    let secret = "secret-api-key\nopaque\0source";
    let t = text(secret)?;
    assert_eq!(t.as_str(), secret);
    assert!(!format!("{t:?}").contains("secret"));
    assert!(Cursor::new(secret).is_err());
    let c = Cursor::new("opaque:next+position")?;
    assert_eq!(
        serde_json::from_str::<Cursor>(&serde_json::to_string(&c)?)?,
        c
    );
    assert!(!format!("{c:?}").contains("position"));
    assert!(text(&"x".repeat(SourceText::MAX_BYTES + 1)).is_err());
    Ok(())
}
#[test]
fn structured_metadata_retains_types_and_rejects_duplicates_depth_and_size() -> TestResult {
    let mut m = BTreeMap::new();
    m.insert(
        "raw_amount".into(),
        MetadataValue::new(MetadataNode::Text(text("9007199254740993")?))?,
    );
    m.insert(
        "number".into(),
        MetadataValue::new(MetadataNode::Number(ExactDecimal::parse(
            "9007199254740993.125",
        )?))?,
    );
    let tree = MetadataValue::new(MetadataNode::Object(m))?;
    assert_eq!(
        serde_json::from_str::<MetadataValue>(&serde_json::to_string(&tree)?)?,
        tree
    );
    assert!(!format!("{tree:?}").contains("raw_amount"));
    assert!(
        serde_json::from_str::<MetadataValue>(
            r#"{"kind":"object","value":{"x":{"kind":"null"},"x":{"kind":"null"}}}"#
        )
        .is_err()
    );
    let mut node = MetadataValue::new(MetadataNode::Null)?;
    for _ in 0..MetadataValue::MAX_DEPTH {
        node = MetadataValue::new(MetadataNode::Array(vec![node]))?;
    }
    assert!(MetadataValue::new(MetadataNode::Array(vec![node])).is_err());
    assert!(
        MetadataValue::new(MetadataNode::Array(vec![
            MetadataValue::new(
                MetadataNode::Null
            )?;
            4096
        ]))
        .is_err()
    );
    Ok(())
}
#[test]
fn page_keyset_modes_and_binary_ranges_reject_incoherent_queries() -> TestResult {
    assert!(owner(AssetPosition::Page { page: 0 }, AssetSort::Created).is_err());
    assert!(owner(AssetPosition::Cursor { cursor: None }, AssetSort::Created).is_err());
    assert!(
        owner(
            AssetPosition::Range {
                before: None,
                after: None
            },
            AssetSort::Id
        )
        .is_err()
    );
    let q = owner(AssetPosition::Cursor { cursor: None }, AssetSort::Id)?;
    let next = q.with_cursor(Cursor::new("opaque")?)?;
    assert_eq!(next.owner(), q.owner());
    assert_eq!(next.options(), q.options());
    assert_eq!(next.limit(), q.limit());
    let mut v = serde_json::to_value(&q)?;
    v["limit"] = json!(1001);
    assert!(serde_json::from_value::<OwnerRequest>(v).is_err());
    Ok(())
}
#[test]
fn owner_pages_reject_duplicates_wrong_single_owner_capacity_and_range() -> TestResult {
    let q = owner(AssetPosition::Page { page: 1 }, AssetSort::Id)?;
    let a = Asset::new(asset_data(Pubkey::from_bytes([1; 32]))?)?;
    assert!(
        OwnerPage::new(
            q.clone(),
            vec![a.clone(), a.clone()],
            2,
            None,
            None,
            None,
            None
        )
        .is_err()
    );
    assert!(OwnerPage::new(q.clone(), vec![a.clone()], 0, None, None, None, None).is_err());
    let mut wrong = asset_data(Pubkey::from_bytes([2; 32]))?;
    wrong.ownership = Some(Ownership {
        frozen: false,
        delegated: false,
        delegate: None,
        model: text("single")?,
        owner: Some(Pubkey::from_bytes([8; 32])),
    });
    assert!(
        OwnerPage::new(
            q.clone(),
            vec![Asset::new(wrong)?],
            1,
            None,
            None,
            None,
            None
        )
        .is_err()
    );
    let range = owner(
        AssetPosition::Range {
            before: Some(Pubkey::from_bytes([2; 32])),
            after: Some(Pubkey::from_bytes([1; 32])),
        },
        AssetSort::Id,
    )?;
    assert!(OwnerPage::new(range, vec![a.clone()], 1, None, None, None, None).is_err());
    assert!(
        OwnerPage::new(
            q,
            vec![a],
            1,
            None,
            None,
            Some(Cursor::new("cursor")?),
            None
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn batches_preserve_duplicates_and_error_positions_but_reject_missing_reordered_items() -> TestResult
{
    let a = Signature::from_bytes([1; 64]);
    let b = Signature::from_bytes([2; 64]);
    let q = ParseRequest::new(vec![a, a, b], Commitment::Finalized)?;
    let values = vec![
        result(a)?,
        TransactionResult {
            signature: a,
            outcome: ParseOutcome::Error {
                code: text("transaction_not_found")?,
            },
        },
        result(b)?,
    ];
    let batch = ParsedBatch::new(q.clone(), values.clone())?;
    assert_eq!(
        serde_json::from_str::<ParsedBatch>(&serde_json::to_string(&batch)?)?,
        batch
    );
    assert!(ParsedBatch::new(q.clone(), values[..2].to_vec()).is_err());
    let mut reordered = values;
    reordered.swap(0, 2);
    assert!(ParsedBatch::new(q, reordered).is_err());
    assert!(ParseRequest::new(vec![a], Commitment::Processed).is_err());
    assert!(ParseRequest::new(vec![a; 101], Commitment::Confirmed).is_err());
    Ok(())
}
#[test]
fn parser_and_execution_failures_do_not_become_success_or_pending() -> TestResult {
    let mut data = parsed()?.data().clone();
    data.error = Some(MetadataValue::new(MetadataNode::Text(text(
        "opaque raw error",
    )?))?);
    assert!(ParsedTransaction::new(data.clone()).is_err());
    data.transaction_status = TransactionStatus::Failed;
    assert!(ParsedTransaction::new(data).is_ok());
    let failed = TransactionResult {
        signature: Signature::from_bytes([1; 64]),
        outcome: ParseOutcome::Error {
            code: text("transaction_not_found")?,
        },
    };
    let page = HistoryPage::new(history()?, vec![failed], None)?;
    assert!(matches!(
        page.results()[0].outcome,
        ParseOutcome::Error { .. }
    ));
    Ok(())
}
#[test]
fn history_integer_bounds_order_and_cursor_are_not_fabricated_snapshots() -> TestResult {
    assert!(Bounds::new(Some(u64::MAX), None, None, None).is_err());
    assert!(Bounds::new(None, None, Some(0), None).is_err());
    assert!(Bounds::new(Some(5), Some(5), None, None).is_err());
    assert!(Bounds::new(Some(5), None, Some(6), None).is_err());
    let sig = Signature::from_bytes([1; 64]);
    let r = result(sig)?;
    let q = history()?;
    assert!(HistoryPage::new(q.clone(), vec![r.clone(), r.clone()], None).is_err());
    let page = HistoryPage::new(q.clone(), vec![r], Some(Cursor::new("100:0")?))?;
    let next = q.with_token(page.pagination_token().unwrap().clone());
    assert!(HistoryPage::new(next, vec![], Some(Cursor::new("100:0")?)).is_err());
    let mut d = parsed()?.data().clone();
    d.slot = 111;
    let out = TransactionResult {
        signature: sig,
        outcome: ParseOutcome::Ok {
            parsed: Box::new(ParsedTransaction::new(d)?),
        },
    };
    assert!(HistoryPage::new(q, vec![out], None).is_err());
    Ok(())
}
#[test]
fn observations_correlate_exact_query_and_keep_das_index_separate_from_inclusion() -> TestResult {
    let a = Asset::new(asset_data(Pubkey::from_bytes([1; 32]))?)?;
    let network = Network::new(Hash::from_bytes([8; 32]), "fixture")?;
    let ctx = Context::new(
        network.clone(),
        Request::Asset(AssetRequest {
            id: a.id(),
            options: options(),
        }),
        Some(100),
        Source::new("fixture", "getAsset", "0.1.0")?,
        Timestamp::from_unix_seconds(999),
    )?;
    let obs = Observation::new(a.clone(), ctx)?;
    assert_eq!(
        serde_json::from_str::<Observation<Asset>>(&serde_json::to_string(&obs)?)?,
        obs
    );
    let wrong = Context::new(
        network.clone(),
        Request::Asset(AssetRequest {
            id: Pubkey::from_bytes([2; 32]),
            options: options(),
        }),
        None,
        Source::new("fixture", "getAsset", "0.1.0")?,
        Timestamp::from_unix_seconds(999),
    )?;
    assert!(Observation::new(a, wrong).is_err());
    let query = ParseRequest::new(vec![Signature::from_bytes([1; 64])], Commitment::Confirmed)?;
    assert!(
        Context::new(
            network,
            Request::ParseTransactions(query),
            Some(100),
            Source::new("fixture", "parse", "0.1.0")?,
            Timestamp::from_unix_seconds(999)
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn history_token_preserves_all_original_predicates_and_constructor_serde_agree() -> TestResult {
    let base = history()?;
    let before = Signature::from_bytes([1; 64]);
    let after = Signature::from_bytes([2; 64]);
    let query = HistoryRequest::new(
        base.address(),
        base.limit(),
        base.commitment(),
        base.direction(),
        Some(before),
        Some(after),
        base.slot(),
        Some(Bounds::new(
            None,
            Some(1_600_000_000),
            Some(1_800_000_000),
            None,
        )?),
        None,
    )?;
    let continuation = query.with_token(Cursor::new("opaque:position")?);
    assert_eq!(continuation.signatures(), query.signatures());
    assert_eq!(continuation.slot(), query.slot());
    assert_eq!(continuation.time(), query.time());
    assert_eq!(continuation.address(), query.address());
    assert_eq!(continuation.limit(), query.limit());
    assert_eq!(continuation.direction(), query.direction());
    assert_eq!(continuation.commitment(), query.commitment());
    assert_eq!(
        serde_json::from_str::<HistoryRequest>(&serde_json::to_string(&continuation)?)?,
        continuation
    );
    Ok(())
}
