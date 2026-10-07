// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact decimal value, canonical wire, and bounded parsing contracts.

use regit_web3::{
    domain::{Amount, ExactDecimal},
    error::{Error, ValidationError},
};
use serde_json::json;

#[test]
fn signed_fractional_and_scientific_inputs_preserve_the_exact_value() {
    for (input, canonical) in [
        ("-9007199254740993.12500", "-9007199254740993.125"),
        ("1.25e-3", "0.00125"),
        ("-1.25E+3", "-1250"),
        ("100.5000", "100.5"),
        ("1e000000000000000000000000000000000000001", "10"),
        ("0.00000123000", "0.00000123"),
        ("1000000000000000000000", "1000000000000000000000"),
    ] {
        let value = ExactDecimal::parse(input).unwrap();
        assert_eq!(value.canonical(), canonical);
        assert_eq!(value.to_string(), canonical);
        assert_eq!(input.parse::<ExactDecimal>().unwrap(), value);
    }
}

#[test]
fn precision_exceeds_ieee754_and_unsigned_256_bit_limits_without_rounding() {
    let first = ExactDecimal::parse("9007199254740992").unwrap();
    let second = ExactDecimal::parse("9007199254740993").unwrap();
    assert!(first < second);
    assert_eq!(second.canonical(), "9007199254740993");

    let beyond_u256 =
        "115792089237316195423570985008687907853269984665640564039457584007913129639936";
    assert_eq!(
        ExactDecimal::parse(beyond_u256).unwrap().canonical(),
        beyond_u256
    );
    assert_eq!(
        Amount::from_decimal(beyond_u256, None).unwrap_err(),
        Error::Validation(ValidationError::AmountOverflow)
    );
}

#[test]
fn signed_issued_amount_exponents_remain_exact() {
    let minimum = ExactDecimal::parse("-1000000000000000e-96").unwrap();
    assert_eq!(minimum.canonical(), format!("-0.{}1", "0".repeat(80)));
    assert!(minimum.is_negative());
    assert!(!minimum.is_integer());

    let maximum = ExactDecimal::parse("9999999999999999e80").unwrap();
    assert_eq!(
        maximum.canonical(),
        format!("9999999999999999{}", "0".repeat(80))
    );
    assert!(maximum.is_integer());
}

#[test]
fn numeric_equality_ordering_and_zero_ignore_redundant_notation() {
    for input in ["0", "-0", "0.0000", "-0.000e+4096", "0e-4096"] {
        let value = ExactDecimal::parse(input).unwrap();
        assert_eq!(value.canonical(), "0");
        assert!(value.is_zero());
        assert!(value.is_integer());
        assert!(!value.is_negative());
        assert_eq!(value, ExactDecimal::parse("0").unwrap());
    }
    assert_eq!(
        ExactDecimal::parse("100.0000").unwrap(),
        ExactDecimal::parse("1e2").unwrap()
    );
    let negative = ExactDecimal::parse("-2").unwrap();
    let zero = ExactDecimal::parse("0").unwrap();
    let fraction = ExactDecimal::parse("0.0001").unwrap();
    assert!(negative < zero);
    assert!(zero < fraction);
    assert!(negative.is_integer());
    assert!(!fraction.is_integer());
}

#[test]
fn malformed_and_nonfinite_inputs_have_fixed_validation_errors() {
    for input in [
        "", "-", "+1", "01", "-01", ".5", "1.", "1.2.3", "1e", "e1", "1e+", "1e--2", "1e1e1",
        "NaN", "Infinity", "-inf", "1_000", "1,000", "0x10", " 1", "1 ", "1\n", "１", "1e１",
        "--1",
    ] {
        assert_eq!(
            ExactDecimal::parse(input).unwrap_err(),
            Error::Validation(ValidationError::InvalidDecimal)
        );
    }
}

#[test]
fn input_exponent_scale_and_expanded_output_limits_are_enforced() {
    let excessive_input = "9".repeat(ExactDecimal::MAX_TEXT_BYTES + 1);
    for input in [
        excessive_input.as_str(),
        "1e4097",
        "1e-4097",
        "0e4097",
        "0e-4097",
        "1e9223372036854775808",
        "1e-9223372036854775808",
        "1.0e-4096",
        "0.0e-4096",
        "10e4096",
        "1e4096",
        "1e-4095",
        "-1e4095",
    ] {
        assert_eq!(
            ExactDecimal::parse(input).unwrap_err(),
            Error::Validation(ValidationError::DecimalOutOfBounds)
        );
    }
}

#[test]
fn accepted_boundary_values_serialize_and_reparse_under_the_same_limits() {
    let largest_coefficient = "9".repeat(ExactDecimal::MAX_TEXT_BYTES);
    for input in [
        largest_coefficient.as_str(),
        "1e4095",
        "1e-4094",
        "-1e-4093",
    ] {
        let value = ExactDecimal::parse(input).unwrap();
        assert_eq!(value.canonical().len(), ExactDecimal::MAX_TEXT_BYTES);
        let serialized = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<ExactDecimal>(&serialized).unwrap(),
            value
        );
        assert_eq!(ExactDecimal::parse(&value.canonical()).unwrap(), value);
    }
}

#[test]
fn serde_requires_strings_and_reuses_all_constructor_validation() {
    let value = ExactDecimal::parse("1.2300e-3").unwrap();
    assert_eq!(serde_json::to_value(&value).unwrap(), json!("0.00123"));
    assert_eq!(
        serde_json::from_value::<ExactDecimal>(json!("1.2300e-3")).unwrap(),
        value
    );
    for raw in ["1", "1.25", "-2", "true", "null", "[]", "{}"] {
        assert!(serde_json::from_str::<ExactDecimal>(raw).is_err());
    }
    for input in ["01", "NaN", "1_000", "1e4097", "0.0e-4096", "1e4096"] {
        assert!(serde_json::from_value::<ExactDecimal>(json!(input)).is_err());
    }
}

#[test]
fn formatting_precision_never_rounds_or_expands_a_stored_value() {
    let value = ExactDecimal::parse("1.234567890123456789").unwrap();
    assert_eq!(format!("{value:.2}"), "1.234567890123456789");
    assert_eq!(
        format!("{value:.precision$}", precision = 60_000),
        "1.234567890123456789"
    );
}
