// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Typed failure categories and secret-safe domain diagnostics.

#![cfg(test)]
use regit_web3::domain::{
    Address, Amount, Asset, BlockHash, ChainId, ExactDecimal, NetworkId, Source,
};
use regit_web3::error::{Error, ProviderError, ValidationError};
use serde_json::json;

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn error_categories_and_reasons_roundtrip_as_fixed_structured_values() {
    for (error, expected) in [
        (Error::Configuration, json!({"category": "configuration"})),
        (
            Error::Validation(ValidationError::InvalidAmount),
            json!({"category": "validation", "reason": "invalid_amount"}),
        ),
        (
            Error::Validation(ValidationError::InvalidDecimal),
            json!({"category": "validation", "reason": "invalid_decimal"}),
        ),
        (
            Error::Validation(ValidationError::DecimalOutOfBounds),
            json!({"category": "validation", "reason": "decimal_out_of_bounds"}),
        ),
        (
            Error::UnsupportedCapability,
            json!({"category": "unsupported_capability"}),
        ),
        (Error::Timeout, json!({"category": "timeout"})),
        (
            Error::Provider(ProviderError::Rpc),
            json!({"category": "provider", "reason": "rpc"}),
        ),
        (
            Error::Provider(ProviderError::InvalidResponse),
            json!({"category": "provider", "reason": "invalid_response"}),
        ),
        (
            Error::Provider(ProviderError::RateLimited),
            json!({"category": "provider", "reason": "rate_limited"}),
        ),
        (
            Error::UnavailableData,
            json!({"category": "unavailable_data"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(error).unwrap(), expected);
        assert_eq!(json!(error.code()), expected["category"]);
        assert_eq!(serde_json::from_value::<Error>(expected).unwrap(), error);
        assert!(!error.to_string().is_empty());
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn validation_errors_never_retain_credential_bearing_bad_input() {
    let bad_input = "https://fixture-user:fixture-password@example.invalid/?token=fixture-token";
    let chain = ChainId::from_decimal("1").unwrap();
    let valid_network = NetworkId::new(chain, "mainnet").unwrap();
    let errors = [
        Amount::from_decimal(bad_input, None).unwrap_err(),
        ExactDecimal::parse(bad_input).unwrap_err(),
        ExactDecimal::parse(&format!(
            "{bad_input}{}",
            "0".repeat(ExactDecimal::MAX_TEXT_BYTES)
        ))
        .unwrap_err(),
        ChainId::from_decimal(bad_input).unwrap_err(),
        Address::parse(bad_input).unwrap_err(),
        BlockHash::parse(bad_input).unwrap_err(),
        NetworkId::new(chain, bad_input).unwrap_err(),
        Asset::native(valid_network, 18, Some(bad_input.to_owned())).unwrap_err(),
        Source::new(bad_input, "eth_getBalance", "0.1.0").unwrap_err(),
        Source::new("fixture", bad_input, "0.1.0").unwrap_err(),
        Source::new("fixture", "eth_getBalance", bad_input).unwrap_err(),
    ];
    for error in errors {
        for diagnostic in [
            error.to_string(),
            format!("{error:?}"),
            serde_json::to_string(&error).unwrap(),
        ] {
            for sensitive in [
                bad_input,
                "fixture-user",
                "fixture-password",
                "fixture-token",
            ] {
                assert!(!diagnostic.contains(sensitive));
            }
        }
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn structured_errors_cannot_import_arbitrary_provider_or_validation_messages() {
    let untrusted = "Authorization: Bearer fixture-token";
    for category in ["provider", "validation"] {
        assert!(
            serde_json::from_value::<Error>(json!({"category": category, "reason": untrusted}))
                .is_err()
        );
    }
    assert!(serde_json::from_value::<Error>(json!({"category": untrusted})).is_err());
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn overflow_and_malformed_amounts_have_distinct_typed_failures() {
    assert_eq!(
        Amount::from_decimal("-1", None).unwrap_err(),
        Error::Validation(ValidationError::InvalidAmount)
    );
    assert_eq!(
        Amount::from_decimal(
            "115792089237316195423570985008687907853269984665640564039457584007913129639936",
            None,
        )
        .unwrap_err(),
        Error::Validation(ValidationError::AmountOverflow)
    );
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn solana_validation_reasons_have_fixed_distinct_serialized_diagnostics() {
    for (reason, expected) in [
        (
            ValidationError::InvalidSolanaPubkey,
            "invalid_solana_pubkey",
        ),
        (ValidationError::InvalidSolanaHash, "invalid_solana_hash"),
        (
            ValidationError::InvalidSolanaSignature,
            "invalid_solana_signature",
        ),
        (
            ValidationError::SolanaAmountOverflow,
            "solana_amount_overflow",
        ),
        (
            ValidationError::ObservationOperationMismatch,
            "observation_operation_mismatch",
        ),
        (
            ValidationError::ContextSlotBelowMinimum,
            "context_slot_below_minimum",
        ),
        (
            ValidationError::InvalidSolanaAccount,
            "invalid_solana_account",
        ),
        (
            ValidationError::SolanaAccountDataTooLarge,
            "solana_account_data_too_large",
        ),
    ] {
        let error = Error::Validation(reason);
        let expected = json!({"category":"validation","reason":expected});
        assert_eq!(serde_json::to_value(error).unwrap(), expected);
        assert_eq!(serde_json::from_value::<Error>(expected).unwrap(), error);
        assert!(!error.to_string().is_empty());
    }
}
