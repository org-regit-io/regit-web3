// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

mod types;
use crate::domain::rubic::json::{Bounded, document};
use crate::{
    domain::{
        Address, ExactDecimal,
        evm::{Data, Quantity},
        rubic::{
            Account, Asset, AssetIdentifier, Catalogue, Chain, ChainInfo, Chains,
            DestinationTransaction, EvmPayload, Family, ForeignFilters, FreshQuote, FreshQuoteData,
            Identifier, Payload, PreparationRequest, PreparedSwap, PreparedSwapData,
            ProviderStatus, Quote, QuoteData, QuoteRequest, Routes, SourceAdditionalData, SourceId,
            Status, StatusQuery, SwapKind, Text, TransactionId, Warning,
        },
        solana::{Signature, UnsignedTransaction},
    },
    error::{Error, ProviderError},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, de::IgnoredAny};
use std::collections::BTreeMap;
pub(super) use types::Switch;
use types::{
    EstimateWire, FeesWire, LegWire, Number, TokenAmountWire, TokenWire, required_optional,
};
pub(super) const MAX_DOCUMENT: usize = crate::domain::rubic::json::MAX_DOCUMENT;
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tokens {
    from: TokenAmountWire,
    to: TokenWire,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransactionWire {
    approval_address: Option<Address>,
    permit2_address: Option<Address>,
    data: Option<String>,
    to: Option<Address>,
    value: Option<String>,
    deposit_address: Option<IgnoredAny>,
    amount_to_send: Option<IgnoredAny>,
    exchange_id: Option<IgnoredAny>,
    extra_fields: Option<IgnoredAny>,
    ton_messages: Option<IgnoredAny>,
    psbt: Option<IgnoredAny>,
    sign_inputs: Option<IgnoredAny>,
    fee_limit: Option<IgnoredAny>,
    call_value: Option<IgnoredAny>,
    signature: Option<IgnoredAny>,
    arguments: Option<IgnoredAny>,
    raw_parameter: Option<IgnoredAny>,
    transaction: Option<IgnoredAny>,
}
impl TransactionWire {
    fn direct(&self) -> Result<(), Error> {
        if self.deposit_address.is_some()
            || self.amount_to_send.is_some()
            || self.exchange_id.is_some()
            || self.extra_fields.is_some()
            || self.ton_messages.is_some()
            || self.psbt.is_some()
            || self.sign_inputs.is_some()
            || self.fee_limit.is_some()
            || self.call_value.is_some()
            || self.signature.is_some()
            || self.arguments.is_some()
            || self.raw_parameter.is_some()
            || self.transaction.is_some()
        {
            return Err(invalid());
        }
        Ok(())
    }
    fn payload(self, source: &Chain) -> Result<Payload, Error> {
        self.direct()?;
        let bytes = self.data.ok_or_else(invalid)?;
        match source.family() {
            Family::Evm { .. } => Ok(Payload::Evm(EvmPayload {
                to: self.to.ok_or_else(invalid)?,
                value: Quantity::from_decimal(&self.value.ok_or_else(invalid)?)
                    .map_err(|_| invalid())?,
                input: Data::parse(&bytes).map_err(|_| invalid())?,
            })),
            Family::Solana { .. } => {
                if self.to.is_some()
                    || self.value.is_some()
                    || self.approval_address.is_some()
                    || self.permit2_address.is_some()
                    || bytes.len() > 16_384
                {
                    return Err(invalid());
                }
                let raw = STANDARD.decode(&bytes).map_err(|_| invalid())?;
                if STANDARD.encode(&raw) != bytes {
                    return Err(invalid());
                }
                Ok(Payload::Solana(
                    UnsignedTransaction::from_bytes(raw).map_err(|_| invalid())?,
                ))
            }
            Family::Provider { .. } => Err(Error::UnsupportedCapability),
        }
    }
}
#[derive(Deserialize)]
struct WarningWire {
    code: Option<Number>,
    #[serde(default)]
    reason: IgnoredAny,
    #[serde(default)]
    message: IgnoredAny,
}
impl WarningWire {
    fn typed(self) -> Result<Warning, Error> {
        let _ = (self.reason, self.message);
        Ok(Warning {
            code: self.code.as_ref().map(Number::u64).transpose()?,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteWire {
    id: String,
    tokens: Tokens,
    #[serde(rename = "swapType")]
    kind: SwapKind,
    provider_type: String,
    estimate: EstimateWire,
    fees: FeesWire,
    routing: Bounded<LegWire, 128>,
    transaction: TransactionWire,
    warnings: Bounded<WarningWire, 128>,
    use_rubic_contract: bool,
    private: bool,
}
impl QuoteWire {
    fn typed(self, request: &QuoteRequest, c: &Catalogue) -> Result<Quote, Error> {
        self.transaction.direct()?;
        if self.private {
            return Err(invalid());
        }
        Quote::new(QuoteData {
            request: request.clone(),
            id: Identifier::new(&self.id).map_err(|_| invalid())?,
            input: self.tokens.from.typed(c)?,
            destination: self.tokens.to.typed(c)?,
            provider: Identifier::new(&self.provider_type).map_err(|_| invalid())?,
            kind: self.kind,
            estimate: self.estimate.typed(request.data().destination.decimals())?,
            fees: self.fees.typed(c)?,
            legs: self
                .routing
                .0
                .into_iter()
                .map(|l| l.typed(c))
                .collect::<Result<_, _>>()?,
            warnings: self
                .warnings
                .0
                .into_iter()
                .map(WarningWire::typed)
                .collect::<Result<_, _>>()?,
            use_rubic_contract: self.use_rubic_contract,
            approval_address: self.transaction.approval_address,
            permit2_address: self.transaction.permit2_address,
        })
        .map_err(|_| invalid())
    }
}
pub(super) fn quote_best(
    bytes: &[u8],
    request: &QuoteRequest,
    c: &Catalogue,
) -> Result<Quote, Error> {
    let raw = document(bytes)?;
    let w: QuoteWire = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    w.typed(request, c)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Echo {
    src_token_address: String,
    dst_token_address: String,
    src_token_blockchain: String,
    dst_token_blockchain: String,
    src_token_amount: String,
    slippage: Number,
    timeout: u8,
    referrer: String,
    native_blacklist: Bounded<String, 128>,
    foreign_blacklist: Option<BTreeMap<String, Bounded<String, 128>>>,
    provider_tags: Bounded<String, 4>,
    enable_testnets: Switch,
    enable_checks: Switch,
    skip_fee_providers: Switch,
    show_dangerous_routes: Switch,
    show_failed_routes: Switch,
    id: Option<String>,
    from_address: Option<String>,
    receiver: Option<String>,
    integrator_address: Option<Address>,
    preferred_provider: Option<String>,
    use_deposit_trade_if_available: Option<bool>,
}
impl Echo {
    fn matches(
        &self,
        request: &QuoteRequest,
        sender: Option<&Account>,
        receiver: Option<&Account>,
        id: Option<&Identifier>,
    ) -> Result<(), Error> {
        let r = request.data();
        let mut expected = r
            .excluded_providers
            .iter()
            .map(Identifier::as_str)
            .collect::<Vec<_>>();
        let mut actual = self
            .native_blacklist
            .0
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        expected.sort_unstable();
        actual.sort_unstable();
        let mut tags = self
            .provider_tags
            .0
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        tags.sort_unstable();
        if self.src_token_blockchain != r.source.chain().alias().as_str()
            || self.dst_token_blockchain != r.destination.chain().alias().as_str()
            || asset_address(&self.src_token_address, &r.source)? != r.source.source_address()
            || asset_address(&self.dst_token_address, &r.destination)?
                != r.destination.source_address()
            || types::human(&self.src_token_amount, r.source.decimals())? != r.amount
            || self.slippage.exact()?
                != ExactDecimal::parse(&r.slippage.fraction()).map_err(|_| invalid())?
            || self.timeout != r.calculation_timeout
            || self.referrer != r.referrer.as_str()
            || expected != actual
            || tags != ["notDeposits", "notPrivate"]
            || self.enable_testnets.0
                != (r.source.chain().testnet() || r.destination.chain().testnet())
            || self.enable_checks.0
            || self.skip_fee_providers.0 != r.skip_fee_providers
            || self.show_dangerous_routes.0 != r.allow_dangerous_routes
            || self.show_failed_routes.0
            || self.use_deposit_trade_if_available == Some(true)
            || self.preferred_provider.as_deref()
                != r.preferred_provider.as_ref().map(Identifier::as_str)
            || r.integrator
                .is_some_and(|v| Some(v) != self.integrator_address)
            || self.id.as_deref() != id.map(Identifier::as_str)
        {
            return Err(invalid());
        }
        match (sender, self.from_address.as_deref()) {
            (Some(a), Some(v)) => {
                if account_address(v, a)? != a.source_address() {
                    return Err(invalid());
                }
            }
            (None, None) => {}
            _ => return Err(invalid()),
        }
        match (receiver, self.receiver.as_deref()) {
            (Some(a), Some(v)) => {
                if account_address(v, a)? != a.source_address() {
                    return Err(invalid());
                }
            }
            (None, None) => {}
            _ => return Err(invalid()),
        }
        Ok(())
    }
    fn foreign(&self) -> Result<Vec<ForeignFilters>, Error> {
        let Some(map) = &self.foreign_blacklist else {
            return Ok(Vec::new());
        };
        if map.len() > 16 {
            return Err(invalid());
        }
        map.iter()
            .map(|(p, v)| {
                ForeignFilters::new(
                    Identifier::new(p).map_err(|_| invalid())?,
                    v.0.iter()
                        .map(|s| Text::new(s).map_err(|_| invalid()))
                        .collect::<Result<_, _>>()?,
                )
                .map_err(|_| invalid())
            })
            .collect()
    }
}
fn asset_address(v: &str, asset: &Asset) -> Result<String, Error> {
    match asset.identifier() {
        AssetIdentifier::EvmToken(_) | AssetIdentifier::Native
            if matches!(asset.chain().family(), Family::Evm { .. }) =>
        {
            Address::parse(v)
                .map(|a| a.to_string())
                .map_err(|_| invalid())
        }
        AssetIdentifier::SolanaMint(_) => crate::domain::solana::Pubkey::parse(v)
            .map(|p| p.to_string())
            .map_err(|_| invalid()),
        _ => Ok(v.into()),
    }
}
fn account_address(v: &str, account: &Account) -> Result<String, Error> {
    match account {
        Account::Evm(_) => Address::parse(v)
            .map(|a| a.to_string())
            .map_err(|_| invalid()),
        Account::Solana(_) => crate::domain::solana::Pubkey::parse(v)
            .map(|p| p.to_string())
            .map_err(|_| invalid()),
        Account::Provider(_) => Ok(v.into()),
    }
}

#[derive(Deserialize)]
struct All {
    quote: Echo,
    routes: Bounded<QuoteWire, 128>,
    failed: Option<Bounded<IgnoredAny, 128>>,
}
pub(super) fn quote_all(
    bytes: &[u8],
    request: &QuoteRequest,
    c: &Catalogue,
) -> Result<Routes, Error> {
    let raw = document(bytes)?;
    let w: All = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    w.quote.matches(
        request,
        request.data().sender.as_ref(),
        request.data().receiver.as_ref(),
        None,
    )?;
    if w.failed.is_some_and(|v| !v.0.is_empty()) {
        return Err(invalid());
    }
    Routes::new(
        request.clone(),
        w.routes
            .0
            .into_iter()
            .map(|q| q.typed(request, c))
            .collect::<Result<_, _>>()?,
    )
    .map_err(|_| invalid())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Swap {
    quote: Echo,
    tokens: Option<Tokens>,
    #[serde(rename = "swapType")]
    kind: SwapKind,
    provider_type: String,
    estimate: EstimateWire,
    fees: FeesWire,
    routing: Bounded<LegWire, 128>,
    transaction: TransactionWire,
    warnings: Bounded<WarningWire, 128>,
    use_rubic_contract: bool,
    private: bool,
    unique_info: Option<BTreeMap<String, Box<serde_json::value::RawValue>>>,
}
pub(super) fn prepared(
    bytes: &[u8],
    request: &PreparationRequest,
    c: &Catalogue,
) -> Result<PreparedSwap, Error> {
    let raw = document(bytes)?;
    let w: Swap = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    let r = request.data();
    let original = r.quote.data();
    w.quote.matches(
        &original.request,
        Some(&r.sender),
        Some(&r.receiver),
        Some(&original.id),
    )?;
    if w.private || w.unique_info.as_ref().is_some_and(|v| v.len() > 32) {
        return Err(invalid());
    }
    w.transaction.direct()?;
    let (input, destination) = match w.tokens {
        Some(tokens) => (Some(tokens.from.typed(c)?), Some(tokens.to.typed(c)?)),
        None => (None, None),
    };
    let fresh = FreshQuote::new(FreshQuoteData {
        request: original.request.clone(),
        id: original.id.clone(),
        input,
        destination,
        provider: Identifier::new(&w.provider_type).map_err(|_| invalid())?,
        kind: w.kind,
        estimate: w
            .estimate
            .typed(original.request.data().destination.decimals())?,
        fees: w.fees.typed(c)?,
        legs: w
            .routing
            .0
            .into_iter()
            .map(|l| l.typed(c))
            .collect::<Result<_, _>>()?,
        warnings: w
            .warnings
            .0
            .into_iter()
            .map(WarningWire::typed)
            .collect::<Result<_, _>>()?,
        use_rubic_contract: w.use_rubic_contract,
        approval_address: w.transaction.approval_address,
        permit2_address: w.transaction.permit2_address,
    })
    .map_err(|_| invalid())?;
    let payload = w
        .transaction
        .payload(original.request.data().source.chain())?;
    let mut provider_ids = Vec::new();
    let mut additional_data = None;
    for (kind, value) in w.unique_info.unwrap_or_default() {
        if kind == "additionalData" {
            additional_data = Some(SourceAdditionalData::new(value.get()).map_err(|_| invalid())?);
        } else {
            let text: String = serde_json::from_str(value.get()).map_err(|_| invalid())?;
            provider_ids.push(SourceId {
                kind: Identifier::new(&kind).map_err(|_| invalid())?,
                value: Text::new(&text).map_err(|_| invalid())?,
            });
        }
    }
    PreparedSwap::new(PreparedSwapData {
        source_integrator: w.quote.integrator_address,
        source_foreign_filters: w.quote.foreign()?,
        source_provider_ids: provider_ids,
        source_additional_data: additional_data,
        request: request.clone(),
        fresh_quote: fresh,
        payload,
    })
    .map_err(|_| invalid())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Providers {
    cross_chain: Bounded<String, 256>,
    on_chain: Bounded<String, 256>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChainWire {
    name: String,
    #[serde(deserialize_with = "required_optional")]
    id: Option<Number>,
    #[serde(rename = "type")]
    kind: String,
    testnet: bool,
    proxy_available: bool,
    providers: Providers,
}
pub(super) fn chains(bytes: &[u8], include_testnets: bool, limit: u16) -> Result<Chains, Error> {
    let raw = document(bytes)?;
    let rows: Bounded<ChainWire, 512> = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    if rows.0.len() > usize::from(limit) || !include_testnets && rows.0.iter().any(|r| r.testnet) {
        return Err(invalid());
    }
    Chains::new(
        rows.0
            .into_iter()
            .map(|r| {
                Ok::<_, Error>(ChainInfo {
                    alias: Identifier::new(&r.name).map_err(|_| invalid())?,
                    provider_id: r.id.as_ref().map(Number::u64).transpose()?,
                    source_type: Identifier::new(&r.kind).map_err(|_| invalid())?,
                    testnet: r.testnet,
                    proxy_available: r.proxy_available,
                    cross_chain_providers: r
                        .providers
                        .cross_chain
                        .0
                        .into_iter()
                        .map(|s| Identifier::new(&s).map_err(|_| invalid()))
                        .collect::<Result<_, _>>()?,
                    on_chain_providers: r
                        .providers
                        .on_chain
                        .0
                        .into_iter()
                        .map(|s| Identifier::new(&s).map_err(|_| invalid()))
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<_, _>>()?,
    )
    .map_err(|_| invalid())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusWire {
    rubic_id: Option<String>,
    source_tx_hash: Option<String>,
    status: ProviderStatus,
    #[serde(deserialize_with = "required_optional")]
    destination_tx_hash: Option<String>,
    destination_network_chain_id: Option<Number>,
    destination_network_title: Option<Text>,
    to_amount: Option<String>,
    to_amount_wei: Option<String>,
    sub_status: Option<String>,
}
pub(super) fn status(bytes: &[u8], query: StatusQuery) -> Result<Status, Error> {
    let raw = document(bytes)?;
    let w: StatusWire = serde_json::from_str(raw.get()).map_err(|_| invalid())?;
    if w.rubic_id
        .as_deref()
        .is_some_and(|id| id != query.id().as_str())
        || w.source_tx_hash
            .as_deref()
            .map(|hash| transaction_id(hash, query.source()))
            .transpose()?
            .as_ref()
            .is_some_and(|hash| hash != query.source_transaction())
        || w.destination_network_chain_id
            .as_ref()
            .map(Number::u64)
            .transpose()?
            .is_some_and(|id| Some(id) != query.destination().provider_id())
    {
        return Err(invalid());
    }
    let _ = w.destination_network_title;
    let destination = w
        .destination_tx_hash
        .filter(|s| !s.is_empty())
        .map(|s| {
            DestinationTransaction::new(
                query.destination().clone(),
                transaction_id(&s, query.destination())?,
            )
            .map_err(|_| invalid())
        })
        .transpose()?;
    Status::new(
        query,
        w.status,
        destination,
        w.to_amount
            .as_deref()
            .map(|s| {
                crate::domain::market::NonnegativeDecimal::new(
                    ExactDecimal::parse(s).map_err(|_| invalid())?,
                )
                .map_err(|_| invalid())
            })
            .transpose()?,
        w.to_amount_wei
            .as_deref()
            .map(|s| types::raw(s, None))
            .transpose()?,
        w.sub_status
            .as_deref()
            .map(Text::new)
            .transpose()
            .map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())
}
fn transaction_id(v: &str, chain: &Chain) -> Result<TransactionId, Error> {
    match chain.family() {
        Family::Evm { .. } => crate::domain::evm::TransactionId::parse(v)
            .map(TransactionId::Evm)
            .map_err(|_| invalid()),
        Family::Solana { .. } => Signature::parse(v)
            .map(TransactionId::Solana)
            .map_err(|_| invalid()),
        Family::Provider { .. } => Text::new(v)
            .map(TransactionId::Provider)
            .map_err(|_| invalid()),
    }
}
