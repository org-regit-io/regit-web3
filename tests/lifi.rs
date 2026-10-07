// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure LI.FI exact values, identity, review and constructor/serde invariants.
#![cfg(feature = "lifi")]

use regit_web3::{
    domain::{Amount, ExactDecimal, lifi::*},
    error::{Error, ValidationError},
    protocols::lifi::LifiReader,
    wallets::{Preparation, PreparedRequest},
};
use serde_json::json;
use std::fmt::Write;
#[path = "fixtures/lifi/builders.rs"]
mod build;

#[test]
fn catalogue_qualifies_large_numeric_ids_without_guessing_family()
-> Result<(), Box<dyn std::error::Error>> {
    let sui = Chain::new(9_270_000_000_000_000, Family::Move)?;
    let sol = Chain::new(1_151_111_081_099_710, Family::Solana)?;
    let catalogue = ChainCatalogue::new(vec![build::chain(1)?, sui, sol])?;
    assert_eq!(catalogue.resolve(sui.id())?, sui);
    assert_eq!(catalogue.resolve(99999), Err(Error::UnsupportedCapability));
    assert!(Chain::new(sui.id(), Family::Evm).is_err());
    assert!(ChainCatalogue::new(vec![sui, sui]).is_err());
    assert_eq!(
        serde_json::from_str::<Chain>(&serde_json::to_string(&sui)?)?,
        sui
    );
    for v in [
        json!({"id":0,"family":"evm"}),
        json!({"id":sui.id(),"family":"evm"}),
        json!({"id":1.25,"family":"evm"}),
    ] {
        assert!(serde_json::from_value::<Chain>(v).is_err());
    }
    Ok(())
}
#[test]
fn family_identities_and_base_units_never_resolve_symbols_or_round() -> Result<(), Error> {
    let evm = build::chain(1)?;
    assert_eq!(
        Account::new(evm, build::ACCOUNT)?.identifier(),
        build::ACCOUNT
    );
    assert!(Asset::new(evm, "ETH").is_err());
    let sol = Chain::new(1_151_111_081_099_710, Family::Solana)?;
    assert!(Asset::new(sol, build::NATIVE).is_err());
    assert!(Asset::new(sol, "11111111111111111111111111111111").is_ok());
    let btc = Chain::new(20_000_000_000_001, Family::Utxo)?;
    assert!(Account::new(btc, "not-a-bitcoin-address").is_err());
    let source = Chain::new(728_126_428, Family::Tron)?;
    // This is intentionally bounded source syntax, not a TRON checksum claim.
    assert_eq!(
        Account::new(source, "source-account")?.identifier(),
        "source-account"
    );
    for bad in ["", "secret\nvalue", "contains space"] {
        assert!(Account::new(source, bad).is_err());
    }
    let amount = Amount::from_decimal("900719925474099312345678901234567890", Some(18))?;
    assert_eq!(
        amount.formatted().as_deref(),
        Some("900719925474099312.345678901234567890")
    );
    assert_eq!(
        Slippage::new(ExactDecimal::parse("0.000000000000000001")?)?
            .value()
            .canonical(),
        "0.000000000000000001"
    );
    for bad in ["-0.1", "1", "2"] {
        assert!(Slippage::new(ExactDecimal::parse(bad)?).is_err());
    }
    Ok(())
}
#[test]
fn request_constructor_and_serde_reject_cross_family_accounts_and_zero()
-> Result<(), Box<dyn std::error::Error>> {
    let request = build::request()?;
    assert_eq!(
        serde_json::from_str::<Request>(&serde_json::to_string(&request)?)?,
        request
    );
    let mut fields = request.data().clone();
    fields.from_account = Account::new(build::chain(8453)?, build::ACCOUNT)?;
    assert!(Request::new(fields.clone()).is_err());
    assert!(serde_json::from_value::<Request>(serde_json::to_value(fields)?).is_err());
    let mut fields = request.data().clone();
    fields.amount = Amount::from_decimal("0", None)?;
    assert!(Request::new(fields).is_err());
    Ok(())
}
#[test]
fn step_validates_precision_cost_minima_and_duplicates_on_every_entry_path()
-> Result<(), Box<dyn std::error::Error>> {
    let step = build::step()?;
    assert_eq!(
        serde_json::from_str::<Step>(&serde_json::to_string(&step)?)?,
        step
    );
    assert_eq!(
        step.data()
            .estimate
            .as_ref()
            .ok_or(Error::UnavailableData)?
            .data()
            .execution_seconds
            .canonical(),
        "45.60000000000000001"
    );
    let mut bad = step.data().clone();
    bad.included_steps = vec![step.clone()];
    assert!(Step::new(bad.clone()).is_err());
    assert!(serde_json::from_value::<Step>(serde_json::to_value(bad)?).is_err());
    let mut estimate = step
        .data()
        .estimate
        .as_ref()
        .ok_or(Error::UnavailableData)?
        .data()
        .clone();
    estimate.to_amount_min = Amount::from_decimal("999999999999999999", Some(18))?;
    assert!(Estimate::new(estimate.clone()).is_err());
    assert!(serde_json::from_value::<Estimate>(serde_json::to_value(estimate)?).is_err());
    let mut action = step.data().action.data().clone();
    action.from_amount = Amount::from_decimal("10000000000000000", None)?;
    assert!(Action::new(action).is_err());
    assert!(Limits::new(0, 256, 128).is_err());
    assert!(Limits::new(65, 256, 128).is_err());
    Ok(())
}
#[test]
fn encoded_payloads_name_only_the_checks_actually_applied() -> Result<(), Box<dyn std::error::Error>>
{
    let evm = build::chain(42161)?;
    let p = EncodedPayload::new(evm, PayloadEncoding::EvmCallData, "0xabcdef", None)?;
    assert!(!format!("{p:?}").contains("abcdef"));
    assert_eq!(
        serde_json::from_str::<EncodedPayload>(&serde_json::to_string(&p)?)?,
        p
    );
    assert!(EncodedPayload::new(evm, PayloadEncoding::EvmCallData, "0xabc", None).is_err());
    assert!(EncodedPayload::new(evm, PayloadEncoding::SolanaBase64, "AQID", None).is_err());
    let sol = Chain::new(1_151_111_081_099_710, Family::Solana)?;
    assert!(EncodedPayload::new(sol, PayloadEncoding::SolanaBase64, "AQID", None).is_ok());
    assert!(EncodedPayload::new(sol, PayloadEncoding::SolanaBase64, "AR==", None).is_err());
    let btc = Chain::new(20_000_000_000_001, Family::Utxo)?;
    assert!(EncodedPayload::new(btc, PayloadEncoding::BitcoinPsbtHex, "010203", None).is_err());
    let psbt = bitcoin::psbt::Psbt::from_unsigned_tx(bitcoin::Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![bitcoin::TxIn {
            previous_output: bitcoin::OutPoint {
                txid: "11".repeat(32).parse()?,
                vout: 0,
            },
            script_sig: bitcoin::ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness: bitcoin::Witness::default(),
        }],
        output: vec![bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(546),
            script_pubkey: bitcoin::ScriptBuf::from_bytes(vec![0x51]),
        }],
    })?;
    let mut hex = String::new();
    for byte in psbt.serialize() {
        write!(hex, "{byte:02x}")?;
    }
    let parsed = EncodedPayload::new(btc, PayloadEncoding::BitcoinPsbtHex, &hex, None)?;
    assert_eq!(
        serde_json::from_str::<EncodedPayload>(&serde_json::to_string(&parsed)?)?,
        parsed
    );
    let bch = Chain::new(20_000_000_000_002, Family::Utxo)?;
    assert!(EncodedPayload::new(bch, PayloadEncoding::BitcoinPsbtHex, "010203", None).is_err());
    assert!(EncodedPayload::new(bch, PayloadEncoding::UtxoHex, "010203", None).is_ok());
    let tron = Chain::new(728_126_428, Family::Tron)?;
    assert!(
        EncodedPayload::new(
            tron,
            PayloadEncoding::TronProtobufHex,
            "0x010203",
            Some(Identifier::new("TriggerSmartContract")?)
        )
        .is_ok()
    );
    Ok(())
}
#[test]
fn prepared_review_preserves_old_snapshot_and_requires_matching_fresh_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let selected = build::handle()?;
    let mut fresh = selected.value.data().clone();
    fresh.payload = Some(build::payload()?);
    let prepared = PreparedStep::new(
        &selected,
        Step::new(fresh.clone())?,
        build::origin("advanced-stepTransaction")?,
    )?;
    assert_eq!(prepared.original_request(), &selected.request);
    assert_eq!(prepared.selected_step(), &selected.value);
    assert_eq!(prepared.selected_origin(), &selected.origin);
    assert!(prepared.selected_step().data().payload.is_none());
    assert_eq!(prepared.payload().chain(), build::chain(42161)?);
    assert!(prepared.validate().is_ok());
    assert_eq!(
        serde_json::from_str::<PreparedStep>(&serde_json::to_string(&prepared)?)?,
        prepared
    );
    assert!(PreparedRequest::new(prepared.clone()).is_ok());
    fresh.id = Identifier::new("different-selection")?;
    assert!(PreparedStep::new(&selected, Step::new(fresh)?, build::origin("prepare")?).is_err());
    let mut value = serde_json::to_value(&prepared)?;
    value["network"][0]["id"] = json!(8453);
    assert!(serde_json::from_value::<PreparedStep>(value).is_err());
    Ok(())
}
#[test]
fn route_serde_rechecks_original_request_limits_and_handle_attribution()
-> Result<(), Box<dyn std::error::Error>> {
    let h = build::handle()?;
    let a = h.value.data().action.data();
    let e = h
        .value
        .data()
        .estimate
        .as_ref()
        .ok_or(Error::UnavailableData)?
        .data();
    let data = RouteData {
        id: Identifier::new("route")?,
        from: a.from_token.clone(),
        from_account: a.from_account.clone(),
        to: a.to_token.clone(),
        to_account: a.to_account.clone(),
        from_amount: a.from_amount,
        to_amount: e.to_amount,
        to_amount_min: e.to_amount_min,
        from_amount_usd: e.from_amount_usd.clone(),
        to_amount_usd: e.to_amount_usd.clone(),
        gas_cost_usd: None,
        contains_switch_chain: Some(false),
    };
    let r = Route::new(data.clone(), vec![h.clone()])?;
    let mut wrong_precision = serde_json::to_value(&r)?;
    wrong_precision["data"]["to"]["decimals"] = json!(17);
    wrong_precision["data"]["to_amount"]["decimals"] = json!(17);
    wrong_precision["data"]["to_amount_min"]["decimals"] = json!(17);
    assert!(serde_json::from_value::<Route<build::Handle>>(wrong_precision).is_err());
    let routes = Routes::new(
        h.request.clone(),
        h.origin.clone(),
        vec![r.clone()],
        None,
        Limits::new(1, 1, 1)?,
    )?;
    assert_eq!(
        serde_json::from_str::<Routes<build::Handle>>(&serde_json::to_string(&routes)?)?,
        routes
    );
    assert!(
        Routes::new(
            h.request.clone(),
            h.origin.clone(),
            vec![r.clone(), r],
            None,
            Limits::new(1, 1, 1)?
        )
        .is_err()
    );
    let mut bad = serde_json::to_value(routes)?;
    bad["alternatives"][0]["steps"][0]["origin"]["source"]["provider_id"] = json!("another-source");
    assert!(serde_json::from_value::<Routes<build::Handle>>(bad).is_err());
    assert!(Route::new(data, vec![h.clone(), h]).is_err());
    Ok(())
}
#[test]
fn status_keeps_missing_facts_and_explicit_refunds_separate_from_success()
-> Result<(), Box<dyn std::error::Error>> {
    let query = build::query(8453)?;
    let data = StatusData {
        progress: Progress::NotFound,
        substatus: None,
        tool: None,
        transfer_id: None,
        sending: None,
        receiving: None,
        fees: None,
    };
    let unknown = Status::new(query.clone(), data.clone(), build::origin("status")?)?;
    assert_eq!(unknown.receiving_role(), ReceivingRole::Unreported);
    assert_eq!(
        serde_json::from_str::<Status>(&serde_json::to_string(&unknown)?)?,
        unknown
    );
    let refund = Transaction::new(TransactionData {
        chain: query.from(),
        id: None,
        token: None,
        amount: None,
        timestamp: None,
        value: None,
        gas_price: None,
        gas_used: None,
        gas_token: None,
        gas_amount: None,
        gas_amount_usd: None,
        included_legs: None,
    })?;
    let mut reported = data;
    reported.progress = Progress::Done;
    reported.substatus = Some(Identifier::new("REFUNDED")?);
    reported.receiving = Some(refund);
    let status = Status::new(query.clone(), reported.clone(), build::origin("status")?)?;
    assert_eq!(status.receiving_role(), ReceivingRole::Refund);
    reported.substatus = Some(Identifier::new("COMPLETED")?);
    assert!(Status::new(query, reported, build::origin("status")?).is_err());
    Ok(())
}
#[test]
fn provider_transfer_ids_are_not_step_uuids_and_reported_ids_must_match()
-> Result<(), Box<dyn std::error::Error>> {
    let id = TransferId::new(&format!("0x{}", "11".repeat(32)))?;
    assert_eq!(
        serde_json::from_str::<TransferId>(&serde_json::to_string(&id)?)?,
        id
    );
    let step_uuid = "bf1da5e9-ba03-4cc2-abe0-e0ceaf6ee132:0";
    assert!(TransferId::new(step_uuid).is_err());
    assert!(serde_json::from_value::<TransferId>(json!(step_uuid)).is_err());
    let q = StatusQuery::new(
        build::chain(42161)?,
        build::chain(8453)?,
        StatusSelector::ProviderTransfer(id.clone()),
        None,
    )?;
    let mut data = StatusData {
        progress: Progress::Pending,
        substatus: None,
        tool: None,
        transfer_id: Some(id),
        sending: None,
        receiving: None,
        fees: None,
    };
    let valid = Status::new(q.clone(), data.clone(), build::origin("status")?)?;
    assert_eq!(
        serde_json::from_str::<Status>(&serde_json::to_string(&valid)?)?,
        valid
    );
    data.transfer_id = Some(TransferId::new(&format!("0x{}", "22".repeat(32)))?);
    assert!(Status::new(q, data, build::origin("status")?).is_err());
    let mut invalid = serde_json::to_value(valid)?;
    invalid["data"]["transfer_id"] = json!(format!("0x{}", "22".repeat(32)));
    assert!(serde_json::from_value::<Status>(invalid).is_err());
    Ok(())
}
#[test]
fn fixed_validation_diagnostics_never_echo_invalid_identifiers() {
    let error = Identifier::new("private-secret\n").err();
    assert_eq!(
        error,
        Some(Error::Validation(ValidationError::InvalidLifiIdentity))
    );
    assert!(!format!("{error:?}").contains("private-secret"));
}

struct Offline;
impl LifiReader for Offline {
    type StepHandle = build::Handle;
    fn get_quote(
        &self,
        r: Request,
    ) -> impl std::future::Future<Output = Result<Self::StepHandle, Error>> + Send {
        std::future::ready((|| {
            let mut h = build::handle()?;
            if !h.value.data().action.matches_request(&r) {
                return Err(Error::Validation(ValidationError::InvalidLifiRecord));
            }
            h.request = r;
            Ok(h)
        })())
    }
    async fn get_routes(&self, r: Request) -> Result<Routes<Self::StepHandle>, Error> {
        let h = self.get_quote(r.clone()).await?;
        let a = h.value.data().action.data();
        let e = h
            .value
            .data()
            .estimate
            .as_ref()
            .ok_or(Error::UnavailableData)?
            .data();
        let route = Route::new(
            RouteData {
                id: Identifier::new("offline-route")?,
                from: a.from_token.clone(),
                from_account: a.from_account.clone(),
                to: a.to_token.clone(),
                to_account: a.to_account.clone(),
                from_amount: a.from_amount,
                to_amount: e.to_amount,
                to_amount_min: e.to_amount_min,
                from_amount_usd: None,
                to_amount_usd: None,
                gas_cost_usd: None,
                contains_switch_chain: Some(false),
            },
            vec![h.clone()],
        )?;
        Routes::new(r, h.origin, vec![route], None, Limits::new(1, 1, 1)?)
    }
    fn prepare_step(
        &self,
        h: &Self::StepHandle,
    ) -> impl std::future::Future<Output = Result<PreparedStep, Error>> + Send {
        std::future::ready((|| {
            let mut fresh = h.value.data().clone();
            fresh.payload = Some(build::payload()?);
            PreparedStep::new(h, Step::new(fresh)?, build::origin("prepare")?)
        })())
    }
    fn get_status(
        &self,
        q: StatusQuery,
    ) -> impl std::future::Future<Output = Result<Status, Error>> + Send {
        std::future::ready((|| {
            Status::new(
                q,
                StatusData {
                    progress: Progress::NotFound,
                    substatus: None,
                    tool: None,
                    transfer_id: None,
                    sending: None,
                    receiving: None,
                    fees: None,
                },
                build::origin("status")?,
            )
        })())
    }
}
#[test]
fn independent_pure_backend_handles_and_futures_compose_as_send() -> Result<(), Error> {
    fn send<T: std::future::Future + Send>(_: T) {}
    let backend = Offline;
    let request = build::request()?;
    let handle = build::handle()?;
    send(backend.get_quote(request.clone()));
    send(backend.get_routes(request));
    send(backend.prepare_step(&handle));
    send(backend.get_status(build::query(8453)?));
    Ok(())
}
