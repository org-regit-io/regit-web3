// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact decimal value, canonical wire, and bounded parsing contracts.

#![cfg(test)]

use regit_web3::{
    domain::{Amount, ArithmeticError, ExactDecimal, RoundingMode},
    error::{Error, ValidationError},
};
use serde_json::json;

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
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

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
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

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
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

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
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

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
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

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
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

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
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

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
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

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn formatting_precision_never_rounds_or_expands_a_stored_value() {
    let value = ExactDecimal::parse("1.234567890123456789").unwrap();
    assert_eq!(format!("{value:.2}"), "1.234567890123456789");
    assert_eq!(
        format!("{value:.precision$}", precision = 60_000),
        "1.234567890123456789"
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn checked_arithmetic_preserves_sign_scale_alignment_and_large_exact_digits() {
    for (left, right, sum, difference, product) in [
        ("0.1", "0.2", "0.3", "-0.1", "0.02"),
        ("-2.5", "1.25", "-1.25", "-3.75", "-3.125"),
        ("-2.5", "-1.25", "-3.75", "-1.25", "3.125"),
        ("1e3", "1e-3", "1000.001", "999.999", "1"),
        (
            "9007199254740993.125",
            "0.875",
            "9007199254740994",
            "9007199254740992.25",
            "7881299347898368.984375",
        ),
    ] {
        let left = ExactDecimal::parse(left).unwrap();
        let right = ExactDecimal::parse(right).unwrap();
        assert_eq!(left.checked_add(&right).unwrap().canonical(), sum);
        assert_eq!(left.checked_sub(&right).unwrap().canonical(), difference);
        assert_eq!(left.checked_mul(&right).unwrap().canonical(), product);
        assert_eq!(left.checked_add(&right), right.checked_add(&left));
        assert_eq!(left.checked_mul(&right), right.checked_mul(&left));
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn arithmetic_normalizes_zero_and_preserves_serde_and_reparse_invariants() {
    let zero = ExactDecimal::parse("0").unwrap();
    for input in ["-1.23", "1e4095", "1e-4094", "999999999999999999.999999"] {
        let value = ExactDecimal::parse(input).unwrap();
        assert_eq!(value.checked_sub(&value).unwrap(), zero);
        assert_eq!(value.checked_mul(&zero).unwrap(), zero);
        assert_eq!(value.checked_add(&zero).unwrap(), value);
        let squared = value
            .checked_mul(&ExactDecimal::parse("1").unwrap())
            .unwrap();
        assert_eq!(squared, value);
        assert_eq!(ExactDecimal::parse(&squared.canonical()).unwrap(), squared);
        assert_eq!(
            serde_json::from_value::<ExactDecimal>(serde_json::to_value(&squared).unwrap())
                .unwrap(),
            squared
        );
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn arithmetic_enforces_carry_product_and_expanded_result_bounds() {
    let maximal = ExactDecimal::parse(&"9".repeat(ExactDecimal::MAX_TEXT_BYTES)).unwrap();
    let one = ExactDecimal::parse("1").unwrap();
    assert_eq!(maximal.checked_add(&one), Err(ArithmeticError::OutOfBounds));
    assert_eq!(
        maximal.checked_mul(&maximal),
        Err(ArithmeticError::OutOfBounds)
    );
    assert_eq!(maximal.checked_sub(&maximal).unwrap().canonical(), "0");

    let huge = ExactDecimal::parse("1e4095").unwrap();
    let tiny = ExactDecimal::parse("1e-4094").unwrap();
    assert_eq!(huge.checked_add(&tiny), Err(ArithmeticError::OutOfBounds));
    assert_eq!(huge.checked_sub(&tiny), Err(ArithmeticError::OutOfBounds));
    for input in ["1e3000", "1e-3000"] {
        let value = ExactDecimal::parse(input).unwrap();
        assert_eq!(value.checked_mul(&value), Err(ArithmeticError::OutOfBounds));
    }
    assert_eq!(
        ExactDecimal::parse("1e2048")
            .unwrap()
            .checked_mul(&ExactDecimal::parse("1e-2048").unwrap())
            .unwrap()
            .canonical(),
        "1"
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn directed_rounding_is_explicit_and_signed() {
    for (mode, positive, negative) in [
        (RoundingMode::TowardZero, "1.2", "-1.2"),
        (RoundingMode::AwayFromZero, "1.3", "-1.3"),
        (RoundingMode::Floor, "1.2", "-1.3"),
        (RoundingMode::Ceiling, "1.3", "-1.2"),
    ] {
        assert_eq!(
            ExactDecimal::parse("1.21")
                .unwrap()
                .quantize(1, mode)
                .unwrap()
                .canonical(),
            positive
        );
        assert_eq!(
            ExactDecimal::parse("-1.21")
                .unwrap()
                .quantize(1, mode)
                .unwrap()
                .canonical(),
            negative
        );
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn nearest_rounding_resolves_signed_ties_and_nonzero_tail_digits() {
    for (input, half_up, half_down, half_even) in [
        ("1.25", "1.3", "1.2", "1.2"),
        ("1.35", "1.4", "1.3", "1.4"),
        ("-1.25", "-1.3", "-1.2", "-1.2"),
        ("-1.35", "-1.4", "-1.3", "-1.4"),
        ("1.25001", "1.3", "1.3", "1.3"),
        ("-1.25001", "-1.3", "-1.3", "-1.3"),
        ("1.24999", "1.2", "1.2", "1.2"),
        ("0.05", "0.1", "0", "0"),
        ("-0.05", "-0.1", "0", "0"),
    ] {
        let value = ExactDecimal::parse(input).unwrap();
        for (mode, expected) in [
            (RoundingMode::HalfUp, half_up),
            (RoundingMode::HalfDown, half_down),
            (RoundingMode::HalfEven, half_even),
        ] {
            assert_eq!(value.quantize(1, mode).unwrap().canonical(), expected);
        }
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn quantization_carries_rounds_coarse_scales_and_can_require_exactness() {
    for (input, scale, mode, expected) in [
        ("9.995", 2, RoundingMode::HalfUp, "10"),
        ("-9.995", 2, RoundingMode::HalfEven, "-10"),
        ("149", -2, RoundingMode::HalfUp, "100"),
        ("150", -2, RoundingMode::HalfEven, "200"),
        ("250", -2, RoundingMode::HalfEven, "200"),
        ("-250", -2, RoundingMode::HalfEven, "-200"),
        ("1.2300", 2, RoundingMode::RejectInexact, "1.23"),
        ("1200", -2, RoundingMode::RejectInexact, "1200"),
        ("1e-4094", -4096, RoundingMode::TowardZero, "0"),
    ] {
        assert_eq!(
            ExactDecimal::parse(input)
                .unwrap()
                .quantize(scale, mode)
                .unwrap()
                .canonical(),
            expected
        );
    }
    assert_eq!(
        ExactDecimal::parse("1.2301")
            .unwrap()
            .quantize(2, RoundingMode::RejectInexact),
        Err(ArithmeticError::Inexact)
    );
    assert_eq!(
        ExactDecimal::parse("1234")
            .unwrap()
            .quantize(-2, RoundingMode::RejectInexact),
        Err(ArithmeticError::Inexact)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn quantization_checks_scale_before_expansion_even_for_zero_and_padding() {
    for input in ["0", "1", "-1", "1e-4094"] {
        let value = ExactDecimal::parse(input).unwrap();
        for scale in [i64::MIN, i64::MAX, -4097, 4097] {
            assert_eq!(
                value.quantize(scale, RoundingMode::HalfEven),
                Err(ArithmeticError::OutOfBounds)
            );
        }
        assert_eq!(
            value.quantize(4096, RoundingMode::RejectInexact).unwrap(),
            value
        );
    }
    assert_eq!(
        ExactDecimal::parse("1")
            .unwrap()
            .quantize(-4096, RoundingMode::AwayFromZero),
        Err(ArithmeticError::OutOfBounds)
    );
    let maximal = ExactDecimal::parse(&"9".repeat(ExactDecimal::MAX_TEXT_BYTES)).unwrap();
    assert_eq!(
        maximal.quantize(-1, RoundingMode::HalfUp),
        Err(ArithmeticError::OutOfBounds)
    );
}
