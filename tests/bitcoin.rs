// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Offline Bitcoin identities, exact records and backend-independent capabilities.

#![cfg(feature = "bitcoin")]

use std::{
    future::{Future, ready},
    task::{Context as TaskContext, Poll, Waker},
};

use regit_web3::{
    chains::bitcoin::BitcoinReader,
    domain::{
        ExactDecimal, Source, Timestamp,
        bitcoin::{
            Address, AddressBalance, BlockHash, BlockReference, Completeness, Context,
            FeeEstimates, HistoryCursor, HistoryEntry, HistoryPage, MempoolDelta, Network,
            NetworkId, Observation, Operation, Satoshis, TransactionStatus, Txid,
        },
    },
    error::{Error, ValidationError},
};
use serde_json::json;

const ADDRESS: &str = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT";

fn address() -> Result<Address, Error> {
    Address::parse(ADDRESS, Network::Mainnet)
}
fn txid(index: u8) -> Result<Txid, Error> {
    Txid::parse(&format!("{index:064x}"))
}
fn context(operation: Operation) -> Result<Context, Error> {
    Context::new(
        NetworkId::new(Network::Mainnet, "main")?,
        operation,
        Source::new("fixture", "address", "0.1.0")?,
        Timestamp::from_unix_seconds(100),
    )
}
fn block() -> BlockReference {
    BlockReference::new(
        7,
        Network::Mainnet.genesis_hash(),
        Timestamp::from_unix_seconds(50),
    )
}

#[test]
fn standard_genesis_identities_are_full_distinct_and_validated() {
    let networks = [
        Network::Mainnet,
        Network::Testnet3,
        Network::Testnet4,
        Network::Signet,
        Network::Regtest,
    ];
    let hashes = networks.map(|network| network.genesis_hash().to_string());
    assert_eq!(
        hashes[0],
        "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
    );
    assert_eq!(
        hashes[1],
        "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943"
    );
    assert_eq!(
        hashes[4],
        "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206"
    );
    for (index, hash) in hashes.iter().enumerate() {
        assert!(!hashes[..index].contains(hash));
        let identity = NetworkId::new(networks[index], "caller-alias").unwrap();
        assert_eq!(identity.genesis_hash().to_string(), *hash);
        let mut wire = serde_json::to_value(&identity).unwrap();
        assert_eq!(
            serde_json::from_value::<NetworkId>(wire.clone()).unwrap(),
            identity
        );
        wire["genesis_hash"] = json!("00".repeat(32));
        assert!(serde_json::from_value::<NetworkId>(wire).is_err());
    }
    for invalid in [
        "",
        "not an alias",
        "https://secret.invalid",
        "authorization:token",
    ] {
        assert!(NetworkId::new(Network::Mainnet, invalid).is_err());
    }
}

#[test]
fn address_checksum_and_declared_network_are_distinct_from_endpoint_identity() {
    let main = address().unwrap();
    assert_eq!(main.to_string(), ADDRESS);
    assert_eq!(main.network(), Network::Mainnet);
    assert_eq!(
        Address::parse(ADDRESS, Network::Signet).unwrap_err(),
        Error::Validation(ValidationError::BitcoinAddressNetworkMismatch)
    );
    let shared = "mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn";
    for network in [
        Network::Testnet3,
        Network::Testnet4,
        Network::Signet,
        Network::Regtest,
    ] {
        let parsed = Address::parse(shared, network).unwrap();
        assert_eq!(parsed.network(), network);
        assert_eq!(parsed.to_string(), shared);
        assert_eq!(
            serde_json::from_value::<Address>(serde_json::to_value(&parsed).unwrap()).unwrap(),
            parsed
        );
    }
    for invalid in [
        "",
        "FAKE_SECRET",
        "1BoatSLRHtKNngkdXEeobR76b53LETtpyX",
        " 1BoatSLRHtKNngkdXEeobR76b53LETtpyT",
    ] {
        let error = Address::parse(invalid, Network::Mainnet).unwrap_err();
        assert!(!error.to_string().contains("FAKE_SECRET"));
    }
    let mut value = serde_json::to_value(main).unwrap();
    value["network"] = json!("testnet3");
    assert!(serde_json::from_value::<Address>(value).is_err());
}

#[test]
fn hashes_use_full_width_and_canonical_lowercase_hex() {
    for value in ["00".repeat(32), "AB".repeat(32)] {
        assert_eq!(
            BlockHash::parse(&value).unwrap().to_string(),
            value.to_lowercase()
        );
        assert_eq!(
            serde_json::to_value(Txid::parse(&value).unwrap()).unwrap(),
            json!(value.to_lowercase())
        );
    }
    for invalid in ["", "0x00", "abc", &"g".repeat(64)] {
        assert!(BlockHash::parse(invalid).is_err());
        assert!(Txid::parse(invalid).is_err());
    }
}

#[test]
fn satoshis_and_signed_mempool_delta_are_exact_without_rounding() {
    for raw in [0, 9_007_199_254_740_993, u64::MAX] {
        let value = Satoshis::from_decimal(&raw.to_string()).unwrap();
        assert_eq!(value.raw(), raw);
        assert_eq!(value.amount().decimals(), Some(8));
        assert_eq!(serde_json::to_value(value).unwrap(), json!(raw.to_string()));
    }
    assert_eq!(
        Satoshis::new(1).amount().formatted().as_deref(),
        Some("0.00000001")
    );
    for invalid in ["18446744073709551616", "01", "+1", "-1", "1.0", "1e2", " 1"] {
        assert!(Satoshis::from_decimal(invalid).is_err());
    }
    for (funded, spent, expected) in [
        (0, u64::MAX, -i128::from(u64::MAX)),
        (u64::MAX, 0, i128::from(u64::MAX)),
        (1, 1, 0),
    ] {
        let delta = MempoolDelta::from_sums(funded, spent);
        assert_eq!(delta.raw(), expected);
        assert_eq!(
            serde_json::from_value::<MempoolDelta>(json!(expected.to_string())).unwrap(),
            delta
        );
    }
    for invalid in [
        "-0",
        "18446744073709551616",
        "-18446744073709551616",
        "01",
        "1.0",
    ] {
        assert!(serde_json::from_value::<MempoolDelta>(json!(invalid)).is_err());
    }
    assert!(serde_json::from_value::<Satoshis>(json!(1)).is_err());
}

#[test]
fn confirmed_balance_rejects_underrun_and_keeps_negative_mempool_delta() {
    let balance =
        AddressBalance::from_stats(address().unwrap(), u64::MAX, 1, 0, u64::MAX, 7, 8).unwrap();
    assert_eq!(balance.confirmed().raw(), u64::MAX - 1);
    assert_eq!(balance.mempool_delta().raw(), -i128::from(u64::MAX));
    assert_eq!(
        (balance.confirmed_tx_count(), balance.mempool_tx_count()),
        (7, 8)
    );
    assert!(AddressBalance::from_stats(address().unwrap(), 1, 2, 0, 0, 0, 0).is_err());
    assert_eq!(
        serde_json::from_value::<AddressBalance>(serde_json::to_value(&balance).unwrap()).unwrap(),
        balance
    );
}

#[test]
fn history_limits_and_next_cursor_do_not_claim_exhaustive_mempool_history() {
    let mut entries = (1..=50)
        .map(|index| {
            HistoryEntry::new(
                txid(index).unwrap(),
                Satoshis::new(1),
                TransactionStatus::Unconfirmed,
            )
        })
        .collect::<Vec<_>>();
    entries.extend((51..=75).map(|index| {
        HistoryEntry::new(
            txid(index).unwrap(),
            Satoshis::new(2),
            TransactionStatus::Confirmed(block()),
        )
    }));
    let page =
        HistoryPage::new(address().unwrap(), HistoryCursor::Recent, entries.clone()).unwrap();
    assert_eq!(page.entries().len(), 75);
    assert_eq!(page.confirmed_completeness(), Completeness::MayHaveMore);
    assert_eq!(page.mempool_completeness(), Some(Completeness::MayHaveMore));
    assert_eq!(
        page.next_cursor(),
        Some(HistoryCursor::Confirmed {
            after: Some(txid(75).unwrap())
        })
    );
    assert_eq!(
        serde_json::from_value::<HistoryPage>(serde_json::to_value(&page).unwrap()).unwrap(),
        page
    );
    assert!(
        HistoryPage::new(
            address().unwrap(),
            HistoryCursor::Confirmed { after: None },
            entries.clone()
        )
        .is_err()
    );
    entries.push(HistoryEntry::new(
        txid(76).unwrap(),
        Satoshis::new(0),
        TransactionStatus::Unconfirmed,
    ));
    assert!(HistoryPage::new(address().unwrap(), HistoryCursor::Recent, entries).is_err());
    let duplicate = HistoryEntry::new(
        txid(1).unwrap(),
        Satoshis::new(0),
        TransactionStatus::Unconfirmed,
    );
    assert!(
        HistoryPage::new(
            address().unwrap(),
            HistoryCursor::Recent,
            vec![duplicate.clone(), duplicate]
        )
        .is_err()
    );
    let empty = HistoryPage::new(
        address().unwrap(),
        HistoryCursor::Confirmed { after: None },
        vec![],
    )
    .unwrap();
    assert_eq!(empty.confirmed_completeness(), Completeness::Complete);
    assert_eq!(empty.mempool_completeness(), None);
    assert_eq!(empty.next_cursor(), None);
}

#[test]
fn fee_estimates_have_exact_rates_positive_unique_horizons_and_validated_serde() {
    let fees = FeeEstimates::new([
        (
            u64::MAX,
            ExactDecimal::parse("9007199254740993.125").unwrap(),
        ),
        (1, ExactDecimal::parse("0").unwrap()),
    ])
    .unwrap();
    assert_eq!(fees.rates()[&u64::MAX].canonical(), "9007199254740993.125");
    assert_eq!(
        serde_json::from_value::<FeeEstimates>(serde_json::to_value(&fees).unwrap()).unwrap(),
        fees
    );
    for invalid in [
        r#"{"1":"2","1":"3"}"#,
        r#"{"0":"1"}"#,
        r#"{"01":"1"}"#,
        r#"{"1":"-1"}"#,
        r#"{"1":1}"#,
    ] {
        assert!(serde_json::from_str::<FeeEstimates>(invalid).is_err());
    }
}

#[test]
fn observation_serde_checks_schema_query_network_and_value_identity() {
    let requested = address().unwrap();
    let balance = AddressBalance::from_stats(requested.clone(), 3, 2, 0, 0, 1, 0).unwrap();
    let observation = Observation::address_balance(
        balance,
        context(Operation::AddressBalance { address: requested }).unwrap(),
    )
    .unwrap();
    let wire = serde_json::to_value(&observation).unwrap();
    assert_eq!(wire["context"]["schema_version"], 1);
    assert_eq!(wire["value"]["confirmed"], "1");
    assert!(wire["context"].get("block").is_none());
    assert!(wire["context"].get("finality").is_none());
    assert_eq!(
        serde_json::from_value::<Observation<AddressBalance>>(wire.clone()).unwrap(),
        observation
    );
    let mut bad = wire.clone();
    bad["context"]["schema_version"] = json!(2);
    assert!(serde_json::from_value::<Observation<AddressBalance>>(bad).is_err());
    let mut bad = wire.clone();
    bad["context"]["operation"] = json!({"kind":"fee_estimates"});
    assert!(serde_json::from_value::<Observation<AddressBalance>>(bad).is_err());
    let mut bad = wire;
    bad["context"]["network"] =
        serde_json::to_value(NetworkId::new(Network::Signet, "signet").unwrap()).unwrap();
    assert!(serde_json::from_value::<Observation<AddressBalance>>(bad).is_err());
}

struct LocalReader;
impl BitcoinReader for LocalReader {
    fn get_address_balance(
        &self,
        address: Address,
    ) -> impl Future<Output = Result<Observation<AddressBalance>, Error>> + Send {
        let result =
            AddressBalance::from_stats(address.clone(), 7, 2, 0, 1, 2, 1).and_then(|balance| {
                Observation::address_balance(
                    balance,
                    context(Operation::AddressBalance { address })?,
                )
            });
        ready(result)
    }
    fn get_address_history(
        &self,
        address: Address,
        cursor: HistoryCursor,
    ) -> impl Future<Output = Result<Observation<HistoryPage>, Error>> + Send {
        ready(
            HistoryPage::new(address.clone(), cursor, vec![]).and_then(|page| {
                Observation::address_history(
                    page,
                    context(Operation::AddressHistory { address, cursor })?,
                )
            }),
        )
    }
    fn get_fee_estimates(
        &self,
    ) -> impl Future<Output = Result<Observation<FeeEstimates>, Error>> + Send {
        ready(
            FeeEstimates::new([]).and_then(|fees| {
                Observation::fee_estimates(fees, context(Operation::FeeEstimates)?)
            }),
        )
    }
    fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + Send {
        ready(
            context(Operation::TransactionStatus { txid }).and_then(|context| {
                Observation::transaction_status(TransactionStatus::Unconfirmed, context)
            }),
        )
    }
}
#[test]
fn caller_backend_returns_send_future_without_http_or_runtime() {
    fn assert_send<T: Send>(_: &T) {}
    let reader = LocalReader;
    let future = reader.get_address_balance(address().unwrap());
    assert_send(&future);
    let mut future = std::pin::pin!(future);
    let mut context = TaskContext::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(Ok(value)) => {
            assert_eq!(value.value().confirmed().raw(), 5);
            assert_eq!(value.value().mempool_delta().raw(), -1);
        }
        _ => panic!("offline reader must be ready"),
    }
}
