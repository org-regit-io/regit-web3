// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Exact unsigned EVM encoding and bounded signed-envelope qualification.

#![cfg(feature = "evm")]

use alloy_rlp::Header;
use regit_web3::{
    domain::evm::{
        AccessListEntry, Address, ChainId, Data, FeeTerms, PreparedTransaction, Quantity,
        SignedSubmission, TransactionCall, TransactionRequest, TransactionRequestData,
        TransferIntent, U256, Word,
    },
    error::{Error, ValidationError},
    wallets::{HandoffId, HandoffRequest, Preparation, PreparedRequest},
};
use serde_json::json;
use std::{
    future::{Future, ready},
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const SIGNED: &str = "0xf86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";

fn request(intent: TransferIntent, fees: FeeTerms) -> Result<TransactionRequest, Error> {
    TransactionRequest::new(TransactionRequestData {
        chain_id: ChainId::from(1),
        sender: Address::from_bytes([7; 20]),
        nonce: 9,
        gas_limit: 21_000,
        fees,
        intent,
    })
}
fn native() -> Result<TransferIntent, Error> {
    Ok(TransferIntent::Native {
        to: Address::parse("0x3535353535353535353535353535353535353535")?,
        value: Quantity::from_decimal("1000000000000000000")?,
    })
}
fn legacy() -> FeeTerms {
    FeeTerms::Legacy {
        gas_price: Quantity::from(20_000_000_000),
    }
}
fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    Header {
        list: true,
        payload_length: items.iter().map(Vec::len).sum(),
    }
    .encode(&mut out);
    for item in items {
        out.extend_from_slice(item);
    }
    out
}
fn uint(value: u64) -> Vec<u8> {
    alloy_rlp::encode(value)
}
fn raw(bytes: &[u8]) -> Vec<u8> {
    alloy_rlp::encode(bytes)
}
fn typed_fields(kind: u8) -> Vec<Vec<u8>> {
    let mut fields = vec![uint(1), uint(0), uint(2)];
    if kind == 2 {
        fields.push(uint(3));
    }
    fields.extend([
        uint(100_000),
        raw(&[5; 20]),
        uint(0),
        raw(&[]),
        rlp_list(&[]),
        uint(0),
        uint(1),
        uint(1),
    ]);
    fields
}
fn signed_typed(kind: u8, fields: &[Vec<u8>]) -> Result<SignedSubmission, Error> {
    let mut bytes = vec![kind];
    bytes.extend(rlp_list(fields));
    SignedSubmission::new(ChainId::from(1), Data::new(bytes)?)
}

#[test]
fn eip155_published_signing_bytes_and_digest_match_exactly() -> TestResult {
    let prepared = request(native()?, legacy())?.prepare()?;
    assert_eq!(
        prepared.unsigned().signing_payload().to_hex(),
        "0xec098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a764000080018080"
    );
    assert_eq!(
        prepared.unsigned().signing_hash().to_string(),
        "0xdaf5a779ae972f972197303d7b574746c7ef83eadac0f2791ad23db92e4c8e53"
    );
    let signed = SignedSubmission::new(ChainId::from(1), Data::parse(SIGNED)?)?;
    assert_eq!(signed.fields().nonce, 9);
    assert_eq!(signed.fields().to, Some(prepared.unsigned().to()));
    assert_eq!(signed.fields().value, prepared.unsigned().value());
    assert_eq!(signed.fields().fees, *prepared.unsigned().fees());
    assert!(!signed.fields().y_parity);
    assert_ne!(
        signed.transaction_id().bytes(),
        prepared.unsigned().signing_hash().bytes()
    );
    assert_eq!(
        SignedSubmission::new(ChainId::from(2), Data::parse(SIGNED)?),
        Err(Error::Validation(ValidationError::NetworkMismatch))
    );
    Ok(())
}

#[test]
fn token_transfer_and_approval_use_exact_raw_units_and_zero_native_value() -> TestResult {
    let contract = Address::from_bytes([1; 20]);
    let target = Address::from_bytes([2; 20]);
    for approval in [false, true] {
        let intent = if approval {
            TransferIntent::Erc20Approval {
                contract,
                spender: target,
                amount: Quantity::new(U256::MAX),
            }
        } else {
            TransferIntent::Erc20Transfer {
                contract,
                to: target,
                amount: Quantity::new(U256::MAX),
            }
        };
        let prepared = request(intent, legacy())?.prepare()?;
        let payload = prepared.unsigned();
        assert_eq!(payload.to(), contract);
        assert_eq!(payload.value(), Quantity::from(0));
        let mut expected = if approval {
            vec![0x09, 0x5e, 0xa7, 0xb3]
        } else {
            vec![0xa9, 0x05, 0x9c, 0xbb]
        };
        expected.extend([0; 12]);
        expected.extend(target.bytes());
        expected.extend([0xff; 32]);
        assert_eq!(payload.input().bytes(), expected);
        assert_eq!(
            TransactionCall::from_prepared(&prepared).data().input,
            payload.input().clone()
        );
    }
    let revoked = request(
        TransferIntent::Erc20Approval {
            contract,
            spender: target,
            amount: Quantity::from(0),
        },
        legacy(),
    )?
    .prepare()?;
    assert_eq!(&revoked.unsigned().input().bytes()[36..], &[0; 32]);
    Ok(())
}

#[test]
fn typed_envelopes_preserve_legal_ordered_access_list_duplicates() -> TestResult {
    let entry = AccessListEntry {
        address: Address::from_bytes([3; 20]),
        storage_keys: vec![Word::from_bytes([4; 32]); 2],
    };
    for kind in [1, 2] {
        let fees = if kind == 1 {
            FeeTerms::AccessList {
                gas_price: Quantity::from(2),
                access_list: vec![entry.clone(); 2],
            }
        } else {
            FeeTerms::DynamicFee {
                max_fee_per_gas: Quantity::from(3),
                max_priority_fee_per_gas: Quantity::from(2),
                access_list: vec![entry.clone(); 2],
            }
        };
        let prepared = request(native()?, fees.clone())?.prepare()?;
        assert_eq!(prepared.unsigned().signing_payload().bytes()[0], kind);
        let mut fields = typed_fields(kind);
        let key = raw(&[4; 32]);
        let access_entry = rlp_list(&[raw(&[3; 20]), rlp_list(&[key.clone(), key])]);
        let index = if kind == 1 { 7 } else { 8 };
        fields[index] = rlp_list(&[access_entry.clone(), access_entry]);
        let signed = signed_typed(kind, &fields)?;
        assert_eq!(signed.fields().fees, fees);
        assert!(signed.fields().input.bytes().is_empty());
    }
    Ok(())
}

#[test]
fn preparation_and_signed_serde_rebuild_checked_snapshots_without_opaque_debug() -> TestResult {
    let prepared = request(native()?, legacy())?.prepare()?;
    let serialized = serde_json::to_value(&prepared)?;
    assert_eq!(
        serde_json::from_value::<PreparedTransaction>(serialized.clone())?,
        prepared
    );
    for (field, value) in [
        ("nonce", json!(u64::MAX)),
        ("gas_limit", json!(0)),
        ("extra", json!("SECRET")),
    ] {
        let mut bad = serialized.clone();
        bad[field] = value;
        assert!(serde_json::from_value::<PreparedTransaction>(bad).is_err());
    }
    let signed = SignedSubmission::new(ChainId::from(1), Data::parse(SIGNED)?)?;
    let mut value = serde_json::to_value(&signed)?;
    assert_eq!(
        serde_json::from_value::<SignedSubmission>(value.clone())?,
        signed
    );
    value["transaction_id"] = json!(signed.transaction_id().to_string());
    assert!(serde_json::from_value::<SignedSubmission>(value).is_err());
    for debug in [
        format!("{prepared:?}"),
        format!("{:?}", prepared.unsigned()),
        format!("{signed:?}"),
        format!("{:?}", TransactionCall::from_prepared(&prepared)),
    ] {
        assert!(!debug.contains("353535"));
        assert!(!debug.contains("f86c"));
    }
    let handoff = HandoffRequest::new(
        HandoffId::new("caller-owned-001")?,
        PreparedRequest::new(prepared.clone())?,
    );
    assert_eq!(
        handoff.prepared().preparation().network(),
        &ChainId::from(1)
    );
    Ok(())
}

#[test]
fn explicit_preparation_rejects_bad_fee_nonce_gas_and_access_bounds() -> TestResult {
    let data = request(native()?, legacy())?.data().clone();
    let mut bad = data.clone();
    bad.nonce = u64::MAX;
    assert!(TransactionRequest::new(bad).is_err());
    let mut bad = data.clone();
    bad.gas_limit = 0;
    assert!(TransactionRequest::new(bad).is_err());
    let mut bad = data.clone();
    bad.chain_id = ChainId::new(U256::MAX);
    assert!(TransactionRequest::new(bad).is_err());
    let bad = FeeTerms::DynamicFee {
        max_fee_per_gas: Quantity::from(1),
        max_priority_fee_per_gas: Quantity::from(2),
        access_list: vec![],
    };
    assert!(request(native()?, bad).is_err());
    let entry = AccessListEntry {
        address: Address::from_bytes([1; 20]),
        storage_keys: vec![],
    };
    assert!(
        request(
            native()?,
            FeeTerms::AccessList {
                gas_price: Quantity::from(0),
                access_list: vec![entry.clone(); 1025]
            }
        )
        .is_err()
    );
    let mut entry = entry;
    entry.storage_keys = vec![Word::from_bytes([2; 32]); 4097];
    assert!(
        request(
            native()?,
            FeeTerms::AccessList {
                gas_price: Quantity::from(0),
                access_list: vec![entry]
            }
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn signed_decoder_rejects_noncanonical_recursive_wrong_width_and_scalar_vectors() -> TestResult {
    let valid = typed_fields(2);
    for (index, replacement) in [
        (0, raw(&[0, 1])),
        (1, vec![0]),
        (1, raw(&[0xff; 9])),
        (4, uint(0)),
        (5, raw(&[5; 19])),
        (6, raw(&[1; 33])),
        (8, rlp_list(&[rlp_list(&[])])),
        (9, uint(2)),
        (10, uint(0)),
        (11, uint(0)),
        (11, raw(&[0xff; 32])),
        (10, vec![0x81, 0x01]),
        (1, rlp_list(&[uint(1)])),
    ] {
        let mut fields = valid.clone();
        fields[index] = replacement;
        assert!(signed_typed(2, &fields).is_err(), "field{index}");
    }
    let mut fields = valid.clone();
    fields.push(uint(0));
    assert!(signed_typed(2, &fields).is_err());
    let mut fields = valid.clone();
    fields.pop();
    assert!(signed_typed(2, &fields).is_err());
    let mut bytes = vec![2];
    bytes.extend(rlp_list(&valid));
    bytes.push(0);
    assert!(SignedSubmission::new(ChainId::from(1), Data::new(bytes)?).is_err());
    for bytes in [
        vec![],
        vec![2, 0xf8, 1, 0xc0],
        vec![2, 0xf9, 0, 60],
        vec![2, 0xff, 255, 255, 255, 255, 255, 255, 255, 255],
    ] {
        assert!(SignedSubmission::new(ChainId::from(1), Data::new(bytes)?).is_err());
    }
    for kind in [0, 3, 4, 0x7f] {
        assert_eq!(
            SignedSubmission::new(ChainId::from(1), Data::new(vec![kind, 0xc0])?),
            Err(Error::UnsupportedCapability)
        );
    }
    let mut legacy_fields = vec![
        uint(0),
        uint(1),
        uint(21_000),
        raw(&[5; 20]),
        uint(0),
        raw(&[]),
        uint(27),
        uint(1),
        uint(1),
    ];
    assert_eq!(
        SignedSubmission::new(ChainId::from(1), Data::new(rlp_list(&legacy_fields))?),
        Err(Error::UnsupportedCapability)
    );
    legacy_fields[6] = uint(34);
    assert!(SignedSubmission::new(ChainId::from(1), Data::new(rlp_list(&legacy_fields))?).is_err());
    Ok(())
}

struct StructuralRejectionVerifier(AtomicUsize);
impl regit_web3::wallets::SignedPayloadVerifier<PreparedTransaction, SignedSubmission>
    for StructuralRejectionVerifier
{
    fn verify_binding(
        &self,
        prepared: &PreparedRequest<PreparedTransaction>,
        signed: &SignedSubmission,
    ) -> impl Future<Output = Result<regit_web3::wallets::VerificationDecision, Error>> + Send {
        self.0.fetch_add(1, Ordering::SeqCst);
        let unsigned = prepared.preparation().unsigned();
        let actual = signed.fields();
        let equal = actual.chain_id == unsigned.chain_id()
            && actual.nonce == unsigned.nonce()
            && actual.gas_limit == unsigned.gas_limit()
            && actual.to == Some(unsigned.to())
            && actual.value == unsigned.value()
            && actual.input == *unsigned.input()
            && actual.fees == *unsigned.fees();
        // This fixture can reject structural mismatch. Even full field equality
        // cannot confirm recovered sender/signature, so it remains unsupported.
        ready(if equal {
            Err(Error::UnsupportedCapability)
        } else {
            Ok(regit_web3::wallets::VerificationDecision::Rejected)
        })
    }
}
fn poll_ready<F: Future + Send>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => unreachable!("ready caller fixture"),
    }
}

#[test]
fn evm_handoff_checks_correlation_before_actual_signed_content_verification() -> TestResult {
    use regit_web3::wallets::{HandoffResponse, verify_handoff};
    let prepared = request(native()?, legacy())?.prepare()?;
    let original = PreparedRequest::new(prepared.clone())?;
    let handoff = HandoffRequest::new(HandoffId::new("reviewed-1")?, original.clone());
    let verifier = StructuralRejectionVerifier(AtomicUsize::new(0));
    for altered in [0, 1, 2, 3] {
        let mut data = prepared.request().data().clone();
        if altered == 1 {
            data.chain_id = ChainId::from(2);
        }
        if altered == 2 {
            data.intent = TransferIntent::Native {
                to: Address::from_bytes([5; 20]),
                value: Quantity::from(0),
            };
        }
        if altered == 3 {
            data.gas_limit += 1;
        }
        let echoed = PreparedRequest::new(TransactionRequest::new(data)?.prepare()?)?;
        let id = HandoffId::new(if altered == 0 {
            "other-id"
        } else {
            "reviewed-1"
        })?;
        let response = HandoffResponse::new(
            id,
            echoed,
            SignedSubmission::new(ChainId::from(1), Data::parse(SIGNED)?)?,
        );
        assert!(matches!(
            poll_ready(verify_handoff(&handoff, response, &verifier)),
            Err(Error::Validation(ValidationError::WalletBindingMismatch))
        ));
    }
    assert_eq!(verifier.0.load(Ordering::SeqCst), 0);
    let different_signed = signed_typed(2, &typed_fields(2))?;
    let response = HandoffResponse::new(handoff.id().clone(), original.clone(), different_signed);
    assert!(matches!(
        poll_ready(verify_handoff(&handoff, response, &verifier)),
        Err(Error::Validation(ValidationError::SignedPayloadRejected))
    ));
    assert_eq!(verifier.0.load(Ordering::SeqCst), 1);
    let response = HandoffResponse::new(
        handoff.id().clone(),
        original,
        SignedSubmission::new(ChainId::from(1), Data::parse(SIGNED)?)?,
    );
    assert!(matches!(
        poll_ready(verify_handoff(&handoff, response, &verifier)),
        Err(Error::UnsupportedCapability)
    ));
    assert_eq!(verifier.0.load(Ordering::SeqCst), 2);
    Ok(())
}

#[test]
fn signed_access_lists_reject_excess_entry_key_and_nested_shape_bounds() -> TestResult {
    let valid = typed_fields(2);
    let key = raw(&[4; 32]);
    let entry = rlp_list(&[raw(&[3; 20]), rlp_list(std::slice::from_ref(&key))]);
    for access in [
        rlp_list(&vec![entry.clone(); 1025]),
        rlp_list(&[rlp_list(&[raw(&[3; 20]), rlp_list(&vec![key; 4097])])]),
        rlp_list(&[rlp_list(&[
            raw(&[3; 20]),
            rlp_list(&[rlp_list(&[uint(1)])]),
        ])]),
        rlp_list(&[rlp_list(&[raw(&[3; 20]), rlp_list(&[raw(&[4; 31])])])]),
        rlp_list(&[rlp_list(&[raw(&[]), rlp_list(&[])])]),
    ] {
        let mut fields = valid.clone();
        fields[8] = access;
        assert!(signed_typed(2, &fields).is_err());
    }
    let mut fields = valid.clone();
    fields[1] = uint(u64::MAX);
    assert!(signed_typed(2, &fields).is_err());
    fields = valid.clone();
    fields[3] = uint(1);
    assert!(signed_typed(2, &fields).is_err());
    let mut fields = valid;
    fields[5] = raw(&[]);
    assert_eq!(signed_typed(2, &fields)?.fields().to, None);
    Ok(())
}
