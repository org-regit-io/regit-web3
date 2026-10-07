// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Offline canonical transaction and indexed-fact contracts through public APIs.

#![cfg(feature = "bitcoin")]

use std::{
    future::{Future, ready},
    task::{Context as TaskContext, Poll, Waker},
};

use bitcoin::{Amount as BitcoinAmount, ScriptBuf, Sequence, Witness, absolute, transaction};
use regit_web3::{
    chains::bitcoin::TransactionReader,
    domain::{
        Source, Timestamp,
        bitcoin::{
            Bytes, Context, Network, NetworkId, Observation, Operation, OutPoint, PreviousOutput,
            Satoshis, Transaction, TransactionBody, TransactionStatus, Txid,
        },
    },
    error::{Error, ValidationError},
};

const GENESIS_HEX: &str = include_str!("fixtures/bitcoin/genesis.hex");
const GENESIS_TXID: &str = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";
const SEGWIT_HEX: &str = include_str!("fixtures/bitcoin/segwit.hex");
const SEGWIT_TXID: &str = "14396c6a212fce4a794501b5840c718a700daa9e462427fed22a7f38d5197b1e";

fn raw_transaction(outputs: &[u64]) -> bitcoin::Transaction {
    bitcoin::Transaction {
        version: transaction::Version(i32::MIN),
        lock_time: absolute::LockTime::from_consensus(u32::MAX),
        input: vec![bitcoin::TxIn {
            previous_output: bitcoin::OutPoint {
                txid: bitcoin::Txid::from_raw_hash(bitcoin::hashes::Hash::from_byte_array([1; 32])),
                vout: u32::MAX,
            },
            script_sig: ScriptBuf::from_bytes(vec![0x51]),
            sequence: Sequence(u32::MAX),
            witness: Witness::new(),
        }],
        output: outputs
            .iter()
            .map(|value| bitcoin::TxOut {
                value: BitcoinAmount::from_sat(*value),
                script_pubkey: ScriptBuf::from_bytes(vec![0x6a]),
            })
            .collect(),
    }
}

fn body(outputs: &[u64]) -> Result<TransactionBody, Error> {
    TransactionBody::from_bytes(&bitcoin::consensus::serialize(&raw_transaction(outputs)))
}

#[test]
fn genesis_retains_coinbase_identity_fields_and_nonaddress_script()
-> Result<(), Box<dyn std::error::Error>> {
    let body = TransactionBody::from_hex(GENESIS_HEX)?;
    assert_eq!(body.txid().to_string(), GENESIS_TXID);
    assert_eq!(body.wtxid().to_string(), GENESIS_TXID);
    assert_eq!(body.version(), 1);
    assert_eq!(body.lock_time(), 0);
    assert!(body.is_coinbase());
    assert_eq!((body.size(), body.weight(), body.vsize()), (204, 816, 204));
    let input = body.inputs().next().ok_or(Error::UnavailableData)?;
    assert_eq!(input.previous_output(), None);
    assert_eq!(input.sequence(), u32::MAX);
    assert_eq!(input.witness().len(), 0);
    assert!((2..=100).contains(&input.script_sig().len()));
    let output = body.outputs().next().ok_or(Error::UnavailableData)?;
    assert_eq!(output.value().raw(), 5_000_000_000);
    assert_eq!(output.script_pubkey().len(), 67);
    assert_eq!(output.address(Network::Mainnet), None);
    let encoded = serde_json::to_string(&body)?;
    assert_eq!(serde_json::from_str::<TransactionBody>(&encoded)?, body);
    let transaction = Transaction::new(
        Network::Mainnet,
        body,
        vec![None],
        Some(Satoshis::new(0)),
        TransactionStatus::Unconfirmed,
    )?;
    assert_eq!(transaction.fee_from_previous_outputs(), None);
    assert_eq!(transaction.reported_fee(), Some(Satoshis::new(0)));
    Ok(())
}

#[test]
fn segwit_preserves_witness_and_network_independent_body() -> Result<(), Box<dyn std::error::Error>>
{
    let body = TransactionBody::from_hex(SEGWIT_HEX)?;
    assert_eq!(body.txid().to_string(), SEGWIT_TXID);
    assert_ne!(body.txid().to_string(), body.wtxid().to_string());
    assert_eq!((body.size(), body.weight(), body.vsize()), (303, 882, 221));
    assert!(!body.is_coinbase());
    let input = body.inputs().next().ok_or(Error::UnavailableData)?;
    assert!(input.previous_output().is_some());
    assert_eq!(input.witness().len(), 2);
    let output = body
        .outputs()
        .find(|output| output.address(Network::Mainnet).is_some())
        .ok_or(Error::UnavailableData)?;
    let mainnet = output
        .address(Network::Mainnet)
        .ok_or(Error::UnavailableData)?;
    let testnet = output
        .address(Network::Testnet4)
        .ok_or(Error::UnavailableData)?;
    assert_eq!(mainnet.network(), Network::Mainnet);
    assert_eq!(testnet.network(), Network::Testnet4);
    assert_ne!(mainnet.to_string(), testnet.to_string());
    assert_eq!(TransactionBody::from_bytes(&body.to_bytes())?, body);
    Ok(())
}

#[test]
fn canonical_bytes_and_raw_serde_reject_malformed_trailing_and_unbounded_inputs()
-> Result<(), Error> {
    let body = TransactionBody::from_hex(GENESIS_HEX)?;
    let mut trailing = body.to_bytes();
    trailing.push(0);
    assert_eq!(
        TransactionBody::from_bytes(&trailing),
        Err(ValidationError::InvalidBitcoinTransaction.into())
    );
    let mut noncanonical = body.to_bytes();
    noncanonical.splice(4..5, [0xfd, 1, 0]);
    assert!(TransactionBody::from_bytes(&noncanonical).is_err());
    for invalid in ["0", "AA", "0x00", "00\n", "secret-provider-body"] {
        assert_eq!(
            Bytes::from_hex(invalid),
            Err(ValidationError::InvalidBitcoinBytes.into())
        );
    }
    assert!(Bytes::from_hex("")?.as_slice().is_empty());
    assert_eq!(
        Bytes::new(vec![0; Bytes::MAX_LEN + 1]),
        Err(ValidationError::InvalidBitcoinBytes.into())
    );
    assert!(TransactionBody::from_bytes(&vec![0; Bytes::MAX_LEN + 1]).is_err());
    assert!(
        serde_json::from_str::<Bytes>(&format!("\"{}\"", "00".repeat(Bytes::MAX_LEN + 1))).is_err()
    );
    Ok(())
}

#[test]
fn structural_null_duplicate_coinbase_and_empty_output_failures_are_explicit() {
    let mut transaction = raw_transaction(&[1]);
    transaction.output.clear();
    assert!(TransactionBody::from_bytes(&bitcoin::consensus::serialize(&transaction)).is_err());
    transaction = raw_transaction(&[1]);
    transaction.input.push(transaction.input[0].clone());
    assert!(TransactionBody::from_bytes(&bitcoin::consensus::serialize(&transaction)).is_err());
    transaction.input[1].previous_output = bitcoin::OutPoint::null();
    assert!(TransactionBody::from_bytes(&bitcoin::consensus::serialize(&transaction)).is_err());
    for length in [1, 101] {
        let mut coinbase = raw_transaction(&[1]);
        coinbase.input[0].previous_output = bitcoin::OutPoint::null();
        coinbase.input[0].script_sig = ScriptBuf::from_bytes(vec![0; length]);
        assert!(TransactionBody::from_bytes(&bitcoin::consensus::serialize(&coinbase)).is_err());
    }
}

#[test]
fn exact_money_and_fee_arithmetic_rejects_wide_output_and_overflow_without_panics()
-> Result<(), Error> {
    let maximum = BitcoinAmount::MAX_MONEY.to_sat();
    let body = body(&[maximum - 1])?;
    assert_eq!(
        body.outputs()
            .next()
            .ok_or(Error::UnavailableData)?
            .value()
            .raw(),
        maximum - 1
    );
    assert_eq!((body.version(), body.lock_time()), (i32::MIN, u32::MAX));
    let previous = PreviousOutput::new(Satoshis::new(maximum), Bytes::from_hex("51")?)?;
    let transaction = Transaction::new(
        Network::Mainnet,
        body.clone(),
        vec![Some(previous.clone())],
        Some(Satoshis::new(1)),
        TransactionStatus::Unconfirmed,
    )?;
    assert_eq!(
        transaction.fee_from_previous_outputs(),
        Some(Satoshis::new(1))
    );
    for (fee, valid) in [(maximum, false), (2, false), (1, true)] {
        assert_eq!(
            Transaction::new(
                Network::Mainnet,
                body.clone(),
                vec![None],
                Some(Satoshis::new(fee)),
                TransactionStatus::Unconfirmed
            )
            .is_ok(),
            valid
        );
    }
    assert!(
        Transaction::new(
            Network::Mainnet,
            body,
            vec![Some(previous)],
            Some(Satoshis::new(2)),
            TransactionStatus::Unconfirmed
        )
        .is_err()
    );
    for outputs in [
        vec![maximum + 1],
        vec![9_007_199_254_740_993],
        vec![u64::MAX],
        vec![u64::MAX, 1],
        vec![maximum, maximum],
    ] {
        assert_eq!(
            TransactionBody::from_bytes(&bitcoin::consensus::serialize(&raw_transaction(&outputs))),
            Err(ValidationError::InvalidBitcoinTransaction.into())
        );
    }
    Ok(())
}

#[test]
fn known_prevout_lower_bound_rejects_impossible_fee_with_an_unknown_input()
-> Result<(), Box<dyn std::error::Error>> {
    let mut raw = raw_transaction(&[5]);
    let mut second = raw.input[0].clone();
    second.previous_output.vout = 0;
    raw.input.push(second);
    let body = TransactionBody::from_bytes(&bitcoin::consensus::serialize(&raw))?;
    let known = PreviousOutput::new(Satoshis::new(10), Bytes::from_hex("51")?)?;
    let previous_outputs = vec![Some(known), None];
    for fee in [None, Some(Satoshis::new(5)), Some(Satoshis::new(6))] {
        let transaction = Transaction::new(
            Network::Mainnet,
            body.clone(),
            previous_outputs.clone(),
            fee,
            TransactionStatus::Unconfirmed,
        )?;
        assert_eq!(transaction.previous_outputs()[1], None);
        assert_eq!(transaction.fee_from_previous_outputs(), None);
        assert_eq!(
            serde_json::from_str::<Transaction>(&serde_json::to_string(&transaction)?)?,
            transaction
        );
        let mut invalid = serde_json::to_value(&transaction)?;
        invalid["reported_fee"] = serde_json::Value::String("1".to_owned());
        assert!(serde_json::from_value::<Transaction>(invalid).is_err());
    }
    assert_eq!(
        Transaction::new(
            Network::Mainnet,
            body,
            previous_outputs,
            Some(Satoshis::new(1)),
            TransactionStatus::Unconfirmed,
        ),
        Err(ValidationError::InvalidBitcoinTransaction.into())
    );
    Ok(())
}

#[test]
fn absent_index_facts_stay_nullable_and_constructor_serde_identity_agree()
-> Result<(), Box<dyn std::error::Error>> {
    let body = TransactionBody::from_hex(SEGWIT_HEX)?;
    let transaction = Transaction::new(
        Network::Mainnet,
        body.clone(),
        vec![None],
        None,
        TransactionStatus::Unconfirmed,
    )?;
    assert_eq!(transaction.reported_fee(), None);
    assert_eq!(transaction.fee_from_previous_outputs(), None);
    assert!(!transaction.body().is_coinbase());
    let context = Context::new(
        NetworkId::new(Network::Mainnet, "main")?,
        Operation::Transaction { txid: body.txid() },
        Source::new("fixture", "tx-with-hex", "0.1.0")?,
        Timestamp::from_unix_seconds(100),
    )?;
    let observation = Observation::transaction(transaction.clone(), context)?;
    assert_eq!(
        serde_json::from_str::<Observation<Transaction>>(&serde_json::to_string(&observation)?)?,
        observation
    );
    let mut value = serde_json::to_value(&transaction)?;
    value
        .as_object_mut()
        .ok_or(Error::Configuration)?
        .remove("reported_fee");
    assert!(serde_json::from_value::<Transaction>(value).is_err());
    let mismatch = Context::new(
        NetworkId::new(Network::Testnet3, "test")?,
        Operation::Transaction { txid: body.txid() },
        Source::new("fixture", "tx-with-hex", "0.1.0")?,
        Timestamp::from_unix_seconds(100),
    )?;
    assert_eq!(
        Observation::transaction(transaction, mismatch),
        Err(ValidationError::NetworkMismatch.into())
    );
    assert!(OutPoint::new(Txid::parse(&"0".repeat(64))?, u32::MAX).is_err());
    Ok(())
}

#[test]
fn caller_transaction_reader_requires_no_runtime_or_reader_send_sync() -> Result<(), Error> {
    struct Reader {
        value: Observation<Transaction>,
        _local: std::rc::Rc<()>,
    }
    impl TransactionReader for Reader {
        fn get_transaction(
            &self,
            _txid: Txid,
        ) -> impl Future<Output = Result<Observation<Transaction>, Error>> + Send {
            ready(Ok(self.value.clone()))
        }
    }
    let body = TransactionBody::from_hex(GENESIS_HEX)?;
    let txid = body.txid();
    let value = Transaction::new(
        Network::Mainnet,
        body,
        vec![None],
        None,
        TransactionStatus::Unconfirmed,
    )?;
    let context = Context::new(
        NetworkId::new(Network::Mainnet, "main")?,
        Operation::Transaction { txid },
        Source::new("fixture", "tx-with-hex", "0.1.0")?,
        Timestamp::from_unix_seconds(100),
    )?;
    let reader = Reader {
        value: Observation::transaction(value, context)?,
        _local: std::rc::Rc::new(()),
    };
    let future = reader.get_transaction(txid);
    let mut future = std::pin::pin!(future);
    let mut context = TaskContext::from_waker(Waker::noop());
    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Ok(_))
    ));
    Ok(())
}
