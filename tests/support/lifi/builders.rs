// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use regit_web3::{
    domain::{Amount, ExactDecimal, Source, Timestamp, lifi::*},
    error::Error,
};
use serde::{Deserialize, Serialize};

pub(super) const ACCOUNT: &str = "0x204dedcf79dbbb02359205f4f64ce2cbdd483906";
pub(super) const NATIVE: &str = "0x0000000000000000000000000000000000000000";
pub(super) const STATUS_HASH: &str =
    "0xe1ffdcf09d5aa92a2d89b1b39db3f8cadf09428a296cce0d5e387595ac83d08f";
pub(super) fn chain(id: u64) -> Result<Chain, Error> {
    Chain::new(id, Family::Evm)
}
pub(super) fn request() -> Result<Request, Error> {
    let from = chain(42161)?;
    let to = chain(8453)?;
    Request::new(RequestData {
        from: Asset::new(from, NATIVE)?,
        to: Asset::new(to, NATIVE)?,
        amount: Amount::from_decimal("10000000000000000", None)?,
        from_account: Account::new(from, ACCOUNT)?,
        to_account: Some(Account::new(to, ACCOUNT)?),
        slippage: Slippage::new(ExactDecimal::parse("0.005")?)?,
        allow_switch_chain: false,
    })
}
pub(super) fn origin(method: &str) -> Result<Origin, Error> {
    Ok(Origin {
        source: Source::new("lifi-fixture", method, "0.1.0")?,
        retrieved_at: Timestamp::from_unix_seconds(1_791_340_000),
    })
}
pub(super) fn token(chain: Chain) -> Result<Token, Error> {
    Token::new(TokenData {
        asset: Asset::new(chain, NATIVE)?,
        decimals: 18,
        symbol: Text::new("ETH")?,
        name: Some(Text::new("Ether")?),
        price_usd: Some(ExactDecimal::parse("2613.530000000000001")?),
    })
}
pub(super) fn step() -> Result<Step, Error> {
    let r = request()?;
    let from = r.data().from.chain();
    let to = r.data().to.chain();
    let action = Action::new(ActionData {
        from_token: token(from)?,
        to_token: token(to)?,
        from_amount: Amount::from_decimal("10000000000000000", Some(18))?,
        from_account: Some(r.data().from_account.clone()),
        to_account: r.data().to_account.clone(),
        slippage: Some(r.data().slippage.clone()),
    })?;
    let estimate = Estimate::new(EstimateData {
        tool: Identifier::new("bridge")?,
        from_amount: action.data().from_amount,
        to_amount: Amount::from_decimal("9973849077309328", Some(18))?,
        to_amount_min: Amount::from_decimal("9923979831922781", Some(18))?,
        from_amount_usd: Some(ExactDecimal::parse("26.13530000000001")?),
        to_amount_usd: Some(ExactDecimal::parse("26.0716")?),
        approval_account: None,
        fees: Some(vec![]),
        gas_costs: None,
        execution_seconds: ExactDecimal::parse("45.60000000000000001")?,
        skip_approval: Some(true),
        expiry: Expiry::Unreported,
    })?;
    Step::new(StepData {
        id: Identifier::new("quoted-step:0")?,
        kind: StepKind::Lifi,
        tool: Identifier::new("bridge")?,
        action,
        estimate: Some(estimate),
        included_steps: vec![],
        payload: None,
        transfer_id: None,
    })
}
pub(super) fn payload() -> Result<Payload, Error> {
    let chain = chain(42161)?;
    Ok(Payload::Evm(Box::new(EvmPayload::new(EvmPayloadData {
        chain,
        from: Some(Account::new(chain, ACCOUNT)?),
        to: Account::new(chain, "0x1231DEB6f5749EF6cE6943a275A1D3E7486F4EaE")?,
        value: Amount::from_decimal("10000000000000000", None)?,
        data: EncodedPayload::new(chain, PayloadEncoding::EvmCallData, "0x12345678", None)?,
        gas_limit: None,
        gas_price: None,
        max_priority_fee_per_gas: None,
        max_fee_per_gas: None,
        nonce: None,
        transaction_type: None,
        access_list: None,
    })?)))
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct Handle {
    pub(super) value: Step,
    pub(super) request: Request,
    pub(super) origin: Origin,
}
impl StepView for Handle {
    fn step(&self) -> &Step {
        &self.value
    }
    fn request(&self) -> &Request {
        &self.request
    }
    fn origin(&self) -> &Origin {
        &self.origin
    }
}
pub(super) fn handle() -> Result<Handle, Error> {
    Ok(Handle {
        value: step()?,
        request: request()?,
        origin: origin("quote")?,
    })
}
pub(super) fn query(to: u64) -> Result<StatusQuery, Error> {
    StatusQuery::new(
        chain(42161)?,
        chain(to)?,
        StatusSelector::SendingTransaction(TransactionId::new(chain(42161)?, STATUS_HASH)?),
        None,
    )
}
