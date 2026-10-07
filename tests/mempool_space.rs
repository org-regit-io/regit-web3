// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Offline exact mempool records, bounds and constructor/serde parity.
#![cfg(test)]
#![cfg(feature = "mempool-space")]

use regit_web3::{
    domain::{
        ExactDecimal, Source, Timestamp,
        bitcoin::{Network, NetworkId, Satoshis, Txid},
        mempool_space::{
            Context, FeeHistogramBin, FeeRate, MempoolSummary, Observation, Operation,
            RecentTransaction, RecentTransactions, RecommendedFees, TransactionIds,
            TransactionLimit, VirtualSize,
        },
    },
    error::{Error, ValidationError},
};

fn rate(value: &str) -> Result<FeeRate, Error> {
    FeeRate::parse(value)
}
fn id(value: u8) -> Result<Txid, Error> {
    Txid::parse(&format!("{value:064x}"))
}
fn context(operation: Operation) -> Result<Context, Error> {
    Ok(Context::new(
        NetworkId::new(Network::Mainnet, "bitcoin")?,
        operation,
        Source::new("mempool-public", "fixture", "0.1.0")?,
        Timestamp::from_unix_seconds(123),
    ))
}
fn recent(value: u8) -> Result<RecentTransaction, Error> {
    RecentTransaction::new(
        id(value)?,
        Satoshis::new(23),
        VirtualSize::new(ExactDecimal::parse("152.25")?)?,
        Satoshis::new(1_999_999_999_999_993),
    )
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn fee_rates_and_source_virtual_sizes_preserve_exact_units()
-> Result<(), Box<dyn std::error::Error>> {
    let fees = RecommendedFees::new(
        rate("0.100000000000000001")?,
        rate("2")?,
        rate("1")?,
        rate("0")?,
        rate("3")?,
    );
    assert_eq!(fees.fastest().value().canonical(), "0.100000000000000001");
    assert_eq!(
        serde_json::from_str::<RecommendedFees>(&serde_json::to_string(&fees)?)?,
        fees
    );
    assert!(rate("-1").is_err());
    assert!(serde_json::from_str::<FeeRate>("\"-0.1\"").is_err());
    assert!(serde_json::from_str::<FeeRate>("0.1").is_err());
    let size = VirtualSize::new(ExactDecimal::parse("152.25")?)?;
    assert_eq!(size.value().canonical(), "152.25");
    for value in ["0", "-0.25", "1000000.01", "152.1"] {
        assert!(VirtualSize::new(ExactDecimal::parse(value)?).is_err());
    }
    for value in [0, 100_001, u32::MAX] {
        assert!(TransactionLimit::new(value).is_err());
    }
    assert_eq!(TransactionLimit::new(100_000)?.get(), 100_000);
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn summary_bins_are_individual_descending_and_can_be_partial_or_empty()
-> Result<(), Box<dyn std::error::Error>> {
    let bins = vec![
        FeeHistogramBin::new(rate("53.01")?, 10),
        FeeHistogramBin::new(rate("38.56")?, 20),
    ];
    let summary = MempoolSummary::new(2, 40, Satoshis::new(9_007_199_254_740_993), bins)?;
    assert_eq!(summary.histogram()[1].virtual_bytes(), 20);
    assert_eq!(
        serde_json::from_str::<MempoolSummary>(&serde_json::to_string(&summary)?)?,
        summary
    );
    assert!(
        MempoolSummary::new(2, 40, Satoshis::new(0), vec![])?
            .histogram()
            .is_empty()
    );
    assert!(MempoolSummary::new(0, 0, Satoshis::new(0), vec![]).is_ok());
    for bins in [
        vec![
            FeeHistogramBin::new(rate("1")?, 10),
            FeeHistogramBin::new(rate("1.0")?, 10),
        ],
        vec![
            FeeHistogramBin::new(rate("1")?, 10),
            FeeHistogramBin::new(rate("2")?, 10),
        ],
        vec![FeeHistogramBin::new(rate("1")?, 41)],
        vec![
            FeeHistogramBin::new(rate("2")?, u64::MAX),
            FeeHistogramBin::new(rate("1")?, 1),
        ],
    ] {
        assert!(MempoolSummary::new(2, 40, Satoshis::new(0), bins).is_err());
    }
    let mut value = serde_json::to_value(summary)?;
    value["transaction_count"] = serde_json::json!(0);
    assert!(serde_json::from_value::<MempoolSummary>(value).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn recent_entries_retain_fractional_size_and_reject_duplicates_and_money_overrun()
-> Result<(), Box<dyn std::error::Error>> {
    let value = RecentTransactions::new(vec![recent(1)?])?;
    assert_eq!(
        value.transactions()[0].virtual_size().value().canonical(),
        "152.25"
    );
    assert_eq!(
        serde_json::from_str::<RecentTransactions>(&serde_json::to_string(&value)?)?,
        value
    );
    assert!(RecentTransactions::new(vec![recent(1)?, recent(1)?]).is_err());
    assert!(RecentTransactions::new((1..=11).map(recent).collect::<Result<Vec<_>, _>>()?).is_err());
    assert!(
        RecentTransaction::new(
            id(1)?,
            Satoshis::new(1),
            VirtualSize::new(ExactDecimal::parse("1")?)?,
            Satoshis::new(bitcoin::Amount::MAX_MONEY.to_sat())
        )
        .is_err()
    );
    let mut value = serde_json::to_value(recent(1)?)?;
    value["virtual_size"] = serde_json::json!("0");
    assert!(serde_json::from_value::<RecentTransaction>(value).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn full_ids_fail_wholly_and_observations_validate_schema_operation_and_capacity()
-> Result<(), Box<dyn std::error::Error>> {
    let limit = TransactionLimit::new(2)?;
    assert!(TransactionIds::new(limit, vec![id(1)?, id(2)?, id(3)?]).is_err());
    assert!(TransactionIds::new(limit, vec![id(1)?, id(1)?]).is_err());
    let value = TransactionIds::new(limit, vec![id(2)?, id(1)?])?;
    let observed =
        Observation::transaction_ids(value.clone(), context(Operation::TransactionIds { limit })?)?;
    assert_eq!(observed.value().txids(), &[id(2)?, id(1)?]);
    assert_eq!(observed.context().retrieved_at().unix_seconds(), 123);
    assert_eq!(
        serde_json::from_str::<Observation<TransactionIds>>(&serde_json::to_string(&observed)?)?,
        observed
    );
    assert_eq!(
        Observation::transaction_ids(
            value,
            context(Operation::TransactionIds {
                limit: TransactionLimit::new(1)?
            })?
        ),
        Err(ValidationError::ObservationOperationMismatch.into())
    );
    for (field, value) in [
        ("schema_version", serde_json::json!(2)),
        (
            "value",
            serde_json::json!({"limit":1,"txids":[id(1)?,id(2)?]}),
        ),
    ] {
        let mut encoded = serde_json::to_value(&observed)?;
        encoded[field] = value;
        assert!(serde_json::from_value::<Observation<TransactionIds>>(encoded).is_err());
    }
    let duplicate = serde_json::to_string(&observed)?.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert!(serde_json::from_str::<Observation<TransactionIds>>(&duplicate).is_err());
    Ok(())
}
