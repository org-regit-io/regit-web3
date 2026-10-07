// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Offline THOR identities, exact units, source records and replaceable capabilities.
#![cfg(feature = "thorchain")]

use bech32::{Bech32, Bech32m, ByteIterExt, Fe32, Fe32IterExt, Hrp};
use regit_web3::{
    chains::thorchain::ThorchainReader,
    domain::{
        Source, Timestamp, U256,
        thorchain::{
            AccountPrefix, Address, Asset, AssetKind, BasisPoints, Chain, ChainAddress,
            ChainTransaction, ChainTransactionData, Coin, CollectionLimit, CompletedStage,
            ConfirmationStage, Context, InboundAddresses, LastBlock, LastBlocks, Network,
            NetworkData, Observation, ObservedStage, Operation, PlannedOutbound,
            PlannedOutboundData, Pool, PoolData, PoolStatus, Pools, ProtocolAmount, QuoteFees,
            QuoteInputResolution, RawQuantity, RuneBalance, Stages, StreamingStatus,
            SwapParameters, SwapQuote, SwapQuoteData, SwapRequest, SwapStatus, Text,
            TransactionMemo, TransactionStatus, Txid, TxidKind,
        },
    },
    error::{Error, ValidationError},
};
use serde_json::json;
use std::{
    future::{Future, ready},
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};
const ACCOUNT: &str = "thor1dheycdevq39qlkxs2a6wuuzyn4aqxhve4qxtxt";
fn amount(value: &str) -> Result<ProtocolAmount, Error> {
    ProtocolAmount::from_decimal(value)
}
fn raw(value: &str) -> Result<RawQuantity, Error> {
    RawQuantity::from_decimal(value)
}
fn id() -> Result<Txid, Error> {
    Txid::parse(&"ab".repeat(32))
}
fn network() -> Result<Network, Error> {
    Network::new("thorchain-1", AccountPrefix::Thor, "mainnet")
}
fn context(operation: Operation, seconds: u64) -> Result<Context, Error> {
    Context::new(
        network()?,
        operation,
        Source::new("fixture", "read", "0.1.0")?,
        Timestamp::from_unix_seconds(seconds),
    )
}
fn request() -> Result<SwapRequest, Error> {
    SwapRequest::new(
        Asset::parse("BTC.BTC")?,
        Asset::parse("ETH.ETH")?,
        amount("100000000")?,
        SwapParameters::default(),
    )
}
fn pool_data() -> Result<PoolData, Error> {
    Ok(PoolData {
        asset: Asset::parse("BTC.BTC")?,
        status: PoolStatus::Available,
        decimals: Some(8),
        pending_inbound_asset: amount("156079849")?,
        pending_inbound_rune: amount("0")?,
        balance_asset: amount("19264178616")?,
        balance_rune: amount("2184144053461505")?,
        asset_tor_price: amount("8516372400403")?,
        pool_units: raw("10169555322973480759703440")?,
        lp_units: raw("196544890977133")?,
        synth_units: raw("10169555322776935868726307")?,
        synth_supply: amount("51741641679")?,
        savers_depth: amount("51613139211")?,
        savers_units: raw("49167830592")?,
        savers_fill_bps: raw("0")?,
        savers_capacity_remaining: amount("0")?,
        synth_mint_paused: true,
        synth_supply_remaining: amount("0")?,
        derived_depth_bps: raw("10000")?,
        trading_halted: Some(false),
        volume_asset: None,
        volume_rune: None,
    })
}
fn quote_data() -> Result<SwapQuoteData, Error> {
    Ok(SwapQuoteData {
        input_resolution: QuoteInputResolution::Unreported,
        expected_amount_out: amount("3157691359")?,
        fees: QuoteFees {
            asset: Asset::parse("ETH.ETH")?,
            affiliate: Some(amount("0")?),
            outbound: Some(amount("9270")?),
            liquidity: amount("6487472")?,
            total: amount("6496742")?,
            slippage_bps: 20,
            total_bps: 20,
        },
        expiry: Timestamp::from_unix_seconds(200),
        warning: Text::new("source warning")?,
        notes: Text::new("")?,
        inbound_address: Some(ChainAddress::new(
            Chain::parse("BTC")?,
            "bc1qsourcefixture",
        )?),
        router: None,
        memo: None,
        inbound_confirmation_blocks: Some(1),
        inbound_confirmation_seconds: Some(600),
        outbound_delay_blocks: 113,
        outbound_delay_seconds: 678,
        dust_threshold: Some(raw("1000")?),
        recommended_min_amount_in: Some(amount("6309")?),
        recommended_gas_rate: Some(raw("3")?),
        gas_rate_units: Some(Text::new("satsperbyte")?),
        max_streaming_quantity: Some(16),
        streaming_swap_blocks: Some(6),
        streaming_swap_seconds: Some(36),
        total_swap_seconds: Some(1278),
    })
}
fn stages() -> Stages {
    Stages {
        inbound_observed: ObservedStage {
            started: None,
            pre_confirmation_count: Some(107),
            final_count: 107,
            completed: true,
        },
        inbound_confirmation_counted: None,
        inbound_finalised: Some(CompletedStage { completed: true }),
        swap_status: Some(SwapStatus {
            pending: false,
            streaming: Some(StreamingStatus {
                interval: 0,
                quantity: 1,
                count: 3,
            }),
        }),
        swap_finalised: Some(CompletedStage { completed: true }),
        outbound_delay: None,
        outbound_signed: None,
    }
}
fn transaction(txid: Txid) -> Result<ChainTransaction, Error> {
    ChainTransaction::new(ChainTransactionData {
        id: txid,
        chain: Chain::parse("BTC")?,
        from_address: ChainAddress::new(Chain::parse("BTC")?, "bc1qsource")?,
        to_address: ChainAddress::new(Chain::parse("BTC")?, "bc1qdestination")?,
        coins: Some(vec![Coin {
            asset: Asset::parse("BTC.BTC")?,
            amount: amount("9007199254740993")?,
            decimals: Some(8),
        }]),
        gas: None,
        memo: None,
    })
}

#[test]
fn exact_chain_id_and_supported_prefix_are_independent_facts() {
    let custom = Network::new("thorchain-next", AccountPrefix::Sthor, "caller-alias").unwrap();
    assert_eq!(custom.chain_id(), "thorchain-next");
    assert_eq!(custom.account_prefix(), AccountPrefix::Sthor);
    assert_eq!(
        serde_json::from_value::<Network>(serde_json::to_value(&custom).unwrap()).unwrap(),
        custom
    );
    for invalid in ["", "thor chain", "https://secret.invalid", &"a".repeat(51)] {
        assert!(Network::new(invalid, AccountPrefix::Thor, "main").is_err());
    }
    let other = bech32::encode::<Bech32>(Hrp::parse("sthor").unwrap(), &[7u8; 20]).unwrap();
    assert_eq!(
        Context::new(
            network().unwrap(),
            Operation::RuneBalance {
                address: Address::parse(&other).unwrap()
            },
            Source::new("test", "read", "0.1.0").unwrap(),
            Timestamp::from_unix_seconds(1)
        )
        .unwrap_err(),
        Error::Validation(ValidationError::NetworkMismatch)
    );
}
#[test]
fn maintained_account_checks_checksum_case_payload_and_canonical_padding() {
    let canonical = Address::parse(ACCOUNT).unwrap();
    assert_eq!(
        Address::parse(&ACCOUNT.to_ascii_uppercase()).unwrap(),
        canonical
    );
    for prefix in ["thor", "tthor", "sthor", "cthor"] {
        let address = bech32::encode::<Bech32>(Hrp::parse(prefix).unwrap(), &[7u8; 20]).unwrap();
        assert_eq!(Address::parse(&address).unwrap().prefix().as_str(), prefix);
    }
    for invalid in [
        ACCOUNT.replacen('t', "T", 1),
        format!("{}a", &ACCOUNT[..ACCOUNT.len() - 1]),
        bech32::encode::<Bech32m>(Hrp::parse("thor").unwrap(), &[7u8; 20]).unwrap(),
        bech32::encode::<Bech32>(Hrp::parse("cosmos").unwrap(), &[7u8; 20]).unwrap(),
        bech32::encode::<Bech32>(Hrp::parse("thor").unwrap(), &[7u8; 21]).unwrap(),
    ] {
        assert!(Address::parse(&invalid).is_err());
        assert!(serde_json::from_value::<Address>(json!(invalid)).is_err());
    }
    let hrp = Hrp::parse("thor").unwrap();
    let invalid_padding: String = [7u8; 20]
        .into_iter()
        .bytes_to_fes()
        .chain(std::iter::once(Fe32::Q))
        .with_checksum::<Bech32>(&hrp)
        .chars()
        .collect();
    assert!(bech32::primitives::decode::CheckedHrpstring::new::<Bech32>(&invalid_padding).is_ok());
    assert!(Address::parse(&invalid_padding).is_err());
}
#[test]
fn full_asset_forms_preserve_holding_chain_and_contract_identity() {
    for (input, kind, held) in [
        ("btc.btc", AssetKind::LayerOne, false),
        ("BTC/BTC", AssetKind::Synthetic, true),
        ("BTC~BTC", AssetKind::Trade, true),
        ("BTC-BTC", AssetKind::Secured, true),
        ("THOR.RUNE", AssetKind::LayerOne, true),
    ] {
        let asset = Asset::parse(input).unwrap();
        assert_eq!(asset.kind(), kind);
        assert_eq!(asset.is_thor_held(), held);
        assert_eq!(
            serde_json::from_value::<Asset>(json!(asset.to_string())).unwrap(),
            asset
        );
    }
    assert_eq!(
        Asset::parse("eth.usdc-0xa0b86991").unwrap().to_string(),
        "ETH.USDC-0XA0B86991"
    );
    for invalid in [
        "BTC",
        "b",
        "THOR/RUNE",
        "THOR~RUNE",
        "THOR-RUNE",
        "BT.BTC",
        "BTC.B TC",
        "BTC/",
        "BTC.BTC/OTHER",
    ] {
        assert!(Asset::parse(invalid).is_err());
    }
    let params = SwapParameters {
        destination: Some(ChainAddress::new(Chain::parse("THOR").unwrap(), ACCOUNT).unwrap()),
        ..SwapParameters::default()
    };
    assert!(
        SwapRequest::new(
            Asset::parse("BTC.BTC").unwrap(),
            Asset::parse("ETH~USDC-0XA0B86991").unwrap(),
            amount("100").unwrap(),
            params
        )
        .is_ok()
    );
    assert!(
        SwapRequest::new(
            Asset::parse("BTC.BTC").unwrap(),
            Asset::parse("ETH.ETH").unwrap(),
            amount("100").unwrap(),
            SwapParameters {
                destination: Some(
                    ChainAddress::new(Chain::parse("THOR").unwrap(), ACCOUNT).unwrap()
                ),
                ..SwapParameters::default()
            }
        )
        .is_err()
    );
    assert!(ChainAddress::new(Chain::parse("BTC").unwrap(), "bad address").is_err());
    assert!(ChainAddress::new(Chain::parse("BTC").unwrap(), "é").is_err());
}
#[test]
fn transaction_identifiers_retain_all_supported_source_forms_and_semantic_aliases() {
    let bare = id().unwrap();
    let prefixed = Txid::parse(&format!("0x{bare}")).unwrap();
    assert_ne!(bare, prefixed);
    assert!(bare.same_transaction(&prefixed));
    assert_eq!(prefixed.kind(), TxidKind::PrefixedHash);
    assert!(prefixed.to_string().starts_with("0X"));
    let indexed = Txid::parse(&format!("{bare}-1")).unwrap();
    let padded = Txid::parse(&format!("{bare}-01")).unwrap();
    assert_eq!(indexed.kind(), TxidKind::CosmosIndexed);
    assert!(indexed.same_transaction(&padded));
    assert!(!bare.same_transaction(&indexed));
    assert!(!indexed.same_transaction(&Txid::parse(&format!("{bare}-2")).unwrap()));
    let signature = bs58::encode([4u8; 64]).into_string();
    assert!((87..=88).contains(&signature.len()));
    let sol = Txid::parse(&signature).unwrap();
    assert_eq!(sol.kind(), TxidKind::SolanaSignature);
    assert_eq!(sol.as_str(), signature);
    let case_change: String = signature
        .chars()
        .map(|c| if c == 'a' { 'A' } else { c })
        .collect();
    assert_ne!(case_change, signature);
    let different = Txid::parse(&case_change).unwrap();
    assert!(!sol.same_transaction(&different));
    for valid in [bare, prefixed, indexed, padded, sol] {
        assert_eq!(
            serde_json::from_value::<Txid>(json!(valid.to_string())).unwrap(),
            valid
        );
    }
    for invalid in [
        "ab".repeat(31),
        "gg".repeat(32),
        format!("0z{}", id().unwrap()),
        format!("{}-18446744073709551616", id().unwrap()),
        format!("{}-", id().unwrap()),
        format!("{}-x", id().unwrap()),
        "0".repeat(88),
        format!("1{signature}"),
        "é".repeat(32),
    ] {
        assert!(
            Txid::parse(&invalid).is_err(),
            "accepted invalid shape: {invalid}"
        );
        assert!(serde_json::from_value::<Txid>(json!(invalid)).is_err());
    }
    assert!(Txid::parse(&"0".repeat(64)).unwrap().is_blank());
}
#[test]
fn protocol_and_external_native_values_are_exact_and_distinct() {
    let value = amount("9007199254740993").unwrap();
    assert_eq!(value.raw().to_string(), "9007199254740993");
    assert_eq!(value.amount().decimals(), Some(8));
    let maximum = ProtocolAmount::new(U256::MAX);
    assert_eq!(
        ProtocolAmount::from_decimal(&U256::MAX.to_string()).unwrap(),
        maximum
    );
    assert_eq!(
        serde_json::from_value::<ProtocolAmount>(serde_json::to_value(maximum).unwrap()).unwrap(),
        maximum
    );
    for invalid in ["-1", "+1", "01", "1.0", "1e8", &format!("{}0", U256::MAX)] {
        assert!(ProtocolAmount::from_decimal(invalid).is_err());
        assert!(RawQuantity::from_decimal(invalid).is_err());
    }
    let quote = SwapQuote::new(
        request().unwrap(),
        quote_data().unwrap(),
        Timestamp::from_unix_seconds(100),
    )
    .unwrap();
    assert_eq!(quote.data().dust_threshold.unwrap(), raw("1000").unwrap());
    assert_eq!(
        quote.data().recommended_min_amount_in.unwrap(),
        amount("6309").unwrap()
    );
    assert_eq!(
        quote.data().gas_rate_units.as_ref().unwrap().as_str(),
        "satsperbyte"
    );
}
#[test]
fn pool_ownership_sum_is_checked_without_float_or_overflow() {
    let valid = Pool::new(pool_data().unwrap()).unwrap();
    assert_eq!(
        valid.data().pool_units.raw().to_string(),
        "10169555322973480759703440"
    );
    assert_eq!(
        serde_json::from_value::<Pool>(serde_json::to_value(&valid).unwrap()).unwrap(),
        valid
    );
    let mut wrong = pool_data().unwrap();
    wrong.pool_units = raw("1").unwrap();
    assert!(Pool::new(wrong).is_err());
    let mut overflow = pool_data().unwrap();
    overflow.lp_units = RawQuantity::new(U256::MAX);
    overflow.synth_units = raw("1").unwrap();
    assert!(Pool::new(overflow).is_err());
    let mut wrapped = pool_data().unwrap();
    wrapped.asset = Asset::parse("BTC/BTC").unwrap();
    assert!(Pool::new(wrapped).is_err());
    assert!(Pools::new(vec![valid.clone(), valid]).is_err());
}
#[test]
fn quote_expiry_fee_denomination_and_partial_components_are_strict() {
    assert_eq!(
        SwapQuote::new(
            request().unwrap(),
            quote_data().unwrap(),
            Timestamp::from_unix_seconds(200)
        )
        .unwrap_err(),
        Error::UnavailableData
    );
    let mut data = quote_data().unwrap();
    data.fees.asset = Asset::parse("BTC.BTC").unwrap();
    assert!(SwapQuote::new(request().unwrap(), data, Timestamp::from_unix_seconds(100)).is_err());
    let mut data = quote_data().unwrap();
    data.fees.total = amount("6496743").unwrap();
    assert!(SwapQuote::new(request().unwrap(), data, Timestamp::from_unix_seconds(100)).is_err());
    let mut data = quote_data().unwrap();
    data.fees.affiliate = None;
    data.fees.total = amount("6496743").unwrap();
    let quote =
        SwapQuote::new(request().unwrap(), data, Timestamp::from_unix_seconds(100)).unwrap();
    assert!(quote.data().fees.affiliate.is_none());
    let mut data = quote_data().unwrap();
    data.fees.liquidity = ProtocolAmount::new(U256::MAX);
    assert!(SwapQuote::new(request().unwrap(), data, Timestamp::from_unix_seconds(100)).is_err());
    let mut data = quote_data().unwrap();
    data.gas_rate_units = None;
    assert!(SwapQuote::new(request().unwrap(), data, Timestamp::from_unix_seconds(100)).is_err());
    assert!(BasisPoints::new(10001).is_err());
    assert!(
        SwapRequest::new(
            Asset::parse("BTC.BTC").unwrap(),
            Asset::parse("ETH.ETH").unwrap(),
            amount("0").unwrap(),
            SwapParameters::default()
        )
        .is_err()
    );
}
#[test]
fn schema_query_and_observation_time_bindings_survive_deserialization() {
    let quote = SwapQuote::new(
        request().unwrap(),
        quote_data().unwrap(),
        Timestamp::from_unix_seconds(100),
    )
    .unwrap();
    let obs = Observation::swap_quote(
        quote,
        context(
            Operation::SwapQuote {
                request: Box::new(request().unwrap()),
            },
            100,
        )
        .unwrap(),
    )
    .unwrap();
    let mut value = serde_json::to_value(&obs).unwrap();
    assert_eq!(
        serde_json::from_value::<Observation<SwapQuote>>(value.clone()).unwrap(),
        obs
    );
    value["context"]["retrieved_at"] = json!(101);
    assert!(serde_json::from_value::<Observation<SwapQuote>>(value).is_err());
    let balance = Observation::rune_balance(
        RuneBalance::new(Address::parse(ACCOUNT).unwrap(), amount("0").unwrap()),
        context(
            Operation::RuneBalance {
                address: Address::parse(ACCOUNT).unwrap(),
            },
            100,
        )
        .unwrap(),
    )
    .unwrap();
    let mut value = serde_json::to_value(&balance).unwrap();
    value["context"]["schema_version"] = json!(2);
    assert!(serde_json::from_value::<Observation<RuneBalance>>(value).is_err());
    assert!(
        Observation::pool(
            Pool::new(pool_data().unwrap()).unwrap(),
            context(
                Operation::Pool {
                    asset: Asset::parse("ETH.ETH").unwrap()
                },
                100
            )
            .unwrap()
        )
        .is_err()
    );
}
#[test]
fn source_heights_counts_and_completion_are_not_invented_finality() {
    let block = LastBlock::new(
        Chain::parse("ETH").unwrap(),
        900_000_000,
        28_135_394,
        28_135_398,
    )
    .unwrap();
    assert_eq!(block.last_observed_in(), 900_000_000);
    assert!(LastBlock::new(Chain::parse("BTC").unwrap(), 1, 3, 2).is_err());
    assert!(LastBlocks::new(vec![block.clone(), block]).is_err());
    let status = TransactionStatus::new(
        id().unwrap(),
        Some(transaction(id().unwrap()).unwrap()),
        None,
        Some(vec![]),
        stages(),
    )
    .unwrap();
    assert_eq!(
        status
            .stages()
            .swap_status
            .as_ref()
            .unwrap()
            .streaming
            .as_ref()
            .unwrap()
            .count,
        3
    );
    assert!(status.stages().outbound_signed.is_none());
    assert!(status.planned_outbounds().is_none());
    assert_eq!(status.outbounds(), Some(&[][..]));
    assert!(status.transaction().unwrap().data().gas.is_none());
    let serialized = serde_json::to_value(&status).unwrap();
    assert!(serialized["planned_outbounds"].is_null());
    assert_eq!(serialized["outbounds"], json!([]));
    assert_eq!(
        serde_json::from_value::<TransactionStatus>(serialized).unwrap(),
        status
    );
}
#[test]
fn status_checks_identity_chains_refunds_and_nonblank_duplicate_ids() {
    let alias = Txid::parse(&format!("0x{}", id().unwrap())).unwrap();
    assert!(
        TransactionStatus::new(
            alias,
            Some(transaction(id().unwrap()).unwrap()),
            None,
            None,
            stages()
        )
        .is_ok()
    );
    assert!(
        TransactionStatus::new(
            Txid::parse(&"cd".repeat(32)).unwrap(),
            Some(transaction(id().unwrap()).unwrap()),
            None,
            None,
            stages()
        )
        .is_err()
    );
    assert!(
        TransactionStatus::new(
            id().unwrap(),
            Some(transaction(id().unwrap()).unwrap()),
            None,
            Some(vec![
                transaction(id().unwrap()).unwrap(),
                transaction(id().unwrap()).unwrap()
            ]),
            stages()
        )
        .is_err()
    );
    let blank = Txid::parse(&"0".repeat(64)).unwrap();
    assert!(
        TransactionStatus::new(
            id().unwrap(),
            None,
            None,
            Some(vec![
                transaction(blank.clone()).unwrap(),
                transaction(blank).unwrap()
            ]),
            stages()
        )
        .is_ok()
    );
    let mut invalid = stages();
    invalid.inbound_confirmation_counted = Some(ConfirmationStage {
        counting_start_height: Some(100),
        chain: Some(Chain::parse("BTC").unwrap()),
        external_observed_height: Some(10),
        external_confirmation_delay_height: Some(9),
        remaining_confirmation_seconds: Some(30),
        completed: false,
    });
    assert!(
        TransactionStatus::new(
            id().unwrap(),
            Some(transaction(id().unwrap()).unwrap()),
            None,
            None,
            invalid
        )
        .is_err()
    );
    let planned = PlannedOutbound::new(PlannedOutboundData {
        chain: Chain::parse("BTC").unwrap(),
        to_address: ChainAddress::new(Chain::parse("BTC").unwrap(), "bc1qrefund").unwrap(),
        coin: Coin {
            asset: Asset::parse("BTC.BTC").unwrap(),
            amount: amount("100").unwrap(),
            decimals: None,
        },
        refund: true,
    })
    .unwrap();
    let status =
        TransactionStatus::new(id().unwrap(), None, Some(vec![planned]), None, stages()).unwrap();
    assert!(status.planned_outbounds().unwrap()[0].data().refund);
}
#[test]
fn caller_limits_and_redacted_source_text_are_enforced() {
    assert!(CollectionLimit::new(0).is_err());
    assert!(CollectionLimit::new(100_001).is_err());
    let text = Text::new("private-source-memo").unwrap();
    assert!(!format!("{text:?}").contains("private-source-memo"));
    assert!(Text::new("bad\nsource").is_err());
    assert!(Text::new(&"x".repeat(4097)).is_err());
    let status = TransactionStatus::new(
        id().unwrap(),
        Some(transaction(id().unwrap()).unwrap()),
        None,
        Some(vec![]),
        stages(),
    )
    .unwrap();
    let obs = Observation::transaction_status(
        status,
        context(
            Operation::TransactionStatus {
                txid: id().unwrap(),
                limit: CollectionLimit::new(1).unwrap(),
            },
            100,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_value::<Observation<TransactionStatus>>(
            serde_json::to_value(&obs).unwrap()
        )
        .unwrap(),
        obs
    );
}
struct IndependentReader {
    balance: Rc<RuneBalance>,
}
impl ThorchainReader for IndependentReader {
    fn get_rune_balance(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<Observation<RuneBalance>, Error>> + Send {
        ready(
            context(Operation::RuneBalance { address }, 100).and_then(|context| {
                Observation::rune_balance(self.balance.as_ref().clone(), context)
            }),
        )
    }
    fn get_pool(&self, _: Asset) -> impl Future<Output = Result<Observation<Pool>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_pools(
        &self,
        _: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<Pools>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_network(&self) -> impl Future<Output = Result<Observation<NetworkData>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_swap_quote(
        &self,
        _: SwapRequest,
    ) -> impl Future<Output = Result<Observation<SwapQuote>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_inbound_addresses(
        &self,
        _: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<InboundAddresses>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_last_blocks(
        &self,
        _: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<LastBlocks>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_transaction_status(
        &self,
        _: Txid,
        _: CollectionLimit,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
}
#[test]
fn independent_non_send_implementor_returns_owned_send_futures_without_runtime() {
    let address = Address::parse(ACCOUNT).unwrap();
    let reader = IndependentReader {
        balance: Rc::new(RuneBalance::new(
            address.clone(),
            amount("9007199254740993").unwrap(),
        )),
    };
    let mut future = std::pin::pin!(reader.get_rune_balance(address));
    let waker = Waker::noop();
    let mut cx = TaskContext::from_waker(waker);
    let Poll::Ready(Ok(observation)) = future.as_mut().poll(&mut cx) else {
        panic!("ready capability did not return its observation")
    };
    assert_eq!(
        observation.value().amount(),
        amount("9007199254740993").unwrap()
    );
}

#[test]
fn deserialization_stops_at_the_collection_ceiling_before_duplicate_validation() {
    let block = LastBlock::new(Chain::parse("BTC").unwrap(), 1, 1, 1).unwrap();
    let item = serde_json::to_string(&block).unwrap();
    let data = format!(
        "[{}]",
        std::iter::repeat_n(item.as_str(), 100_001)
            .collect::<Vec<_>>()
            .join(",")
    );
    let error = serde_json::from_str::<LastBlocks>(&data).unwrap_err();
    assert!(error.to_string().contains("collection exceeds bound"));
}

#[test]
fn quote_tolerance_modes_enforce_constructor_and_serde_parity() {
    for (ordinary, liquidity, accepted) in [
        (Some(10_000), None, true),
        (Some(0), None, true),
        (None, Some(0), true),
        (None, Some(9999), true),
        (None, Some(10_000), false),
        (Some(100), Some(20), false),
        (Some(0), Some(0), false),
    ] {
        let parameters = SwapParameters {
            tolerance_bps: ordinary.map(BasisPoints::new).transpose().unwrap(),
            liquidity_tolerance_bps: liquidity.map(BasisPoints::new).transpose().unwrap(),
            ..SwapParameters::default()
        };
        let result = SwapRequest::new(
            Asset::parse("BTC.BTC").unwrap(),
            Asset::parse("ETH.ETH").unwrap(),
            amount("100000000").unwrap(),
            parameters.clone(),
        );
        let mut wire = serde_json::to_value(request().unwrap()).unwrap();
        wire["parameters"] = serde_json::to_value(parameters).unwrap();
        let decoded = serde_json::from_value::<SwapRequest>(wire);
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(decoded.is_ok(), accepted);
        if accepted {
            assert_eq!(decoded.unwrap(), result.unwrap());
        }
    }
}

#[test]
fn observed_transaction_memos_preserve_raw_utf8_controls_empty_and_byte_bounds() {
    let raw = "private memo\n\0\t雪";
    for text in [raw, ""] {
        let memo = TransactionMemo::new(text).unwrap();
        assert_eq!(memo.as_str(), text);
        assert_eq!(
            serde_json::from_value::<TransactionMemo>(serde_json::to_value(&memo).unwrap())
                .unwrap(),
            memo
        );
        assert!(!format!("{memo:?}").contains("private memo"));
        let mut data = transaction(id().unwrap()).unwrap().data().clone();
        data.memo = Some(memo);
        let tx = ChainTransaction::new(data).unwrap();
        assert_eq!(tx.data().memo.as_ref().unwrap().as_str(), text);
        assert_eq!(
            serde_json::from_value::<ChainTransaction>(serde_json::to_value(&tx).unwrap()).unwrap(),
            tx
        );
    }
    assert!(TransactionMemo::new(&"é".repeat(125)).is_ok());
    assert!(TransactionMemo::new(&"é".repeat(126)).is_err());
    assert!(TransactionMemo::new(&"x".repeat(251)).is_err());
    assert!(serde_json::from_value::<TransactionMemo>(json!("x".repeat(251))).is_err());
    assert!(Text::new(raw).is_err());
}

#[test]
fn explicit_quote_input_resolution_correlates_reported_assets_without_inferring_absence() {
    for resolution in [
        QuoteInputResolution::Unreported,
        QuoteInputResolution::SourceReported {
            asset: Asset::parse("BTC.BTC").unwrap(),
        },
    ] {
        let mut data = quote_data().unwrap();
        data.input_resolution = resolution.clone();
        let quote =
            SwapQuote::new(request().unwrap(), data, Timestamp::from_unix_seconds(100)).unwrap();
        assert_eq!(quote.data().input_resolution, resolution);
        assert_eq!(
            serde_json::from_value::<SwapQuote>(serde_json::to_value(&quote).unwrap()).unwrap(),
            quote
        );
    }
    let mut data = quote_data().unwrap();
    data.input_resolution = QuoteInputResolution::SourceReported {
        asset: Asset::parse("BTC/BTC").unwrap(),
    };
    assert!(SwapQuote::new(request().unwrap(), data, Timestamp::from_unix_seconds(100)).is_err());
    let quote = SwapQuote::new(
        request().unwrap(),
        quote_data().unwrap(),
        Timestamp::from_unix_seconds(100),
    )
    .unwrap();
    let mut wire = serde_json::to_value(&quote).unwrap();
    wire["data"]["input_resolution"] = json!({"kind":"source_reported","asset":"BTC/BTC"});
    assert!(serde_json::from_value::<SwapQuote>(wire).is_err());
}
