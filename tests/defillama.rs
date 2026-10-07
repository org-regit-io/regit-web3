// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure provider units, time ordering, identity and nullable-value invariants.
#![cfg(feature = "defillama")]
use regit_web3::{
    domain::{
        ExactDecimal, Timestamp,
        defillama::{
            AnalyticsMetric, AnalyticsRequest, BreakdownValue, ProviderId, Stablecoin,
            StablecoinHistory, StablecoinId, StablecoinPoint, StablecoinScope, TvlHistory,
            TvlScope, UsdPoint, YieldHistory, YieldPoint, YieldRates,
        },
        market::{Label, NonnegativeDecimal, UtcDateTime},
    },
    error::Error,
};
fn value(raw: &str) -> Result<NonnegativeDecimal, Error> {
    NonnegativeDecimal::new(ExactDecimal::parse(raw)?)
}
#[test]
fn exact_units_signed_apy_and_null_components_are_retained()
-> Result<(), Box<dyn std::error::Error>> {
    let rates = YieldRates::new(
        Some(ExactDecimal::parse("-0.000000000000000001")?),
        None,
        Some(ExactDecimal::parse("2.5e2")?),
    );
    assert_eq!(
        rates.apy().as_ref().unwrap().canonical(),
        "-0.000000000000000001"
    );
    assert!(rates.base().is_none());
    assert_eq!(rates.reward().as_ref().unwrap().canonical(), "250");
    let amount = BreakdownValue::new(Label::new("peggedEUR")?, value("9007199254740993.125")?);
    let usd = BreakdownValue::new(Label::new("peggedEUR")?, value("9999999999999999.625")?);
    let point = StablecoinPoint::new(Timestamp::from_unix_seconds(1), vec![amount], vec![usd])?;
    assert_ne!(point.circulating(), point.circulating_usd());
    let asset = Stablecoin::new(
        StablecoinId::new(1)?,
        Label::new("Coin")?,
        Label::new("COIN")?,
        Label::new("peggedEUR")?,
        point.circulating().to_vec(),
        None,
        vec![Label::new("Ethereum")?],
    )?;
    assert!(asset.price_usd().is_none());
    let json = serde_json::to_string(&asset)?;
    assert_eq!(serde_json::from_str::<Stablecoin>(&json)?, asset);
    assert!(serde_json::from_str::<YieldRates>(r#"{"apy":"1","reward":null}"#).is_err());
    Ok(())
}
#[test]
fn duplicate_and_reversed_time_series_fail_in_constructor_and_serde()
-> Result<(), Box<dyn std::error::Error>> {
    let point = UsdPoint::new(Timestamp::from_unix_seconds(10), value("1")?);
    assert!(TvlHistory::new(TvlScope::All, vec![point.clone(), point.clone()]).is_err());
    let next = UsdPoint::new(Timestamp::from_unix_seconds(11), value("2")?);
    assert!(TvlHistory::new(TvlScope::All, vec![next, point]).is_err());
    assert!(
        serde_json::from_str::<TvlHistory>(
            r#"{"scope":{"scope":"all"},"points":[{"date":1,"usd":"1"},{"date":1,"usd":"2"}]}"#
        )
        .is_err()
    );
    let rates = YieldRates::new(None, None, None);
    let plain = YieldPoint::new(
        UtcDateTime::parse("2026-10-07T00:00:00Z")?,
        value("1")?,
        rates.clone(),
    );
    let fraction = YieldPoint::new(
        UtcDateTime::parse("2026-10-07T00:00:00.000Z")?,
        value("1")?,
        rates.clone(),
    );
    assert!(YieldHistory::new(ProviderId::parse("pool")?, vec![plain, fraction]).is_err());

    for (a, b) in [
        ("2026-10-07T00:00:00.1Z", "2026-10-07T00:00:00.100Z"),
        ("2026-10-07T00:00:00Z", "2026-10-07T00:00:00.000Z"),
    ] {
        let a = YieldPoint::new(
            UtcDateTime::parse(a)?,
            value("1")?,
            YieldRates::new(None, None, None),
        );
        let b = YieldPoint::new(
            UtcDateTime::parse(b)?,
            value("1")?,
            YieldRates::new(None, None, None),
        );
        assert!(YieldHistory::new(ProviderId::parse("pool")?, vec![a, b]).is_err());
    }
    let a = YieldPoint::new(
        UtcDateTime::parse("2026-10-07T00:00:00.9Z")?,
        value("1")?,
        YieldRates::new(None, None, None),
    );
    let b = YieldPoint::new(
        UtcDateTime::parse("2026-10-07T00:00:01Z")?,
        value("1")?,
        YieldRates::new(None, None, None),
    );
    assert!(YieldHistory::new(ProviderId::parse("pool")?, vec![a, b]).is_ok());

    let early = YieldPoint::new(
        UtcDateTime::parse("2026-10-07T00:00:00.001Z")?,
        value("1")?,
        rates.clone(),
    );
    let late = YieldPoint::new(
        UtcDateTime::parse("2026-10-07T00:00:00.01Z")?,
        value("1")?,
        rates,
    );
    assert!(YieldHistory::new(ProviderId::parse("pool")?, vec![early, late]).is_ok());
    Ok(())
}
#[test]
fn reporting_components_and_stablecoin_identity_are_not_assumed_or_summed() -> Result<(), Error> {
    assert!(StablecoinId::new(0).is_err());
    let component = BreakdownValue::new(Label::new("Ethereum-borrowed")?, value("5")?);
    assert!(
        StablecoinPoint::new(
            Timestamp::from_unix_seconds(1),
            vec![component.clone(), component],
            vec![]
        )
        .is_err()
    );
    let scope = StablecoinScope::new(Some(Label::new("Ethereum")?), Some(StablecoinId::new(1)?));
    assert!(StablecoinHistory::new(scope, vec![]).is_ok());
    let request = AnalyticsRequest::new(ProviderId::parse("aave")?, AnalyticsMetric::Revenue);
    assert_eq!(request.metric(), AnalyticsMetric::Revenue);
    Ok(())
}
