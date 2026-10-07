// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::time::Duration;

use crate::error::Error;

/// Positive bounded transport settings and an explicit safe-read retry count.
///
/// The request timeout bounds the entire operation, including retries, their
/// delays, and response-body consumption. The connect timeout independently
/// bounds each connection attempt within that total budget. The retry count
/// bounds additional safe-read attempts as documented by each backend. All
/// network, negotiation, verification and read stages share one request timeout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RpcLimits {
    connect_timeout: Duration,
    request_timeout: Duration,
    max_response_bytes: usize,
    max_retries: u8,
}

impl RpcLimits {
    /// Validates explicit time, response-size, and additional-attempt limits.
    ///
    /// # Errors
    ///
    /// Timeouts must be positive and at most one day, with connect timeout no
    /// larger than request timeout. Response size must be 1–16 MiB inclusive.
    /// Additional safe-read attempts must be 0–8 inclusive.
    pub fn new(
        connect_timeout: Duration,
        request_timeout: Duration,
        max_response_bytes: usize,
        max_retries: u8,
    ) -> Result<Self, Error> {
        let maximum_timeout = Duration::from_hours(24);
        if connect_timeout.is_zero()
            || request_timeout.is_zero()
            || connect_timeout > request_timeout
            || request_timeout > maximum_timeout
            || !(1..=16 * 1024 * 1024).contains(&max_response_bytes)
            || max_retries > 8
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            connect_timeout,
            request_timeout,
            max_response_bytes,
            max_retries,
        })
    }

    /// Returns the per-connection-attempt timeout.
    #[must_use]
    pub const fn connect_timeout(self) -> Duration {
        self.connect_timeout
    }

    /// Returns the total operation budget, including all retries and body reads.
    #[must_use]
    pub const fn request_timeout(self) -> Duration {
        self.request_timeout
    }

    /// Returns the maximum accepted response-body size in bytes.
    #[must_use]
    pub const fn max_response_bytes(self) -> usize {
        self.max_response_bytes
    }

    /// Returns the bound on additional safe-read attempts after an initial attempt.
    ///
    /// All stages and their retries share the operation's request timeout.
    #[must_use]
    pub const fn max_retries(self) -> u8 {
        self.max_retries
    }
}
