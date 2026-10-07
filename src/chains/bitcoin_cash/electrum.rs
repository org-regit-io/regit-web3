// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{BitcoinCashReader, wire};
use crate::{
    config::RpcLimits,
    domain::{
        Source, Timestamp,
        bitcoin_cash::{
            Address, AddressBalance, BlockHeader, CollectionLimit, Context, FeeEstimate, FeeTarget,
            History, HistoryRange, HistoryUpperBound, NetworkIdentity, Observation, Operation,
            ProtocolMetadata, ReadValue, SourceHeight, SourceInclusion, SourceText, TokenFilter,
            Transaction, TransactionStatus, Txid, UnspentOutputs,
        },
    },
    error::{Error, ProviderError, ValidationError},
    transport::OperationBudget,
};
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, ServerName},
};
use serde::de::DeserializeOwned;
use std::{
    future::Future,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tokio_rustls::{TlsConnector, client::TlsStream};

fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn transport() -> Error {
    Error::Provider(ProviderError::Transport)
}

fn protocol_version(value: &str) -> Result<(u16, u16), Error> {
    let (major, minor) = value.split_once('.').ok_or_else(invalid)?;
    let parse = |part: &str| {
        if part.is_empty()
            || part.len() > 5
            || !part.bytes().all(|b| b.is_ascii_digit())
            || part.len() > 1 && part.starts_with('0')
        {
            return Err(invalid());
        }
        part.parse::<u16>().map_err(|_| invalid())
    };
    Ok((parse(major)?, parse(minor)?))
}

/// Explicit TLS connection target and separately supplied certificate identity.
/// Host and server name remain excluded from diagnostics. No plaintext variant exists.
#[derive(Clone)]
pub struct ElectrumEndpoint {
    host: String,
    port: u16,
    server_name: ServerName<'static>,
}
impl ElectrumEndpoint {
    /// Validates explicit host, port and TLS certificate server name.
    /// # Errors
    /// Rejects empty/excessive hosts, URL syntax, whitespace and invalid TLS names or zero ports.
    pub fn new(
        host: impl Into<String>,
        port: u16,
        server_name: impl Into<String>,
    ) -> Result<Self, Error> {
        let host = host.into();
        let name = server_name.into();
        if host.is_empty()
            || host.len() > 253
            || !host.is_ascii()
            || port == 0
            || host
                .bytes()
                .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
            || host.contains(['/', '@', '?', '#', '\\'])
            || host.contains(':') && host.parse::<std::net::IpAddr>().is_err()
        {
            return Err(Error::Configuration);
        }
        let server_name = ServerName::try_from(name).map_err(|_| Error::Configuration)?;
        Ok(Self {
            host,
            port,
            server_name,
        })
    }
}
impl std::fmt::Debug for ElectrumEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ElectrumEndpoint { host: [REDACTED], server_name: [REDACTED] }")
    }
}

/// Explicit caller-owned DER trust anchors, with no ambient certificate discovery.
/// Certificate and hostname validation remain enabled; no insecure verifier is exposed.
#[derive(Clone)]
pub struct TlsTrustRoots {
    roots: RootCertStore,
}
impl TlsTrustRoots {
    /// Validates a bounded explicit collection of DER certificates for trusted roots.
    /// # Errors
    /// Requires 1..=1024 certificates, each 1..=65536 bytes, totalling at most two MiB.
    pub fn new(certificates: Vec<Vec<u8>>) -> Result<Self, Error> {
        if certificates.is_empty() || certificates.len() > 1024 {
            return Err(Error::Configuration);
        }
        let mut total = 0_usize;
        let mut roots = RootCertStore::empty();
        for certificate in certificates {
            total = total
                .checked_add(certificate.len())
                .ok_or(Error::Configuration)?;
            if certificate.is_empty() || certificate.len() > 65_536 || total > 2 * 1024 * 1024 {
                return Err(Error::Configuration);
            }
            roots
                .add(CertificateDer::from(certificate))
                .map_err(|_| Error::Configuration)?;
        }
        Ok(Self { roots })
    }
}
impl std::fmt::Debug for TlsTrustRoots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsTrustRoots")
            .field("count", &self.roots.len())
            .finish()
    }
}

/// Explicit BCH network/TLS source, trust anchors, limits and provider attribution.
/// Configuration does not consult environment variables or contact a server.
#[derive(Clone, Debug)]
pub struct ElectrumConfig {
    network: NetworkIdentity,
    endpoint: ElectrumEndpoint,
    trust_roots: TlsTrustRoots,
    limits: RpcLimits,
    provider_id: String,
}
impl ElectrumConfig {
    /// Records validated caller configuration, without HTTP or credentials.
    /// # Errors
    /// Rejects invalid non-secret source attribution.
    pub fn new(
        network: NetworkIdentity,
        endpoint: ElectrumEndpoint,
        trust_roots: TlsTrustRoots,
        limits: RpcLimits,
        provider_id: impl Into<String>,
    ) -> Result<Self, Error> {
        let provider_id = provider_id.into();
        let _source = Source::new(&provider_id, "electrum-cash", env!("CARGO_PKG_VERSION"))?;
        Ok(Self {
            network,
            endpoint,
            trust_roots,
            limits,
            provider_id,
        })
    }
    /// Returns exact expected chain facts, independent of address namespace compatibility.
    #[must_use]
    pub const fn network(&self) -> &NetworkIdentity {
        &self.network
    }
    /// Returns redacted explicit TLS source configuration.
    #[must_use]
    pub const fn endpoint(&self) -> &ElectrumEndpoint {
        &self.endpoint
    }
    /// Returns validated shared-deadline/body/retry settings.
    #[must_use]
    pub const fn limits(&self) -> RpcLimits {
        self.limits
    }
    /// Returns caller non-secret provider attribution.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }
}

/// Certificate-verified TLS Electrum-Cash 1.6 BCH reader.
///
/// Establishment and every read verify negotiated protocol, token support,
/// complete genesis and the exact supplied fork checkpoint. Matching source
/// headers does not establish chainwork, consensus or a read snapshot. Safe read
/// retries re-establish and verify the same explicit source within one deadline.
#[derive(Clone)]
pub struct ElectrumClient {
    config: ElectrumConfig,
    tls: Arc<ClientConfig>,
}
impl std::fmt::Debug for ElectrumClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElectrumClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
impl ElectrumClient {
    /// Returns the explicit expected network, TLS source, trust and operation limits.
    #[must_use]
    pub const fn config(&self) -> &ElectrumConfig {
        &self.config
    }

    /// Establishes and verifies the explicit TLS source and expected fork identity.
    /// # Errors
    /// Reports absent runtime, invalid TLS setup/certificates, source mismatch or bounded failures.
    /// # Panics
    /// Tokio may panic if the caller runtime lacks I/O or time drivers.
    pub async fn connect(config: ElectrumConfig) -> Result<Self, Error> {
        let budget = OperationBudget::new(config.limits)?;
        let tls = ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| Error::Configuration)?
        .with_root_certificates(config.trust_roots.roots.clone())
        .with_no_client_auth();
        let client = Self {
            config,
            tls: Arc::new(tls),
        };
        let _session = budget.run(client.verified_session()).await?;
        Ok(client)
    }
    async fn verified_session(&self) -> Result<Session, Error> {
        let endpoint = &self.config.endpoint;
        let connect = async {
            let stream = TcpStream::connect((endpoint.host.as_str(), endpoint.port))
                .await
                .map_err(|_| transport())?;
            TlsConnector::from(Arc::clone(&self.tls))
                .connect(endpoint.server_name.clone(), stream)
                .await
                .map_err(|_| transport())
        };
        let stream = tokio::time::timeout(self.config.limits.connect_timeout(), connect)
            .await
            .map_err(|_| Error::Timeout)??;
        let mut session = Session {
            stream: BufReader::new(stream),
            next_id: 1,
            maximum: self.config.limits.max_response_bytes(),
            metadata: None,
        };
        let version: [String; 2] = session
            .rpc("server.version", &serde_json::json!(["regit-web3", "1.6"]))
            .await?;
        if version[1] != "1.6" {
            return Err(Error::UnsupportedCapability);
        }
        let features: wire::Features = session
            .rpc("server.features", &serde_json::json!([]))
            .await?;
        if features.hash_function != "sha256" || features.cashtokens != Some(true) {
            return Err(Error::UnsupportedCapability);
        }
        if features.genesis_hash != self.config.network.genesis_hash() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        if features.server_version != version[0] {
            return Err(invalid());
        }
        if protocol_version(&features.protocol_min)? > (1, 6)
            || protocol_version(&features.protocol_max)? < (1, 6)
        {
            return Err(Error::UnsupportedCapability);
        }
        session.metadata = Some(ProtocolMetadata {
            software: SourceText::new(version[0].clone()).map_err(|_| invalid())?,
            version: SourceText::new(version[1].clone()).map_err(|_| invalid())?,
            cash_tokens: true,
        });
        let genesis: String = session
            .rpc("blockchain.block.header", &serde_json::json!([0, 0]))
            .await?;
        if BlockHeader::from_hex(&genesis)
            .map_err(|_| invalid())?
            .hash()
            != self.config.network.genesis_hash()
        {
            return Err(ValidationError::NetworkMismatch.into());
        }
        let expected = self.config.network.fork_checkpoint();
        let fork: String = session
            .rpc(
                "blockchain.block.header",
                &serde_json::json!([expected.height(), 0]),
            )
            .await?;
        if BlockHeader::from_hex(&fork).map_err(|_| invalid())?.hash() != expected.hash() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(session)
    }
    fn check_address(&self, address: &Address) -> Result<(), Error> {
        if address.namespace() != self.config.network.namespace() {
            return Err(ValidationError::NetworkMismatch.into());
        }
        Ok(())
    }
    fn context(&self, operation: Operation, method: &str) -> Result<Context, Error> {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Configuration)?
            .as_secs();
        Context::new(
            self.config.network.clone(),
            operation,
            Source::new(&self.config.provider_id, method, env!("CARGO_PKG_VERSION"))?,
            Timestamp::from_unix_seconds(seconds),
        )
    }
    async fn execute<T, F, Fut>(
        &self,
        operation: Operation,
        method: &str,
        read: F,
    ) -> Result<Observation<T>, Error>
    where
        T: ReadValue,
        F: Fn(Session) -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        let budget = OperationBudget::new(self.config.limits)?;
        budget
            .run(async {
                let mut retries = 0;
                loop {
                    let result = async {
                        let session = self.verified_session().await?;
                        let metadata = session.metadata.clone().ok_or_else(invalid)?;
                        Ok::<_, Error>((read(session).await?, metadata))
                    }
                    .await;
                    match result {
                        Err(Error::Provider(ProviderError::Transport))
                            if retries < self.config.limits.max_retries() =>
                        {
                            retries += 1;
                        }
                        result => {
                            let (value, metadata) = result?;
                            return Observation::new(
                                self.context(operation.clone(), method)?
                                    .with_protocol_metadata(metadata),
                                value,
                            );
                        }
                    }
                }
            })
            .await
    }
    /// Reads actual BCH native balance and signed unconfirmed delta with an explicit token filter.
    /// # Errors
    /// Reports wrong namespace, source mismatch, malformed source or bounded failures.
    /// # Panics
    /// Tokio may panic if the runtime lacks I/O or time drivers.
    pub async fn get_address_balance(
        &self,
        address: Address,
        token_filter: TokenFilter,
    ) -> Result<Observation<AddressBalance>, Error> {
        self.check_address(&address)?;
        self.execute(
            Operation::AddressBalance {
                address: address.clone(),
                token_filter,
            },
            "address-balance",
            move |mut session| {
                let address = address.clone();
                async move {
                    let value: wire::Balance = session
                        .rpc(
                            "blockchain.scripthash.get_balance",
                            &serde_json::json!([
                                address.script_hash().to_string(),
                                token_filter.as_str()
                            ]),
                        )
                        .await?;
                    wire::balance(address, token_filter, &value)
                }
            },
        )
        .await
    }
    /// Reads all source history in one exact caller interval and capacity.
    /// # Errors
    /// Rejects source shape/order/range/capacity disagreement and bounded transport failure.
    /// # Panics
    /// Tokio may panic if the runtime lacks I/O or time drivers.
    pub async fn get_address_history(
        &self,
        address: Address,
        range: HistoryRange,
    ) -> Result<Observation<History>, Error> {
        self.check_address(&address)?;
        self.execute(
            Operation::AddressHistory {
                address: address.clone(),
                range,
            },
            "address-history",
            move |mut session| {
                let address = address.clone();
                async move {
                    let upper = match range.upper() {
                        HistoryUpperBound::Height { height } => i64::from(height),
                        HistoryUpperBound::OpenTip => -1,
                    };
                    let value: wire::HistoryList = session
                        .rpc(
                            "blockchain.scripthash.get_history",
                            &serde_json::json!([
                                address.script_hash().to_string(),
                                range.from_height(),
                                upper
                            ]),
                        )
                        .await?;
                    wire::history(address, range, value)
                }
            },
        )
        .await
    }
    /// Reads an exact decimal source estimate in BCH per 1,000 bytes.
    /// # Errors
    /// Reports malformed estimates/source or bounded transport failure; `-1` remains unavailable.
    /// # Panics
    /// Tokio may panic if the runtime lacks I/O or time drivers.
    pub async fn get_fee_estimate(
        &self,
        target: FeeTarget,
    ) -> Result<Observation<FeeEstimate>, Error> {
        self.execute(
            Operation::FeeEstimate { target },
            "estimate-fee",
            move |mut session| async move {
                let value: serde_json::Number = session
                    .rpc(
                        "blockchain.estimatefee",
                        &serde_json::json!([target.blocks()]),
                    )
                    .await?;
                wire::fee(target, &value)
            },
        )
        .await
    }
    /// Reads source transaction height and separately correlated source header identity.
    /// # Errors
    /// Rejects missing/contradictory status facts and bounded transport failures.
    /// # Panics
    /// Tokio may panic if the runtime lacks I/O or time drivers.
    pub async fn get_transaction_status(
        &self,
        txid: Txid,
    ) -> Result<Observation<TransactionStatus>, Error> {
        self.execute(
            Operation::TransactionStatus { txid },
            "transaction-status",
            move |mut session| async move { status(&mut session, txid).await },
        )
        .await
    }
    /// Reads complete BCH daemon source fields plus exact separately retrieved raw bytes.
    /// # Errors
    /// Rejects unsupported/incomplete source shape, wrong identities, capacities and bounded failures.
    /// # Panics
    /// Tokio may panic if the runtime lacks I/O or time drivers.
    pub async fn get_transaction(
        &self,
        txid: Txid,
        limit: CollectionLimit,
    ) -> Result<Observation<Transaction>, Error> {
        let namespace = self.config.network.namespace();
        self.execute(
            Operation::Transaction { txid, limit },
            "transaction",
            move |mut session| async move {
                let value: wire::TransactionWire = session
                    .rpc(
                        "blockchain.transaction.get",
                        &serde_json::json!([txid.to_string(), true]),
                    )
                    .await?;
                let raw: String = session
                    .rpc(
                        "blockchain.transaction.get",
                        &serde_json::json!([txid.to_string(), false]),
                    )
                    .await?;
                let status = status(&mut session, txid).await?;
                wire::transaction(namespace, txid, limit, value, &raw, status)
            },
        )
        .await
    }
    /// Reads complete source UTXOs, preserving supplied exact `CashToken` metadata.
    /// # Errors
    /// Rejects wrong namespace/filter, duplicates, malformed/over-capacity source or bounded failures.
    /// # Panics
    /// Tokio may panic if the runtime lacks I/O or time drivers.
    pub async fn get_unspent_outputs(
        &self,
        address: Address,
        token_filter: TokenFilter,
        limit: CollectionLimit,
    ) -> Result<Observation<UnspentOutputs>, Error> {
        self.check_address(&address)?;
        self.execute(
            Operation::UnspentOutputs {
                address: address.clone(),
                token_filter,
                limit,
            },
            "list-unspent",
            move |mut session| {
                let address = address.clone();
                async move {
                    let value: wire::UnspentList = session
                        .rpc(
                            "blockchain.scripthash.listunspent",
                            &serde_json::json!([
                                address.script_hash().to_string(),
                                token_filter.as_str()
                            ]),
                        )
                        .await?;
                    wire::unspent(address, token_filter, limit, value)
                }
            },
        )
        .await
    }
}

impl BitcoinCashReader for ElectrumClient {
    async fn get_address_balance(
        &self,
        a: Address,
        f: TokenFilter,
    ) -> Result<Observation<AddressBalance>, Error> {
        Self::get_address_balance(self, a, f).await
    }
    async fn get_address_history(
        &self,
        a: Address,
        r: HistoryRange,
    ) -> Result<Observation<History>, Error> {
        Self::get_address_history(self, a, r).await
    }
    async fn get_fee_estimate(&self, t: FeeTarget) -> Result<Observation<FeeEstimate>, Error> {
        Self::get_fee_estimate(self, t).await
    }
    async fn get_transaction_status(
        &self,
        t: Txid,
    ) -> Result<Observation<TransactionStatus>, Error> {
        Self::get_transaction_status(self, t).await
    }
    async fn get_transaction(
        &self,
        t: Txid,
        l: CollectionLimit,
    ) -> Result<Observation<Transaction>, Error> {
        Self::get_transaction(self, t, l).await
    }
    async fn get_unspent_outputs(
        &self,
        a: Address,
        f: TokenFilter,
        l: CollectionLimit,
    ) -> Result<Observation<UnspentOutputs>, Error> {
        Self::get_unspent_outputs(self, a, f, l).await
    }
}

struct Session {
    stream: BufReader<TlsStream<TcpStream>>,
    next_id: u64,
    maximum: usize,
    metadata: Option<ProtocolMetadata>,
}
impl Session {
    async fn rpc<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<T, Error> {
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or_else(invalid)?;
        let mut request = serde_json::to_vec(
            &serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        )
        .map_err(|_| invalid())?;
        request.push(b'\n');
        self.stream
            .get_mut()
            .write_all(&request)
            .await
            .map_err(|_| transport())?;
        self.stream
            .get_mut()
            .flush()
            .await
            .map_err(|_| transport())?;
        let mut line = Vec::new();
        loop {
            let buffer = self.stream.fill_buf().await.map_err(|_| transport())?;
            if buffer.is_empty() {
                return Err(transport());
            }
            let end = buffer.iter().position(|b| *b == b'\n');
            let count = end.unwrap_or(buffer.len());
            if line
                .len()
                .checked_add(count)
                .is_none_or(|n| n > self.maximum)
            {
                return Err(Error::Provider(ProviderError::ResponseTooLarge));
            }
            line.extend_from_slice(&buffer[..count]);
            let consume = count + usize::from(end.is_some());
            self.stream.consume(consume);
            if end.is_some() {
                break;
            }
        }
        wire::response(&line, id)
    }
}

async fn status(session: &mut Session, txid: Txid) -> Result<TransactionStatus, Error> {
    let height: Option<u32> = session
        .rpc(
            "blockchain.transaction.get_height",
            &serde_json::json!([txid.to_string()]),
        )
        .await?;
    let (height, inclusion) = match height {
        None => (SourceHeight::Unknown, None),
        Some(0) => (SourceHeight::Zero, None),
        Some(height) => {
            let value: wire::Confirmed = session
                .rpc(
                    "blockchain.transaction.get_confirmed_blockhash",
                    &serde_json::json!([txid.to_string(), true]),
                )
                .await?;
            if value.height != height {
                return Err(invalid());
            }
            let header = BlockHeader::from_hex(&value.header).map_err(|_| invalid())?;
            let inclusion =
                SourceInclusion::new(height, value.hash, header).map_err(|_| invalid())?;
            (SourceHeight::Positive { height }, Some(inclusion))
        }
    };
    TransactionStatus::new(txid, height, inclusion).map_err(|_| invalid())
}
