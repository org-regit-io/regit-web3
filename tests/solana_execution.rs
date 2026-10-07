// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure canonical Solana execution/preparation invariants and public source vectors.

#![cfg(test)]
#![cfg(feature = "solana")]

use regit_web3::{
    domain::{Source, Timestamp, solana::*},
    error::{Error, ValidationError},
    wallets::PreparedRequest,
};
use serde_json::json;
type TestResult = Result<(), Box<dyn std::error::Error>>;
const GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
fn network() -> Result<Network, Error> {
    Network::new(Hash::parse(GENESIS)?, "mainnet")
}
fn preparation() -> Result<TransferPreparation, Error> {
    TransferPreparation::new(
        network()?,
        TransferIntent::Native {
            fee_payer: Pubkey::from_bytes([1; 32]),
            sender: Pubkey::from_bytes([1; 32]),
            recipient: Pubkey::from_bytes([2; 32]),
            lamports: 9_007_199_254_740_993,
        },
        BlockhashLifetime {
            blockhash: Hash::from_bytes([3; 32]),
            last_valid_block_height: 99,
        },
    )
}
fn signed_legacy() -> Result<SignedTransaction, Box<dyn std::error::Error>> {
    let message = preparation()?
        .unsigned_transaction()
        .message()
        .decoded_message();
    let transaction = solana_transaction::versioned::VersionedTransaction {
        message,
        signatures: vec![solana_signature::Signature::from([7; 64])],
    };
    Ok(SignedTransaction::from_bytes(wincode::serialize(
        &transaction,
    )?)?)
}
fn context(request: ExecutionRequest, slot: Option<u64>) -> Result<ExecutionContext, Error> {
    ExecutionContext::new(
        network()?,
        request,
        slot,
        Source::new("fixture", "method", "0.1.0")?,
        Timestamp::from_unix_seconds(100),
    )
}
fn metadata(count: usize) -> TransactionMetadataData {
    TransactionMetadataData {
        outcome: ExecutionOutcome::Succeeded,
        fee_lamports: 9_007_199_254_740_993,
        pre_balances: vec![u64::MAX; count],
        post_balances: vec![u64::MAX; count],
        loaded_addresses: None,
        pre_token_balances: None,
        post_token_balances: None,
        inner_instructions: None,
        log_messages: None,
        return_data: None,
        rewards: None,
        compute_units_consumed: Some(u64::MAX),
        cost_units: None,
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn canonical_legacy_preparation_encodes_exact_system_transfer_and_wallet_review() -> TestResult {
    let p = preparation()?;
    let m = p.unsigned_transaction().message().decoded_message();
    assert_eq!(
        p.unsigned_transaction().message().version(),
        TransactionVersion::Legacy
    );
    assert_eq!(m.instructions().len(), 1);
    assert_eq!(
        m.instructions()[0].data,
        [
            2_u32.to_le_bytes().as_slice(),
            9_007_199_254_740_993_u64.to_le_bytes().as_slice()
        ]
        .concat()
    );
    assert_eq!(p.lifetime().last_valid_block_height, 99);
    assert_eq!(
        p.unsigned_transaction().message().recent_blockhash(),
        p.lifetime().blockhash
    );
    let request = PreparedRequest::new(p.clone())?;
    assert_eq!(
        request.review().unsigned_payload(),
        p.unsigned_transaction()
    );
    assert_eq!(
        serde_json::from_value::<TransferPreparation>(serde_json::to_value(&p)?)?,
        p
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn classic_spl_transfer_checked_has_exact_amount_precision_and_explicit_accounts() -> TestResult {
    let p = TransferPreparation::new(
        network()?,
        TransferIntent::TokenChecked {
            fee_payer: Pubkey::from_bytes([1; 32]),
            source_account: Pubkey::from_bytes([2; 32]),
            mint: Pubkey::from_bytes([3; 32]),
            destination_account: Pubkey::from_bytes([4; 32]),
            authority: Pubkey::from_bytes([5; 32]),
            raw_amount: u64::MAX,
            decimals: 255,
        },
        BlockhashLifetime {
            blockhash: Hash::from_bytes([6; 32]),
            last_valid_block_height: u64::MAX,
        },
    )?;
    let m = p.unsigned_transaction().message().decoded_message();
    assert_eq!(p.unsigned_transaction().message().required_signatures(), 2);
    let i = &m.instructions()[0];
    assert_eq!(
        i.data,
        [&[12], u64::MAX.to_le_bytes().as_slice(), &[255]].concat()
    );
    let a = p.unsigned_transaction().message().static_accounts();
    assert_eq!(
        a[usize::from(i.program_id_index)],
        Pubkey::parse("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")?
    );
    assert_eq!(
        i.accounts
            .iter()
            .map(|i| a[usize::from(*i)])
            .collect::<Vec<_>>(),
        [
            Pubkey::from_bytes([2; 32]),
            Pubkey::from_bytes([3; 32]),
            Pubkey::from_bytes([4; 32]),
            Pubkey::from_bytes([5; 32])
        ]
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn preparation_serde_rejects_changed_intent_or_message_but_preserves_unencoded_height() -> TestResult
{
    let p = preparation()?;
    let original = serde_json::to_value(&p)?;
    for (key, value) in [
        ("lamports", json!(1)),
        ("recipient", json!(Pubkey::from_bytes([9; 32]))),
    ] {
        let mut bad = original.clone();
        bad["intent"]["transfer"][key] = value;
        assert!(serde_json::from_value::<TransferPreparation>(bad).is_err());
    }
    let mut height = original;
    height["intent"]["lifetime"]["last_valid_block_height"] = json!(100);
    let changed: TransferPreparation = serde_json::from_value(height)?;
    assert_ne!(p, changed);
    assert_eq!(p.unsigned_transaction(), changed.unsigned_transaction());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn unsigned_and_structurally_signed_classes_are_distinct_without_crypto_claim() -> TestResult {
    let p = preparation()?;
    let unsigned = p.unsigned_transaction();
    let signed = signed_legacy()?;
    assert!(SignedTransaction::from_bytes(unsigned.bytes().to_vec()).is_err());
    assert!(UnsignedTransaction::from_bytes(signed.bytes().to_vec()).is_err());
    assert_eq!(signed.signature(), Signature::from_bytes([7; 64]));
    assert_eq!(signed.message(), unsigned.message());
    assert_eq!(
        serde_json::from_value::<SignedTransaction>(serde_json::to_value(&signed)?)?,
        signed
    );
    assert_eq!(
        serde_json::from_value::<UnsignedTransaction>(serde_json::to_value(unsigned)?)?,
        *unsigned
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn malformed_trailing_noncanonical_and_oversized_byte_inputs_are_fixed_errors() -> TestResult {
    let p = preparation()?;
    let bytes = p.unsigned_transaction().bytes();
    for v in [
        vec![],
        vec![0; 4097],
        {
            let mut v = bytes.to_vec();
            v.push(0);
            v
        },
        {
            let mut v = bytes.to_vec();
            v.splice(0..1, [0x81, 0]);
            v
        },
        {
            let mut v = bytes.to_vec();
            v[0] = 0;
            v
        },
    ] {
        assert_eq!(
            UnsignedTransaction::from_bytes(v).unwrap_err(),
            Error::Validation(ValidationError::InvalidSolanaTransaction)
        );
    }
    for hex in ["ABCDEF", "0x00", "0", "zz"] {
        assert!(serde_json::from_value::<UnsignedMessage>(json!(hex)).is_err());
    }
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn actual_mainnet_v1_source_exceeds_legacy_size_and_retains_real_signature() -> TestResult {
    let bytes = const_hex::decode(
        include_str!("fixtures/solana_execution/mainnet_v1_transaction.hex").trim(),
    )?;
    assert!(bytes.len() > 1232);
    let tx = SignedTransaction::from_bytes(bytes)?;
    assert_eq!(tx.message().version(), TransactionVersion::V1);
    assert_eq!(
        tx.signature(),
        Signature::parse(
            "4ofUzrGeDGsRvg7oTzC133nSACkXMP9vagDw3RMyswa2AsAB9Vp9Vyh7EAY2WudZDjf43NJkN1KJLyo2mVYDvg5x"
        )?
    );
    assert_eq!(
        tx.message().account_count(),
        tx.message().static_accounts().len()
    );
    println!("source_v1_fee_payer={}", tx.message().static_accounts()[0]);
    let u = UnsignedTransaction::from_message(tx.message().clone())?;
    assert_eq!(u.message(), tx.message());
    assert_eq!(u.bytes().len(), tx.bytes().len());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn maintained_v0_lookup_message_roundtrips_without_inventing_loaded_addresses() -> TestResult {
    let solana_message::VersionedMessage::Legacy(m) = preparation()?
        .unsigned_transaction()
        .message()
        .decoded_message()
    else {
        return Err("expected legacy".into());
    };
    let v = solana_message::v0::Message {
        header: m.header,
        account_keys: m.account_keys,
        recent_blockhash: m.recent_blockhash,
        instructions: m.instructions,
        address_table_lookups: vec![solana_message::v0::MessageAddressTableLookup {
            account_key: solana_address::Address::new_from_array([9; 32]),
            writable_indexes: vec![1],
            readonly_indexes: vec![2],
        }],
    };
    let m = UnsignedMessage::from_message(solana_message::VersionedMessage::V0(v))?;
    assert_eq!(m.version(), TransactionVersion::V0);
    assert_eq!(m.account_count(), m.static_accounts().len() + 2);
    assert_eq!(UnsignedMessage::from_bytes(m.bytes().to_vec())?, m);
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn metadata_retains_exact_above_f64_units_and_validates_native_account_counts() -> TestResult {
    let body = signed_legacy()?;
    let count = body.message().account_count();
    let meta = TransactionMetadata::new(metadata(count))?;
    let tx = Transaction::new(body.clone(), 90, Some(-1), None, Some(meta))?;
    assert_eq!(
        tx.metadata().map(|m| m.data().fee_lamports),
        Some(9_007_199_254_740_993)
    );
    assert_eq!(
        serde_json::from_value::<Transaction>(serde_json::to_value(&tx)?)?,
        tx
    );
    assert!(
        Transaction::new(
            body,
            90,
            None,
            None,
            Some(TransactionMetadata::new(metadata(count + 1))?)
        )
        .is_err()
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn metadata_checks_loaded_count_token_indices_duplicate_groups_and_execution_index() -> TestResult {
    let body = signed_legacy()?;
    let count = body.message().account_count();
    let mut loaded = metadata(count);
    loaded.loaded_addresses = Some(LoadedAddresses {
        writable: vec![Pubkey::from_bytes([1; 32])],
        readonly: vec![],
    });
    assert!(
        Transaction::new(
            body.clone(),
            90,
            None,
            None,
            Some(TransactionMetadata::new(loaded)?)
        )
        .is_err()
    );
    let b = TokenBalanceRecord {
        account_index: 255,
        mint: Pubkey::from_bytes([1; 32]),
        owner: None,
        program_id: None,
        raw_amount: u64::MAX,
        decimals: 9,
    };
    let mut token = metadata(count);
    token.pre_token_balances = Some(vec![b.clone()]);
    assert!(
        Transaction::new(
            body.clone(),
            90,
            None,
            None,
            Some(TransactionMetadata::new(token.clone())?)
        )
        .is_err()
    );
    token.pre_token_balances = Some(vec![b.clone(), b]);
    assert!(TransactionMetadata::new(token).is_err());
    let mut inner = metadata(count);
    inner.inner_instructions = Some(vec![
        InnerInstructions {
            index: 0,
            instructions: vec![],
        },
        InnerInstructions {
            index: 0,
            instructions: vec![],
        },
    ]);
    assert!(TransactionMetadata::new(inner).is_err());
    let mut failed = metadata(count);
    failed.outcome = ExecutionOutcome::Failed {
        error: solana_transaction_error::TransactionError::InstructionError(
            1,
            solana_transaction_error::TransactionError::VARIANTS
                .iter()
                .find_map(|e| {
                    if let solana_transaction_error::TransactionError::InstructionError(_, e) = e {
                        Some(e.clone())
                    } else {
                        None
                    }
                })
                .ok_or("missing maintained variant")?,
        ),
    };
    assert!(
        Transaction::new(
            body,
            90,
            None,
            None,
            Some(TransactionMetadata::new(failed)?)
        )
        .is_err()
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn rpc_request_controls_are_method_specific_and_context_constructor_serde_match() -> TestResult {
    assert!(TransactionReadOptions::new(Commitment::Processed, 1).is_err());
    assert!(TransactionReadOptions::new(Commitment::Confirmed, 2).is_err());
    let request = ExecutionRequest::LatestBlockhash {
        options: ReadOptions::new(Commitment::Confirmed, Some(100)),
    };
    assert!(context(request.clone(), None).is_err());
    assert!(context(request.clone(), Some(99)).is_err());
    let c = context(request, Some(100))?;
    assert_eq!(
        serde_json::from_value::<ExecutionContext>(serde_json::to_value(&c)?)?,
        c
    );
    let height = ExecutionRequest::BlockHeight {
        commitment: Commitment::Confirmed,
    };
    assert!(context(height.clone(), Some(100)).is_err());
    assert!(context(height, None).is_ok());
    let tx = ExecutionRequest::Transaction {
        signature: Signature::from_bytes([1; 64]),
        options: TransactionReadOptions::new(Commitment::Confirmed, 1)?,
    };
    assert!(context(tx, Some(100)).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn status_and_transaction_observations_reject_wrong_query_and_source_slot_facts() -> TestResult {
    let s = Signature::from_bytes([1; 64]);
    let r = ExecutionRequest::Status {
        signature: s,
        options: StatusOptions {
            search_transaction_history: true,
        },
    };
    let v = StatusLookup {
        signature: s,
        status: Some(SignatureStatus {
            inclusion_slot: 101,
            confirmations: Some(1),
            confirmation_status: Some(Commitment::Confirmed),
            outcome: ExecutionOutcome::Succeeded,
        }),
    };
    assert!(ExecutionObservation::status(v.clone(), context(r.clone(), Some(100))?).is_err());
    let good = ExecutionObservation::status(v, context(r, Some(101))?)?;
    let mut bad = serde_json::to_value(&good)?;
    bad["value"]["signature"] = json!(Signature::from_bytes([2; 64]));
    assert!(serde_json::from_value::<ExecutionObservation<StatusLookup>>(bad).is_err());
    let body = signed_legacy()?;
    let value = TransactionLookup {
        signature: s,
        transaction: Some(Transaction::new(body, 90, None, None, None)?),
    };
    assert!(
        ExecutionObservation::transaction(
            value,
            context(
                ExecutionRequest::Transaction {
                    signature: s,
                    options: TransactionReadOptions::new(Commitment::Confirmed, 1)?
                },
                None
            )?
        )
        .is_err()
    );
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn logs_preserve_empty_and_control_bytes_without_diagnostic_leak_or_unbounded_recording()
-> TestResult {
    let log = LogMessage::new("FAKE_SECRET\n\0")?;
    assert_eq!(log.as_str(), "FAKE_SECRET\n\0");
    assert!(!format!("{log:?}").contains("FAKE_SECRET"));
    assert_eq!(
        serde_json::from_value::<LogMessage>(serde_json::to_value(&log)?)?,
        log
    );
    assert!(LogMessage::new("x".repeat(16385)).is_err());
    assert!(ExecutionBytes::new(vec![0; 4097]).is_err());
    assert!(
        Simulation::new(
            ExecutionOutcome::Succeeded,
            None,
            None,
            Some(vec![LogMessage::new("")?; 1025]),
            None
        )
        .is_err()
    );
    let sim = Simulation::new(
        ExecutionOutcome::Succeeded,
        Some(u64::MAX),
        Some(u64::MAX),
        Some(vec![LogMessage::new("")?, log]),
        None,
    )?;
    assert_eq!(
        serde_json::from_value::<Simulation>(serde_json::to_value(&sim)?)?,
        sim
    );
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn partially_signed_multi_authority_payload_is_neither_unsigned_nor_signed() -> TestResult {
    let prepared = TransferPreparation::new(
        network()?,
        TransferIntent::Native {
            fee_payer: Pubkey::from_bytes([1; 32]),
            sender: Pubkey::from_bytes([2; 32]),
            recipient: Pubkey::from_bytes([3; 32]),
            lamports: 1,
        },
        BlockhashLifetime {
            blockhash: Hash::from_bytes([4; 32]),
            last_valid_block_height: 5,
        },
    )?;
    assert_eq!(
        prepared
            .unsigned_transaction()
            .message()
            .required_signatures(),
        2
    );
    let partially_signed = solana_transaction::versioned::VersionedTransaction {
        message: prepared.unsigned_transaction().message().decoded_message(),
        signatures: vec![
            solana_signature::Signature::from([7; 64]),
            solana_signature::Signature::default(),
        ],
    };
    let bytes = wincode::serialize(&partially_signed)?;
    assert!(UnsignedTransaction::from_bytes(bytes.clone()).is_err());
    assert!(SignedTransaction::from_bytes(bytes).is_err());
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn format_specific_packet_and_v1_header_bounds_are_enforced_before_encoding() -> TestResult {
    let mut legacy = preparation()?
        .unsigned_transaction()
        .message()
        .decoded_message();
    let solana_message::VersionedMessage::Legacy(ref mut message) = legacy else {
        return Err("expected legacy".into());
    };
    message.instructions[0].data = vec![0; 1232];
    assert_eq!(
        UnsignedMessage::from_message(legacy).unwrap_err(),
        Error::Validation(ValidationError::InvalidSolanaTransaction)
    );
    let actual = SignedTransaction::from_bytes(const_hex::decode(
        include_str!("fixtures/solana_execution/mainnet_v1_transaction.hex").trim(),
    )?)?;
    let mut v1 = actual.message().decoded_message();
    let solana_message::VersionedMessage::V1(ref mut message) = v1 else {
        return Err("expected v1".into());
    };
    message.header.num_required_signatures = 13;
    assert!(UnsignedMessage::from_message(v1).is_err());
    let mut v1 = actual.message().decoded_message();
    let solana_message::VersionedMessage::V1(ref mut message) = v1 else {
        return Err("expected v1".into());
    };
    message.instructions = vec![message.instructions[0].clone(); 65];
    assert!(UnsignedMessage::from_message(v1).is_err());
    Ok(())
}
