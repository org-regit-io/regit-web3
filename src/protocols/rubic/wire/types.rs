// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::invalid;
use crate::domain::rubic::json::Bounded;
use crate::{
    domain::{
        Address, Amount, ExactDecimal,
        market::NonnegativeDecimal,
        rubic::{
            Asset, AssetIdentifier, Catalogue, Family, Leg, LegKind, SOLANA_NATIVE_ASSET_ADDRESS,
            Token, TokenAmount,
        },
        solana::Pubkey,
    },
    error::Error,
};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

#[derive(Deserialize)]
#[serde(transparent)]
pub(super) struct Number(Box<RawValue>);
impl Number {
    pub(super) fn exact(&self) -> Result<ExactDecimal, Error> {
        ExactDecimal::parse(self.0.get()).map_err(|_| invalid())
    }
    pub(super) fn positive(&self) -> Result<NonnegativeDecimal, Error> {
        NonnegativeDecimal::new(self.exact()?).map_err(|_| invalid())
    }
    pub(super) fn u64(&self) -> Result<u64, Error> {
        let a = Amount::from_decimal(self.0.get(), None).map_err(|_| invalid())?;
        u64::try_from(a.raw()).map_err(|_| invalid())
    }
}
pub(super) fn required_optional<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TokenWire {
    pub(super) address: String,
    pub(super) blockchain: String,
    #[serde(deserialize_with = "required_optional")]
    pub(super) blockchain_id: Option<Number>,
    pub(super) decimals: u8,
    pub(super) name: String,
    pub(super) symbol: String,
    pub(super) price: Option<Number>,
}
impl TokenWire {
    pub(super) fn typed(self, catalogue: &Catalogue) -> Result<Token, Error> {
        let chain = catalogue
            .resolve(&self.blockchain)
            .map_err(|_| invalid())?
            .clone();
        if chain.provider_id() != self.blockchain_id.as_ref().map(Number::u64).transpose()? {
            return Err(invalid());
        }
        let id = match chain.family() {
            Family::Evm { .. } => {
                let address = Address::parse(&self.address).map_err(|_| invalid())?;
                if address.bytes() == [0; 20] {
                    AssetIdentifier::Native
                } else {
                    AssetIdentifier::EvmToken(address)
                }
            }
            Family::Solana { .. } => {
                if self.address == SOLANA_NATIVE_ASSET_ADDRESS {
                    AssetIdentifier::Native
                } else {
                    AssetIdentifier::SolanaMint(
                        Pubkey::parse(&self.address).map_err(|_| invalid())?,
                    )
                }
            }
            Family::Provider { .. } => {
                if self.address == "0x0000000000000000000000000000000000000000" {
                    AssetIdentifier::Native
                } else {
                    AssetIdentifier::Provider(
                        crate::domain::rubic::Text::new(&self.address).map_err(|_| invalid())?,
                    )
                }
            }
        };
        Ok(Token {
            asset: Asset::new(chain, id, self.decimals).map_err(|_| invalid())?,
            name: crate::domain::rubic::Text::new(&self.name).map_err(|_| invalid())?,
            symbol: crate::domain::rubic::Text::new(&self.symbol).map_err(|_| invalid())?,
            price_usd: self.price.as_ref().map(Number::positive).transpose()?,
        })
    }
}
pub(super) struct TokenAmountWire {
    token: TokenWire,
    amount: String,
}
impl<'de> Deserialize<'de> for TokenAmountWire {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Units {
            amount: String,
        }
        let raw = Box::<RawValue>::deserialize(d)?;
        let token = serde_json::from_str(raw.get()).map_err(serde::de::Error::custom)?;
        let units: Units = serde_json::from_str(raw.get()).map_err(serde::de::Error::custom)?;
        Ok(Self {
            token,
            amount: units.amount,
        })
    }
}
#[derive(Clone, Copy, Deserialize, serde::Serialize)]
#[serde(transparent)]
pub(in crate::protocols::rubic) struct Switch(pub(in crate::protocols::rubic) bool);
impl TokenAmountWire {
    pub(super) fn typed(self, c: &Catalogue) -> Result<TokenAmount, Error> {
        let token = self.token.typed(c)?;
        let amount = human(&self.amount, token.asset.decimals())?;
        TokenAmount::new(token, amount).map_err(|_| invalid())
    }
}
pub(super) fn human(value: &str, decimals: u8) -> Result<Amount, Error> {
    let number = ExactDecimal::parse(value).map_err(|_| invalid())?;
    if number.is_negative() {
        return Err(invalid());
    }
    let text = number.canonical();
    let (integer, fraction) = text.split_once('.').unwrap_or((&text, ""));
    if fraction.len() > usize::from(decimals) {
        return Err(invalid());
    }
    let digits = format!(
        "{integer}{fraction}{}",
        "0".repeat(usize::from(decimals) - fraction.len())
    );
    let digits = digits.trim_start_matches('0');
    Amount::from_decimal(if digits.is_empty() { "0" } else { digits }, Some(decimals))
        .map_err(|_| invalid())
}
pub(super) fn raw(v: &str, decimals: Option<u8>) -> Result<Amount, Error> {
    Amount::from_decimal(v, decimals).map_err(|_| invalid())
}
#[derive(Deserialize)]
pub(super) struct LegWire {
    #[serde(rename = "type")]
    kind: LegKind,
    provider: String,
    path: Bounded<TokenAmountWire, 128>,
}
impl LegWire {
    pub(super) fn typed(self, c: &Catalogue) -> Result<Leg, Error> {
        Leg::new(
            self.kind,
            crate::domain::rubic::Identifier::new(&self.provider).map_err(|_| invalid())?,
            self.path
                .0
                .into_iter()
                .map(|p| p.typed(c))
                .collect::<Result<_, _>>()?,
        )
        .map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EstimateWire {
    destination_token_amount: String,
    destination_token_min_amount: String,
    destination_wei_amount: String,
    destination_wei_min_amount: String,
    destination_usd_amount: Option<Number>,
    destination_usd_min_amount: Option<Number>,
    duration_in_minutes: Number,
    slippage: Number,
    price_impact: Option<Number>,
    intermidiate_token_wei_amount: Option<String>,
}
impl EstimateWire {
    pub(super) fn typed(self, decimals: u8) -> Result<crate::domain::rubic::Estimate, Error> {
        let out = raw(&self.destination_wei_amount, Some(decimals))?;
        let min = raw(&self.destination_wei_min_amount, Some(decimals))?;
        if out != human(&self.destination_token_amount, decimals)?
            || min != human(&self.destination_token_min_amount, decimals)?
        {
            return Err(invalid());
        }
        crate::domain::rubic::Estimate::new(crate::domain::rubic::EstimateData {
            output: out,
            minimum_output: min,
            output_usd: self
                .destination_usd_amount
                .as_ref()
                .map(Number::positive)
                .transpose()?,
            minimum_output_usd: self
                .destination_usd_min_amount
                .as_ref()
                .map(Number::positive)
                .transpose()?,
            duration_minutes: self.duration_in_minutes.positive()?,
            slippage_fraction: self.slippage.positive()?,
            price_impact: self.price_impact.as_ref().map(Number::exact).transpose()?,
            intermediate_raw: self
                .intermidiate_token_wei_amount
                .as_deref()
                .map(|s| raw(s, None))
                .transpose()?,
        })
        .map_err(|_| invalid())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GasWire {
    gas_limit: Option<String>,
    gas_price: Option<String>,
    base_fee: Option<String>,
    max_fee_per_gas: Option<String>,
    max_priority_fee_per_gas: Option<String>,
    total_wei_amount: Option<String>,
    total_usd_amount: Option<Number>,
}
impl GasWire {
    fn typed(self, decimals: u8) -> Result<crate::domain::rubic::GasFees, Error> {
        Ok(crate::domain::rubic::GasFees {
            gas_limit: self
                .gas_limit
                .as_deref()
                .map(|s| raw(s, None))
                .transpose()?,
            gas_price: self
                .gas_price
                .as_deref()
                .map(|s| raw(s, None))
                .transpose()?,
            base_fee: self.base_fee.as_deref().map(|s| raw(s, None)).transpose()?,
            max_fee_per_gas: self
                .max_fee_per_gas
                .as_deref()
                .map(|s| raw(s, None))
                .transpose()?,
            max_priority_fee_per_gas: self
                .max_priority_fee_per_gas
                .as_deref()
                .map(|s| raw(s, None))
                .transpose()?,
            total: self
                .total_wei_amount
                .as_deref()
                .map(|s| raw(s, Some(decimals)))
                .transpose()?,
            total_usd: self
                .total_usd_amount
                .as_ref()
                .map(Number::positive)
                .transpose()?,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixedWire {
    #[serde(rename = "fixedAmount")]
    amount: String,
    #[serde(rename = "fixedWeiAmount")]
    raw: String,
    #[serde(rename = "fixedUsdAmount")]
    usd: Option<Number>,
}
impl FixedWire {
    fn typed(self, decimals: u8) -> Result<crate::domain::rubic::FixedFee, Error> {
        let amount = raw(&self.raw, Some(decimals))?;
        if amount != human(&self.amount, decimals)? {
            return Err(invalid());
        }
        Ok(crate::domain::rubic::FixedFee {
            amount,
            usd: self.usd.as_ref().map(Number::positive).transpose()?,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FeesWire {
    gas_token_fees: NativeFees,
    percent_fees: PercentFees,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeFees {
    gas: GasWire,
    native_token: TokenWire,
    protocol: FixedWire,
    provider: FixedWire,
}
#[derive(Deserialize)]
struct PercentFees {
    percent: Number,
    #[serde(deserialize_with = "required_optional")]
    token: Option<TokenWire>,
}
impl FeesWire {
    pub(super) fn typed(self, c: &Catalogue) -> Result<crate::domain::rubic::Fees, Error> {
        let native = self.gas_token_fees.native_token.typed(c)?;
        let d = native.asset.decimals();
        Ok(crate::domain::rubic::Fees {
            native_token: native,
            gas: self.gas_token_fees.gas.typed(d)?,
            protocol: self.gas_token_fees.protocol.typed(d)?,
            provider: self.gas_token_fees.provider.typed(d)?,
            percent: self.percent_fees.percent.positive()?,
            percent_token: self.percent_fees.token.map(|t| t.typed(c)).transpose()?,
        })
    }
}
