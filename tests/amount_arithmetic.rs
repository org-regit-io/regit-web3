// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Checked unsigned arithmetic, declared precision, and decimal conversion.

#![cfg(test)]

use regit_web3::domain::{Amount, ArithmeticError, ExactDecimal, RoundingMode, U256};
use serde_json::json;

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn checked_base_unit_arithmetic_preserves_declared_precision_and_serde() {
    let left = Amount::from_decimal("9007199254740993", Some(6)).unwrap();
    let right = Amount::from_decimal("7", Some(6)).unwrap();
    let sum = left.checked_add(right).unwrap();
    assert_eq!(sum.raw().to_string(), "9007199254741000");
    assert_eq!(sum.decimals(), Some(6));
    assert_eq!(sum.checked_sub(right).unwrap(), left);
    let product = left.checked_mul(U256::from(3)).unwrap();
    assert_eq!(product.raw().to_string(), "27021597764222979");
    assert_eq!(product.decimals(), Some(6));
    assert_eq!(
        serde_json::to_value(product).unwrap(),
        json!({"raw": "27021597764222979", "decimals": 6, "formatted": "27021597764.222979"})
    );
    assert_eq!(
        serde_json::from_value::<Amount>(serde_json::to_value(sum).unwrap()).unwrap(),
        sum
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn unsigned_overflow_underflow_and_zero_are_checked() {
    let maximal = Amount::new(U256::MAX, Some(0));
    let one = Amount::new(U256::ONE, Some(0));
    let zero = Amount::new(U256::ZERO, Some(0));
    assert_eq!(maximal.checked_add(one), Err(ArithmeticError::Overflow));
    assert_eq!(zero.checked_sub(one), Err(ArithmeticError::Underflow));
    assert_eq!(
        maximal.checked_mul(U256::from(2)),
        Err(ArithmeticError::Overflow)
    );
    assert_eq!(maximal.checked_add(zero).unwrap(), maximal);
    assert_eq!(maximal.checked_mul(U256::ONE).unwrap(), maximal);
    assert_eq!(maximal.checked_mul(U256::ZERO).unwrap(), zero);
    assert_eq!(one.checked_sub(one).unwrap(), zero);
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn arithmetic_requires_known_matching_precision_even_for_zero() {
    let unknown = Amount::new(U256::ZERO, None);
    let known = Amount::new(U256::ZERO, Some(6));
    let other = Amount::new(U256::ZERO, Some(18));
    for (left, right) in [(unknown, known), (known, unknown), (unknown, unknown)] {
        assert_eq!(
            left.checked_add(right),
            Err(ArithmeticError::UnknownPrecision)
        );
        assert_eq!(
            left.checked_sub(right),
            Err(ArithmeticError::UnknownPrecision)
        );
    }
    assert_eq!(
        known.checked_add(other),
        Err(ArithmeticError::PrecisionMismatch)
    );
    assert_eq!(
        other.checked_sub(known),
        Err(ArithmeticError::PrecisionMismatch)
    );
    assert_eq!(
        unknown.checked_mul(U256::ZERO),
        Err(ArithmeticError::UnknownPrecision)
    );
    assert_eq!(
        unknown.to_exact_decimal(),
        Err(ArithmeticError::UnknownPrecision)
    );
    assert_eq!(
        unknown.checked_rescale(0, RoundingMode::RejectInexact),
        Err(ArithmeticError::UnknownPrecision)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn decimal_conversion_roundtrips_uint256_and_all_declared_precision() {
    for decimals in 0..=u8::MAX {
        for raw in [U256::ZERO, U256::ONE, U256::from(1000), U256::MAX] {
            let amount = Amount::new(raw, Some(decimals));
            let decimal = amount.to_exact_decimal().unwrap();
            assert_eq!(
                Amount::from_exact_decimal(&decimal, decimals).unwrap(),
                amount
            );
            assert_eq!(
                ExactDecimal::parse(&amount.formatted().unwrap()).unwrap(),
                decimal
            );
        }
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn decimal_conversion_handles_trailing_zeros_and_scientific_notation_exactly() {
    for (input, decimals, expected) in [
        ("1.2300", 6, "1230000"),
        ("1.23e3", 2, "123000"),
        ("1.25e-3", 5, "125"),
        ("-0.000", 18, "0"),
        ("9007199254740993.125", 3, "9007199254740993125"),
    ] {
        let amount =
            Amount::from_exact_decimal(&ExactDecimal::parse(input).unwrap(), decimals).unwrap();
        assert_eq!(amount.raw().to_string(), expected);
        assert_eq!(amount.decimals(), Some(decimals));
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn conversion_rejects_negative_and_inexact_values_without_an_implicit_rule() {
    let negative = ExactDecimal::parse("-0.0001").unwrap();
    assert_eq!(
        Amount::from_exact_decimal(&negative, 0),
        Err(ArithmeticError::NegativeAmount)
    );
    assert_eq!(
        Amount::from_exact_decimal_rounded(&negative, 0, RoundingMode::TowardZero),
        Err(ArithmeticError::NegativeAmount)
    );
    for input in ["1.001", "1e-4094"] {
        let value = ExactDecimal::parse(input).unwrap();
        assert_eq!(
            Amount::from_exact_decimal(&value, 2),
            Err(ArithmeticError::Inexact)
        );
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn conversion_rounding_has_explicit_ties_carry_and_tiny_value_behavior() {
    for (input, decimals, mode, expected) in [
        ("1.225", 2, RoundingMode::HalfEven, "122"),
        ("1.235", 2, RoundingMode::HalfEven, "124"),
        ("1.225", 2, RoundingMode::HalfUp, "123"),
        ("1.225", 2, RoundingMode::HalfDown, "122"),
        ("9.995", 2, RoundingMode::HalfUp, "1000"),
        ("1e-4094", 2, RoundingMode::TowardZero, "0"),
        ("1e-4094", 2, RoundingMode::AwayFromZero, "1"),
    ] {
        let amount = Amount::from_exact_decimal_rounded(
            &ExactDecimal::parse(input).unwrap(),
            decimals,
            mode,
        )
        .unwrap();
        assert_eq!(amount.raw().to_string(), expected);
        assert_eq!(amount.decimals(), Some(decimals));
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn conversion_checks_range_before_expanding_base_units_and_after_rounding() {
    let maximal = ExactDecimal::parse(&U256::MAX.to_string()).unwrap();
    assert_eq!(
        Amount::from_exact_decimal(&maximal, 0).unwrap().raw(),
        U256::MAX
    );
    for input in [
        "115792089237316195423570985008687907853269984665640564039457584007913129639936",
        "1e4095",
    ] {
        assert_eq!(
            Amount::from_exact_decimal(&ExactDecimal::parse(input).unwrap(), 0),
            Err(ArithmeticError::Overflow)
        );
    }
    assert_eq!(
        Amount::from_exact_decimal(&maximal, 1),
        Err(ArithmeticError::Overflow)
    );
    assert_eq!(
        Amount::from_exact_decimal(&ExactDecimal::parse("1").unwrap(), u8::MAX),
        Err(ArithmeticError::Overflow)
    );

    let near_maximal = ExactDecimal::parse(&format!("{}.5", U256::MAX)).unwrap();
    assert_eq!(
        Amount::from_exact_decimal_rounded(&near_maximal, 0, RoundingMode::TowardZero)
            .unwrap()
            .raw(),
        U256::MAX
    );
    assert_eq!(
        Amount::from_exact_decimal_rounded(&near_maximal, 0, RoundingMode::HalfUp),
        Err(ArithmeticError::Overflow)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn rescaling_changes_base_units_only_at_explicit_precision_and_rounding() {
    let amount = Amount::from_decimal("12345", Some(4)).unwrap();
    let increased = amount
        .checked_rescale(6, RoundingMode::RejectInexact)
        .unwrap();
    assert_eq!(increased.raw().to_string(), "1234500");
    assert_eq!(increased.decimals(), Some(6));
    assert_eq!(increased.to_exact_decimal(), amount.to_exact_decimal());
    assert_eq!(
        increased
            .checked_rescale(4, RoundingMode::RejectInexact)
            .unwrap(),
        amount
    );
    assert_eq!(
        amount.checked_rescale(2, RoundingMode::RejectInexact),
        Err(ArithmeticError::Inexact)
    );
    assert_eq!(
        amount
            .checked_rescale(2, RoundingMode::HalfUp)
            .unwrap()
            .raw()
            .to_string(),
        "123"
    );
    assert_eq!(
        amount
            .checked_rescale(2, RoundingMode::AwayFromZero)
            .unwrap()
            .raw()
            .to_string(),
        "124"
    );
    assert_eq!(
        Amount::new(U256::MAX, Some(0)).checked_rescale(1, RoundingMode::RejectInexact),
        Err(ArithmeticError::Overflow)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn arithmetic_errors_serialize_fixed_tags_without_operand_payloads() {
    for (error, tag) in [
        (ArithmeticError::OutOfBounds, "out_of_bounds"),
        (ArithmeticError::Overflow, "overflow"),
        (ArithmeticError::Underflow, "underflow"),
        (ArithmeticError::UnknownPrecision, "unknown_precision"),
        (ArithmeticError::PrecisionMismatch, "precision_mismatch"),
        (ArithmeticError::Inexact, "inexact"),
        (ArithmeticError::NegativeAmount, "negative_amount"),
    ] {
        assert_eq!(serde_json::to_value(error).unwrap(), json!(tag));
        assert_eq!(
            serde_json::from_value::<ArithmeticError>(json!(tag)).unwrap(),
            error
        );
    }
    for wire in [
        json!("unknown"),
        json!("Overflow"),
        json!(null),
        json!(1),
        json!({"inexact": {"operand": "arbitrary caller text"}}),
        json!({"overflow": "arbitrary caller text"}),
    ] {
        assert!(serde_json::from_value::<ArithmeticError>(wire).is_err());
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn explicit_rounding_modes_have_stable_snake_case_wire_tags() {
    for (mode, tag) in [
        (RoundingMode::RejectInexact, "reject_inexact"),
        (RoundingMode::TowardZero, "toward_zero"),
        (RoundingMode::AwayFromZero, "away_from_zero"),
        (RoundingMode::Floor, "floor"),
        (RoundingMode::Ceiling, "ceiling"),
        (RoundingMode::HalfUp, "half_up"),
        (RoundingMode::HalfDown, "half_down"),
        (RoundingMode::HalfEven, "half_even"),
    ] {
        assert_eq!(serde_json::to_value(mode).unwrap(), json!(tag));
        assert_eq!(
            serde_json::from_value::<RoundingMode>(json!(tag)).unwrap(),
            mode
        );
    }
    for wire in [
        json!("unknown"),
        json!("halfEven"),
        json!(null),
        json!(0),
        json!({"half_even": {"scale": 2}}),
    ] {
        assert!(serde_json::from_value::<RoundingMode>(wire).is_err());
    }
}
