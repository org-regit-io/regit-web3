// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use serde::{Deserialize, Serialize};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_message::{Message, VersionedMessage};

use super::{Hash, Network, Pubkey, UnsignedMessage, UnsignedTransaction};
use crate::error::{Error, ValidationError};

/// An explicit recent blockhash and its source-reported last valid block height.
/// The height is caller policy metadata; only the hash is encoded in the message.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockhashLifetime {
    /// Exact recent blockhash to encode without implicit refresh.
    pub blockhash: Hash,
    /// Last valid block height, distinct from any slot or wall-clock expiry.
    pub last_valid_block_height: u64,
}

/// Explicit ordinary transfer intent, without creating accounts or inferring owners.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransferIntent {
    /// Legacy System Program transfer of exact SOL lamports.
    Native {
        /// Caller-specified fee payer, which must externally sign.
        fee_payer: Pubkey,
        /// Caller-specified sending System account, which must externally sign.
        sender: Pubkey,
        /// Caller-specified recipient; funding and ownership are unverified.
        recipient: Pubkey,
        /// Exact unsigned 64-bit lamport amount.
        lamports: u64,
    },
    /// Classic SPL Token `TransferChecked` between explicitly existing accounts.
    /// This profile does not infer associated accounts or Token-2022 policies.
    TokenChecked {
        /// Caller-specified fee payer, which must externally sign.
        fee_payer: Pubkey,
        /// Caller-specified existing source token account.
        source_account: Pubkey,
        /// Caller-specified mint checked by the instruction.
        mint: Pubkey,
        /// Caller-specified existing destination token account.
        destination_account: Pubkey,
        /// Single signing authority; multisignature policies are outside this profile.
        authority: Pubkey,
        /// Exact token base units, without scaled UI conversion.
        raw_amount: u64,
        /// Caller-specified mint decimals encoded for the on-chain check.
        decimals: u8,
    },
}
impl TransferIntent {
    /// Returns the explicit fee payer.
    #[must_use]
    pub const fn fee_payer(&self) -> Pubkey {
        match self {
            Self::Native { fee_payer, .. } | Self::TokenChecked { fee_payer, .. } => *fee_payer,
        }
    }
    fn instruction(&self) -> Result<Instruction, Error> {
        let address = |p: Pubkey| Address::new_from_array(p.bytes());
        Ok(match *self {
            Self::Native {
                sender,
                recipient,
                lamports,
                ..
            } => solana_system_interface::instruction::transfer(
                &address(sender),
                &address(recipient),
                lamports,
            ),
            Self::TokenChecked {
                source_account,
                mint,
                destination_account,
                authority,
                raw_amount,
                decimals,
                ..
            } => Instruction {
                program_id: address(Pubkey::parse(
                    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                )?),
                accounts: vec![
                    AccountMeta::new(address(source_account), false),
                    AccountMeta::new_readonly(address(mint), false),
                    AccountMeta::new(address(destination_account), false),
                    AccountMeta::new_readonly(address(authority), true),
                ],
                // Only maintained instruction data packing is reused here: its older
                // public-key/instruction types are not part of this API.
                data: spl_token_interface::instruction::TokenInstruction::TransferChecked {
                    amount: raw_amount,
                    decimals,
                }
                .pack(),
            },
        })
    }
}

/// Immutable legacy transfer preparation and generic external-wallet adapter.
///
/// Construction encodes one supported instruction, without signing, custody,
/// submission, account lookup, funding proof, durable nonce, account creation,
/// fee cap or caller approval. Last-valid height is retained but not encoded.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Fields")]
pub struct TransferPreparation {
    network: Network,
    intent: TransferReview,
    unsigned_transaction: UnsignedTransaction,
}
impl TransferPreparation {
    /// Encodes an ordinary legacy transfer using caller-supplied identities and lifetime.
    /// # Errors
    /// Rejects structurally invalid or excessive preparations with a fixed diagnostic.
    pub fn new(
        network: Network,
        intent: TransferIntent,
        lifetime: BlockhashLifetime,
    ) -> Result<Self, Error> {
        let instruction = intent.instruction()?;
        // The supported single instruction has at most six distinct addresses,
        // below the maintained compiler's account-index capacity.
        let message = Message::new_with_blockhash(
            &[instruction],
            Some(&Address::new_from_array(intent.fee_payer().bytes())),
            &solana_hash::Hash::new_from_array(lifetime.blockhash.bytes()),
        );
        let unsigned_transaction = UnsignedTransaction::from_message(
            UnsignedMessage::from_message(VersionedMessage::Legacy(message))?,
        )?;
        Ok(Self {
            network,
            intent: TransferReview {
                transfer: intent,
                lifetime,
            },
            unsigned_transaction,
        })
    }
    /// Returns the caller-supplied full genesis identity and alias.
    #[must_use]
    pub const fn network(&self) -> &Network {
        &self.network
    }
    /// Returns complete reviewable transfer intent.
    #[must_use]
    pub const fn intent(&self) -> &TransferReview {
        &self.intent
    }
    /// Returns the encoded hash and separately retained block-height policy.
    #[must_use]
    pub const fn lifetime(&self) -> BlockhashLifetime {
        self.intent.lifetime
    }
    /// Returns exact zero-placeholder transaction bytes, without signatures.
    #[must_use]
    pub const fn unsigned_transaction(&self) -> &UnsignedTransaction {
        &self.unsigned_transaction
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields {
    network: Network,
    intent: TransferReview,
    unsigned_transaction: UnsignedTransaction,
}
impl TryFrom<Fields> for TransferPreparation {
    type Error = Error;
    fn try_from(f: Fields) -> Result<Self, Error> {
        let value = Self::new(f.network, f.intent.transfer, f.intent.lifetime)?;
        if value.unsigned_transaction != f.unsigned_transaction {
            return Err(ValidationError::InvalidSolanaPreparation.into());
        }
        Ok(value)
    }
}
impl crate::wallets::Preparation for TransferPreparation {
    type Network = Network;
    type Intent = TransferReview;
    type UnsignedPayload = UnsignedTransaction;
    fn network(&self) -> &Network {
        &self.network
    }
    fn intent(&self) -> &TransferReview {
        &self.intent
    }
    fn unsigned_payload(&self) -> &UnsignedTransaction {
        &self.unsigned_transaction
    }
    fn validate(&self) -> Result<(), Error> {
        if Self::new(
            self.network.clone(),
            self.intent.transfer.clone(),
            self.intent.lifetime,
        )? != *self
        {
            return Err(ValidationError::InvalidSolanaPreparation.into());
        }
        Ok(())
    }
}

/// Complete reviewable transfer and lifetime metadata, including unencoded height policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferReview {
    /// Exact account identities, units, program profile and signing-role intent.
    pub transfer: TransferIntent,
    /// Encoded recent hash and separately retained caller block-height policy.
    pub lifetime: BlockhashLifetime,
}
