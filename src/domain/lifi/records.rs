// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Account, Asset, Chain, Identifier, Payload, Slippage, Text};
use crate::{
    domain::{Amount, ExactDecimal, Source, Timestamp},
    error::Error,
    wallets::Preparation,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;

fn nonnegative(value: &ExactDecimal) -> bool {
    !value.is_negative()
}
fn optional_nonnegative(value: Option<&ExactDecimal>) -> bool {
    value.is_none_or(nonnegative)
}
fn amount_for(amount: Amount, token: &Token) -> bool {
    amount.decimals() == Some(token.data().decimals)
}

/// An explicit caller capacity, checked without truncating source collections.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LimitsFields")]
pub struct Limits {
    routes: u16,
    steps: u16,
    costs: u16,
}
impl Limits {
    /// Allows at most 64 routes, 256 total steps per route and 128 costs per list.
    ///
    /// # Errors
    /// Rejects zero and exceeded global limits.
    pub fn new(routes: u16, steps: u16, costs: u16) -> Result<Self, Error> {
        if !(1..=64).contains(&routes) || !(1..=256).contains(&steps) || !(1..=128).contains(&costs)
        {
            return Err(super::invalid_record());
        }
        Ok(Self {
            routes,
            steps,
            costs,
        })
    }
    /// Returns maximum route alternatives.
    #[must_use]
    pub const fn routes(self) -> u16 {
        self.routes
    }
    /// Returns maximum total steps, including nested included steps, per route.
    #[must_use]
    pub const fn steps(self) -> u16 {
        self.steps
    }
    /// Returns maximum entries in each fee/gas-cost list.
    #[must_use]
    pub const fn costs(self) -> u16 {
        self.costs
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitsFields {
    routes: u16,
    steps: u16,
    costs: u16,
}
impl TryFrom<LimitsFields> for Limits {
    type Error = Error;
    fn try_from(v: LimitsFields) -> Result<Self, Error> {
        Self::new(v.routes, v.steps, v.costs)
    }
}

/// Explicit transfer request values; addresses and amounts are never inferred.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestData {
    /// Exact source token identity.
    pub from: Asset,
    /// Exact destination token identity.
    pub to: Asset,
    /// Positive source base units; optional caller precision is retained.
    pub amount: Amount,
    /// Explicit source account.
    pub from_account: Account,
    /// Explicit destination account, or an honestly unspecified source default.
    pub to_account: Option<Account>,
    /// Exact per-step slippage proportion.
    pub slippage: Slippage,
    /// Whether routes may refer to intermediate chains in the caller catalogue.
    pub allow_switch_chain: bool,
}
/// Immutable validated transfer inputs, shared by quote and routes operations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RequestData", into = "RequestData")]
pub struct Request(RequestData);
impl Request {
    /// Checks nonzero units and account-chain agreement.
    ///
    /// # Errors
    /// Rejects contradictory accounts or zero source amount.
    pub fn new(data: RequestData) -> Result<Self, Error> {
        if data.amount.raw().is_zero()
            || data.from_account.chain() != data.from.chain()
            || data
                .to_account
                .as_ref()
                .is_some_and(|a| a.chain() != data.to.chain())
        {
            return Err(super::invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns immutable original caller inputs.
    #[must_use]
    pub const fn data(&self) -> &RequestData {
        &self.0
    }
}
impl TryFrom<RequestData> for Request {
    type Error = Error;
    fn try_from(v: RequestData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Request> for RequestData {
    fn from(v: Request) -> Self {
        v.0
    }
}

/// Source token precision and display metadata, separate from identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenData {
    /// Qualified token identifier.
    pub asset: Asset,
    /// Provider-reported base-unit precision.
    pub decimals: u8,
    /// Provider display symbol; no symbol-to-identity resolution occurs.
    pub symbol: Text,
    /// Provider display name, if present.
    pub name: Option<Text>,
    /// Exact nonnegative USD reference price, if reported.
    pub price_usd: Option<ExactDecimal>,
}
/// Checked precision/display metadata for one exact provider token identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TokenData", into = "TokenData")]
pub struct Token(TokenData);
impl Token {
    /// Checks reported USD values without deriving a token identity from names.
    ///
    /// # Errors
    /// Rejects negative USD reference prices.
    pub fn new(data: TokenData) -> Result<Self, Error> {
        if !optional_nonnegative(data.price_usd.as_ref()) {
            return Err(super::invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns immutable identity, precision and source metadata.
    #[must_use]
    pub const fn data(&self) -> &TokenData {
        &self.0
    }
}
impl TryFrom<TokenData> for Token {
    type Error = Error;
    fn try_from(v: TokenData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Token> for TokenData {
    fn from(v: Token) -> Self {
        v.0
    }
}

/// Source-reported step action, distinct from the original transfer request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionData {
    /// Source token, including its actually reported precision.
    pub from_token: Token,
    /// Destination token, including its actually reported precision.
    pub to_token: Token,
    /// Source step units with the source token precision.
    pub from_amount: Amount,
    /// Sender if echoed by the source.
    pub from_account: Option<Account>,
    /// Destination if echoed by the source.
    pub to_account: Option<Account>,
    /// Slippage if actually reported, without an assumed default.
    pub slippage: Option<Slippage>,
}
/// Checked step asset, amount and optional address identities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ActionData", into = "ActionData")]
pub struct Action(ActionData);
impl Action {
    /// Checks source units and account qualification.
    ///
    /// # Errors
    /// Rejects incompatible precision and mismatched account chains.
    pub fn new(data: ActionData) -> Result<Self, Error> {
        if !amount_for(data.from_amount, &data.from_token)
            || data
                .from_account
                .as_ref()
                .is_some_and(|a| a.chain() != data.from_token.data().asset.chain())
            || data
                .to_account
                .as_ref()
                .is_some_and(|a| a.chain() != data.to_token.data().asset.chain())
        {
            return Err(super::invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns immutable source action data.
    #[must_use]
    pub const fn data(&self) -> &ActionData {
        &self.0
    }
    /// Checks echoed identity/units/settings against original caller inputs.
    /// Missing source addresses/slippage remain missing and are not fabricated.
    #[must_use]
    pub fn matches_request(&self, request: &Request) -> bool {
        let r = request.data();
        let a = &self.0;
        a.from_token.data().asset == r.from
            && a.to_token.data().asset == r.to
            && a.from_amount.raw() == r.amount.raw()
            && r.amount
                .decimals()
                .is_none_or(|d| d == a.from_token.data().decimals)
            && a.from_account.as_ref().is_none_or(|v| v == &r.from_account)
            && r.to_account.as_ref().is_none_or(|expected| {
                a.to_account
                    .as_ref()
                    .is_none_or(|actual| actual == expected)
            })
            && a.slippage.as_ref().is_none_or(|s| s == &r.slippage)
    }
    pub(super) fn same_intent(&self, other: &Self) -> bool {
        let a = &self.0;
        let b = &other.0;
        a.from_token.data().asset == b.from_token.data().asset
            && a.to_token.data().asset == b.to_token.data().asset
            && a.from_token.data().decimals == b.from_token.data().decimals
            && a.to_token.data().decimals == b.to_token.data().decimals
            && a.from_amount == b.from_amount
            && a.from_account == b.from_account
            && a.to_account == b.to_account
            && a.slippage == b.slippage
    }
}
impl TryFrom<ActionData> for Action {
    type Error = Error;
    fn try_from(v: ActionData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Action> for ActionData {
    fn from(v: Action) -> Self {
        v.0
    }
}

/// Exact source fee values. Amounts use the fee token's units, not source units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeeData {
    /// Source display name, not a unique fee identity.
    pub name: Text,
    /// Source display description.
    pub description: Text,
    /// Actual charged token and units.
    pub token: Token,
    /// Fee base units.
    pub amount: Amount,
    /// Exact source USD valuation.
    pub amount_usd: ExactDecimal,
    /// Exact source-declared fee proportion; no amount formula is inferred.
    pub percentage: ExactDecimal,
    /// Whether the provider includes this fee in the transfer amount.
    pub included: bool,
}
/// Checked exact fee units and nonnegative source values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FeeData", into = "FeeData")]
pub struct Fee(FeeData);
impl Fee {
    /// Checks precision and nonnegative source values.
    ///
    /// # Errors
    /// Rejects incompatible units or negative estimates.
    pub fn new(data: FeeData) -> Result<Self, Error> {
        if !amount_for(data.amount, &data.token)
            || !nonnegative(&data.amount_usd)
            || !nonnegative(&data.percentage)
        {
            return Err(super::invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns immutable fee data. Composite fees already include nested fees.
    #[must_use]
    pub const fn data(&self) -> &FeeData {
        &self.0
    }
}
impl TryFrom<FeeData> for Fee {
    type Error = Error;
    fn try_from(v: FeeData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Fee> for FeeData {
    fn from(v: Fee) -> Self {
        v.0
    }
}

/// Exact provider gas/transaction-cost estimate with its source token units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GasCostData {
    /// Source category such as SEND/APPROVE/SUM/FEE, retained without inference.
    pub kind: Identifier,
    /// Source's exact suggested unit price; it need not be an integer.
    pub price: ExactDecimal,
    /// Source's exact estimated units.
    pub estimate: Amount,
    /// Source's exact suggested limit.
    pub limit: Amount,
    /// Cost token identity and precision.
    pub token: Token,
    /// Source's exact estimated cost in the declared token.
    pub amount: Amount,
    /// Source's exact USD estimate.
    pub amount_usd: ExactDecimal,
}
/// Checked source gas-cost units. No cross-chain aggregate is invented.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "GasCostData", into = "GasCostData")]
pub struct GasCost(GasCostData);
impl GasCost {
    /// Checks exact units, precision and nonnegative estimates.
    ///
    /// # Errors
    /// Rejects incompatible precision or negative values.
    pub fn new(data: GasCostData) -> Result<Self, Error> {
        if data.estimate.decimals().is_some()
            || data.limit.decimals().is_some()
            || !amount_for(data.amount, &data.token)
            || !nonnegative(&data.price)
            || !nonnegative(&data.amount_usd)
        {
            return Err(super::invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns immutable source cost data.
    #[must_use]
    pub const fn data(&self) -> &GasCostData {
        &self.0
    }
}
impl TryFrom<GasCostData> for GasCost {
    type Error = Error;
    fn try_from(v: GasCostData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<GasCost> for GasCostData {
    fn from(v: GasCost) -> Self {
        v.0
    }
}

/// Actual source expiry metadata; no universal LI.FI quote TTL is assumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Expiry {
    /// No universal expiry was supplied. Payload-contained bounds are separate.
    Unreported,
    /// A backend explicitly reported an expiry in whole Unix seconds.
    SourceReported(Timestamp),
}
/// Provider estimate values, in their declared token and time units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EstimateData {
    /// Source tool identity.
    pub tool: Identifier,
    /// Exact source token units.
    pub from_amount: Amount,
    /// Exact expected destination units.
    pub to_amount: Amount,
    /// Source minimum expected destination units.
    pub to_amount_min: Amount,
    /// Source USD input valuation, if supplied.
    pub from_amount_usd: Option<ExactDecimal>,
    /// Source USD output valuation, if supplied.
    pub to_amount_usd: Option<ExactDecimal>,
    /// Source approval spender text, if supplied; chain-bound and not an approval.
    pub approval_account: Option<Account>,
    /// Missing and present-empty fee lists remain distinct.
    pub fees: Option<Vec<Fee>>,
    /// Missing and present-empty gas lists remain distinct.
    pub gas_costs: Option<Vec<GasCost>>,
    /// Exact estimated seconds; fractional durations are preserved.
    pub execution_seconds: ExactDecimal,
    /// Explicit source skip-approval suggestion, if present.
    pub skip_approval: Option<bool>,
    /// Universal expiry metadata, independent of payload contents.
    pub expiry: Expiry,
}
/// Immutable checked estimate; cross-field token agreement is checked by Step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EstimateData", into = "EstimateData")]
pub struct Estimate(EstimateData);
impl Estimate {
    /// Checks nonnegative values, minimum units and global cost capacities.
    ///
    /// # Errors
    /// Rejects impossible minima, contradictory precision or oversized lists.
    pub fn new(data: EstimateData) -> Result<Self, Error> {
        if data.to_amount_min.raw() > data.to_amount.raw()
            || data.to_amount.decimals() != data.to_amount_min.decimals()
            || !optional_nonnegative(data.from_amount_usd.as_ref())
            || !optional_nonnegative(data.to_amount_usd.as_ref())
            || !nonnegative(&data.execution_seconds)
            || data.fees.as_ref().is_some_and(|v| v.len() > 128)
            || data.gas_costs.as_ref().is_some_and(|v| v.len() > 128)
        {
            return Err(super::invalid_record());
        }
        Ok(Self(data))
    }
    /// Returns immutable exact source values; nested costs must not be added again.
    #[must_use]
    pub const fn data(&self) -> &EstimateData {
        &self.0
    }
}
impl TryFrom<EstimateData> for Estimate {
    type Error = Error;
    fn try_from(v: EstimateData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Estimate> for EstimateData {
    fn from(v: Estimate) -> Self {
        v.0
    }
}

/// Current provider step kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// Composite executable provider step.
    Lifi,
    /// One swap leg.
    Swap,
    /// One bridge leg.
    Cross,
    /// One protocol leg such as fee collection.
    Protocol,
    /// Source custom leg; encoded content remains opaque.
    Custom,
}
/// Typed provider step with source estimate and optional encoded preparation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepData {
    /// Exact provider step ID, distinct from transaction hashes.
    pub id: Identifier,
    /// Source step category.
    pub kind: StepKind,
    /// Exact provider tool key.
    pub tool: Identifier,
    /// Typed action values; display metadata does not define identity.
    pub action: Action,
    /// An actual estimate, or explicitly unavailable.
    pub estimate: Option<Estimate>,
    /// Bounded included legs, preserved individually without summing costs.
    pub included_steps: Vec<Step>,
    /// Actual source payload, or explicitly absent.
    pub payload: Option<Payload>,
    /// Provider transfer ID, distinct from this step ID and chain tx hashes.
    pub transfer_id: Option<super::TransferId>,
}
/// Checked immutable step. JSON construction repeats all cross-field validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StepData", into = "StepData")]
pub struct Step(StepData);
impl Step {
    /// Validates identities, precision, optional payload and bounded nested legs.
    ///
    /// # Errors
    /// Rejects duplicate step IDs, depth above eight or total steps above 256.
    pub fn new(data: StepData) -> Result<Self, Error> {
        let v = Self(data);
        let mut ids = BTreeSet::new();
        v.validate_tree(0, &mut ids)?;
        Ok(v)
    }
    fn validate_tree(&self, depth: usize, ids: &mut BTreeSet<Identifier>) -> Result<(), Error> {
        let d = &self.0;
        let a = d.action.data();
        if depth > 8 || !ids.insert(d.id.clone()) || ids.len() > 256 {
            return Err(super::invalid_record());
        }
        if let Some(e) = &d.estimate {
            let e = e.data();
            if e.tool != d.tool
                || e.from_amount != a.from_amount
                || !amount_for(e.to_amount, &a.to_token)
                || !amount_for(e.to_amount_min, &a.to_token)
                || e.approval_account
                    .as_ref()
                    .is_some_and(|p| p.chain() != a.from_token.data().asset.chain())
            {
                return Err(super::invalid_record());
            }
        }
        if let Some(payload) = &d.payload {
            if payload.chain() != a.from_token.data().asset.chain() {
                return Err(super::invalid_payload());
            }
            if let Payload::Evm(p) = payload
                && let Some(from) = &p.data().from
                && a.from_account
                    .as_ref()
                    .is_some_and(|expected| expected != from)
            {
                return Err(super::invalid_payload());
            }
        }
        for child in &d.included_steps {
            child.validate_tree(depth + 1, ids)?;
        }
        Ok(())
    }
    /// Returns immutable source step values, without provider continuation bytes.
    #[must_use]
    pub const fn data(&self) -> &StepData {
        &self.0
    }
    /// Checks caller-specific nested-step and cost capacities without truncation.
    ///
    /// # Errors
    /// Rejects any exceeded capacity.
    pub fn check_limits(&self, limits: Limits) -> Result<usize, Error> {
        let mut count = 1;
        if let Some(e) = &self.0.estimate
            && (e
                .data()
                .fees
                .as_ref()
                .is_some_and(|v| v.len() > usize::from(limits.costs))
                || e.data()
                    .gas_costs
                    .as_ref()
                    .is_some_and(|v| v.len() > usize::from(limits.costs)))
        {
            return Err(super::invalid_record());
        }
        for child in &self.0.included_steps {
            count += child.check_limits(limits)?;
        }
        if count > usize::from(limits.steps) {
            return Err(super::invalid_record());
        }
        Ok(count)
    }
}
impl TryFrom<StepData> for Step {
    type Error = Error;
    fn try_from(v: StepData) -> Result<Self, Error> {
        Self::new(v)
    }
}
impl From<Step> for StepData {
    fn from(v: Step) -> Self {
        v.0
    }
}

/// Attributed source retrieval, without an invented common block or finality.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    /// Explicit provider/method/version labels, without endpoint or credentials.
    pub source: Source,
    /// Whole Unix seconds after source retrieval, distinct from data timestamps.
    pub retrieved_at: Timestamp,
}
/// Pure extension contract for an immutable backend-specific continuation handle.
/// Implementations expose typed source facts, not arbitrary provider JSON.
pub trait StepView: Clone + Send + Sync {
    /// Returns the immutable selected typed step.
    fn step(&self) -> &Step;
    /// Returns immutable original whole-transfer caller inputs.
    fn request(&self) -> &Request;
    /// Returns attribution for the response that created the handle.
    fn origin(&self) -> &Origin;
}

/// Exact route endpoint identities and estimates, separate from executable steps.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteData {
    /// Exact provider alternative ID.
    pub id: Identifier,
    /// Source token identity.
    pub from: Token,
    /// Source sender if actually reported at route level.
    pub from_account: Option<Account>,
    /// Source units.
    pub from_amount: Amount,
    /// Destination token identity.
    pub to: Token,
    /// Destination account if actually reported at route level.
    pub to_account: Option<Account>,
    /// Expected destination units.
    pub to_amount: Amount,
    /// Minimum expected destination units.
    pub to_amount_min: Amount,
    /// Source USD valuation, if supplied.
    pub from_amount_usd: Option<ExactDecimal>,
    /// Destination USD valuation, if supplied.
    pub to_amount_usd: Option<ExactDecimal>,
    /// Source-reported gas USD total, if supplied; no manual aggregation occurs.
    pub gas_cost_usd: Option<ExactDecimal>,
    /// Explicit chain-switch indication, if supplied.
    pub contains_switch_chain: Option<bool>,
}
/// One route alternative with immutable backend continuation handles.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Route<H> {
    data: RouteData,
    steps: Vec<H>,
}
impl<H: StepView> Route<H> {
    /// Checks units and step endpoint identities; no atomic snapshot is inferred.
    ///
    /// # Errors
    /// Rejects empty/oversized step lists and contradictory endpoint identities.
    pub fn new(data: RouteData, steps: Vec<H>) -> Result<Self, Error> {
        if steps.is_empty()
            || steps.len() > 256
            || !amount_for(data.from_amount, &data.from)
            || !amount_for(data.to_amount, &data.to)
            || !amount_for(data.to_amount_min, &data.to)
            || data.to_amount_min.raw() > data.to_amount.raw()
            || !optional_nonnegative(data.from_amount_usd.as_ref())
            || !optional_nonnegative(data.to_amount_usd.as_ref())
            || !optional_nonnegative(data.gas_cost_usd.as_ref())
            || data
                .from_account
                .as_ref()
                .is_some_and(|a| a.chain() != data.from.data().asset.chain())
            || data
                .to_account
                .as_ref()
                .is_some_and(|a| a.chain() != data.to.data().asset.chain())
        {
            return Err(super::invalid_record());
        }
        let first = steps
            .first()
            .ok_or_else(super::invalid_record)?
            .step()
            .data()
            .action
            .data();
        let last = steps
            .last()
            .ok_or_else(super::invalid_record)?
            .step()
            .data()
            .action
            .data();
        if first.from_token.data().asset != data.from.data().asset
            || first.from_token.data().decimals != data.from.data().decimals
            || first.from_amount != data.from_amount
            || last.to_token.data().asset != data.to.data().asset
            || last.to_token.data().decimals != data.to.data().decimals
        {
            return Err(super::invalid_record());
        }
        let mut ids = BTreeSet::new();
        for h in &steps {
            h.step().validate_tree(0, &mut ids)?;
        }
        for pair in steps.windows(2) {
            if pair[0].step().data().action.data().to_token.data().asset
                != pair[1].step().data().action.data().from_token.data().asset
                || pair[0].step().data().action.data().to_token.data().decimals
                    != pair[1]
                        .step()
                        .data()
                        .action
                        .data()
                        .from_token
                        .data()
                        .decimals
            {
                return Err(super::invalid_record());
            }
        }
        Ok(Self { data, steps })
    }
    /// Returns immutable route estimates and endpoint identities.
    #[must_use]
    pub const fn data(&self) -> &RouteData {
        &self.data
    }
    /// Returns every selected-provider executable step in supplied order.
    #[must_use]
    pub fn steps(&self) -> &[H] {
        &self.steps
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteFields<H> {
    data: RouteData,
    steps: Vec<H>,
}
impl<'de, H: Deserialize<'de> + StepView> Deserialize<'de> for Route<H> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = RouteFields::<H>::deserialize(d)?;
        Self::new(v.data, v.steps).map_err(serde::de::Error::custom)
    }
}

/// Bounded route alternatives and explicit unavailable tool IDs.
/// A source tool failure omits arbitrary provider messages deliberately.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFailure {
    /// Exact source overall path display text, not an executable identity.
    pub overall_path: Text,
    /// Exact source subpath display text.
    pub subpath: Text,
    /// Tool key reported by the provider.
    pub tool: Identifier,
    /// Source category such as `NO_QUOTE`.
    pub kind: Identifier,
    /// Source machine-readable error code.
    pub code: Identifier,
}

/// Source-reported unavailable path metadata, without private free-form errors.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableRoutes {
    /// Every filtered path in supplied order; reasons are intentionally omitted.
    pub filtered_paths: Vec<Text>,
    /// Every failed overall path, including paths with an empty subpath map.
    pub failed_paths: Vec<Text>,
    /// Every tool failure, retaining exact path/category/code distinctions.
    pub failures: Vec<ToolFailure>,
}

/// Bounded route alternatives and source-reported unavailable path metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Routes<H> {
    request: Request,
    origin: Origin,
    alternatives: Vec<Route<H>>,
    unavailable: Option<UnavailableRoutes>,
    limits: Limits,
}
impl<H: StepView> Routes<H> {
    /// Checks original request correlation, attribution and every caller capacity.
    /// Empty alternatives remain empty rather than an invented successful route.
    ///
    /// # Errors
    /// Rejects duplicate route IDs, mismatched handles or exceeded limits.
    pub fn new(
        request: Request,
        origin: Origin,
        routes: Vec<Route<H>>,
        unavailable: Option<UnavailableRoutes>,
        limits: Limits,
    ) -> Result<Self, Error> {
        if routes.len() > usize::from(limits.routes)
            || unavailable.as_ref().is_some_and(|u| {
                u.filtered_paths.len() > 4096
                    || u.failed_paths.len() > 4096
                    || u.failures.len() > 4096
            })
        {
            return Err(super::invalid_record());
        }
        let mut ids = BTreeSet::new();
        for route in &routes {
            let d = route.data();
            let r = request.data();
            if !ids.insert(d.id.clone())
                || d.from.data().asset != r.from
                || d.to.data().asset != r.to
                || d.from_amount.raw() != r.amount.raw()
                || r.amount
                    .decimals()
                    .is_some_and(|p| p != d.from.data().decimals)
                || d.from_account
                    .as_ref()
                    .is_some_and(|a| a != &r.from_account)
                || r.to_account.as_ref().is_some_and(|expected| {
                    d.to_account
                        .as_ref()
                        .is_some_and(|actual| actual != expected)
                })
                || (!r.allow_switch_chain && d.contains_switch_chain == Some(true))
            {
                return Err(super::invalid_record());
            }
            let mut count = 0;
            for h in route.steps() {
                if h.request() != &request || h.origin() != &origin {
                    return Err(super::invalid_record());
                }
                count += h.step().check_limits(limits)?;
            }
            if count > usize::from(limits.steps) {
                return Err(super::invalid_record());
            }
            let first = route
                .steps()
                .first()
                .ok_or_else(super::invalid_record)?
                .step()
                .data()
                .action
                .data();
            let last = route
                .steps()
                .last()
                .ok_or_else(super::invalid_record)?
                .step()
                .data()
                .action
                .data();
            if first
                .from_account
                .as_ref()
                .is_some_and(|actual| actual != &r.from_account)
                || r.to_account.as_ref().is_some_and(|expected| {
                    last.to_account
                        .as_ref()
                        .is_some_and(|actual| actual != expected)
                })
                || route.steps().iter().any(|h| {
                    h.step()
                        .data()
                        .action
                        .data()
                        .slippage
                        .as_ref()
                        .is_some_and(|s| s != &r.slippage)
                })
            {
                return Err(super::invalid_record());
            }
        }
        Ok(Self {
            request,
            origin,
            alternatives: routes,
            unavailable,
            limits,
        })
    }
    /// Returns immutable original request values.
    #[must_use]
    pub const fn request(&self) -> &Request {
        &self.request
    }
    /// Returns actual source retrieval attribution.
    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }
    /// Returns all bounded route alternatives, without implicit selection.
    #[must_use]
    pub fn routes(&self) -> &[Route<H>] {
        &self.alternatives
    }
    /// Returns source-reported unavailable tool identifiers, without private messages.
    #[must_use]
    pub const fn unavailable(&self) -> Option<&UnavailableRoutes> {
        self.unavailable.as_ref()
    }
    /// Returns capacities that were checked when accepting this response.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }
}

#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "H: Deserialize<'de> + StepView")
)]
struct RoutesFields<H> {
    request: Request,
    origin: Origin,
    alternatives: Vec<Route<H>>,
    unavailable: Option<UnavailableRoutes>,
    limits: Limits,
}
impl<'de, H: Deserialize<'de> + StepView> Deserialize<'de> for Routes<H> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = RoutesFields::<H>::deserialize(d)?;
        Self::new(v.request, v.origin, v.alternatives, v.unavailable, v.limits)
            .map_err(serde::de::Error::custom)
    }
}

/// A freshly prepared source payload alongside the immutable selected snapshot.
/// A changed estimate/payload requires fresh caller review. No submission occurs
/// and metadata agreement does not independently verify encoded transfer intent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PreparedFields")]
pub struct PreparedStep {
    original_request: Request,
    selected_step: Step,
    selected_origin: Origin,
    refreshed_step: Step,
    origin: Origin,
    network: (Chain, Chain),
    #[serde(skip_serializing)]
    payload: Payload,
}
impl PreparedStep {
    /// Correlates IDs, tool, action identities/units/settings and actual payload.
    ///
    /// # Errors
    /// Rejects rewritten selected intent or an absent source preparation payload.
    pub fn new<H: StepView>(
        selected: &H,
        refreshed_step: Step,
        origin: Origin,
    ) -> Result<Self, Error> {
        Self::from_fields(
            selected.request().clone(),
            selected.step().clone(),
            selected.origin().clone(),
            refreshed_step,
            origin,
        )
    }
    fn from_fields(
        original_request: Request,
        selected_step: Step,
        selected_origin: Origin,
        refreshed_step: Step,
        origin: Origin,
    ) -> Result<Self, Error> {
        let a = selected_step.data();
        let b = refreshed_step.data();
        if a.id != b.id
            || a.tool != b.tool
            || a.kind != b.kind
            || !a.action.same_intent(&b.action)
            || b.payload.is_none()
            || b.estimate.is_none()
        {
            return Err(super::invalid_record());
        }
        let payload = b
            .payload
            .as_ref()
            .ok_or_else(super::invalid_payload)?
            .clone();
        let network = (
            b.action.data().from_token.data().asset.chain(),
            b.action.data().to_token.data().asset.chain(),
        );
        Ok(Self {
            original_request,
            selected_step,
            selected_origin,
            refreshed_step,
            origin,
            network,
            payload,
        })
    }
    /// Returns original whole-transfer intent, even when preparing a later route step.
    #[must_use]
    pub const fn original_request(&self) -> &Request {
        &self.original_request
    }
    /// Returns the exact previously selected typed step and its old estimate.
    #[must_use]
    pub const fn selected_step(&self) -> &Step {
        &self.selected_step
    }
    /// Returns source attribution of the selected snapshot.
    #[must_use]
    pub const fn selected_origin(&self) -> &Origin {
        &self.selected_origin
    }
    /// Returns the fresh source step, estimate and payload for new caller review.
    #[must_use]
    pub const fn refreshed_step(&self) -> &Step {
        &self.refreshed_step
    }
    /// Returns attribution for this preparation response.
    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }
    /// Returns the actual source payload; this carries no signature/intent proof.
    #[must_use]
    pub const fn payload(&self) -> &Payload {
        &self.payload
    }
}
impl Preparation for PreparedStep {
    type Network = (Chain, Chain);
    type Intent = Step;
    type UnsignedPayload = Payload;
    fn network(&self) -> &Self::Network {
        &self.network
    }
    fn intent(&self) -> &Self::Intent {
        &self.refreshed_step
    }
    fn unsigned_payload(&self) -> &Self::UnsignedPayload {
        self.payload()
    }
    fn validate(&self) -> Result<(), Error> {
        Self::from_fields(
            self.original_request.clone(),
            self.selected_step.clone(),
            self.selected_origin.clone(),
            self.refreshed_step.clone(),
            self.origin.clone(),
        )
        .map(|_| ())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparedFields {
    original_request: Request,
    selected_step: Step,
    selected_origin: Origin,
    refreshed_step: Step,
    origin: Origin,
    network: (Chain, Chain),
}
impl TryFrom<PreparedFields> for PreparedStep {
    type Error = Error;
    fn try_from(v: PreparedFields) -> Result<Self, Error> {
        let p = Self::from_fields(
            v.original_request,
            v.selected_step,
            v.selected_origin,
            v.refreshed_step,
            v.origin,
        )?;
        if p.network != v.network {
            return Err(super::invalid_record());
        }
        Ok(p)
    }
}
