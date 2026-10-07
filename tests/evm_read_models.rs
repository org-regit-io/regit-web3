// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure EVM read contracts and constructor/serialization invariants.

#![cfg(test)]
#![cfg(feature = "evm")]

use regit_web3::{
    chains::evm::{Erc20Reader, TransactionReader},
    domain::evm::{
        AccessListEntry, Data, Erc20Allowance, Erc20Balance, Erc20Metadata, ExecutionOutcome,
        Inclusion, Log, MetadataText, MetadataUnavailable, MetadataValue, OperationContext,
        OperationObservation, Quantity, ReadOperation, ReadState, Receipt, ReceiptData,
        ReceiptLookup, SignatureFields, Transaction, TransactionData, TransactionId,
        TransactionKind, TransactionLookup, TransactionState, TransactionStatus, Word,
    },
    domain::{
        Address, BlockContext, BlockHash, BlockSelector, ChainId, Finality, NetworkId, Source,
        Timestamp, U256,
    },
    error::{Error, ValidationError},
};
use serde_json::json;
use std::{
    future::{Future, ready},
    rc::Rc,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn address(n: u8) -> Address {
    Address::from_bytes([n; 20])
}
fn hash(n: u8) -> TransactionId {
    TransactionId::from_bytes([n; 32])
}
fn inclusion() -> Inclusion {
    Inclusion::new(BlockHash::from_bytes([7; 32]), 42, 3)
}
fn context(operation: ReadOperation, state: ReadState) -> Result<OperationContext, Error> {
    OperationContext::new(
        operation,
        NetworkId::new(ChainId::from(1), "main")?,
        state,
        Source::new("fixture", "read", "0.1.0")?,
        Timestamp::from_unix_seconds(123),
    )
}
fn canonical() -> ReadState {
    ReadState::CanonicalHash {
        requested_selector: BlockSelector::Safe,
        block: BlockContext::new(
            42,
            BlockHash::from_bytes([7; 32]),
            Timestamp::from_unix_seconds(100),
        ),
    }
}
fn transaction_data() -> Result<TransactionData, Error> {
    Ok(TransactionData {
        hash: hash(1),
        from: address(1),
        to: Some(address(2)),
        nonce: Quantity::from(1),
        gas_limit: Quantity::from(21_000),
        value: Quantity::new(U256::MAX),
        input: Data::parse("0xdeadbeef")?,
        kind: TransactionKind::Legacy {
            gas_price: Quantity::new(U256::MAX),
        },
        reported_chain_id: Some(ChainId::from(1)),
        reported_gas_price: Some(Quantity::new(U256::MAX)),
        signature: SignatureFields {
            r: Quantity::from(1),
            s: Quantity::from(2),
            v: Some(Quantity::from(37)),
            y_parity: None,
        },
        inclusion: Some(inclusion()),
    })
}
fn receipt_data() -> Result<ReceiptData, Error> {
    Ok(ReceiptData {
        transaction_hash: hash(1),
        inclusion: inclusion(),
        from: address(1),
        to: Some(address(2)),
        contract_address: None,
        transaction_type: Some(0),
        execution: ExecutionOutcome::Failed,
        gas_used: Quantity::from(21_000),
        cumulative_gas_used: Quantity::from(30_000),
        effective_gas_price: Some(Quantity::new(U256::MAX)),
        blob_gas_used: None,
        blob_gas_price: None,
        logs_bloom: Data::new(vec![0; 256])?,
        logs: vec![Log::new(
            hash(1),
            inclusion(),
            9,
            address(3),
            Data::parse("0x1234")?,
            vec![Word::from_bytes([4; 32])],
        )?],
    })
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn exact_quantity_hash_and_opaque_bytes_validate_without_echoing_inputs() -> TestResult {
    assert_eq!(
        Quantity::from_decimal(&U256::MAX.to_string())?.value(),
        U256::MAX
    );
    for value in [
        "00",
        "-1",
        "1.0",
        " 1",
        "0x1",
        "115792089237316195423570985008687907853269984665640564039457584007913129639936",
    ] {
        assert!(Quantity::from_decimal(value).is_err());
    }
    assert_eq!(
        serde_json::to_string(&Quantity::new(U256::MAX))?,
        format!("\"{}\"", U256::MAX)
    );
    assert_eq!(
        TransactionId::parse(&format!("0x{}", "AB".repeat(32)))?.to_string(),
        format!("0x{}", "ab".repeat(32))
    );
    assert_eq!(
        TransactionId::parse("secret-invalid").err(),
        Some(Error::Validation(ValidationError::InvalidEvmTransactionId))
    );
    for value in ["0x0", "ff", "0xzz"] {
        assert!(Data::parse(value).is_err());
    }
    assert!(Data::parse("0x")?.bytes().is_empty());
    let bytes = Data::parse("0x736563726574")?;
    assert!(!format!("{bytes:?}").contains("736563726574"));
    assert!(Data::new(vec![0; Data::MAX_BYTES]).is_ok());
    assert!(Data::new(vec![0; Data::MAX_BYTES + 1]).is_err());
    assert!(serde_json::from_str::<Data>("\"0x0\"").is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn token_raw_units_and_metadata_availability_do_not_invent_precision() -> TestResult {
    let balance = Erc20Balance::new(ChainId::from(1), address(1), address(2), U256::MAX);
    assert_eq!(balance.amount().decimals(), None);
    assert_eq!(balance.amount().formatted(), None);
    let observation =
        OperationObservation::new(balance, context(ReadOperation::Erc20Balance, canonical())?)?;
    let mut value = serde_json::to_value(&observation)?;
    assert_eq!(
        serde_json::from_value::<OperationObservation<Erc20Balance>>(value.clone())?,
        observation
    );
    value["value"]["amount"]["decimals"] = json!(18);
    assert!(serde_json::from_value::<OperationObservation<Erc20Balance>>(value).is_err());
    let metadata = Erc20Metadata::new(
        ChainId::from(1),
        address(1),
        MetadataValue::Available(MetadataText::new(String::new())?),
        MetadataValue::Reverted,
        MetadataValue::Unavailable(MetadataUnavailable::Unsupported),
    );
    assert_eq!(
        metadata.decimals(),
        &MetadataValue::Unavailable(MetadataUnavailable::Unsupported)
    );
    let observed = OperationObservation::new(
        metadata,
        context(ReadOperation::Erc20Metadata, canonical())?,
    )?;
    assert_eq!(
        serde_json::from_str::<OperationObservation<Erc20Metadata>>(&serde_json::to_string(
            &observed
        )?)?,
        observed
    );
    let secret = MetadataText::new("SECRET_TOKEN".to_owned())?;
    assert!(!format!("{secret:?}").contains("SECRET_TOKEN"));
    assert!(MetadataText::new("a".repeat(4097)).is_err());
    assert!(serde_json::from_value::<MetadataText>(json!("a".repeat(4097))).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn observations_reject_network_operation_anchor_and_finality_tampering() -> TestResult {
    let balance = Erc20Balance::new(ChainId::from(1), address(1), address(2), U256::ZERO);
    assert!(
        OperationObservation::new(
            balance.clone(),
            context(ReadOperation::Erc20Allowance, canonical())?
        )
        .is_err()
    );
    assert!(
        OperationObservation::new(
            balance.clone(),
            context(ReadOperation::Erc20Balance, ReadState::Pending)?
        )
        .is_err()
    );
    let observed =
        OperationObservation::new(balance, context(ReadOperation::Erc20Balance, canonical())?)?;
    assert_eq!(observed.context().finality(), Finality::Unknown);
    for (path, bad) in [
        ("schema_version", json!(2)),
        ("finality", json!("finalized")),
        ("confirmations", json!(99)),
        ("operation", json!("receipt")),
    ] {
        let mut wire = serde_json::to_value(&observed)?;
        wire["context"][path] = bad;
        assert!(serde_json::from_value::<OperationObservation<Erc20Balance>>(wire).is_err());
    }
    let mut wire = serde_json::to_value(&observed)?;
    wire["context"]["network"]["chain_id"] = json!("2");
    assert!(serde_json::from_value::<OperationObservation<Erc20Balance>>(wire).is_err());
    let state = ReadState::CanonicalHash {
        requested_selector: BlockSelector::Number(41),
        block: BlockContext::new(
            42,
            BlockHash::from_bytes([7; 32]),
            Timestamp::from_unix_seconds(1),
        ),
    };
    assert!(context(ReadOperation::Erc20Balance, state).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn transactions_preserve_u256_values_and_reject_replay_or_parity_mismatch() -> TestResult {
    let transaction = Transaction::new(ChainId::from(1), transaction_data()?)?;
    assert_eq!(transaction.data().value.value(), U256::MAX);
    assert_eq!(
        serde_json::from_str::<Transaction>(&serde_json::to_string(&transaction)?)?,
        transaction
    );
    for mutate in [0, 1, 2, 3] {
        let mut data = transaction_data()?;
        match mutate {
            0 => data.reported_chain_id = Some(ChainId::from(2)),
            1 => data.signature.v = Some(Quantity::from(39)),
            2 => data.signature.v = Some(Quantity::from(34)),
            _ => data.nonce = Quantity::from(u64::MAX),
        }
        assert!(Transaction::new(ChainId::from(1), data).is_err());
    }
    let mut unprotected = transaction_data()?;
    unprotected.reported_chain_id = None;
    unprotected.signature.v = Some(Quantity::from(27));
    assert!(Transaction::new(ChainId::from(1), unprotected).is_ok());
    let mut typed = transaction_data()?;
    typed.kind = TransactionKind::DynamicFee {
        max_fee_per_gas: Quantity::new(U256::MAX),
        max_priority_fee_per_gas: Quantity::new(U256::MAX),
        access_list: vec![],
    };
    typed.signature.v = Some(Quantity::from(1));
    typed.signature.y_parity = Some(true);
    assert!(Transaction::new(ChainId::from(1), typed.clone()).is_ok());
    typed.signature.y_parity = Some(false);
    assert!(Transaction::new(ChainId::from(1), typed).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn legal_ordered_access_list_duplicates_are_preserved_and_bounds_enforced() -> TestResult {
    let entry = AccessListEntry {
        address: address(3),
        storage_keys: vec![Word::from_bytes([4; 32]); 2],
    };
    let mut data = transaction_data()?;
    data.kind = TransactionKind::AccessList {
        gas_price: Quantity::from(2),
        access_list: vec![entry.clone(), entry.clone()],
    };
    data.signature.v = Some(Quantity::from(0));
    data.signature.y_parity = Some(false);
    let tx = Transaction::new(ChainId::from(1), data.clone())?;
    let TransactionKind::AccessList { access_list, .. } = &tx.data().kind else {
        return Err("wrong transaction form".into());
    };
    assert_eq!(access_list, &vec![entry.clone(), entry.clone()]);
    data.kind = TransactionKind::AccessList {
        gas_price: Quantity::from(2),
        access_list: vec![entry; 1025],
    };
    assert!(Transaction::new(ChainId::from(1), data).is_err());
    Ok(())
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn receipts_keep_failed_and_historical_unknown_execution_separate_from_inclusion() -> TestResult {
    let receipt = Receipt::new(ChainId::from(1), receipt_data()?)?;
    assert_eq!(receipt.execution(), ExecutionOutcome::Failed);
    assert_eq!(
        serde_json::from_str::<Receipt>(&serde_json::to_string(&receipt)?)?,
        receipt
    );
    for execution in [
        ExecutionOutcome::Succeeded,
        ExecutionOutcome::PreByzantium(Word::from_bytes([5; 32])),
        ExecutionOutcome::Unknown,
    ] {
        let mut data = receipt_data()?;
        data.execution = execution;
        assert_eq!(Receipt::new(ChainId::from(1), data)?.execution(), execution);
    }
    for variant in 0..5 {
        let mut data = receipt_data()?;
        match variant {
            0 => data.gas_used = Quantity::from(30_001),
            1 => data.logs_bloom = Data::new(vec![0; 255])?,
            2 => data.logs.push(data.logs[0].clone()),
            3 => data.contract_address = Some(address(3)),
            _ => {
                data.logs[0] = Log::new(
                    hash(9),
                    inclusion(),
                    9,
                    address(3),
                    Data::parse("0x")?,
                    vec![],
                )?;
            }
        }
        assert!(Receipt::new(ChainId::from(1), data).is_err());
    }
    assert!(
        Log::new(
            hash(1),
            inclusion(),
            9,
            address(3),
            Data::parse("0x")?,
            vec![Word::from_bytes([4; 32]); 5]
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
fn lifecycle_distinguishes_absence_pending_inclusion_and_missing_execution() -> TestResult {
    let none_tx = TransactionLookup::new(ChainId::from(1), hash(1), None)?;
    let none_receipt = ReceiptLookup::new(ChainId::from(1), hash(1), None)?;
    assert_eq!(
        TransactionStatus::new(none_tx, none_receipt.clone())?.state(),
        TransactionState::NotObserved
    );
    let mut data = transaction_data()?;
    data.inclusion = None;
    let pending = TransactionLookup::new(
        ChainId::from(1),
        hash(1),
        Some(Transaction::new(ChainId::from(1), data)?),
    )?;
    assert_eq!(
        TransactionStatus::new(pending.clone(), none_receipt.clone())?.state(),
        TransactionState::Pending
    );
    let included = TransactionLookup::new(
        ChainId::from(1),
        hash(1),
        Some(Transaction::new(ChainId::from(1), transaction_data()?)?),
    )?;
    assert_eq!(
        TransactionStatus::new(included.clone(), none_receipt)?.state(),
        TransactionState::Included {
            inclusion: inclusion(),
            execution: None
        }
    );
    let receipt = ReceiptLookup::new(
        ChainId::from(1),
        hash(1),
        Some(Receipt::new(ChainId::from(1), receipt_data()?)?),
    )?;
    let status = TransactionStatus::new(pending, receipt.clone())?;
    assert_eq!(
        status.state(),
        TransactionState::Included {
            inclusion: inclusion(),
            execution: Some(ExecutionOutcome::Failed)
        }
    );
    let mut serialized = serde_json::to_value(&status)?;
    serialized["state"]["execution"]["kind"] = json!("succeeded");
    assert!(serde_json::from_value::<TransactionStatus>(serialized).is_err());
    let mut data = receipt_data()?;
    data.inclusion = Inclusion::new(BlockHash::from_bytes([8; 32]), 42, 3);
    data.logs.clear();
    let different = ReceiptLookup::new(
        ChainId::from(1),
        hash(1),
        Some(Receipt::new(ChainId::from(1), data)?),
    )?;
    assert!(TransactionStatus::new(included, different).is_err());
    assert!(
        TransactionLookup::new(
            ChainId::from(1),
            hash(2),
            status.transaction().transaction().cloned()
        )
        .is_err()
    );
    Ok(())
}
struct LocalReader(Rc<()>);
impl Erc20Reader for LocalReader {
    fn get_erc20_balance(
        &self,
        contract: Address,
        owner: Address,
        _: Option<BlockSelector>,
    ) -> impl Future<Output = Result<OperationObservation<Erc20Balance>, Error>>
    + regit_web3::future::MaybeSend {
        let _ = &self.0;
        ready(
            context(ReadOperation::Erc20Balance, canonical()).and_then(|context| {
                OperationObservation::new(
                    Erc20Balance::new(ChainId::from(1), contract, owner, U256::ZERO),
                    context,
                )
            }),
        )
    }
    fn get_erc20_allowance(
        &self,
        _: Address,
        _: Address,
        _: Address,
        _: Option<BlockSelector>,
    ) -> impl Future<Output = Result<OperationObservation<Erc20Allowance>, Error>>
    + regit_web3::future::MaybeSend {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_erc20_metadata(
        &self,
        _: Address,
        _: Option<BlockSelector>,
    ) -> impl Future<Output = Result<OperationObservation<Erc20Metadata>, Error>>
    + regit_web3::future::MaybeSend {
        ready(Err(Error::UnsupportedCapability))
    }
}
impl TransactionReader for LocalReader {
    fn get_transaction(
        &self,
        _: TransactionId,
    ) -> impl Future<Output = Result<OperationObservation<TransactionLookup>, Error>>
    + regit_web3::future::MaybeSend {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_receipt(
        &self,
        _: TransactionId,
    ) -> impl Future<Output = Result<OperationObservation<ReceiptLookup>, Error>>
    + regit_web3::future::MaybeSend {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_transaction_status(
        &self,
        _: TransactionId,
    ) -> impl Future<Output = Result<OperationObservation<TransactionStatus>, Error>>
    + regit_web3::future::MaybeSend {
        ready(Err(Error::UnsupportedCapability))
    }
}
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn pure_reader_contracts_return_send_futures_without_send_sync_supertraits() {
    fn require_send<T: Send>(value: T) -> T {
        value
    }
    let reader = LocalReader(Rc::new(()));
    drop(require_send(reader.get_erc20_balance(
        address(1),
        address(2),
        None,
    )));
    drop(require_send(reader.get_erc20_allowance(
        address(1),
        address(2),
        address(3),
        None,
    )));
    drop(require_send(reader.get_erc20_metadata(address(1), None)));
    drop(require_send(reader.get_transaction(hash(1))));
    drop(require_send(reader.get_receipt(hash(1))));
    drop(require_send(reader.get_transaction_status(hash(1))));
}
