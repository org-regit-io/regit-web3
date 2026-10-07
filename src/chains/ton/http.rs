// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{TonReader, TonSubmitter, wire};
use crate::{
    config::{HttpConfig, RpcLimits},
    domain::{
        Source, Timestamp,
        ton::{
            AccountBalance, Address, Block, Context, Cursor, FeeEstimate, FeeRequest, Hash,
            HistoryPage, HistoryRequest, MessageStatus, Network, NetworkData, Observation,
            ObservationValue, Operation, SignedSubmission, SubmissionResult, Transaction,
            TransactionStatus,
        },
    },
    error::{Error, ProviderError, ValidationError},
    transport::{HttpClient, OperationBudget, submission_unknown},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fmt,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, sleep, sleep_until};

/// Caller-supplied expected TON zero-state and optional outgoing API-v2 configuration.
/// The endpoint includes `/api/v2`; optional API keys are explicit sensitive
/// endpoint headers. No endpoint, key, runtime or environment is discovered.
#[derive(Clone, Debug)]
pub struct TonHttpConfig {
    network: Network,
    http: HttpConfig,
    request_spacing: Duration,
}
impl TonHttpConfig {
    /// Enforces the two-MiB response ceiling without selecting public endpoints.
    ///
    /// # Errors
    /// Rejects larger HTTP response limits.
    pub fn new(network: Network, http: HttpConfig) -> Result<Self, Error> {
        if http.limits().max_response_bytes() > wire::MAX_BODY {
            return Err(Error::Configuration);
        }
        Ok(Self {
            network,
            http,
            request_spacing: Duration::ZERO,
        })
    }
    /// Selects a caller-supplied interval between scheduled HTTP attempts on this client.
    /// Zero disables scheduling delay. Network preflights, retries and one-shot
    /// writes share this scheduling state; waits consume the operation deadline.
    /// This is client-local scheduling, not a provider-wide rate-limit guarantee.
    /// Cancelled reservations can conservatively leave an unused time slot.
    ///
    /// # Errors
    /// Rejects intervals above 60 seconds.
    pub fn with_request_spacing(mut self, spacing: Duration) -> Result<Self, Error> {
        if spacing > Duration::from_secs(60) {
            return Err(Error::Configuration);
        }
        self.request_spacing = spacing;
        Ok(self)
    }
    /// Returns the explicit minimum interval between scheduled attempt slots.
    #[must_use]
    pub const fn request_spacing(&self) -> Duration {
        self.request_spacing
    }
    /// Returns exact expected source zero-state network identity.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
    /// Returns explicit bounded transport settings with redacted endpoint Debug.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}
/// Optional TON Center-compatible API-v2 backend with full zero-state verification.
/// The caller supplies Tokio with I/O/time. Each operation has one total budget;
/// safe reads/estimates retry identical requests. Account reads freeze the selected
/// masterchain sequence across retries and verify the full returned block ID.
/// This is source correlation, not an independently checked consensus proof.
pub struct TonClient {
    config: TonHttpConfig,
    http: HttpClient,
    next_attempt: Mutex<Option<Instant>>,
}
impl TonClient {
    /// Establishes explicit transport only after verifying the expected full zero-state.
    ///
    /// # Errors
    /// Returns fixed runtime/configuration, network, provider or response failures.
    ///
    /// # Panics
    /// Tokio may panic if its caller-supplied runtime lacks I/O or time drivers.
    pub async fn connect(config: TonHttpConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http_config().limits())?;
        // This backend spaces each attempt, so retries belong to its own read
        // loop instead of an unpaced inner transport loop. All other explicit
        // transport limits, headers, endpoint and provider labels are preserved.
        let limits = config.http_config().limits();
        let single_attempt = HttpConfig::new(
            config.http_config().endpoint().clone(),
            RpcLimits::new(
                limits.connect_timeout(),
                limits.request_timeout(),
                limits.max_response_bytes(),
                0,
            )?,
            config.http_config().provider_id(),
        )?;
        let http = HttpClient::new(&single_attempt)?;
        let client = Self {
            config,
            http,
            next_attempt: Mutex::new(None),
        };
        budget.run(client.network(&budget)).await?;
        Ok(client)
    }
    /// Returns explicit expected network and redacted transport configuration.
    #[must_use]
    pub const fn config(&self) -> &TonHttpConfig {
        &self.config
    }
    async fn read<T: DeserializeOwned>(
        &self,
        budget: &OperationBudget,
        method: &str,
        query: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<T, Error> {
        let retries = self.config.http_config().limits().max_retries();
        for attempt in 0..=retries {
            self.schedule(budget).await?;
            let response = self.http.read(&[method], query, body, budget).await;
            match response {
                Ok(response)
                    if (response.status == reqwest::StatusCode::TOO_MANY_REQUESTS
                        || response.status.is_server_error())
                        && attempt < retries => {}
                Err(Error::Provider(ProviderError::Transport) | Error::Timeout)
                    if attempt < retries => {}
                Ok(response) => return wire::decode(&response.into_success()?),
                Err(error) => return Err(error),
            }
            budget
                .run(async {
                    sleep(Duration::from_millis(25)).await;
                    Ok(())
                })
                .await?;
        }
        Err(Error::Provider(ProviderError::Transport))
    }
    async fn schedule(&self, budget: &OperationBudget) -> Result<(), Error> {
        budget.check_remaining()?;
        if self.config.request_spacing().is_zero() {
            return Ok(());
        }
        let target = {
            let mut next = self.next_attempt.lock().map_err(|_| Error::Configuration)?;
            let now = Instant::now();
            let target = next.map_or(now, |n| n.max(now));
            *next = Some(
                target
                    .checked_add(self.config.request_spacing())
                    .ok_or(Error::Configuration)?,
            );
            target
        };
        budget
            .run(async {
                sleep_until(target).await;
                Ok(())
            })
            .await
    }
    async fn network(&self, budget: &OperationBudget) -> Result<NetworkData, Error> {
        let result: wire::Master = self.read(budget, "getMasterchainInfo", &[], None).await?;
        result.into_domain(self.config.network())
    }
    fn context(
        &self,
        operation: Operation,
        block: Option<Block>,
        method: &str,
    ) -> Result<Context, Error> {
        let source = Source::new(
            self.config.http_config().provider_id(),
            method,
            env!("CARGO_PKG_VERSION"),
        )?;
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        Ok(Context::new(
            operation,
            self.config.network(),
            block,
            source,
            Timestamp::from_unix_seconds(time),
        ))
    }
    fn observed<T: ObservationValue>(
        &self,
        value: T,
        block: Option<Block>,
        method: &str,
    ) -> Result<Observation<T>, Error> {
        Observation::new(value, self.context(T::operation(), block, method)?)
            .map_err(|_| wire::invalid())
    }
    async fn history(
        &self,
        budget: &OperationBudget,
        request: HistoryRequest,
    ) -> Result<HistoryPage, Error> {
        let address = request.address().to_raw();
        let limit = request.limit().to_string();
        let archival = if request.archival() { "true" } else { "false" };
        let mut query = vec![
            ("address", address.as_str()),
            ("limit", limit.as_str()),
            ("archival", archival),
        ];
        let lt = request.start().map(|c| c.logical_time().raw().to_string());
        let hash = request.start().map(|c| c.hash().to_base64());
        if let (Some(lt), Some(hash)) = (&lt, &hash) {
            query.extend([("lt", lt.as_str()), ("hash", hash.as_str())]);
        }
        let rows: Vec<wire::TransactionWire> =
            self.read(budget, "getTransactions", &query, None).await?;
        wire::page(rows, request)
    }
    async fn transaction(
        &self,
        budget: &OperationBudget,
        address: Address,
        cursor: Cursor,
    ) -> Result<Transaction, Error> {
        let request = HistoryRequest::new(address, Some(cursor), 1, true)?;
        let page = self.history(budget, request).await?;
        page.transactions()
            .first()
            .cloned()
            .ok_or(Error::UnavailableData)
    }
}
impl fmt::Debug for TonClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TonClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl TonReader for TonClient {
    async fn get_network_data(&self) -> Result<Observation<NetworkData>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                let value = self.network(&budget).await?;
                let block = value.last().clone();
                self.observed(value, Some(block), "getMasterchainInfo")
            })
            .await
    }
    async fn get_account_balance(
        &self,
        address: Address,
    ) -> Result<Observation<AccountBalance>, Error> {
        address.check_network(self.config.network())?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                let selected = self.network(&budget).await?.last().clone();
                let raw = address.to_raw();
                let seqno = selected.seqno().to_string();
                let account: wire::Account = self
                    .read(
                        &budget,
                        "getAddressInformation",
                        &[("address", &raw), ("seqno", &seqno)],
                        None,
                    )
                    .await?;
                let value = account.into_domain(address, &selected)?;
                self.observed(value, Some(selected), "getAddressInformation")
            })
            .await
    }
    async fn get_account_history(
        &self,
        request: HistoryRequest,
    ) -> Result<Observation<HistoryPage>, Error> {
        request.address().check_network(self.config.network())?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.network(&budget).await?;
                self.observed(
                    self.history(&budget, request).await?,
                    None,
                    "getTransactions",
                )
            })
            .await
    }
    async fn get_transaction(
        &self,
        address: Address,
        cursor: Cursor,
    ) -> Result<Observation<Transaction>, Error> {
        address.check_network(self.config.network())?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.network(&budget).await?;
                self.observed(
                    self.transaction(&budget, address, cursor).await?,
                    None,
                    "getTransactions",
                )
            })
            .await
    }
    async fn get_transaction_status(
        &self,
        address: Address,
        cursor: Cursor,
    ) -> Result<Observation<TransactionStatus>, Error> {
        address.check_network(self.config.network())?;
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.network(&budget).await?;
                self.observed(
                    TransactionStatus::new(self.transaction(&budget, address, cursor).await?),
                    None,
                    "getTransactions",
                )
            })
            .await
    }
    async fn get_message_status(
        &self,
        message_hash: Hash,
        request: HistoryRequest,
    ) -> Result<Observation<MessageStatus>, Error> {
        request.address().check_network(self.config.network())?;
        if message_hash == Hash::ZERO {
            return Err(ValidationError::InvalidTonRecord.into());
        }
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.network(&budget).await?;
                let page = self.history(&budget, request).await?;
                self.observed(
                    MessageStatus::scan(message_hash, page).map_err(|_| wire::invalid())?,
                    None,
                    "getTransactions",
                )
            })
            .await
    }
    async fn estimate_fee(&self, request: FeeRequest) -> Result<Observation<FeeEstimate>, Error> {
        if request.network() != self.config.network() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        let body = serde_json::to_vec(&FeeBody {
            address: request.address().to_raw(),
            body: request.body().to_base64(),
            init_code: request.init_code().map(BocText::text),
            init_data: request.init_data().map(BocText::text),
            ignore_chksig: request.ignore_signature(),
        })
        .map_err(|_| Error::Configuration)?;
        budget
            .run(async {
                self.network(&budget).await?;
                let result: wire::Fees =
                    self.read(&budget, "estimateFee", &[], Some(&body)).await?;
                let (source, destinations) = result.into_parts()?;
                self.observed(
                    FeeEstimate::new(request, source, destinations).map_err(|_| wire::invalid())?,
                    None,
                    "estimateFee",
                )
            })
            .await
    }
}
// These helpers only format already checked BOCs, never arbitrary wire JSON.
struct BocText;
impl BocText {
    fn text(value: &crate::domain::ton::Boc) -> String {
        value.to_base64()
    }
}
#[derive(Serialize)]
struct FeeBody {
    address: String,
    body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    init_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    init_data: Option<String>,
    ignore_chksig: bool,
}
#[derive(Serialize)]
struct SubmissionBody {
    boc: String,
}
impl TonSubmitter for TonClient {
    async fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> Result<Observation<SubmissionResult>, Error> {
        if submission.network() != self.config.network() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        let body = serde_json::to_vec(&SubmissionBody {
            boc: submission.boc().to_base64(),
        })
        .map_err(|_| Error::Configuration)?;
        budget.run(self.network(&budget)).await?;
        self.schedule(&budget).await?;
        let response = self
            .http
            .write_once(&["sendBocReturnHash"], &[], &body, &budget)
            .await?
            .into_success()
            .map_err(submission_unknown)?;
        let result: wire::SendResult = wire::decode(&response).map_err(submission_unknown)?;
        let hash = Hash::parse(&result.hash).map_err(submission_unknown)?;
        let value = SubmissionResult::new(submission, hash).map_err(submission_unknown)?;
        let observed = self
            .observed(value, None, "sendBocReturnHash")
            .map_err(submission_unknown)?;
        budget.check_remaining().map_err(submission_unknown)?;
        Ok(observed)
    }
}
