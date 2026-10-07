// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Offline public contracts for Solana identities, values, and capability traits.

#![cfg(feature = "solana")]

use std::{
    future::{Future, ready},
    task::{Context as TaskContext, Poll, Waker},
};

use regit_web3::{
    chains::solana::NativeBalanceReader,
    domain::{
        Amount, Source, Timestamp, U256,
        solana::{
            Account, AccountLookup, Commitment, Context, Hash, NativeBalance, Network, Observation,
            Operation, Pubkey, ReadOptions, Signature, TokenAccountState, TokenAsset, TokenBalance,
        },
    },
    error::{Error, ValidationError},
};
use serde_json::{Value, json};

const GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
const SYSTEM: &str = "11111111111111111111111111111111";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

fn network(alias: &str) -> Result<Network, Error> {
    Network::new(Hash::parse(GENESIS)?, alias)
}

fn context(operation: Operation, slot: u64, options: ReadOptions) -> Result<Context, Error> {
    Context::new(
        operation,
        network("primary")?,
        options,
        slot,
        Source::new("fixture", "getBalance", "0.1.0")?,
        Timestamp::from_unix_seconds(1234),
    )
}

fn native_observation() -> Result<Observation<NativeBalance>, Error> {
    Observation::native_balance(
        NativeBalance::new(
            network("display")?,
            Pubkey::from_bytes([7; 32]),
            9_007_199_254_740_993,
        ),
        context(
            Operation::NativeBalance,
            120,
            ReadOptions::new(Commitment::Confirmed, Some(100)),
        )?,
    )
}

#[test]
fn official_base58_vectors_retain_case_and_full_width() {
    let address = Pubkey::parse(SYSTEM).unwrap();
    assert_eq!(address.bytes(), [0; 32]);
    for value in [TOKEN, TOKEN_2022] {
        let key = Pubkey::parse(value).unwrap();
        assert_eq!(key.to_string(), value);
        assert_eq!(Pubkey::from_bytes(key.bytes()), key);
        assert_eq!(serde_json::to_value(key).unwrap(), json!(value));
        assert_eq!(serde_json::from_value::<Pubkey>(json!(value)).unwrap(), key);
    }
    let genesis = Hash::parse(GENESIS).unwrap();
    assert_eq!(Hash::from_bytes(genesis.bytes()), genesis);
    assert_eq!(genesis.to_string(), GENESIS);
    assert!(Hash::parse(&GENESIS[..32]).is_err());
    let signature_text =
        "4ReKprwf3WdLHRrzp4ctPWNBsQDPL3VZz3zMmoZfcGJMJCHh5Vq937mPdyxhCbw54wNnA6hZ7KfNpQdpt13yY7A9";
    let signature = Signature::parse(signature_text).unwrap();
    assert_eq!(signature.to_string(), signature_text);
    assert_eq!(Signature::from_bytes(signature.bytes()), signature);
    assert_eq!(
        serde_json::from_value::<Signature>(json!(signature_text)).unwrap(),
        signature
    );
    // Every fixed-width byte array remains representable, including addresses
    // that are not checked for Ed25519 curve membership.
    let bytes = [255; 32];
    assert_eq!(
        Pubkey::parse(&Pubkey::from_bytes(bytes).to_string())
            .unwrap()
            .bytes(),
        bytes
    );
    assert_eq!(Signature::from_bytes([0; 64]).bytes(), [0; 64]);
}

#[test]
fn invalid_base58_is_rejected_without_input_in_domain_errors() {
    for invalid in [
        "",
        "0OIl",
        "not-a-key",
        "111",
        " https://fake.invalid/?key=FAKE_SECRET ",
    ] {
        for (error, reason) in [
            (
                Pubkey::parse(invalid).unwrap_err(),
                ValidationError::InvalidSolanaPubkey,
            ),
            (
                Hash::parse(invalid).unwrap_err(),
                ValidationError::InvalidSolanaHash,
            ),
            (
                Signature::parse(invalid).unwrap_err(),
                ValidationError::InvalidSolanaSignature,
            ),
        ] {
            assert_eq!(error, Error::Validation(reason));
            for diagnostic in [
                error.to_string(),
                format!("{error:?}"),
                serde_json::to_string(&error).unwrap(),
            ] {
                assert!(!diagnostic.contains("FAKE_SECRET"));
                assert!(!diagnostic.contains("fake.invalid"));
            }
        }
    }
    assert!(Pubkey::parse(&"1".repeat(33)).is_err());
    assert!(Signature::parse(SYSTEM).is_err());
    assert!(serde_json::from_value::<Pubkey>(json!(vec![0; 32])).is_err());
    assert!(serde_json::from_value::<Hash>(json!(7)).is_err());
    assert!(serde_json::from_value::<Signature>(json!(null)).is_err());
}

#[test]
fn network_aliases_are_validated_but_do_not_replace_genesis_identity() {
    let first = network("mainnet").unwrap();
    let second = network("caller-alias").unwrap();
    assert_eq!(first.genesis_hash(), second.genesis_hash());
    assert_ne!(first.alias(), second.alias());
    for alias in ["", "main net", "https://fake.invalid", "secret:header"] {
        assert_eq!(
            Network::new(first.genesis_hash(), alias).unwrap_err(),
            Error::Validation(ValidationError::InvalidNetworkAlias)
        );
        assert!(
            serde_json::from_value::<Network>(json!({"genesis_hash":GENESIS,"alias":alias}))
                .is_err()
        );
    }
    assert!(Network::new(first.genesis_hash(), "a".repeat(65)).is_err());
    assert!(
        serde_json::from_value::<Network>(json!({"genesis_hash":&GENESIS[..32],"alias":"mainnet"}))
            .is_err()
    );
}

#[test]
fn native_balances_keep_exact_u64_lamports_and_nine_decimals() {
    for (raw, formatted) in [
        (0, "0.000000000"),
        (9_007_199_254_740_993, "9007199.254740993"),
        (u64::MAX, "18446744073.709551615"),
    ] {
        let balance = NativeBalance::new(
            network("mainnet").unwrap(),
            Pubkey::from_bytes([9; 32]),
            raw,
        );
        assert_eq!(balance.amount().raw(), U256::from(raw));
        assert_eq!(balance.amount().decimals(), Some(9));
        assert_eq!(balance.amount().formatted().as_deref(), Some(formatted));
        assert_eq!(
            serde_json::from_value::<NativeBalance>(serde_json::to_value(&balance).unwrap())
                .unwrap(),
            balance
        );
    }
}

#[test]
fn native_balance_deserialization_cannot_bypass_width_or_precision() {
    let balance = NativeBalance::new(network("mainnet").unwrap(), Pubkey::from_bytes([9; 32]), 1);
    for amount in [
        Amount::new(U256::from(u64::MAX) + U256::from(1), Some(9)),
        Amount::new(U256::from(1), None),
        Amount::new(U256::from(1), Some(18)),
    ] {
        let mut wire = serde_json::to_value(&balance).unwrap();
        wire["amount"] = serde_json::to_value(amount).unwrap();
        assert!(serde_json::from_value::<NativeBalance>(wire).is_err());
    }
}

#[test]
fn token_identity_uses_genesis_mint_and_program_independent_of_metadata() {
    let mint = Pubkey::from_bytes([1; 32]);
    let program = Pubkey::parse(TOKEN).unwrap();
    let unknown = TokenAsset::new(network("one").unwrap(), mint, program, None);
    let known = TokenAsset::new(network("two").unwrap(), mint, program, Some(6));
    assert_eq!(unknown.identity(), known.identity());
    assert_eq!(
        unknown.identity().genesis_hash(),
        Hash::parse(GENESIS).unwrap()
    );
    assert_eq!(unknown.identity().mint(), mint);
    assert_eq!(unknown.identity().token_program(), program);
    assert_ne!(
        unknown.identity(),
        TokenAsset::new(
            network("one").unwrap(),
            mint,
            Pubkey::parse(TOKEN_2022).unwrap(),
            None
        )
        .identity()
    );
    assert_ne!(
        unknown.identity(),
        TokenAsset::new(
            network("one").unwrap(),
            Pubkey::from_bytes([2; 32]),
            program,
            None
        )
        .identity()
    );
    assert_ne!(
        unknown.identity(),
        TokenAsset::new(
            Network::new(Hash::from_bytes([3; 32]), "other").unwrap(),
            mint,
            program,
            None
        )
        .identity()
    );
}

#[test]
fn token_precision_is_explicitly_optional_and_amount_is_exact() {
    for decimals in [None, Some(0), Some(6), Some(255)] {
        let asset = TokenAsset::new(
            network("mainnet").unwrap(),
            Pubkey::from_bytes([1; 32]),
            Pubkey::parse(TOKEN).unwrap(),
            decimals,
        );
        let balance = TokenBalance::new(
            Pubkey::from_bytes([4; 32]),
            Pubkey::from_bytes([5; 32]),
            asset,
            u64::MAX,
            TokenAccountState::Frozen,
        );
        assert_eq!(balance.amount().raw(), U256::from(u64::MAX));
        assert_eq!(balance.amount().decimals(), decimals);
        assert_eq!(balance.state(), TokenAccountState::Frozen);
        let wire = serde_json::to_value(&balance).unwrap();
        if decimals.is_none() {
            assert_eq!(wire["asset"]["decimals"], Value::Null);
            assert_eq!(wire["amount"]["formatted"], Value::Null);
        }
        assert_eq!(
            serde_json::from_value::<TokenBalance>(wire).unwrap(),
            balance
        );
    }
}

#[test]
fn token_balance_deserialization_requires_precision_agreement_and_u64_width() {
    let asset = TokenAsset::new(
        network("mainnet").unwrap(),
        Pubkey::from_bytes([1; 32]),
        Pubkey::parse(TOKEN).unwrap(),
        None,
    );
    let balance = TokenBalance::new(
        Pubkey::from_bytes([4; 32]),
        Pubkey::from_bytes([5; 32]),
        asset,
        1,
        TokenAccountState::Initialized,
    );
    for amount in [
        Amount::new(U256::from(1), Some(18)),
        Amount::new(U256::from(u64::MAX) + U256::from(1), None),
    ] {
        let mut wire = serde_json::to_value(&balance).unwrap();
        wire["amount"] = serde_json::to_value(amount).unwrap();
        assert!(serde_json::from_value::<TokenBalance>(wire).is_err());
    }
    let mut wire = serde_json::to_value(balance.asset()).unwrap();
    wire.as_object_mut().unwrap().remove("decimals");
    assert!(serde_json::from_value::<TokenAsset>(wire).is_err());
}

#[test]
fn present_empty_account_is_distinct_from_explicit_absence() {
    let address = Pubkey::from_bytes([4; 32]);
    let account = Account::new(
        network("record").unwrap(),
        address,
        Pubkey::parse(SYSTEM).unwrap(),
        0,
        false,
        Vec::new(),
        u64::MAX,
    )
    .unwrap();
    assert_eq!(account.amount().formatted().as_deref(), Some("0.000000000"));
    assert!(account.data().is_empty());
    assert_eq!(account.rent_epoch(), u64::MAX);
    let present = AccountLookup::new(network("lookup").unwrap(), address, Some(account)).unwrap();
    let absent = AccountLookup::new(network("lookup").unwrap(), address, None).unwrap();
    assert!(present.account().is_some());
    assert!(absent.account().is_none());
    assert_ne!(present, absent);
    let absent_wire = serde_json::to_value(&absent).unwrap();
    assert_eq!(absent_wire["account"], Value::Null);
    assert_eq!(
        serde_json::from_value::<AccountLookup>(absent_wire.clone()).unwrap(),
        absent
    );
    let mut missing_field = absent_wire;
    missing_field.as_object_mut().unwrap().remove("account");
    assert!(serde_json::from_value::<AccountLookup>(missing_field).is_err());
    assert_eq!(
        serde_json::from_value::<AccountLookup>(serde_json::to_value(&present).unwrap()).unwrap(),
        present
    );
}

#[test]
fn account_lookup_identity_is_checked_by_constructor_and_deserializer() {
    let address = Pubkey::from_bytes([4; 32]);
    let account = Account::new(
        network("record").unwrap(),
        address,
        Pubkey::parse(SYSTEM).unwrap(),
        1,
        false,
        vec![1, 2, 3],
        0,
    )
    .unwrap();
    assert_eq!(
        AccountLookup::new(
            network("lookup").unwrap(),
            Pubkey::from_bytes([5; 32]),
            Some(account.clone())
        )
        .unwrap_err(),
        Error::Validation(ValidationError::InvalidSolanaAccount)
    );
    let other = Network::new(Hash::from_bytes([8; 32]), "other").unwrap();
    assert_eq!(
        AccountLookup::new(other, address, Some(account.clone())).unwrap_err(),
        Error::Validation(ValidationError::NetworkMismatch)
    );
    let lookup = AccountLookup::new(network("lookup").unwrap(), address, Some(account)).unwrap();
    let mut wire = serde_json::to_value(&lookup).unwrap();
    wire["account"]["address"] = json!(Pubkey::from_bytes([5; 32]).to_string());
    assert!(serde_json::from_value::<AccountLookup>(wire).is_err());
    let mut wire = serde_json::to_value(lookup).unwrap();
    wire["account"]["amount"] = serde_json::to_value(Amount::new(U256::from(1), Some(6))).unwrap();
    assert!(serde_json::from_value::<AccountLookup>(wire).is_err());
}

#[test]
fn account_data_limit_is_enforced_in_construction_and_streaming_deserialization() {
    let address = Pubkey::from_bytes([4; 32]);
    let account = Account::new(
        network("mainnet").unwrap(),
        address,
        address,
        0,
        true,
        vec![0; Account::MAX_DATA_BYTES],
        0,
    )
    .unwrap();
    assert_eq!(account.data().len(), 10 * 1024 * 1024);
    assert!(account.executable());
    assert_eq!(
        Account::new(
            network("mainnet").unwrap(),
            address,
            address,
            0,
            false,
            vec![0; Account::MAX_DATA_BYTES + 1],
            0
        )
        .unwrap_err(),
        Error::Validation(ValidationError::SolanaAccountDataTooLarge)
    );
    let empty = Account::new(
        network("mainnet").unwrap(),
        address,
        address,
        0,
        false,
        Vec::new(),
        0,
    )
    .unwrap();
    let wire = serde_json::to_string(&empty).unwrap().replace(
        "\"data\":[]",
        &format!("\"data\":[{}0]", "0,".repeat(Account::MAX_DATA_BYTES)),
    );
    let failure = serde_json::from_str::<Account>(&wire).unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("Solana account data exceeds byte limit")
    );
}

#[test]
fn actual_slot_is_distinct_from_requested_minimum_and_retrieval_time() {
    let options = ReadOptions::new(Commitment::Finalized, Some(100));
    for actual in [100, 101, u64::MAX] {
        let value = context(Operation::NativeBalance, actual, options).unwrap();
        assert_eq!(value.slot(), actual);
        assert_eq!(value.requested_options().minimum_context_slot(), Some(100));
        assert_eq!(
            value.requested_options().commitment(),
            Commitment::Finalized
        );
        assert_eq!(value.retrieved_at().unix_seconds(), 1234);
        assert_eq!(
            serde_json::from_value::<Context>(serde_json::to_value(&value).unwrap()).unwrap(),
            value
        );
    }
    assert_eq!(
        context(Operation::NativeBalance, 99, options).unwrap_err(),
        Error::Validation(ValidationError::ContextSlotBelowMinimum)
    );
    let mut wire =
        serde_json::to_value(context(Operation::NativeBalance, 100, options).unwrap()).unwrap();
    wire["slot"] = json!(99);
    assert!(serde_json::from_value::<Context>(wire).is_err());
    let mut missing = serde_json::to_value(ReadOptions::new(Commitment::Processed, None)).unwrap();
    assert_eq!(missing["minimum_context_slot"], Value::Null);
    missing
        .as_object_mut()
        .unwrap()
        .remove("minimum_context_slot");
    assert!(serde_json::from_value::<ReadOptions>(missing).is_err());
}

#[test]
fn native_observation_wire_retains_family_context_and_aliases() {
    let observation = native_observation().unwrap();
    let wire = serde_json::to_value(&observation).unwrap();
    assert_eq!(
        wire,
        json!({
            "schema_version":1,"operation":"native_balance","network":{"genesis_hash":GENESIS,"alias":"primary"},
            "requested_options":{"commitment":"confirmed","minimum_context_slot":100},"slot":120,
            "source":{"provider_id":"fixture","method":"getBalance","integration_version":"0.1.0"},"retrieved_at":1234,
            "value":{"network":{"genesis_hash":GENESIS,"alias":"display"},"address":Pubkey::from_bytes([7;32]).to_string(),
            "amount":{"raw":"9007199254740993","decimals":9,"formatted":"9007199.254740993"}}
        })
    );
    assert_eq!(
        serde_json::from_value::<Observation<NativeBalance>>(wire).unwrap(),
        observation
    );
    assert_eq!(observation.context().source().method(), "getBalance");
}

#[test]
fn observation_constructor_and_serde_validate_operation_genesis_schema_and_slot() {
    let value = native_observation().unwrap().value().clone();
    assert_eq!(
        Observation::native_balance(
            value.clone(),
            context(
                Operation::Account,
                120,
                ReadOptions::new(Commitment::Confirmed, None)
            )
            .unwrap()
        )
        .unwrap_err(),
        Error::Validation(ValidationError::ObservationOperationMismatch)
    );
    let mismatched = Context::new(
        Operation::NativeBalance,
        Network::new(Hash::from_bytes([8; 32]), "other").unwrap(),
        ReadOptions::new(Commitment::Confirmed, None),
        120,
        Source::new("fixture", "getBalance", "0.1.0").unwrap(),
        Timestamp::from_unix_seconds(1234),
    )
    .unwrap();
    assert_eq!(
        Observation::native_balance(value, mismatched).unwrap_err(),
        Error::Validation(ValidationError::NetworkMismatch)
    );
    for (path, replacement) in [
        ("schema_version", json!(2)),
        ("operation", json!("account")),
        ("slot", json!(99)),
    ] {
        let mut wire = serde_json::to_value(native_observation().unwrap()).unwrap();
        wire[path] = replacement;
        assert!(serde_json::from_value::<Observation<NativeBalance>>(wire).is_err());
    }
    let mut wire = serde_json::to_value(native_observation().unwrap()).unwrap();
    wire["value"]["network"]["genesis_hash"] = json!(Hash::from_bytes([8; 32]).to_string());
    assert!(serde_json::from_value::<Observation<NativeBalance>>(wire).is_err());
}

#[test]
fn strict_observation_deserializer_rejects_unknown_and_raw_duplicate_keys() {
    let wire = serde_json::to_string(&native_observation().unwrap()).unwrap();
    for prefix in [
        "\"schema_version\":1,",
        "\"slot\":120,",
        "\"value\":null,",
        "\"extra\":true,",
    ] {
        let manipulated = format!("{{{prefix}{}", &wire[1..]);
        assert!(serde_json::from_str::<Observation<NativeBalance>>(&manipulated).is_err());
    }
    let network_wire = format!(
        "{{\"genesis_hash\":\"{GENESIS}\",\"genesis_hash\":\"{GENESIS}\",\"alias\":\"a\"}}"
    );
    assert!(serde_json::from_str::<Network>(&network_wire).is_err());
}

#[test]
fn token_and_absent_account_observations_roundtrip_without_native_inference() {
    let asset = TokenAsset::new(
        network("display").unwrap(),
        Pubkey::from_bytes([1; 32]),
        Pubkey::parse(TOKEN).unwrap(),
        None,
    );
    let token = TokenBalance::new(
        Pubkey::from_bytes([4; 32]),
        Pubkey::from_bytes([5; 32]),
        asset,
        9_007_199_254_740_993,
        TokenAccountState::Initialized,
    );
    let token = Observation::token_balance(
        token,
        context(
            Operation::TokenBalance,
            0,
            ReadOptions::new(Commitment::Processed, None),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_value::<Observation<TokenBalance>>(serde_json::to_value(&token).unwrap())
            .unwrap(),
        token
    );
    let missing = AccountLookup::new(
        network("display").unwrap(),
        Pubkey::from_bytes([4; 32]),
        None,
    )
    .unwrap();
    let missing = Observation::account(
        missing,
        context(
            Operation::Account,
            5,
            ReadOptions::new(Commitment::Confirmed, Some(4)),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(missing.value().account().is_none());
    assert_eq!(
        serde_json::from_value::<Observation<AccountLookup>>(
            serde_json::to_value(&missing).unwrap()
        )
        .unwrap(),
        missing
    );
}

struct CallerReader {
    // The backend itself need not be Send or Sync when its returned future is.
    network: std::rc::Rc<Network>,
}

impl NativeBalanceReader for CallerReader {
    fn network(&self) -> &Network {
        &self.network
    }

    fn get_native_balance(
        &self,
        address: Pubkey,
        options: ReadOptions,
    ) -> impl Future<Output = Result<Observation<NativeBalance>, Error>> + Send {
        ready(
            Source::new("caller", "native", "0.1.0")
                .and_then(|source| {
                    Context::new(
                        Operation::NativeBalance,
                        (*self.network).clone(),
                        options,
                        200,
                        source,
                        Timestamp::from_unix_seconds(1000),
                    )
                })
                .and_then(|context| {
                    Observation::native_balance(
                        NativeBalance::new((*self.network).clone(), address, 42),
                        context,
                    )
                }),
        )
    }
}

#[test]
fn caller_implemented_reader_is_send_and_needs_no_async_runtime()
-> Result<(), Box<dyn std::error::Error>> {
    fn require_send<T: Send>(_: &T) {}
    let reader = CallerReader {
        network: std::rc::Rc::new(network("caller").unwrap()),
    };
    assert_eq!(reader.network().alias(), "caller");
    let address = Pubkey::from_bytes([3; 32]);
    let future =
        reader.get_native_balance(address, ReadOptions::new(Commitment::Confirmed, Some(100)));
    require_send(&future);
    let mut future = std::pin::pin!(future);
    let result = future
        .as_mut()
        .poll(&mut TaskContext::from_waker(Waker::noop()));
    let Poll::Ready(Ok(observation)) = result else {
        return Err("caller reader should immediately produce an observation".into());
    };
    assert_eq!(observation.value().address(), address);
    assert_eq!(observation.value().amount().raw(), U256::from(42));
    assert_eq!(observation.context().slot(), 200);
    Ok(())
}
