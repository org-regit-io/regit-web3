// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! One deadline shared by every stage and retry of a bounded operation.

use std::future::Future;

use tokio::time::{Instant, timeout_at};

use crate::{config::RpcLimits, error::Error};

pub(crate) struct OperationBudget {
    deadline: Instant,
}

impl OperationBudget {
    pub(crate) fn new(limits: RpcLimits) -> Result<Self, Error> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(Error::Configuration);
        }
        Ok(Self {
            deadline: Instant::now() + limits.request_timeout(),
        })
    }

    // The deadline covers all stages, retries and response reads. A second
    // check rejects success after synchronous decoding or construction ran late.
    pub(crate) async fn run<T, F>(&self, operation: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, Error>>,
    {
        let value = timeout_at(self.deadline, operation)
            .await
            .map_err(|_| Error::Timeout)??;
        if Instant::now() >= self.deadline {
            return Err(Error::Timeout);
        }
        Ok(value)
    }

    #[cfg(any(feature = "xrpl-http", feature = "evm-http", feature = "ton-http"))]
    pub(crate) fn check_remaining(&self) -> Result<(), Error> {
        if Instant::now() >= self.deadline {
            return Err(Error::Timeout);
        }
        Ok(())
    }
}
