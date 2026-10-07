// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use ruint::aliases::U256;
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

use crate::error::{Error, ValidationError};

use super::{ArithmeticError, ExactDecimal, RoundingMode};

/// An exact nonnegative base-unit amount with optional decimal precision.
///
/// The raw value is an unsigned 256-bit integer. Decimal precision does not
/// change the raw value. Formatting preserves every declared fractional digit
/// and returns `None` when precision is unknown. No floating point is used.
///
/// Serialized fields are `raw` (canonical decimal string), `decimals` (integer
/// or null), and `formatted` (exact derived string or null). Deserialization
/// verifies the derived field instead of trusting it.
///
/// ```
/// use regit_web3::domain::Amount;
///
/// # fn main() -> Result<(), regit_web3::error::Error> {
/// let amount = Amount::from_decimal("9007199254740993", Some(6))?;
/// assert_eq!(amount.raw().to_string(), "9007199254740993");
/// assert_eq!(amount.formatted().as_deref(), Some("9007199254.740993"));
/// assert_eq!(Amount::from_decimal("1", None)?.formatted(), None);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(try_from = "AmountFields")]
pub struct Amount {
    raw: U256,
    decimals: Option<u8>,
}

impl Amount {
    /// Constructs an amount from an exact unsigned integer.
    #[must_use]
    pub const fn new(raw: U256, decimals: Option<u8>) -> Self {
        Self { raw, decimals }
    }

    /// Parses canonical unsigned decimal base units and attaches precision.
    ///
    /// # Errors
    ///
    /// Rejects signs, whitespace, leading zeros except `0`, nondecimal notation,
    /// and values outside the unsigned 256-bit range. Errors never echo input.
    pub fn from_decimal(raw: &str, decimals: Option<u8>) -> Result<Self, Error> {
        Ok(Self::new(parse_uint(raw)?, decimals))
    }

    /// Returns the exact base-unit integer.
    #[must_use]
    pub const fn raw(self) -> U256 {
        self.raw
    }

    /// Returns known decimal precision, or `None` without an assumed value.
    #[must_use]
    pub const fn decimals(self) -> Option<u8> {
        self.decimals
    }

    /// Returns an exact fixed-precision string, or `None` if precision is unknown.
    ///
    /// Zero decimals produces an integer string. Positive decimals preserve all
    /// fractional digits, including trailing zeros.
    #[must_use]
    pub fn formatted(self) -> Option<String> {
        let decimals = usize::from(self.decimals?);
        let digits = self.raw.to_string();
        if decimals == 0 {
            return Some(digits);
        }
        if digits.len() > decimals {
            let split = digits.len() - decimals;
            return Some(format!("{}.{}", &digits[..split], &digits[split..]));
        }
        Some(format!("0.{}{digits}", "0".repeat(decimals - digits.len())))
    }

    /// Adds base units while retaining explicitly known equal precision.
    ///
    /// This numeric operation does not establish that the amounts identify the
    /// same asset. Callers must establish asset identity separately.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::UnknownPrecision`] for either unknown precision,
    /// [`ArithmeticError::PrecisionMismatch`] for different declared precision,
    /// or [`ArithmeticError::Overflow`] for an unsigned 256-bit overflow.
    pub fn checked_add(self, other: Self) -> Result<Self, ArithmeticError> {
        let decimals = self.shared_precision(other)?;
        let raw = self
            .raw
            .checked_add(other.raw)
            .ok_or(ArithmeticError::Overflow)?;
        Ok(Self::new(raw, Some(decimals)))
    }

    /// Subtracts base units while retaining explicitly known equal precision.
    ///
    /// This numeric operation does not establish asset identity.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::UnknownPrecision`] for either unknown precision,
    /// [`ArithmeticError::PrecisionMismatch`] for different declared precision,
    /// or [`ArithmeticError::Underflow`] for a negative unsigned result.
    pub fn checked_sub(self, other: Self) -> Result<Self, ArithmeticError> {
        let decimals = self.shared_precision(other)?;
        let raw = self
            .raw
            .checked_sub(other.raw)
            .ok_or(ArithmeticError::Underflow)?;
        Ok(Self::new(raw, Some(decimals)))
    }

    /// Multiplies base units by a dimensionless unsigned integer scalar.
    ///
    /// The declared precision is preserved. This does not multiply two asset
    /// amounts or infer the units of an exchange rate.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::UnknownPrecision`] without declared precision
    /// or [`ArithmeticError::Overflow`] for an unsigned 256-bit overflow.
    pub fn checked_mul(self, scalar: U256) -> Result<Self, ArithmeticError> {
        let decimals = self.decimals.ok_or(ArithmeticError::UnknownPrecision)?;
        let raw = self
            .raw
            .checked_mul(scalar)
            .ok_or(ArithmeticError::Overflow)?;
        Ok(Self::new(raw, Some(decimals)))
    }

    /// Converts base units to their exact decimal numeric value.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::UnknownPrecision`] without declared precision.
    /// The bounded uint256 value and u8 precision fit the decimal resource limits.
    pub fn to_exact_decimal(self) -> Result<ExactDecimal, ArithmeticError> {
        ExactDecimal::from_amount(self)
    }

    /// Converts an exact nonnegative decimal to base units at explicit precision.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::NegativeAmount`] for negative values,
    /// [`ArithmeticError::Inexact`] for fractional base units, or
    /// [`ArithmeticError::Overflow`] for an unsigned 256-bit overflow.
    pub fn from_exact_decimal(value: &ExactDecimal, decimals: u8) -> Result<Self, ArithmeticError> {
        Self::from_exact_decimal_rounded(value, decimals, RoundingMode::RejectInexact)
    }

    /// Converts a decimal to base units with explicit precision and rounding.
    ///
    /// Negative inputs are rejected before rounding, including values that would
    /// otherwise round to zero. No rounding rule or precision is inferred.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::NegativeAmount`] for negative inputs,
    /// [`ArithmeticError::Inexact`] when the selected rule rejects rounding, or
    /// [`ArithmeticError::Overflow`] for an unsigned 256-bit overflow.
    pub fn from_exact_decimal_rounded(
        value: &ExactDecimal,
        decimals: u8,
        rounding: RoundingMode,
    ) -> Result<Self, ArithmeticError> {
        Ok(Self::new(
            value.to_base_units(decimals, rounding)?,
            Some(decimals),
        ))
    }

    /// Changes declared precision while preserving the numeric value or rounding
    /// according to the explicitly supplied rule.
    ///
    /// The raw base-unit integer changes with precision. Use
    /// [`RoundingMode::RejectInexact`] to require exact preservation.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::UnknownPrecision`] without declared precision,
    /// [`ArithmeticError::Inexact`] when the selected rule rejects rounding, or
    /// [`ArithmeticError::Overflow`] if the new base units exceed uint256.
    pub fn checked_rescale(
        self,
        decimals: u8,
        rounding: RoundingMode,
    ) -> Result<Self, ArithmeticError> {
        Self::from_exact_decimal_rounded(&self.to_exact_decimal()?, decimals, rounding)
    }

    fn shared_precision(self, other: Self) -> Result<u8, ArithmeticError> {
        let decimals = self.decimals.ok_or(ArithmeticError::UnknownPrecision)?;
        let other_decimals = other.decimals.ok_or(ArithmeticError::UnknownPrecision)?;
        if decimals != other_decimals {
            return Err(ArithmeticError::PrecisionMismatch);
        }
        Ok(decimals)
    }
}

impl Serialize for Amount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut fields = serializer.serialize_struct("Amount", 3)?;
        fields.serialize_field("raw", &self.raw.to_string())?;
        fields.serialize_field("decimals", &self.decimals)?;
        fields.serialize_field("formatted", &self.formatted())?;
        fields.end()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AmountFields {
    raw: String,
    #[serde(deserialize_with = "super::deserialize_optional")]
    decimals: Option<u8>,
    #[serde(deserialize_with = "super::deserialize_optional")]
    formatted: Option<String>,
}

impl TryFrom<AmountFields> for Amount {
    type Error = Error;

    fn try_from(fields: AmountFields) -> Result<Self, Self::Error> {
        let amount = Self::from_decimal(&fields.raw, fields.decimals)?;
        if amount.formatted() != fields.formatted {
            return Err(ValidationError::InvalidFormattedAmount.into());
        }
        Ok(amount)
    }
}

pub(super) fn parse_uint(raw: &str) -> Result<U256, Error> {
    if raw.is_empty()
        || !raw.bytes().all(|byte| byte.is_ascii_digit())
        || (raw.len() > 1 && raw.starts_with('0'))
    {
        return Err(ValidationError::InvalidAmount.into());
    }
    // A uint256 has at most 78 decimal digits; reject before parsing an
    // arbitrarily large string. The maintained integer checks the final bound.
    if raw.len() > 78 {
        return Err(ValidationError::AmountOverflow.into());
    }
    U256::from_str_radix(raw, 10).map_err(|_| ValidationError::AmountOverflow.into())
}
