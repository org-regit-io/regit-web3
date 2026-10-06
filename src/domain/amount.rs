// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use ruint::aliases::U256;
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

use crate::error::{Error, ValidationError};

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
