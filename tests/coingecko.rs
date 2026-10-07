// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure market request and exact provider-data invariant tests.
#![cfg(test)]
#![cfg(feature = "coingecko")]
use regit_web3::{
    domain::{
        ExactDecimal,
        coingecko::{CoinId, Currency, HistoryRequest, MarketsRequest, PricesRequest, SearchQuery},
        market::{ItemLimit, NonnegativeDecimal, UtcDateTime},
    },
    error::Error,
};
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn request_identity_and_capacity_invariants_survive_deserialization()
-> Result<(), Box<dyn std::error::Error>> {
    let id = CoinId::parse("bitcoin")?;
    let usd = Currency::parse("usd")?;
    assert!(PricesRequest::new(vec![id.clone(), id.clone()], vec![usd.clone()]).is_err());
    assert!(PricesRequest::new(vec![id.clone()], vec![usd.clone(), usd.clone()]).is_err());
    assert!(MarketsRequest::new(usd.clone(), 0, 250, None).is_err());
    assert!(MarketsRequest::new(usd.clone(), 1, 251, None).is_err());
    assert!(MarketsRequest::new(usd.clone(), 1, 1, Some(vec![])).is_err());
    assert!(HistoryRequest::new(id.clone(), usd.clone(), 2, 1).is_err());
    assert!(HistoryRequest::new(id, usd, 0, u64::MAX).is_err());
    assert!(Currency::parse("USD").is_err());
    assert!(CoinId::parse("../secret").is_err());
    assert!(SearchQuery::new(" ").is_err());
    assert!(ItemLimit::new(100_001).is_err());
    assert!(
        serde_json::from_str::<MarketsRequest>(r#"{"currency":"usd","page":1,"count":1}"#).is_err()
    );
    assert!(
        serde_json::from_str::<PricesRequest>(
            r#"{"ids":["bitcoin","bitcoin"],"currencies":["usd"]}"#
        )
        .is_err()
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn exact_signed_and_nonnegative_values_never_round() -> Result<(), Error> {
    let value =
        NonnegativeDecimal::new(ExactDecimal::parse("9007199254740993.000000000000000001")?)?;
    assert_eq!(
        value.value().canonical(),
        "9007199254740993.000000000000000001"
    );
    assert!(NonnegativeDecimal::new(ExactDecimal::parse("-0.1")?).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn utc_calendar_fixed_digits_and_fraction_contract_match_serde()
-> Result<(), Box<dyn std::error::Error>> {
    for raw in [
        "2024-02-29T23:59:59Z",
        "2000-02-29T00:00:00.123456789Z",
        "2026-02-01T01:00:00.0Z",
    ] {
        let parsed = UtcDateTime::parse(raw)?;
        assert_eq!(parsed.as_str(), raw);
        assert_eq!(
            serde_json::from_str::<UtcDateTime>(&serde_json::to_string(raw)?)?,
            parsed
        );
    }
    for raw in [
        "2026-02-01T+1:00:00Z",
        "+026-02-01T01:00:00Z",
        "2026-+2-01T01:00:00Z",
        "2026-02-+1T01:00:00Z",
        "2026-02-01T01:+0:00Z",
        "2026-02-01T01:00:+0Z",
        "2026-02-01T 1:00:00Z",
        "1900-02-29T00:00:00Z",
        "2026-02-29T00:00:00Z",
        "2026-04-31T00:00:00Z",
        "2026-01-01T24:00:00Z",
        "2026-01-01T00:00:60Z",
        "2026-01-01T00:00:00.Z",
        "2026-01-01T00:00:00.1234567890Z",
        "2026-01-01T00:00:00+00:00",
    ] {
        assert!(UtcDateTime::parse(raw).is_err(), "accepted {raw}");
        assert!(
            serde_json::from_str::<UtcDateTime>(&serde_json::to_string(raw)?).is_err(),
            "serde accepted {raw}"
        );
    }
    Ok(())
}
