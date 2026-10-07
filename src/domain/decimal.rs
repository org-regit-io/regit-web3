// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{fmt, str::FromStr};

use bigdecimal::{BigDecimal, num_bigint::Sign};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};

use crate::error::{Error, ValidationError};

use super::{Amount, ArithmeticError, RoundingMode, U256};

/// A finite signed decimal value retained exactly, without floating point.
///
/// Parsing accepts the JSON-number grammar: an optional minus sign, an integer
/// without leading zeros except `0`, an optional nonempty fractional part, and
/// an optional `e`/`E` exponent with an optional sign. Whitespace, separators,
/// nonfinite values, and omitted integer or fractional digits are rejected.
///
/// Serialization is always a string in plain decimal notation, with no leading
/// plus sign, exponent, redundant integer zeros, or fractional trailing zeros.
/// Every zero representation becomes `"0"`. Precision is retained exactly up to
/// the explicit resource bounds; parsing never rounds or infers asset decimals.
/// This is a numeric value, not a declaration of transaction base units.
///
/// Input and canonical output are each limited to [`Self::MAX_TEXT_BYTES`]. The
/// exponent and effective/normalized scale magnitudes are separately bounded.
/// These checks precede expanded formatting, and serialized values remain valid
/// inputs under the same limits. Checked arithmetic retains these invariants,
/// bounds intermediate coefficient work before expansion, and never rounds.
/// Quantization requires an explicit caller-selected rounding rule.
///
/// ```
/// use regit_web3::domain::ExactDecimal;
///
/// # fn main() -> Result<(), regit_web3::error::Error> {
/// let value = ExactDecimal::parse("-9007199254740993.12500")?;
/// assert_eq!(value.to_string(), "-9007199254740993.125");
/// assert_eq!(ExactDecimal::parse("1.25e-3")?.canonical(), "0.00125");
/// assert_eq!(ExactDecimal::parse("-0.00")?.canonical(), "0");
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExactDecimal(BigDecimal);

impl ExactDecimal {
    // Addition can align two opposite scale extremes, and multiplication can
    // combine two maximal coefficients. Include one digit for an addition carry.
    const MAX_ARITHMETIC_DIGITS: u64 = 2 * Self::MAX_TEXT_BYTES as u64 + 1;

    /// Maximum input or canonical-output length in bytes, including punctuation.
    pub const MAX_TEXT_BYTES: usize = 4096;

    /// Maximum absolute exponent supplied in scientific notation.
    pub const MAX_EXPONENT_MAGNITUDE: i64 = 4096;

    /// Maximum absolute effective or normalized decimal scale.
    pub const MAX_SCALE_MAGNITUDE: i64 = 4096;

    /// Parses a finite signed decimal or scientific-notation value exactly.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidDecimal`] for malformed notation and
    /// [`ValidationError::DecimalOutOfBounds`] for exceeded resource limits.
    /// Diagnostics never retain or echo the supplied text.
    pub fn parse(input: &str) -> Result<Self, Error> {
        Self::validate_input(input)?;
        let value = BigDecimal::from_str(input)
            .map_err(|_| Error::from(ValidationError::InvalidDecimal))?
            .normalized();
        let scale = value.fractional_digit_count();
        if !(-Self::MAX_SCALE_MAGNITUDE..=Self::MAX_SCALE_MAGNITUDE).contains(&scale) {
            return Err(ValidationError::DecimalOutOfBounds.into());
        }
        let decimal = Self(value);
        if decimal.canonical_length() > Self::MAX_TEXT_BYTES as u64 {
            return Err(ValidationError::DecimalOutOfBounds.into());
        }
        Ok(decimal)
    }

    /// Returns the exact normalized value in plain decimal notation.
    #[must_use]
    pub fn canonical(&self) -> String {
        self.0.to_plain_string()
    }

    /// Reports whether this value is zero; negative zero is normalized to zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.sign() == Sign::NoSign
    }

    /// Reports whether the exact value is negative.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0.sign() == Sign::Minus
    }

    /// Reports whether the exact value has no fractional part.
    ///
    /// This does not attach asset precision or identify transaction base units.
    #[must_use]
    pub fn is_integer(&self) -> bool {
        self.0.fractional_digit_count() <= 0
    }

    /// Adds two decimal values exactly, with no rounding.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::OutOfBounds`] if intermediate coefficient work
    /// or the normalized result exceeds the decimal resource bounds.
    pub fn checked_add(&self, other: &Self) -> Result<Self, ArithmeticError> {
        self.check_alignment(other)?;
        Self::from_arithmetic(&(&self.0 + &other.0))
    }

    /// Subtracts a decimal value exactly, including signed results.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::OutOfBounds`] if intermediate coefficient work
    /// or the normalized result exceeds the decimal resource bounds.
    pub fn checked_sub(&self, other: &Self) -> Result<Self, ArithmeticError> {
        self.check_alignment(other)?;
        Self::from_arithmetic(&(&self.0 - &other.0))
    }

    /// Multiplies two decimal values exactly, retaining every result digit.
    ///
    /// Intermediate coefficients are limited to twice the input text bound plus
    /// one carry digit. Multiplication never uses a rounded decimal context.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::OutOfBounds`] if intermediate coefficient work
    /// or the normalized result exceeds the decimal resource bounds.
    pub fn checked_mul(&self, other: &Self) -> Result<Self, ArithmeticError> {
        Self::check_work_digits(self.0.digits() + other.0.digits())?;
        Self::from_arithmetic(&(&self.0 * &other.0))
    }

    /// Quantizes to a decimal scale using an explicit rounding rule.
    ///
    /// A scale of `2` means hundredths, `0` means integers, and `-2` means
    /// hundreds. The result remains a normalized numeric value: this method does
    /// not retain display padding or declare asset precision. Increasing the
    /// scale returns the same value without allocating fractional zeros.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::OutOfBounds`] when the requested scale magnitude
    /// or normalized result exceeds this type's bounds. Returns
    /// [`ArithmeticError::Inexact`] when [`RoundingMode::RejectInexact`] would
    /// discard nonzero digits. Bounds are checked even when the value is zero.
    pub fn quantize(&self, scale: i64, rounding: RoundingMode) -> Result<Self, ArithmeticError> {
        if !(-Self::MAX_SCALE_MAGNITUDE..=Self::MAX_SCALE_MAGNITUDE).contains(&scale) {
            return Err(ArithmeticError::OutOfBounds);
        }
        if self.is_zero() || scale >= self.0.fractional_digit_count() {
            return Ok(self.clone());
        }
        // A normalized nonzero coefficient has no trailing zeros: lowering its
        // scale necessarily discards a nonzero digit. Reject before any work.
        let mode = rounding.decimal_mode().ok_or(ArithmeticError::Inexact)?;
        Self::check_work_digits(self.0.digits() + 1)?;
        Self::from_arithmetic(&self.0.with_scale_round(scale, mode))
    }

    pub(super) fn to_base_units(
        &self,
        decimals: u8,
        rounding: RoundingMode,
    ) -> Result<U256, ArithmeticError> {
        if self.is_negative() {
            return Err(ArithmeticError::NegativeAmount);
        }
        let scale = i64::from(decimals);
        let rounded = self.quantize(scale, rounding)?;
        if rounded.is_zero() {
            return Ok(U256::ZERO);
        }
        let expansion = (scale - rounded.0.fractional_digit_count()).unsigned_abs();
        // Check the uint256 text bound before scaling an integer coefficient.
        if rounded.0.digits() + expansion > 78 {
            return Err(ArithmeticError::Overflow);
        }
        let (coefficient, _) = rounded.0.with_scale(scale).into_bigint_and_exponent();
        U256::from_str_radix(&coefficient.to_str_radix(10), 10)
            .map_err(|_| ArithmeticError::Overflow)
    }

    pub(super) fn from_amount(amount: Amount) -> Result<Self, ArithmeticError> {
        // A uint256 and u8 precision expand to at most 257 ASCII bytes.
        let formatted = amount
            .formatted()
            .ok_or(ArithmeticError::UnknownPrecision)?;
        Self::parse(&formatted).map_err(|_| ArithmeticError::OutOfBounds)
    }

    fn check_alignment(&self, other: &Self) -> Result<(), ArithmeticError> {
        let scale = self
            .0
            .fractional_digit_count()
            .max(other.0.fractional_digit_count());
        for value in [self, other] {
            let expansion = (scale - value.0.fractional_digit_count()).unsigned_abs();
            Self::check_work_digits(value.0.digits() + expansion + 1)?;
        }
        Ok(())
    }

    fn check_work_digits(digits: u64) -> Result<(), ArithmeticError> {
        if digits > Self::MAX_ARITHMETIC_DIGITS {
            return Err(ArithmeticError::OutOfBounds);
        }
        Ok(())
    }

    fn from_arithmetic(value: &BigDecimal) -> Result<Self, ArithmeticError> {
        let decimal = Self(value.normalized());
        let scale = decimal.0.fractional_digit_count();
        if !(-Self::MAX_SCALE_MAGNITUDE..=Self::MAX_SCALE_MAGNITUDE).contains(&scale)
            || decimal.canonical_length() > Self::MAX_TEXT_BYTES as u64
        {
            return Err(ArithmeticError::OutOfBounds);
        }
        Ok(decimal)
    }

    fn validate_input(input: &str) -> Result<(), Error> {
        if input.len() > Self::MAX_TEXT_BYTES {
            return Err(ValidationError::DecimalOutOfBounds.into());
        }
        let unsigned = input.strip_prefix('-').unwrap_or(input);
        let (mantissa, exponent) = if let Some(position) = unsigned.find(['e', 'E']) {
            let (mantissa, exponent_text) = unsigned.split_at(position);
            let exponent_text = &exponent_text[1..];
            let digits = exponent_text
                .strip_prefix(['-', '+'])
                .unwrap_or(exponent_text);
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(ValidationError::InvalidDecimal.into());
            }
            let exponent = exponent_text
                .parse::<i64>()
                .map_err(|_| Error::from(ValidationError::DecimalOutOfBounds))?;
            if !(-Self::MAX_EXPONENT_MAGNITUDE..=Self::MAX_EXPONENT_MAGNITUDE).contains(&exponent) {
                return Err(ValidationError::DecimalOutOfBounds.into());
            }
            (mantissa, exponent)
        } else {
            (unsigned, 0)
        };
        let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if integer.is_empty()
            || !integer.bytes().all(|byte| byte.is_ascii_digit())
            || (integer.len() > 1 && integer.starts_with('0'))
            || (mantissa.contains('.') && fraction.is_empty())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ValidationError::InvalidDecimal.into());
        }
        let fractional_digits = i64::try_from(fraction.len())
            .map_err(|_| Error::from(ValidationError::DecimalOutOfBounds))?;
        let scale = fractional_digits - exponent;
        if !(-Self::MAX_SCALE_MAGNITUDE..=Self::MAX_SCALE_MAGNITUDE).contains(&scale) {
            return Err(ValidationError::DecimalOutOfBounds.into());
        }
        Ok(())
    }

    fn canonical_length(&self) -> u64 {
        let digits = self.0.digits();
        let scale = self.0.fractional_digit_count();
        let length = if scale <= 0 {
            digits + scale.unsigned_abs()
        } else if digits > scale.unsigned_abs() {
            digits + 1
        } else {
            scale.unsigned_abs() + 2
        };
        length + u64::from(self.is_negative())
    }
}

impl FromStr for ExactDecimal {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

impl fmt::Display for ExactDecimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.canonical())
    }
}

impl fmt::Debug for ExactDecimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ExactDecimal")
            .field(&self.canonical())
            .finish()
    }
}

impl Serialize for ExactDecimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ExactDecimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DecimalVisitor;

        impl Visitor<'_> for DecimalVisitor {
            type Value = ExactDecimal;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an exact decimal string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ExactDecimal::parse(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(DecimalVisitor)
    }
}
