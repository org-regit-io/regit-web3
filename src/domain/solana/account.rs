// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{
    domain::{Amount, U256},
    error::{Error, ValidationError},
};

use super::{Hash, Network, Pubkey};

fn validate_amount(amount: &Amount, decimals: Option<u8>) -> Result<(), Error> {
    if amount.raw() > U256::from(u64::MAX) {
        return Err(ValidationError::SolanaAmountOverflow.into());
    }
    if amount.decimals() != decimals {
        return Err(ValidationError::DecimalMismatch.into());
    }
    Ok(())
}

/// Exact native lamports for a caller-supplied network and account address.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "NativeBalanceFields")]
pub struct NativeBalance {
    network: Network,
    address: Pubkey,
    amount: Amount,
}

impl NativeBalance {
    /// Records an unsigned 64-bit lamport balance with SOL's nine decimals.
    #[must_use]
    pub fn new(network: Network, address: Pubkey, lamports: u64) -> Self {
        Self {
            network,
            address,
            amount: Amount::new(U256::from(lamports), Some(9)),
        }
    }

    /// Returns the supplied network identity and display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }

    /// Returns the account address.
    #[must_use]
    pub const fn address(&self) -> Pubkey {
        self.address
    }

    /// Returns exact lamports and the declared native precision.
    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.amount
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeBalanceFields {
    network: Network,
    address: Pubkey,
    amount: Amount,
}

impl TryFrom<NativeBalanceFields> for NativeBalance {
    type Error = Error;

    fn try_from(fields: NativeBalanceFields) -> Result<Self, Self::Error> {
        validate_amount(&fields.amount, Some(9))?;
        Ok(Self {
            network: fields.network,
            address: fields.address,
            amount: fields.amount,
        })
    }
}

/// Technical SPL asset identity, excluding aliases and decimal metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenIdentity {
    genesis_hash: Hash,
    mint: Pubkey,
    token_program: Pubkey,
}

impl TokenIdentity {
    /// Records the full network genesis, mint, and token program identities.
    #[must_use]
    pub const fn new(genesis_hash: Hash, mint: Pubkey, token_program: Pubkey) -> Self {
        Self {
            genesis_hash,
            mint,
            token_program,
        }
    }

    /// Returns the full network genesis identity.
    #[must_use]
    pub const fn genesis_hash(self) -> Hash {
        self.genesis_hash
    }

    /// Returns the mint address.
    #[must_use]
    pub const fn mint(self) -> Pubkey {
        self.mint
    }

    /// Returns the token program address.
    #[must_use]
    pub const fn token_program(self) -> Pubkey {
        self.token_program
    }
}

/// A caller-supplied SPL mint, token program, and independently optional precision.
///
/// Unknown decimals remain null; no native or EVM precision is inferred. Raw
/// base units do not incorporate Token-2022 scaled or interest-bearing UI values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenAsset {
    network: Network,
    mint: Pubkey,
    token_program: Pubkey,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    decimals: Option<u8>,
}

impl TokenAsset {
    /// Records explicit mint/program identity and optional caller-supplied decimals.
    #[must_use]
    pub const fn new(
        network: Network,
        mint: Pubkey,
        token_program: Pubkey,
        decimals: Option<u8>,
    ) -> Self {
        Self {
            network,
            mint,
            token_program,
            decimals,
        }
    }

    /// Returns technical identity independent of display alias and precision.
    #[must_use]
    pub const fn identity(&self) -> TokenIdentity {
        TokenIdentity::new(self.network.genesis_hash(), self.mint, self.token_program)
    }

    /// Returns the supplied network and its display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }

    /// Returns the mint address.
    #[must_use]
    pub const fn mint(&self) -> Pubkey {
        self.mint
    }

    /// Returns the token program address.
    #[must_use]
    pub const fn token_program(&self) -> Pubkey {
        self.token_program
    }

    /// Returns explicitly supplied precision, or unknown precision.
    #[must_use]
    pub const fn decimals(&self) -> Option<u8> {
        self.decimals
    }
}

/// The explicitly reported SPL token-account state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenAccountState {
    /// The account is not initialized.
    Uninitialized,
    /// The account is initialized.
    Initialized,
    /// The account is frozen.
    Frozen,
}

/// Exact raw units held by a specified SPL token account.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TokenBalanceFields")]
pub struct TokenBalance {
    token_account: Pubkey,
    owner: Pubkey,
    asset: TokenAsset,
    amount: Amount,
    state: TokenAccountState,
}

impl TokenBalance {
    /// Records an unsigned 64-bit raw balance and explicitly supplied account state.
    #[must_use]
    pub fn new(
        token_account: Pubkey,
        owner: Pubkey,
        asset: TokenAsset,
        raw: u64,
        state: TokenAccountState,
    ) -> Self {
        let amount = Amount::new(U256::from(raw), asset.decimals());
        Self {
            token_account,
            owner,
            asset,
            amount,
            state,
        }
    }

    /// Returns the token account address, separately from the mint and owner.
    #[must_use]
    pub const fn token_account(&self) -> Pubkey {
        self.token_account
    }

    /// Returns the reported token-account owner.
    #[must_use]
    pub const fn owner(&self) -> Pubkey {
        self.owner
    }

    /// Returns the explicitly identified asset and optional precision.
    #[must_use]
    pub const fn asset(&self) -> &TokenAsset {
        &self.asset
    }

    /// Returns exact raw units and optional declared precision.
    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.amount
    }

    /// Returns the explicitly reported account state.
    #[must_use]
    pub const fn state(&self) -> TokenAccountState {
        self.state
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenBalanceFields {
    token_account: Pubkey,
    owner: Pubkey,
    asset: TokenAsset,
    amount: Amount,
    state: TokenAccountState,
}

impl TryFrom<TokenBalanceFields> for TokenBalance {
    type Error = Error;

    fn try_from(fields: TokenBalanceFields) -> Result<Self, Self::Error> {
        validate_amount(&fields.amount, fields.asset.decimals())?;
        Ok(Self {
            token_account: fields.token_account,
            owner: fields.owner,
            asset: fields.asset,
            amount: fields.amount,
            state: fields.state,
        })
    }
}

/// A present Solana account with decoded bytes and exact native lamports.
///
/// Data is limited to the protocol's 10 MiB maximum during construction and
/// deserialization. Empty data is a valid present account, never a missing-account
/// sentinel. The legacy rent epoch is retained as supplied, including `u64::MAX`.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AccountFields")]
pub struct Account {
    network: Network,
    address: Pubkey,
    owner: Pubkey,
    amount: Amount,
    executable: bool,
    data: Vec<u8>,
    rent_epoch: u64,
}

impl Account {
    /// The maximum account data length defined by the Solana protocol, 10 MiB.
    pub const MAX_DATA_BYTES: usize = 10 * 1024 * 1024;

    /// Records an explicitly supplied account with decoded data bytes.
    ///
    /// # Errors
    /// Rejects data exceeding [`Self::MAX_DATA_BYTES`].
    pub fn new(
        network: Network,
        address: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: Vec<u8>,
        rent_epoch: u64,
    ) -> Result<Self, Error> {
        if data.len() > Self::MAX_DATA_BYTES {
            return Err(ValidationError::SolanaAccountDataTooLarge.into());
        }
        Ok(Self {
            network,
            address,
            owner,
            amount: Amount::new(U256::from(lamports), Some(9)),
            executable,
            data,
            rent_epoch,
        })
    }

    /// Returns the supplied network and its display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }

    /// Returns the account address.
    #[must_use]
    pub const fn address(&self) -> Pubkey {
        self.address
    }

    /// Returns the reported program owner.
    #[must_use]
    pub const fn owner(&self) -> Pubkey {
        self.owner
    }

    /// Returns native lamports with nine decimals.
    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.amount
    }

    /// Returns the reported executable flag.
    #[must_use]
    pub const fn executable(&self) -> bool {
        self.executable
    }

    /// Returns exact decoded account bytes.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the supplied legacy rent epoch without inference.
    #[must_use]
    pub const fn rent_epoch(&self) -> u64 {
        self.rent_epoch
    }
}

impl fmt::Debug for Account {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Account")
            .field("network", &self.network)
            .field("address", &self.address)
            .field("owner", &self.owner)
            .field("amount", &self.amount)
            .field("executable", &self.executable)
            .field("data_bytes", &self.data.len())
            .field("rent_epoch", &self.rent_epoch)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountFields {
    network: Network,
    address: Pubkey,
    owner: Pubkey,
    amount: Amount,
    executable: bool,
    #[serde(deserialize_with = "deserialize_data")]
    data: Vec<u8>,
    rent_epoch: u64,
}

impl TryFrom<AccountFields> for Account {
    type Error = Error;

    fn try_from(fields: AccountFields) -> Result<Self, Self::Error> {
        validate_amount(&fields.amount, Some(9))?;
        Ok(Self {
            network: fields.network,
            address: fields.address,
            owner: fields.owner,
            amount: fields.amount,
            executable: fields.executable,
            data: fields.data,
            rent_epoch: fields.rent_epoch,
        })
    }
}

fn deserialize_data<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    struct DataVisitor;

    impl<'de> de::Visitor<'de> for DataVisitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("account bytes within the Solana data limit")
        }

        fn visit_seq<A: de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut data = Vec::new();
            while let Some(byte) = sequence.next_element::<u8>()? {
                if data.len() == Account::MAX_DATA_BYTES {
                    return Err(de::Error::custom(
                        ValidationError::SolanaAccountDataTooLarge,
                    ));
                }
                data.push(byte);
            }
            Ok(data)
        }
    }
    deserializer.deserialize_seq(DataVisitor)
}

/// An account lookup retaining explicit absence separately from a present record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AccountLookupFields")]
pub struct AccountLookup {
    network: Network,
    address: Pubkey,
    account: Option<Account>,
}

impl AccountLookup {
    /// Records a present account or explicit source-reported absence.
    ///
    /// # Errors
    /// Rejects a present account with a different address or genesis identity.
    pub fn new(network: Network, address: Pubkey, account: Option<Account>) -> Result<Self, Error> {
        if let Some(value) = &account {
            if value.address() != address {
                return Err(ValidationError::InvalidSolanaAccount.into());
            }
            if value.network().genesis_hash() != network.genesis_hash() {
                return Err(ValidationError::NetworkMismatch.into());
            }
        }
        Ok(Self {
            network,
            address,
            account,
        })
    }

    /// Returns the requested network and display alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }

    /// Returns the requested account address.
    #[must_use]
    pub const fn address(&self) -> Pubkey {
        self.address
    }

    /// Returns the present account, or explicit absence.
    #[must_use]
    pub const fn account(&self) -> Option<&Account> {
        self.account.as_ref()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountLookupFields {
    network: Network,
    address: Pubkey,
    #[serde(deserialize_with = "crate::domain::deserialize_optional")]
    account: Option<Account>,
}

impl TryFrom<AccountLookupFields> for AccountLookup {
    type Error = Error;

    fn try_from(fields: AccountLookupFields) -> Result<Self, Self::Error> {
        Self::new(fields.network, fields.address, fields.account)
    }
}
