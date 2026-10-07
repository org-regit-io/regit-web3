// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Optional bounded outgoing XRPL JSON HTTP backend.

use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    config::HttpConfig,
    domain::{
        Source, Timestamp,
        xrpl::{
            AccountBalance, Address, Context, FeeEstimate, Hash, HistoryPage, HistoryRequest,
            Ledger, LedgerRange, Network, Observation, Operation, PageRequest, SignedSubmission,
            SubmissionResult, Transaction, TransactionStatus, TrustLinePage,
        },
    },
    error::{Error, ProviderError, ValidationError},
    transport::{HttpClient, OperationBudget, submission_unknown},
};

use super::wire::{
    self, AccountRequest, AccountResult, ApiOptions, FeeResult, LedgerRequest, LedgerResult,
    LinesRequest, LinesResult, ServerResult, invalid_response,
};

/// Caller-supplied expected XRPL network and bounded transport configuration.
#[derive(Clone, Debug)]
pub struct XrplHttpConfig {
    network: Network,
    http: HttpConfig,
}
impl XrplHttpConfig {
    /// Records validated explicit settings without loading environment variables.
    #[must_use]
    pub const fn new(network: Network, http: HttpConfig) -> Self {
        Self { network, http }
    }
    /// Returns the expected server network ID and independent display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns bounded transport settings with redacted endpoint diagnostics.
    #[must_use]
    pub const fn http_config(&self) -> &HttpConfig {
        &self.http
    }
}

/// An optional XRPL HTTP backend verifying server-reported network ID per operation.
///
/// The caller supplies a Tokio runtime with I/O and time enabled. No runtime,
/// credentials or endpoints are discovered. API-v2 XRPL `result.status` is
/// decoded independently of JSON-RPC 2.0 envelopes. Read retries retain frozen
/// serialized parameters under one deadline. Validated ledger claims are
/// source-reported; no independent consensus or validator-set proof is implied.
pub struct XrplClient {
    config: XrplHttpConfig,
    http: HttpClient,
}
impl XrplClient {
    /// Establishes a client only after checking explicit `server_info` network ID.
    ///
    /// # Errors
    /// Returns fixed configuration, budget, transport, RPC, malformed-response
    /// or network mismatch errors. Missing network ID is unsupported.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn connect(config: XrplHttpConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.http_config().limits())?;
        let http = HttpClient::new(config.http_config())?;
        let client = Self { config, http };
        budget.run(client.verify_network(&budget)).await?;
        Ok(client)
    }
    /// Returns explicit configuration with redacted endpoint diagnostics.
    #[must_use]
    pub const fn config(&self) -> &XrplHttpConfig {
        &self.config
    }

    /// Submits exactly one explicit caller-supplied signed payload using API v2.
    ///
    /// Byte/hash identity is checked; binary canonicality, embedded network,
    /// signatures and reviewed intent are not verified. Server acceptance and
    /// preliminary engine results never establish validated execution. The
    /// supported submit-only response includes handling flags and echoed bytes;
    /// API versions or servers omitting these facts cannot establish this result.
    ///
    /// Preflight reads may use configured safe-read retries. The write never
    /// retries, for any `fail_hard` value, HTTP status or connection failure.
    /// Dropping or externally timing out this future after dispatch leaves a
    /// potentially submitted transaction; use its known hash for later lookup.
    ///
    /// # Errors
    /// Configuration, network and preflight failures dispatch no write. Once
    /// execution is attempted, unresolved transport, response or deadline errors
    /// return `SubmissionOutcomeUnknown`, never evidence of definite rejection.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> Result<Observation<SubmissionResult>, Error> {
        if submission.network() != self.config.network().identity() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        let body = wire::encode(
            "submit",
            &wire::SubmitRequest {
                api_version: 2,
                tx_blob: submission.payload(),
                fail_hard: submission.fail_hard(),
            },
        )?;
        budget.run(self.verify_network(&budget)).await?;
        let response = self
            .http
            .write_once(&[], &[], &body, &budget)
            .await?
            .into_success()
            .map_err(submission_unknown)?;
        let result: wire::SubmitResult = wire::decode(&response).map_err(submission_unknown)?;
        let value = result
            .into_domain(submission.payload(), submission.hash())
            .map_err(submission_unknown)?;
        let context = self
            .context(Operation::Submission, None, "submit")
            .map_err(submission_unknown)?;
        let observed = Observation::submission(value, context).map_err(submission_unknown)?;
        budget.check_remaining().map_err(submission_unknown)?;
        Ok(observed)
    }

    /// Retrieves exact opaque transaction/metadata bytes with a recomputed transaction ID.
    ///
    /// The result retains reported inclusion; hash agreement does not verify
    /// signatures or canonical binary structure. Unknown inclusion is explicit.
    ///
    /// # Errors
    /// Rejects query/hash/payload mismatch, malformed inclusion and missing
    /// validated metadata. `txnNotFound` is unavailable, never proof of failure.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_transaction(&self, hash: Hash) -> Result<Observation<Transaction>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let result: wire::BinaryResult = self
                    .read(
                        &budget,
                        "tx",
                        &wire::TxRequest {
                            api_version: 2,
                            transaction: hash,
                            binary: true,
                        },
                    )
                    .await?;
                if result.hash != hash {
                    return Err(invalid_response());
                }
                let value = result.into_domain()?;
                let context = self.context(Operation::Transaction, value.ledger(), "tx")?;
                Observation::transaction(value, context).map_err(|_| invalid_response())
            })
            .await
    }

    /// Retrieves actual transaction execution code independently of ledger validation.
    ///
    /// Validated `tec` results are failures that claim transaction cost, not
    /// successful transfers. Missing transactions remain unavailable; unvalidated
    /// lookups can have neither an inclusion ledger nor execution metadata.
    ///
    /// # Errors
    /// Rejects mismatched query identity, malformed result/inclusion or a
    /// validated transaction lacking execution metadata, plus provider failures.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_transaction_status(
        &self,
        hash: Hash,
    ) -> Result<Observation<TransactionStatus>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let result: wire::StatusResult = self
                    .read(
                        &budget,
                        "tx",
                        &wire::TxRequest {
                            api_version: 2,
                            transaction: hash,
                            binary: false,
                        },
                    )
                    .await?;
                if result.hash != hash {
                    return Err(invalid_response());
                }
                let value = result.into_domain()?;
                let context = self.context(Operation::TransactionStatus, value.ledger(), "tx")?;
                Observation::transaction_status(value, context).map_err(|_| invalid_response())
            })
            .await
    }

    /// Reads one bounded binary account-history page over an explicit ledger range.
    ///
    /// The actual searched range and ledger-bound marker remain explicit. Binary
    /// API-v2 entries may omit transaction and ledger hashes: IDs are recomputed
    /// from exact payloads, while omitted ledger hashes remain unknown. Account
    /// involvement and validation are source-reported, not independently proved.
    ///
    /// # Errors
    /// Rejects account mismatch, duplicate IDs, out-of-range/oversized pages,
    /// malformed or unvalidated inclusion and nonadvancing continuation.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_account_history(
        &self,
        account: Address,
        request: HistoryRequest,
    ) -> Result<Observation<HistoryPage>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let range = request.range();
                let result: wire::HistoryResult = self
                    .read(
                        &budget,
                        "account_tx",
                        &wire::HistoryParams {
                            api_version: 2,
                            account,
                            ledger_index_min: range.minimum(),
                            ledger_index_max: range.maximum(),
                            limit: request.limit(),
                            forward: request.forward(),
                            binary: true,
                            marker: request.marker(),
                        },
                    )
                    .await?;
                if result.account != account || result.limit != request.limit() || !result.validated
                {
                    return Err(invalid_response());
                }
                let searched = LedgerRange::new(result.ledger_index_min, result.ledger_index_max)
                    .map_err(|_| invalid_response())?;
                let transactions = result
                    .transactions
                    .into_iter()
                    .map(wire::HistoryEntry::into_domain)
                    .collect::<Result<Vec<_>, _>>()?;
                let value =
                    HistoryPage::new(account, request, searched, transactions, result.marker)
                        .map_err(|_| invalid_response())?;
                Observation::account_history(
                    value,
                    self.context(Operation::AccountHistory, None, "account_tx")?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }

    /// Reads exact XRP drops and account sequencing facts at a validated ledger.
    ///
    /// An absent hash resolves the latest source-validated ledger once. The read
    /// uses that exact hash; retries never re-resolve it. A single total budget
    /// covers network verification, ledger resolution and account retrieval.
    ///
    /// # Errors
    /// Rejects malformed or mismatched account/ledger facts, unvalidated ledgers,
    /// negative or overflow drops, missing accounts and provider failures.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_account_balance(
        &self,
        account: Address,
        ledger_hash: Option<Hash>,
    ) -> Result<Observation<AccountBalance>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let ledger = self.resolve_ledger(&budget, ledger_hash).await?;
                let hash = ledger.hash().ok_or_else(invalid_response)?;
                let result: AccountResult = self
                    .read(
                        &budget,
                        "account_info",
                        &AccountRequest {
                            api_version: 2,
                            account,
                            ledger_hash: hash,
                            signer_lists: false,
                        },
                    )
                    .await?;
                check_ledger(
                    ledger,
                    result.ledger_hash,
                    result.ledger_index,
                    result.validated,
                )?;
                if result.account_data.account != account
                    || result.account_data.ledger_entry_type != "AccountRoot"
                {
                    return Err(invalid_response());
                }
                let root = result.account_data;
                let value = AccountBalance::new(
                    root.account,
                    root.balance,
                    root.sequence,
                    root.owner_count,
                    root.flags,
                );
                Observation::account_balance(
                    value,
                    self.context(Operation::AccountBalance, Some(ledger), "account_info")?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }

    /// Reads one bounded page retaining signed issued balances and trust settings.
    ///
    /// Resuming requires the previous actual ledger hash. The returned marker
    /// remains scoped to the same account, method, source and ledger.
    ///
    /// # Errors
    /// Rejects changed network, account/ledger mismatches, duplicate lines,
    /// overflow or nonrepresentable exact values and nonadvancing markers.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_trust_lines(
        &self,
        account: Address,
        request: PageRequest,
    ) -> Result<Observation<TrustLinePage>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let ledger = self.resolve_ledger(&budget, request.ledger_hash()).await?;
                let hash = ledger.hash().ok_or_else(invalid_response)?;
                let result: LinesResult = self
                    .read(
                        &budget,
                        "account_lines",
                        &LinesRequest {
                            api_version: 2,
                            account,
                            ledger_hash: hash,
                            limit: request.limit(),
                            ignore_default: false,
                            marker: request.marker(),
                        },
                    )
                    .await?;
                check_ledger(
                    ledger,
                    result.ledger_hash,
                    result.ledger_index,
                    result.validated,
                )?;
                if result.account != account
                    || result.limit.is_some_and(|limit| limit != request.limit())
                {
                    return Err(invalid_response());
                }
                let lines = result
                    .lines
                    .into_iter()
                    .map(super::wire::WireLine::into_domain)
                    .collect::<Result<Vec<_>, _>>()?;
                let value = TrustLinePage::new(account, request, lines, result.marker)
                    .map_err(|_| invalid_response())?;
                Observation::trust_lines(
                    value,
                    self.context(Operation::TrustLines, Some(ledger), "account_lines")?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }

    /// Reads exact current fee components without choosing a transaction fee.
    ///
    /// The context retains the open ledger index and has no validated hash.
    ///
    /// # Errors
    /// Rejects missing, negative, fractional or overflow fee drops and invalid
    /// open-ledger identity, in addition to fixed transport/provider failures.
    ///
    /// # Panics
    /// Tokio may panic if the caller's runtime lacks I/O or time drivers.
    pub async fn get_fee_estimate(&self) -> Result<Observation<FeeEstimate>, Error> {
        let budget = OperationBudget::new(self.config.http_config().limits())?;
        budget
            .run(async {
                self.verify_network(&budget).await?;
                let result: FeeResult = self
                    .read(&budget, "fee", &ApiOptions { api_version: 2 })
                    .await?;
                let ledger = Ledger::new(result.ledger_current_index, None, false)
                    .map_err(|_| invalid_response())?;
                let drops = result.drops;
                let value =
                    FeeEstimate::new(drops.base, drops.minimum, drops.median, drops.open_ledger);
                Observation::fee_estimate(
                    value,
                    self.context(Operation::FeeEstimate, Some(ledger), "fee")?,
                )
                .map_err(|_| invalid_response())
            })
            .await
    }

    async fn read<T: DeserializeOwned, P: Serialize + ?Sized>(
        &self,
        budget: &OperationBudget,
        method: &'static str,
        params: &P,
    ) -> Result<T, Error> {
        let body = wire::encode(method, params)?;
        let response = self
            .http
            .read(&[], &[], Some(&body), budget)
            .await?
            .into_success()?;
        wire::decode(&response)
    }
    async fn verify_network(&self, budget: &OperationBudget) -> Result<(), Error> {
        let result: ServerResult = self
            .read(budget, "server_info", &ApiOptions { api_version: 2 })
            .await?;
        let wire::Field::Present(actual) = result.info.network_id else {
            return Err(Error::UnsupportedCapability);
        };
        if actual != self.config.network().identity().number() {
            return Err(Error::Provider(ProviderError::ChainMismatch));
        }
        Ok(())
    }
    async fn resolve_ledger(
        &self,
        budget: &OperationBudget,
        hash: Option<Hash>,
    ) -> Result<Ledger, Error> {
        let result: LedgerResult = self
            .read(
                budget,
                "ledger",
                &LedgerRequest {
                    api_version: 2,
                    ledger_index: hash.is_none().then_some("validated"),
                    ledger_hash: hash,
                    transactions: false,
                    expand: false,
                },
            )
            .await?;
        if !result.validated || hash.is_some_and(|expected| expected != result.ledger_hash) {
            return Err(invalid_response());
        }
        Ledger::new(
            result.ledger_index,
            Some(result.ledger_hash),
            result.validated,
        )
        .map_err(|_| invalid_response())
    }
    fn context(
        &self,
        operation: Operation,
        ledger: Option<Ledger>,
        method: &'static str,
    ) -> Result<Context, Error> {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid_response())?
            .as_secs();
        Ok(Context::new(
            operation,
            self.config.network().clone(),
            ledger,
            Source::new(
                self.config.http_config().provider_id(),
                method,
                env!("CARGO_PKG_VERSION"),
            )?,
            Timestamp::from_unix_seconds(seconds),
        ))
    }
}
fn check_ledger(expected: Ledger, hash: Hash, index: u32, validated: bool) -> Result<(), Error> {
    if expected.hash() != Some(hash) || expected.index() != index || !validated {
        return Err(invalid_response());
    }
    Ok(())
}
impl fmt::Debug for XrplClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XrplClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl super::XrplReader for XrplClient {
    async fn get_account_balance(
        &self,
        account: Address,
        ledger_hash: Option<Hash>,
    ) -> Result<Observation<AccountBalance>, Error> {
        Self::get_account_balance(self, account, ledger_hash).await
    }
    async fn get_trust_lines(
        &self,
        account: Address,
        request: PageRequest,
    ) -> Result<Observation<TrustLinePage>, Error> {
        Self::get_trust_lines(self, account, request).await
    }
    async fn get_fee_estimate(&self) -> Result<Observation<FeeEstimate>, Error> {
        Self::get_fee_estimate(self).await
    }
    async fn get_transaction(&self, hash: Hash) -> Result<Observation<Transaction>, Error> {
        Self::get_transaction(self, hash).await
    }
    async fn get_transaction_status(
        &self,
        hash: Hash,
    ) -> Result<Observation<TransactionStatus>, Error> {
        Self::get_transaction_status(self, hash).await
    }
    async fn get_account_history(
        &self,
        account: Address,
        request: HistoryRequest,
    ) -> Result<Observation<HistoryPage>, Error> {
        Self::get_account_history(self, account, request).await
    }
}
impl super::XrplSubmitter for XrplClient {
    async fn submit_signed(
        &self,
        submission: SignedSubmission,
    ) -> Result<Observation<SubmissionResult>, Error> {
        Self::submit_signed(self, submission).await
    }
}
