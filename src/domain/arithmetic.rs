// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use serde::{Deserialize, Serialize};

/// A fixed diagnostic for checked exact arithmetic and base-unit conversion.
///
/// Arithmetic errors never retain operands or caller-supplied text. Existing
/// parsing and wire validation continue to use [`crate::error::Error`].
/// Serialization is a stable `snake_case` string without operand fields.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ArithmeticError {
    /// The operation or result exceeds the decimal resource bounds.
    OutOfBounds,
    /// An unsigned result exceeds the 256-bit base-unit range.
    Overflow,
    /// Unsigned subtraction would produce a negative result.
    Underflow,
    /// Arithmetic or conversion requires explicitly known amount precision.
    UnknownPrecision,
    /// The two amounts declare different decimal precision.
    PrecisionMismatch,
    /// The requested precision cannot represent the value exactly.
    Inexact,
    /// A negative decimal cannot represent an unsigned amount.
    NegativeAmount,
}

impl fmt::Display for ArithmeticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OutOfBounds => "exact arithmetic exceeds resource bounds",
            Self::Overflow => "unsigned amount overflow",
            Self::Underflow => "unsigned amount underflow",
            Self::UnknownPrecision => "amount precision is unknown",
            Self::PrecisionMismatch => "amount precision differs",
            Self::Inexact => "value is inexact at the requested precision",
            Self::NegativeAmount => "negative value cannot represent an unsigned amount",
        })
    }
}

impl std::error::Error for ArithmeticError {}

/// An explicit rule for discarding fractional digits during quantization.
///
/// No mode is selected implicitly. Half modes round to the nearest value;
/// their names describe how an exact tie is resolved. Rounding is numeric and
/// does not infer an asset's precision or apply a price policy.
/// Serialization is a stable `snake_case` string.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RoundingMode {
    /// Reject any discarded nonzero digits.
    RejectInexact,
    /// Discard digits toward zero.
    TowardZero,
    /// Discard digits away from zero whenever any discarded digit is nonzero.
    AwayFromZero,
    /// Round toward negative infinity.
    Floor,
    /// Round toward positive infinity.
    Ceiling,
    /// Round to nearest, resolving an exact tie away from zero.
    HalfUp,
    /// Round to nearest, resolving an exact tie toward zero.
    HalfDown,
    /// Round to nearest, resolving an exact tie to an even retained digit.
    HalfEven,
}

impl RoundingMode {
    pub(super) const fn decimal_mode(self) -> Option<bigdecimal::RoundingMode> {
        match self {
            Self::RejectInexact => None,
            Self::TowardZero => Some(bigdecimal::RoundingMode::Down),
            Self::AwayFromZero => Some(bigdecimal::RoundingMode::Up),
            Self::Floor => Some(bigdecimal::RoundingMode::Floor),
            Self::Ceiling => Some(bigdecimal::RoundingMode::Ceiling),
            Self::HalfUp => Some(bigdecimal::RoundingMode::HalfUp),
            Self::HalfDown => Some(bigdecimal::RoundingMode::HalfDown),
            Self::HalfEven => Some(bigdecimal::RoundingMode::HalfEven),
        }
    }
}
