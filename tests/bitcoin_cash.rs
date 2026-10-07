// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Offline BCH identity, exact-value, source-record and observation contracts.
#![cfg(feature = "bitcoin-cash")]

use cashaddr::CashEnc;
use regit_web3::{
    domain::{ExactDecimal, Source, Timestamp, bitcoin_cash::*},
    error::Error,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ADDRESS: &str = "bitcoincash:qqvd9p0t8dxrs5twh4e7pznjq8s2vgdc4chzqrew27";
const TXID: &str = "f9540793088014f8f4a7f3ee29233c8e99932ab54e90a93d12811ed1a8445970";
fn address() -> Result<Address, Error> {
    Address::parse(ADDRESS, AddressNamespace::Mainnet)
}
fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + Eq + std::fmt::Debug>(v: &T) -> TestResult {
    assert_eq!(&serde_json::from_str::<T>(&serde_json::to_string(v)?)?, v);
    Ok(())
}
#[test]
fn primary_cashaddr_vectors_preserve_supported_kinds_widths_and_tokens() -> TestResult {
    #[derive(Deserialize)]
    struct Vector {
        #[serde(rename = "payloadSize")]
        size: usize,
        #[serde(rename = "type")]
        kind: u8,
        cashaddr: String,
        payload: String,
    }
    let vectors: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/bitcoin_cash/cashaddr.json"))?;
    assert!(vectors.len() > 30);
    for v in vectors {
        let namespace = if v.cashaddr.starts_with("bitcoincash:") {
            AddressNamespace::Mainnet
        } else if v.cashaddr.starts_with("bchtest:") {
            AddressNamespace::Testnet
        } else {
            AddressNamespace::Regtest
        };
        let a = Address::parse(&v.cashaddr, namespace)?;
        assert_eq!(a.hash(), const_hex::decode(&v.payload)?);
        assert_eq!(a.hash().len(), v.size);
        assert_eq!(a.is_token_aware(), v.kind >= 2);
        assert_eq!(
            a.kind(),
            if v.kind % 2 == 0 {
                AddressKind::PubkeyHash
            } else {
                AddressKind::ScriptHash
            }
        );
        assert_eq!(Address::parse(&v.cashaddr.to_uppercase(), namespace)?, a);
        assert_eq!(
            Address::parse(
                v.cashaddr.split_once(':').ok_or(Error::UnavailableData)?.1,
                namespace
            )?,
            a
        );
        roundtrip(&a)?;
    }
    Ok(())
}
#[test]
fn p2sh32_bytecode_legacy_conversion_and_namespace_are_explicit() -> TestResult {
    let hash = vec![9; 32];
    let a = Address::from_hash(
        AddressNamespace::Mainnet,
        AddressKind::ScriptHash,
        true,
        &hash,
    )?;
    assert_eq!(
        a.locking_bytecode(),
        [vec![0xaa, 0x20], hash, vec![0x87]].concat()
    );
    let old = Address::parse(
        "1BpEi6DfDAUFd7GtittLSdBeYJvcoaVggu",
        AddressNamespace::Mainnet,
    )?;
    assert_eq!(
        old.to_string(),
        "bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a"
    );
    assert!(Address::parse(&old.to_string(), AddressNamespace::Testnet).is_err());
    assert!(
        Address::from_hash(
            AddressNamespace::Mainnet,
            AddressKind::PubkeyHash,
            false,
            &[0; 32]
        )
        .is_err()
    );
    assert_eq!(
        a.script_hash(),
        Address::parse(&a.to_string(), AddressNamespace::Mainnet)?.script_hash()
    );
    Ok(())
}
#[test]
fn malformed_short_checksum_unsupported_kinds_and_mixed_case_are_rejected() -> TestResult {
    for v in [
        "bitcoincash:x64nx6hz",
        "bitcoincash:",
        "BITCOINCASH:qqvd9p0t8dxrs5twh4e7pznjq8s2vgdc4chzqrew27",
        "prefix:qqvd9p0t8dxrs5twh4e7pznjq8s2vgdc4chzqrew27",
        "bitcoincash:qqvd9p0t8dxrs5twh4e7pznjq8s2vgdc4chzqrew20",
        "secret\nprovider",
    ] {
        assert!(Address::parse(v, AddressNamespace::Mainnet).is_err());
    }
    for kind in [4, 15] {
        let encoded = [0; 20].encode("bitcoincash", cashaddr::HashType::try_from(kind)?)?;
        assert!(Address::parse(&encoded, AddressNamespace::Mainnet).is_err());
    }
    let mut fields = serde_json::to_value(address()?)?;
    fields["namespace"] = json!("testnet");
    assert!(serde_json::from_value::<Address>(fields).is_err());
    Ok(())
}
#[test]
fn genesis_and_real_fork_checkpoint_are_distinct_network_facts() -> TestResult {
    let network = NetworkIdentity::mainnet("mainnet")?;
    let genesis = BlockHeader::from_hex(include_str!("fixtures/bitcoin_cash/genesis.hex").trim())?;
    let fork = BlockHeader::from_hex(include_str!("fixtures/bitcoin_cash/fork.hex").trim())?;
    assert_eq!(genesis.hash(), network.genesis_hash());
    assert_eq!(fork.hash(), network.fork_checkpoint().hash());
    assert_eq!(network.fork_checkpoint().height(), 661_648);
    assert_ne!(fork.hash(), genesis.hash());
    roundtrip(&network)?;
    roundtrip(&fork)?;
    let mut bad = serde_json::to_value(fork)?;
    bad["hash"] = json!("11".repeat(32));
    assert!(serde_json::from_value::<BlockHeader>(bad).is_err());
    assert!(ForkCheckpoint::new(0, genesis.hash()).is_err());
    Ok(())
}
#[test]
fn exact_bch_atomic_and_fee_decimal_units_never_round() -> TestResult {
    assert_eq!(
        Satoshis::from_bch(&ExactDecimal::parse("3.12508205")?)?.raw(),
        312_508_205
    );
    assert_eq!(
        Satoshis::parse("9007199254740993")?.raw(),
        9_007_199_254_740_993
    );
    assert_eq!(
        SignedSatoshis::parse("-9007199254740993")?.raw(),
        -9_007_199_254_740_993
    );
    for s in ["01", "+1", "-0", "1.0", "18446744073709551616"] {
        assert!(Satoshis::parse(s).is_err());
    }
    for s in ["-1", "0.000000001", "184467440737.09551616"] {
        assert!(Satoshis::from_bch(&ExactDecimal::parse(s)?).is_err());
    }
    let fee = FeeEstimate::new(FeeTarget::new(6)?, Some(ExactDecimal::parse("1e-5")?))?;
    assert_eq!(
        fee.bch_per_kilobyte()
            .ok_or(Error::UnavailableData)?
            .canonical(),
        "0.00001"
    );
    roundtrip(&fee)?;
    assert!(FeeEstimate::new(FeeTarget::new(1)?, Some(ExactDecimal::parse("-2")?)).is_err());
    Ok(())
}
#[test]
fn cash_tokens_have_distinct_exact_amounts_and_bounded_nft_metadata() -> TestResult {
    let token = TokenData::new(
        TokenCategory::parse(&"ab".repeat(32))?,
        TokenAmount::parse("9007199254740993")?,
        Some(Nft::new(NftCapability::Minting, Bytes::new(vec![0; 40])?)?),
    )?;
    assert_eq!(token.amount(), 9_007_199_254_740_993);
    roundtrip(&token)?;
    assert!(TokenAmount::parse("9223372036854775808").is_err());
    assert!(TokenAmount::parse("01").is_err());
    assert!(Nft::new(NftCapability::Mutable, Bytes::new(vec![0; 41])?).is_err());
    assert!(TokenData::new(token.category(), TokenAmount::new(0)?, None).is_err());
    assert!(format!("{:?}", Bytes::from_hex("aabb")?).contains("length"));
    assert!(!format!("{:?}", Bytes::from_hex("aabb")?).contains("aabb"));
    Ok(())
}
#[test]
fn history_preserves_same_block_order_and_rejects_range_duplicates_and_capacity() -> TestResult {
    let limit = CollectionLimit::new(3)?;
    let range = HistoryRange::new(10, HistoryUpperBound::Height { height: 12 }, limit)?;
    let a = HistoryEntry::new(
        Txid::from_display_bytes([2; 32]),
        HistoryState::Confirmed { height: 10 },
        None,
    )?;
    let b = HistoryEntry::new(
        Txid::from_display_bytes([1; 32]),
        HistoryState::Confirmed { height: 10 },
        None,
    )?;
    let history = History::new(address()?, range, vec![a.clone(), b.clone()])?;
    assert_eq!(history.entries()[0], a);
    roundtrip(&history)?;
    assert!(History::new(address()?, range, vec![a.clone(), a.clone()]).is_err());
    assert!(
        History::new(
            address()?,
            HistoryRange::new(11, HistoryUpperBound::Height { height: 12 }, limit)?,
            vec![a.clone()]
        )
        .is_err()
    );
    assert!(
        History::new(
            address()?,
            HistoryRange::new(10, HistoryUpperBound::Height { height: 10 }, limit)?,
            vec![a.clone()]
        )
        .is_err()
    );
    assert!(
        History::new(
            address()?,
            HistoryRange::new(10, HistoryUpperBound::OpenTip, CollectionLimit::new(1)?)?,
            vec![a, b]
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn history_pending_height_fee_and_order_remain_source_specific() -> TestResult {
    let pending = HistoryEntry::new(
        Txid::from_display_bytes([1; 32]),
        HistoryState::UnconfirmedParents,
        Some(Satoshis::from_raw(7)),
    )?;
    assert!(HistoryEntry::new(pending.txid(), HistoryState::ZeroHeight, None).is_err());
    assert!(
        HistoryEntry::new(
            pending.txid(),
            HistoryState::Confirmed { height: 1 },
            Some(Satoshis::from_raw(7))
        )
        .is_err()
    );
    let range = HistoryRange::new(0, HistoryUpperBound::OpenTip, CollectionLimit::new(3)?)?;
    assert!(
        History::new(
            address()?,
            range,
            vec![
                pending.clone(),
                HistoryEntry::new(
                    Txid::from_display_bytes([2; 32]),
                    HistoryState::Confirmed { height: 1 },
                    None
                )?
            ]
        )
        .is_err()
    );
    let history = History::new(address()?, range, vec![pending])?;
    roundtrip(&history)?;
    Ok(())
}
#[test]
fn unspent_filters_duplicate_outpoints_money_and_capacities_are_checked() -> TestResult {
    let token = TokenData::new(
        TokenCategory::from_display_bytes([2; 32]),
        TokenAmount::new(1)?,
        None,
    )?;
    let output = UnspentOutput {
        txid: Txid::from_display_bytes([3; 32]),
        output_index: 0,
        height: 4,
        value: Satoshis::from_raw(1000),
        token_data: Some(token),
    };
    let values = UnspentOutputs::new(
        address()?,
        TokenFilter::TokensOnly,
        CollectionLimit::new(1)?,
        vec![output.clone()],
    )?;
    roundtrip(&values)?;
    assert!(
        UnspentOutputs::new(
            address()?,
            TokenFilter::ExcludeTokens,
            CollectionLimit::new(2)?,
            vec![output.clone()]
        )
        .is_err()
    );
    assert!(
        UnspentOutputs::new(
            address()?,
            TokenFilter::IncludeTokens,
            CollectionLimit::new(2)?,
            vec![output.clone(), output.clone()]
        )
        .is_err()
    );
    let mut excessive = output;
    excessive.value = Satoshis::from_raw(u64::MAX);
    assert!(
        UnspentOutputs::new(
            address()?,
            TokenFilter::IncludeTokens,
            CollectionLimit::new(1)?,
            vec![excessive]
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn opaque_raw_computed_identity_and_zero_height_do_not_invent_consensus_or_mempool() -> TestResult {
    let raw = RawTransaction::new(Bytes::from_hex(
        include_str!("fixtures/bitcoin_cash/coinbase.hex").trim(),
    )?)?;
    assert_eq!(raw.txid(), Txid::parse(TXID)?);
    roundtrip(&raw)?;
    let zero = TransactionStatus::new(raw.txid(), SourceHeight::Zero, None)?;
    assert!(zero.inclusion().is_none());
    roundtrip(&zero)?;
    let unknown = TransactionStatus::new(raw.txid(), SourceHeight::Unknown, None)?;
    assert_ne!(zero, unknown);
    assert!(
        TransactionStatus::new(raw.txid(), SourceHeight::Positive { height: 1 }, None).is_err()
    );
    assert!(RawTransaction::new(Bytes::new(Vec::new())?).is_err());
    let mut bad = serde_json::to_value(raw)?;
    bad["txid"] = json!("22".repeat(32));
    assert!(serde_json::from_value::<RawTransaction>(bad).is_err());
    Ok(())
}
#[test]
fn observation_schema_query_filters_and_network_are_correlated() -> TestResult {
    let a = address()?;
    let value = AddressBalance {
        address: a.clone(),
        token_filter: TokenFilter::IncludeTokens,
        confirmed: Satoshis::from_raw(9_007_199_254_740_993),
        unconfirmed: SignedSatoshis::new(-3)?,
    };
    let context = Context::new(
        NetworkIdentity::mainnet("mainnet")?,
        Operation::AddressBalance {
            address: a.clone(),
            token_filter: TokenFilter::IncludeTokens,
        },
        Source::new("fixture", "balance", "0.1.0")?,
        Timestamp::from_unix_seconds(123),
    )?;
    let observation = Observation::new(context.clone(), value.clone())?;
    roundtrip(&observation)?;
    let mut bad = serde_json::to_value(&observation)?;
    bad["context"]["schema_version"] = json!(2);
    assert!(serde_json::from_value::<Observation<AddressBalance>>(bad).is_err());
    let mut mismatch = value;
    mismatch.token_filter = TokenFilter::TokensOnly;
    assert!(Observation::new(context, mismatch).is_err());
    assert!(CollectionLimit::new(0).is_err());
    assert!(CollectionLimit::new(100_001).is_err());
    assert!(
        HistoryRange::new(
            10,
            HistoryUpperBound::Height { height: 9 },
            CollectionLimit::new(1)?
        )
        .is_err()
    );
    Ok(())
}

fn full_transaction() -> Result<Transaction, Box<dyn std::error::Error>> {
    let raw = RawTransaction::new(Bytes::from_hex(
        include_str!("fixtures/bitcoin_cash/coinbase.hex").trim(),
    )?)?;
    let header: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/bitcoin_cash/inclusion.json"))?;
    let header = BlockHeader::from_hex(
        header["block_header"]
            .as_str()
            .ok_or(Error::UnavailableData)?,
    )?;
    let inclusion = SourceInclusion::new(971_820, header.hash(), header)?;
    let source: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/bitcoin_cash/coinbase.json"))?;
    let data = TransactionData {
        namespace: AddressNamespace::Mainnet,
        reported_txid: raw.txid(),
        reported_hash: Some(raw.txid()),
        raw,
        version: 1,
        lock_time: 0,
        size: 123,
        inputs: vec![TransactionInput::Coinbase {
            unlocking_bytes: Bytes::from_hex(
                source["vin"][0]["coinbase"]
                    .as_str()
                    .ok_or(Error::UnavailableData)?,
            )?,
            sequence: u32::MAX,
        }],
        outputs: vec![TransactionOutput {
            index: 0,
            value: Satoshis::from_raw(312_508_205),
            locking_bytes: Bytes::from_hex("76a91418d285eb3b4c38516ebd73e08a7201e0a621b8ae88ac")?,
            script_type: SourceText::new("pubkeyhash")?,
            addresses: Some(vec![address()?]),
            required_signatures: Some(1),
            token_data: None,
        }],
        reported_confirmations: Some(1),
        reported_block_time: Some(1_791_342_457),
        reported_block_hash: Some(inclusion.block_hash()),
    };
    Ok(Transaction::new(
        Txid::parse(TXID)?,
        CollectionLimit::new(10)?,
        data,
        TransactionStatus::new(
            Txid::parse(TXID)?,
            SourceHeight::Positive { height: 971_820 },
            Some(inclusion),
        )?,
    )?)
}
#[test]
fn complete_transaction_source_integrity_checks_constructor_and_serde() -> TestResult {
    let tx = full_transaction()?;
    roundtrip(&tx)?;
    for index in 0..7 {
        let mut data = tx.data().clone();
        match index {
            0 => data.size += 1,
            1 => data.reported_txid = Txid::from_display_bytes([9; 32]),
            2 => data.inputs.push(data.inputs[0].clone()),
            3 => data.outputs[0].index = 1,
            4 => data.outputs[0].value = Satoshis::from_raw(u64::MAX),
            5 => {
                data.outputs[0].addresses = Some(vec![Address::from_hash(
                    AddressNamespace::Testnet,
                    AddressKind::PubkeyHash,
                    false,
                    &[1; 20],
                )?]);
            }
            _ => data.reported_block_hash = Some(BlockHash::from_display_bytes([9; 32])),
        }
        assert!(Transaction::new(tx.txid(), tx.limit(), data, tx.status().clone()).is_err());
    }
    let mut json = serde_json::to_value(&tx)?;
    json["data"]["outputs"][0]["value"] = json!(u64::MAX.to_string());
    assert!(serde_json::from_value::<Transaction>(json).is_err());
    let mut data = tx.data().clone();
    data.outputs[0].addresses = None;
    let unavailable = Transaction::new(tx.txid(), tx.limit(), data, tx.status().clone())?;
    assert_eq!(unavailable.data().outputs[0].addresses, None);
    roundtrip(&unavailable)?;
    Ok(())
}
#[test]
fn bounded_raw_text_and_collection_serde_do_not_expose_diagnostics() -> TestResult {
    assert!(Bytes::new(vec![0; Bytes::MAX_BYTES + 1]).is_err());
    for raw in ["0x00", "0", "gg"] {
        assert!(Bytes::from_hex(raw).is_err());
    }
    let secret = SourceText::new("source-private-detail")?;
    assert!(!format!("{secret:?}").contains("source-private-detail"));
    assert!(SourceText::new("bad\ntext").is_err());
    let tx = full_transaction()?;
    let mut encoded = serde_json::to_value(tx)?;
    encoded["data"]["outputs"] = json!([]);
    assert!(serde_json::from_value::<Transaction>(encoded).is_err());
    let mut encoded = serde_json::to_value(HistoryRange::new(
        1,
        HistoryUpperBound::OpenTip,
        CollectionLimit::new(1)?,
    )?)?;
    encoded["limit"] = json!(100_001);
    assert!(serde_json::from_value::<HistoryRange>(encoded).is_err());
    Ok(())
}
