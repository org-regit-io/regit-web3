// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use super::{Address, Boc, Nanotons, Network};
use crate::{error::Error, wallets::Preparation};
use serde::{Deserialize, Serialize};
use tycho_types::{
    cell::{Cell, CellBuilder, CellFamily, HashBytes},
    models::{
        CurrencyCollection, IntAddr, OwnedRelaxedMessage, RelaxedIntMsgInfo, RelaxedMsgInfo,
        StdAddr,
    },
};

/// Immutable ordinary native-transfer intent, with explicit outer-wallet policy.
/// The internal message binds destination/value/bounce/body. Its sender and
/// wallet expiry are review facts for the external wallet, not encoded or
/// cryptographically bound by this internal-message BOC.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IntentFields")]
pub struct TransferIntent {
    network: Network,
    sender: Address,
    destination: Address,
    amount: Nanotons,
    bounce: bool,
    body: Option<Boc>,
    wallet_valid_until: Option<u32>,
}
impl TransferIntent {
    /// Checks ordinary native transfer intent without choosing wallet version or keys.
    ///
    /// # Errors
    /// Rejects zero value and test-only address/mainnet contradictions.
    pub fn new(
        network: Network,
        sender: Address,
        destination: Address,
        amount: Nanotons,
        bounce: bool,
        body: Option<Boc>,
        wallet_valid_until: Option<u32>,
    ) -> Result<Self, Error> {
        sender.check_network(network)?;
        destination.check_network(network)?;
        if amount == Nanotons::ZERO || wallet_valid_until == Some(0) {
            return Err(super::invalid_transfer());
        }
        Ok(Self {
            network,
            sender,
            destination,
            amount,
            bounce,
            body,
            wallet_valid_until,
        })
    }
    /// Returns expected network, not an internal-message embedded network proof.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
    /// Returns caller-reviewed outer-wallet sender policy.
    #[must_use]
    pub const fn sender(&self) -> Address {
        self.sender
    }
    /// Returns exact encoded destination.
    #[must_use]
    pub const fn destination(&self) -> Address {
        self.destination
    }
    /// Returns exact encoded nanotons.
    #[must_use]
    pub const fn amount(&self) -> Nanotons {
        self.amount
    }
    /// Returns explicit encoded bounce policy.
    #[must_use]
    pub const fn bounce(&self) -> bool {
        self.bounce
    }
    /// Returns optional arbitrary bounded body cell, without interpreting wallet intent.
    #[must_use]
    pub const fn body(&self) -> Option<&Boc> {
        self.body.as_ref()
    }
    /// Returns optional outer-wallet expiry in Unix seconds, not encoded in this BOC.
    #[must_use]
    pub const fn wallet_valid_until(&self) -> Option<u32> {
        self.wallet_valid_until
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentFields {
    network: Network,
    sender: Address,
    destination: Address,
    amount: Nanotons,
    bounce: bool,
    body: Option<Boc>,
    wallet_valid_until: Option<u32>,
}
impl TryFrom<IntentFields> for TransferIntent {
    type Error = Error;
    fn try_from(v: IntentFields) -> Result<Self, Error> {
        Self::new(
            v.network,
            v.sender,
            v.destination,
            v.amount,
            v.bounce,
            v.body,
            v.wallet_valid_until,
        )
    }
}

/// Maintained internal-message encoding plus its complete immutable review intent.
/// This unsigned representation is one wallet action, not a ready-to-submit
/// external message, signing digest, fee estimate or proof of wallet approval.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PreparationFields")]
pub struct TransferPreparation {
    network: Network,
    intent: TransferIntent,
    unsigned_payload: Boc,
}
impl TransferPreparation {
    /// Encodes one ordinary internal message using maintained TON TL-B models.
    ///
    /// # Errors
    /// Rejects codec/resource failures without signing or any network access.
    pub fn new(intent: TransferIntent) -> Result<Self, Error> {
        let body = intent
            .body
            .as_ref()
            .map_or_else(|| Ok(Cell::empty_cell()), Boc::root)?;
        let body = body.into();
        let message = OwnedRelaxedMessage {
            info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
                src: None,
                dst: IntAddr::Std(StdAddr::new(
                    intent.destination.workchain(),
                    HashBytes(*intent.destination.account().as_bytes()),
                )),
                value: CurrencyCollection::new(intent.amount.raw()),
                bounce: intent.bounce,
                ..RelaxedIntMsgInfo::default()
            }),
            init: None,
            body,
            layout: None,
        };
        let root = CellBuilder::build_from(message).map_err(|_| super::invalid_transfer())?;
        let unsigned_payload = Boc::from_cell(&root)?;
        Ok(Self {
            network: intent.network,
            intent,
            unsigned_payload,
        })
    }
    /// Returns full immutable intent, including outer-wallet sender/expiry policy.
    #[must_use]
    pub const fn intent(&self) -> &TransferIntent {
        &self.intent
    }
    /// Returns an unsigned internal-message BOC, not a complete wallet request.
    #[must_use]
    pub const fn unsigned_payload(&self) -> &Boc {
        &self.unsigned_payload
    }
}
impl Preparation for TransferPreparation {
    type Network = Network;
    type Intent = TransferIntent;
    type UnsignedPayload = Boc;
    fn network(&self) -> &Network {
        &self.network
    }
    fn intent(&self) -> &TransferIntent {
        &self.intent
    }
    fn unsigned_payload(&self) -> &Boc {
        &self.unsigned_payload
    }
    fn validate(&self) -> Result<(), Error> {
        if Self::new(self.intent.clone())? != *self {
            return Err(super::invalid_transfer());
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparationFields {
    network: Network,
    intent: TransferIntent,
    unsigned_payload: Boc,
}
impl TryFrom<PreparationFields> for TransferPreparation {
    type Error = Error;
    fn try_from(v: PreparationFields) -> Result<Self, Error> {
        let result = Self::new(v.intent)?;
        if result.network != v.network || result.unsigned_payload != v.unsigned_payload {
            return Err(super::invalid_transfer());
        }
        Ok(result)
    }
}
