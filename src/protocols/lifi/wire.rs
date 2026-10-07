// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use crate::{
    domain::{
        Amount, ExactDecimal, Timestamp, U256,
        lifi::{
            AccessEntry, Account, Action, ActionData, Asset, ChainCatalogue, EncodedPayload,
            Estimate, EstimateData, EvmPayload, EvmPayloadData, Expiry, Family, Fee, FeeData,
            GasCost, GasCostData, Identifier, Limits, ObservedLeg, Origin, Payload,
            PayloadEncoding, Progress, RouteData, Slippage, Status, StatusData, StatusQuery, Step,
            StepData, StepKind, Text, Token, TokenData, ToolFailure, Transaction, TransactionData,
            TransactionId, TransferId, UnavailableRoutes,
        },
    },
    error::{Error, ProviderError},
};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub(super) const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
fn invalid() -> Error {
    Error::Provider(ProviderError::InvalidResponse)
}
fn fixed<T>(result: Result<T, Error>) -> Result<T, Error> {
    result.map_err(|_| invalid())
}

// Raw lexical decoding avoids float rounding and serde_json's private-number
// object spoof. Duplicate keys are rejected even in private continuation fields.
pub(super) fn document(bytes: &[u8]) -> Result<Box<RawValue>, Error> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::Provider(ProviderError::ResponseTooLarge));
    }
    let raw: Box<RawValue> = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let mut nodes = 0;
    unique(&raw, 0, &mut nodes)?;
    Ok(raw)
}
fn unique(raw: &RawValue, depth: u8, nodes: &mut usize) -> Result<(), Error> {
    *nodes += 1;
    if depth > 32 || *nodes > 100_000 {
        return Err(invalid());
    }
    let input = raw.get().trim_start();
    if input.starts_with('{') {
        let mut d = serde_json::Deserializer::from_str(input);
        serde::Deserializer::deserialize_map(&mut d, ObjectCheck { depth, nodes })
            .map_err(|_| invalid())?;
    } else if input.starts_with('[') {
        let mut d = serde_json::Deserializer::from_str(input);
        serde::Deserializer::deserialize_seq(&mut d, ArrayCheck { depth, nodes })
            .map_err(|_| invalid())?;
    }
    Ok(())
}
struct ObjectCheck<'a> {
    depth: u8,
    nodes: &'a mut usize,
}
impl<'de> Visitor<'de> for ObjectCheck<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unique bounded source object")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if keys.len() >= 4096 || key.len() > 4096 || !keys.insert(key) {
                return Err(serde::de::Error::custom("invalid source object"));
            }
            let raw = map.next_value::<Box<RawValue>>()?;
            unique(&raw, self.depth + 1, self.nodes).map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}
struct ArrayCheck<'a> {
    depth: u8,
    nodes: &'a mut usize,
}
impl<'de> Visitor<'de> for ArrayCheck<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded source array")
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
        let mut count = 0;
        while let Some(raw) = seq.next_element::<Box<RawValue>>()? {
            count += 1;
            if count > 4096 {
                return Err(serde::de::Error::custom("invalid source array"));
            }
            unique(&raw, self.depth + 1, self.nodes).map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}
struct Bounded<T, const N: usize>(Vec<T>);
impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for Bounded<T, N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Items<T, const N: usize>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for Items<T, N> {
            type Value = Bounded<T, N>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded source list")
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self::Value, S::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<T>()? {
                    if items.len() == N {
                        return Err(serde::de::Error::custom("source list too large"));
                    }
                    items.push(item);
                }
                Ok(Bounded(items))
            }
        }
        d.deserialize_seq(Items::<T, N>(std::marker::PhantomData))
    }
}
fn decode<T: serde::de::DeserializeOwned>(raw: &RawValue) -> Result<T, Error> {
    serde_json::from_str(raw.get()).map_err(|_| invalid())
}
fn decimal(text: &str) -> Result<ExactDecimal, Error> {
    fixed(ExactDecimal::parse(text))
}
fn number(raw: &RawValue) -> Result<ExactDecimal, Error> {
    let text = raw.get();
    if matches!(text.as_bytes().first(), Some(b'"' | b'{' | b'['))
        || matches!(text, "null" | "true" | "false")
    {
        return Err(invalid());
    }
    decimal(text)
}
fn amount(text: &str, decimals: Option<u8>) -> Result<Amount, Error> {
    fixed(Amount::from_decimal(text, decimals))
}
fn uint(raw: &RawValue) -> Result<Amount, Error> {
    let text = if raw.get().starts_with('"') {
        decode::<String>(raw)?
    } else {
        raw.get().to_owned()
    };
    if let Some(hex) = text.strip_prefix("0x") {
        if hex.is_empty() || hex.len() > 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        Ok(Amount::new(
            U256::from_str_radix(hex, 16).map_err(|_| invalid())?,
            None,
        ))
    } else {
        amount(&text, None)
    }
}
fn optional_amount(text: Option<String>, decimals: Option<u8>) -> Result<Option<Amount>, Error> {
    text.map(|v| amount(&v, decimals)).transpose()
}
fn optional_decimal(text: Option<String>) -> Result<Option<ExactDecimal>, Error> {
    text.map(|v| decimal(&v)).transpose()
}
fn id(text: &str) -> Result<Identifier, Error> {
    fixed(Identifier::new(text))
}
fn label(text: &str) -> Result<Text, Error> {
    fixed(Text::new(text))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireToken {
    address: String,
    chain_id: u64,
    symbol: String,
    decimals: u8,
    name: Option<String>,
    #[serde(rename = "priceUSD")]
    price_usd: Option<String>,
}
impl WireToken {
    fn map(self, catalogue: &ChainCatalogue) -> Result<Token, Error> {
        fixed(Token::new(TokenData {
            asset: fixed(Asset::new(catalogue.resolve(self.chain_id)?, &self.address))?,
            decimals: self.decimals,
            symbol: label(&self.symbol)?,
            name: self.name.map(|v| label(&v)).transpose()?,
            price_usd: optional_decimal(self.price_usd)?,
        }))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAction {
    from_chain_id: u64,
    to_chain_id: u64,
    from_token: WireToken,
    to_token: WireToken,
    from_amount: String,
    from_address: Option<String>,
    to_address: Option<String>,
    slippage: Option<Box<RawValue>>,
}
impl WireAction {
    fn map(self, catalogue: &ChainCatalogue) -> Result<Action, Error> {
        let from = catalogue.resolve(self.from_chain_id)?;
        let to = catalogue.resolve(self.to_chain_id)?;
        let from_token = self.from_token.map(catalogue)?;
        let to_token = self.to_token.map(catalogue)?;
        if from_token.data().asset.chain() != from || to_token.data().asset.chain() != to {
            return Err(invalid());
        }
        fixed(Action::new(ActionData {
            from_amount: amount(&self.from_amount, Some(from_token.data().decimals))?,
            from_token,
            to_token,
            from_account: self
                .from_address
                .map(|v| fixed(Account::new(from, &v)))
                .transpose()?,
            to_account: self
                .to_address
                .map(|v| fixed(Account::new(to, &v)))
                .transpose()?,
            slippage: self
                .slippage
                .map(|v| fixed(Slippage::new(number(&v)?)))
                .transpose()?,
        }))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireFee {
    name: String,
    description: String,
    token: WireToken,
    amount: String,
    #[serde(rename = "amountUSD")]
    amount_usd: String,
    percentage: String,
    included: bool,
}
impl WireFee {
    fn map(self, catalogue: &ChainCatalogue) -> Result<Fee, Error> {
        let token = self.token.map(catalogue)?;
        fixed(Fee::new(FeeData {
            name: label(&self.name)?,
            description: label(&self.description)?,
            amount: amount(&self.amount, Some(token.data().decimals))?,
            token,
            amount_usd: decimal(&self.amount_usd)?,
            percentage: decimal(&self.percentage)?,
            included: self.included,
        }))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireGas {
    #[serde(rename = "type")]
    kind: String,
    price: String,
    estimate: String,
    limit: String,
    token: WireToken,
    amount: String,
    #[serde(rename = "amountUSD")]
    amount_usd: String,
}
impl WireGas {
    fn map(self, catalogue: &ChainCatalogue) -> Result<GasCost, Error> {
        let token = self.token.map(catalogue)?;
        fixed(GasCost::new(GasCostData {
            kind: id(&self.kind)?,
            price: decimal(&self.price)?,
            estimate: amount(&self.estimate, None)?,
            limit: amount(&self.limit, None)?,
            amount: amount(&self.amount, Some(token.data().decimals))?,
            token,
            amount_usd: decimal(&self.amount_usd)?,
        }))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireEstimate {
    tool: String,
    from_amount: String,
    to_amount: String,
    to_amount_min: String,
    #[serde(rename = "fromAmountUSD")]
    from_amount_usd: Option<String>,
    #[serde(rename = "toAmountUSD")]
    to_amount_usd: Option<String>,
    approval_address: Option<String>,
    fee_costs: Option<Bounded<WireFee, 128>>,
    gas_costs: Option<Bounded<WireGas, 128>>,
    execution_duration: Box<RawValue>,
    skip_approval: Option<bool>,
}
impl WireEstimate {
    fn map(self, catalogue: &ChainCatalogue, action: &Action) -> Result<Estimate, Error> {
        let a = action.data();
        fixed(Estimate::new(EstimateData {
            tool: id(&self.tool)?,
            from_amount: amount(&self.from_amount, Some(a.from_token.data().decimals))?,
            to_amount: amount(&self.to_amount, Some(a.to_token.data().decimals))?,
            to_amount_min: amount(&self.to_amount_min, Some(a.to_token.data().decimals))?,
            from_amount_usd: optional_decimal(self.from_amount_usd)?,
            to_amount_usd: optional_decimal(self.to_amount_usd)?,
            approval_account: self
                .approval_address
                .filter(|v| !v.is_empty())
                .map(|v| fixed(Account::new(a.from_token.data().asset.chain(), &v)))
                .transpose()?,
            fees: self
                .fee_costs
                .map(|v| v.0.into_iter().map(|f| f.map(catalogue)).collect())
                .transpose()?,
            gas_costs: self
                .gas_costs
                .map(|v| v.0.into_iter().map(|g| g.map(catalogue)).collect())
                .transpose()?,
            execution_seconds: number(&self.execution_duration)?,
            skip_approval: self.skip_approval,
            expiry: Expiry::Unreported,
        }))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireTransactionRequest {
    to: Option<String>,
    from: Option<String>,
    chain_id: Option<u64>,
    data: Option<String>,
    value: Option<Box<RawValue>>,
    gas_limit: Option<Box<RawValue>>,
    gas_price: Option<Box<RawValue>>,
    max_priority_fee_per_gas: Option<Box<RawValue>>,
    max_fee_per_gas: Option<Box<RawValue>>,
    nonce: Option<u64>,
    #[serde(rename = "type")]
    kind: Option<u8>,
    access_list: Option<Bounded<WireAccess, 1024>>,
    custom_data: Option<WireCustom>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAccess {
    address: String,
    storage_keys: Bounded<String, 4096>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCustom {
    contract_type: Option<String>,
}
impl WireTransactionRequest {
    fn map(self, catalogue: &ChainCatalogue, action: &Action) -> Result<Payload, Error> {
        let chain = action.data().from_token.data().asset.chain();
        if let Some(id) = self.chain_id
            && catalogue.resolve(id)? != chain
        {
            return Err(invalid());
        }
        let text = self.data.ok_or_else(invalid)?;
        if chain.family() == Family::Evm {
            if self.chain_id.is_none() || self.custom_data.is_some() {
                return Err(invalid());
            }
            let to = fixed(Account::new(chain, &self.to.ok_or_else(invalid)?))?;
            let data = fixed(EncodedPayload::new(
                chain,
                PayloadEncoding::EvmCallData,
                &text,
                None,
            ))?;
            let access_list = self
                .access_list
                .map(|entries| {
                    entries
                        .0
                        .into_iter()
                        .map(|e| {
                            Ok(AccessEntry {
                                account: fixed(Account::new(chain, &e.address))?,
                                storage_keys: e
                                    .storage_keys
                                    .0
                                    .into_iter()
                                    .map(|v| fixed(crate::domain::BlockHash::parse(&v)))
                                    .collect::<Result<_, _>>()?,
                            })
                        })
                        .collect::<Result<_, Error>>()
                })
                .transpose()?;
            return Ok(Payload::Evm(Box::new(fixed(EvmPayload::new(
                EvmPayloadData {
                    chain,
                    from: self
                        .from
                        .map(|v| fixed(Account::new(chain, &v)))
                        .transpose()?,
                    to,
                    value: uint(&self.value.ok_or_else(invalid)?)?,
                    data,
                    gas_limit: self.gas_limit.map(|v| uint(&v)).transpose()?,
                    gas_price: self.gas_price.map(|v| uint(&v)).transpose()?,
                    max_priority_fee_per_gas: self
                        .max_priority_fee_per_gas
                        .map(|v| uint(&v))
                        .transpose()?,
                    max_fee_per_gas: self.max_fee_per_gas.map(|v| uint(&v)).transpose()?,
                    nonce: self.nonce,
                    transaction_type: self.kind,
                    access_list,
                },
            ))?)));
        }
        let encoding = match chain.family() {
            Family::Solana => PayloadEncoding::SolanaBase64,
            Family::Utxo if chain.id() == 20_000_000_000_001 => PayloadEncoding::BitcoinPsbtHex,
            Family::Utxo => PayloadEncoding::UtxoHex,
            Family::Move => PayloadEncoding::MoveSdkText,
            Family::Tron => PayloadEncoding::TronProtobufHex,
            Family::Stellar => PayloadEncoding::StellarXdrBase64,
            Family::Evm => return Err(invalid()),
        };
        let contract_type = self
            .custom_data
            .and_then(|c| c.contract_type)
            .map(|v| id(&v))
            .transpose()?;
        Ok(Payload::Encoded(fixed(EncodedPayload::new(
            chain,
            encoding,
            &text,
            contract_type,
        ))?))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireStep {
    id: String,
    #[serde(rename = "type")]
    kind: StepKind,
    tool: String,
    action: WireAction,
    estimate: Option<WireEstimate>,
    #[serde(default)]
    included_steps: Option<Bounded<Box<RawValue>, 256>>,
    transaction_request: Option<WireTransactionRequest>,
    transaction_id: Option<String>,
}
pub(super) fn step(
    raw: &RawValue,
    catalogue: &ChainCatalogue,
    limits: Limits,
) -> Result<Step, Error> {
    let w: WireStep = decode(raw)?;
    let action = w.action.map(catalogue)?;
    let estimate = w.estimate.map(|v| v.map(catalogue, &action)).transpose()?;
    let payload = w
        .transaction_request
        .map(|v| v.map(catalogue, &action))
        .transpose()?;
    let children = w.included_steps.map_or_else(Vec::new, |v| v.0);
    let value = fixed(Step::new(StepData {
        id: id(&w.id)?,
        kind: w.kind,
        tool: id(&w.tool)?,
        action,
        estimate,
        included_steps: children
            .iter()
            .map(|v| step(v, catalogue, limits))
            .collect::<Result<_, _>>()?,
        payload,
        transfer_id: w
            .transaction_id
            .map(|v| fixed(TransferId::new(&v)))
            .transpose()?,
    }))?;
    fixed(value.check_limits(limits))?;
    Ok(value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRoutes {
    routes: Bounded<WireRoute, 64>,
    unavailable_routes: Option<WireUnavailable>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRoute {
    id: String,
    from_chain_id: u64,
    to_chain_id: u64,
    from_token: WireToken,
    to_token: WireToken,
    from_amount: String,
    to_amount: String,
    to_amount_min: String,
    #[serde(rename = "fromAmountUSD")]
    from_amount_usd: Option<String>,
    #[serde(rename = "toAmountUSD")]
    to_amount_usd: Option<String>,
    #[serde(rename = "gasCostUSD")]
    gas_cost_usd: Option<String>,
    contains_switch_chain: Option<bool>,
    steps: Bounded<Box<RawValue>, 256>,
    from_address: Option<String>,
    to_address: Option<String>,
}
type MappedRoutes = (
    Vec<(RouteData, Vec<Box<RawValue>>)>,
    Option<UnavailableRoutes>,
);
pub(super) fn routes(raw: &RawValue, catalogue: &ChainCatalogue) -> Result<MappedRoutes, Error> {
    let w: WireRoutes = decode(raw)?;
    let alternatives = w
        .routes
        .0
        .into_iter()
        .map(|r| {
            let from = r.from_token.map(catalogue)?;
            let to = r.to_token.map(catalogue)?;
            if from.data().asset.chain() != catalogue.resolve(r.from_chain_id)?
                || to.data().asset.chain() != catalogue.resolve(r.to_chain_id)?
            {
                return Err(invalid());
            }
            let from_account = r
                .from_address
                .map(|a| fixed(Account::new(from.data().asset.chain(), &a)))
                .transpose()?;
            let to_account = r
                .to_address
                .map(|a| fixed(Account::new(to.data().asset.chain(), &a)))
                .transpose()?;
            Ok((
                RouteData {
                    id: id(&r.id)?,
                    from_amount: amount(&r.from_amount, Some(from.data().decimals))?,
                    to_amount: amount(&r.to_amount, Some(to.data().decimals))?,
                    to_amount_min: amount(&r.to_amount_min, Some(to.data().decimals))?,
                    from,
                    to,
                    from_account,
                    to_account,
                    from_amount_usd: optional_decimal(r.from_amount_usd)?,
                    to_amount_usd: optional_decimal(r.to_amount_usd)?,
                    gas_cost_usd: optional_decimal(r.gas_cost_usd)?,
                    contains_switch_chain: r.contains_switch_chain,
                },
                r.steps.0,
            ))
        })
        .collect::<Result<_, Error>>()?;
    let unavailable = w.unavailable_routes.map(WireUnavailable::map).transpose()?;
    Ok((alternatives, unavailable))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireUnavailable {
    filtered_out: Bounded<WireFiltered, 4096>,
    failed: Bounded<WireFailed, 4096>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireFiltered {
    overall_path: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireFailed {
    overall_path: String,
    subpaths: BTreeMap<String, Bounded<WireFailure, 128>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireFailure {
    tool: String,
    error_type: String,
    code: String,
}
impl WireUnavailable {
    fn map(self) -> Result<UnavailableRoutes, Error> {
        let filtered_paths = self
            .filtered_out
            .0
            .into_iter()
            .map(|v| label(&v.overall_path))
            .collect::<Result<_, _>>()?;
        let mut failed_paths = Vec::new();
        let mut failures = Vec::new();
        for p in self.failed.0 {
            let overall_path = label(&p.overall_path)?;
            failed_paths.push(overall_path.clone());
            for (path, records) in p.subpaths {
                let subpath = label(&path)?;
                for r in records.0 {
                    if failures.len() == 4096 {
                        return Err(invalid());
                    }
                    failures.push(ToolFailure {
                        overall_path: overall_path.clone(),
                        subpath: subpath.clone(),
                        tool: id(&r.tool)?,
                        kind: id(&r.error_type)?,
                        code: id(&r.code)?,
                    });
                }
            }
        }
        Ok(UnavailableRoutes {
            filtered_paths,
            failed_paths,
            failures,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireStatus {
    status: String,
    substatus: Option<String>,
    tool: Option<String>,
    transaction_id: Option<String>,
    sending: Option<WireTransaction>,
    receiving: Option<WireTransaction>,
    fee_costs: Option<Bounded<WireFee, 128>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireTransaction {
    chain_id: u64,
    tx_hash: Option<String>,
    token: Option<WireToken>,
    amount: Option<String>,
    timestamp: Option<u64>,
    value: Option<String>,
    gas_price: Option<String>,
    gas_used: Option<String>,
    gas_token: Option<WireToken>,
    gas_amount: Option<String>,
    #[serde(rename = "gasAmountUSD")]
    gas_amount_usd: Option<String>,
    included_steps: Option<Bounded<WireObservedLeg, 256>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireObservedLeg {
    tool: String,
    from_token: WireToken,
    to_token: WireToken,
    from_amount: String,
    to_amount: String,
    bridged_amount: Option<String>,
}
impl WireObservedLeg {
    fn map(self, catalogue: &ChainCatalogue) -> Result<ObservedLeg, Error> {
        let from_token = self.from_token.map(catalogue)?;
        let to_token = self.to_token.map(catalogue)?;
        Ok(ObservedLeg {
            tool: id(&self.tool)?,
            from_amount: amount(&self.from_amount, Some(from_token.data().decimals))?,
            to_amount: amount(&self.to_amount, Some(to_token.data().decimals))?,
            bridged_amount: optional_amount(self.bridged_amount, Some(to_token.data().decimals))?,
            from_token,
            to_token,
        })
    }
}
impl WireTransaction {
    fn map(self, catalogue: &ChainCatalogue) -> Result<Transaction, Error> {
        let chain = catalogue.resolve(self.chain_id)?;
        let token = self.token.map(|v| v.map(catalogue)).transpose()?;
        let gas_token = self.gas_token.map(|v| v.map(catalogue)).transpose()?;
        fixed(Transaction::new(TransactionData {
            chain,
            id: self
                .tx_hash
                .map(|v| fixed(TransactionId::new(chain, &v)))
                .transpose()?,
            amount: optional_amount(self.amount, token.as_ref().map(|t| t.data().decimals))?,
            token,
            timestamp: self.timestamp.map(Timestamp::from_unix_seconds),
            value: optional_amount(self.value, None)?,
            gas_price: optional_decimal(self.gas_price)?,
            gas_used: optional_amount(self.gas_used, None)?,
            gas_amount: optional_amount(
                self.gas_amount,
                gas_token.as_ref().map(|t| t.data().decimals),
            )?,
            gas_token,
            gas_amount_usd: optional_decimal(self.gas_amount_usd)?,
            included_legs: self
                .included_steps
                .map(|v| v.0.into_iter().map(|l| l.map(catalogue)).collect())
                .transpose()?,
        }))
    }
}
pub(super) fn status(
    raw: &RawValue,
    catalogue: &ChainCatalogue,
    query: StatusQuery,
    origin: Origin,
    limits: Limits,
) -> Result<Status, Error> {
    let w: WireStatus = decode(raw)?;
    let progress = match w.status.as_str() {
        "NOT_FOUND" => Progress::NotFound,
        "INVALID" => Progress::Invalid,
        "PENDING" => Progress::Pending,
        "DONE" => Progress::Done,
        "FAILED" => Progress::Failed,
        _ => return Err(invalid()),
    };
    let fees = w
        .fee_costs
        .map(|v| {
            v.0.into_iter()
                .map(|f| f.map(catalogue))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    if fees
        .as_ref()
        .is_some_and(|f| f.len() > usize::from(limits.costs()))
    {
        return Err(invalid());
    }
    let sending = w.sending.map(|v| v.map(catalogue)).transpose()?;
    let receiving = w.receiving.map(|v| v.map(catalogue)).transpose()?;
    for tx in [&sending, &receiving].into_iter().flatten() {
        if tx
            .data()
            .included_legs
            .as_ref()
            .is_some_and(|l| l.len() > usize::from(limits.steps()))
        {
            return Err(invalid());
        }
    }
    fixed(Status::new(
        query,
        StatusData {
            progress,
            substatus: w.substatus.map(|v| id(&v)).transpose()?,
            tool: w
                .tool
                .filter(|v| !v.is_empty())
                .map(|v| id(&v))
                .transpose()?,
            transfer_id: w
                .transaction_id
                .map(|v| fixed(TransferId::new(&v)))
                .transpose()?,
            sending,
            receiving,
            fees,
        },
        origin,
    ))
}
