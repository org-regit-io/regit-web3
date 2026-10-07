// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Pure XRPL identity, exact amount, pagination and unsigned Payment contracts.

#![cfg(feature = "xrpl")]

use std::{
    future::{Future, ready},
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

use regit_web3::{
    chains::xrpl::XrplReader,
    domain::{
        Source, Timestamp,
        xrpl::{
            AccountBalance, Address, Context, Currency, Destination, Drops, FeeEstimate, Hash,
            HexData, HistoryMarker, HistoryPage, HistoryRequest, IssuedValue, Ledger, LedgerRange,
            LineFlags, Marker, Network, NetworkId, Observation, Operation, PageRequest,
            PaymentAmount, PaymentRequest, ResultCode, Setting, Transaction, TransactionStatus,
            TrustLine, TrustLinePage, XAddress, XAddressCategory,
        },
    },
    error::{Error, ValidationError},
};
use serde_json::{Value, json};

const ACCOUNT: &str = "r9cZA1mLK5R5Am25ArfXFmqgNwjZgnfk59";
const PEER: &str = "rGWrZyQqhTp9Xu7G5Pkayo7bXjH4k4QYpf";

fn network(number: u32) -> Result<Network, Error> {
    Network::new(NetworkId::new(number), "fixture")
}
fn settings() -> LineFlags {
    LineFlags {
        no_ripple: Setting::Enabled,
        no_ripple_peer: Setting::Disabled,
        authorized: Setting::Disabled,
        peer_authorized: Setting::Enabled,
        freeze: Setting::Disabled,
        freeze_peer: Setting::Enabled,
        quality_in: 0,
        quality_out: u32::MAX,
    }
}
fn line(balance: &str) -> Result<TrustLine, Error> {
    TrustLine::new(
        Address::parse(PEER)?,
        Currency::parse("USD")?,
        IssuedValue::parse(balance)?,
        IssuedValue::parse("100")?,
        IssuedValue::parse("200")?,
        settings(),
    )
}
fn context(operation: Operation, ledger: Ledger) -> Result<Context, Error> {
    Ok(Context::new(
        operation,
        network(0)?,
        Some(ledger),
        Source::new("fixture", "account_lines", "2")?,
        Timestamp::from_unix_seconds(1),
    ))
}
fn payment(number: u32, tag: Option<u32>, amount: PaymentAmount) -> Result<PaymentRequest, Error> {
    PaymentRequest::new(
        network(number)?,
        Address::parse(ACCOUNT)?,
        Destination::new(NetworkId::new(number), Address::parse(PEER)?, tag),
        amount,
        Drops::new(12)?,
        1,
        100,
    )
}

#[test]
fn classic_address_matches_official_account_id_and_checksum_vectors()
-> Result<(), Box<dyn std::error::Error>> {
    let address = Address::parse(ACCOUNT)?;
    assert_eq!(
        address.bytes(),
        [
            94, 123, 17, 37, 35, 246, 141, 47, 94, 135, 157, 180, 234, 197, 28, 102, 152, 166, 147,
            4
        ]
    );
    assert_eq!(address.to_string(), ACCOUNT);
    assert_eq!(Address::from_bytes(address.bytes()), address);
    assert_eq!(
        serde_json::from_str::<Address>(&serde_json::to_string(&address)?)?,
        address
    );
    for value in [
        "",
        "r9cZA1mLK5R5Am25ArfXFmqgNwjZgnfk58",
        "sn259rEFXrQrWyx3Q7XneWcwV6dfL",
        "1BoatSLRHtKNngkdXEeobR76b53LETtpyT",
        " r9cZA1mLK5R5Am25ArfXFmqgNwjZgnfk59",
    ] {
        assert_eq!(
            Address::parse(value),
            Err(Error::Validation(ValidationError::InvalidXrplAddress))
        );
    }
    Ok(())
}

#[test]
fn xaddress_retains_maximum_tag_zero_and_network_category() -> Result<(), Box<dyn std::error::Error>>
{
    let maximum = XAddress::parse("XVLhHMPHU98es4dbozjVtdWzVrDjtV18pX8yuPT7y4xaEHi")?;
    assert_eq!(maximum.address(), Address::parse(PEER)?);
    assert_eq!(maximum.tag(), Some(u32::MAX));
    assert!(!maximum.is_test());
    assert_eq!(
        maximum.to_string(),
        "XVLhHMPHU98es4dbozjVtdWzVrDjtV18pX8yuPT7y4xaEHi"
    );
    let none = XAddress::new(maximum.address(), None, true);
    let zero = XAddress::new(maximum.address(), Some(0), true);
    assert_ne!(none.to_string(), zero.to_string());
    assert_eq!(XAddress::parse(&none.to_string())?, none);
    assert_eq!(XAddress::parse(&zero.to_string())?, zero);
    assert!(Destination::from_xaddress(NetworkId::new(0), zero, XAddressCategory::Main).is_err());
    assert_eq!(
        Destination::from_xaddress(NetworkId::new(1), zero, XAddressCategory::Test)?.tag(),
        Some(0)
    );
    assert_eq!(
        Destination::from_xaddress(NetworkId::new(2), zero, XAddressCategory::Test)?.tag(),
        Some(0)
    );
    assert_eq!(
        Destination::from_xaddress(NetworkId::new(1025), maximum, XAddressCategory::Main)?
            .network()
            .number(),
        1025
    );
    assert!(
        Destination::from_xaddress(NetworkId::new(1025), maximum, XAddressCategory::Test).is_err()
    );
    assert_eq!(
        XAddress::parse("X7AcgcsBL6XDcUb289X4mJ8djcdyKaB5hJDWMArnXr61cqZ")?.address(),
        Address::parse(ACCOUNT)?
    );
    Ok(())
}

#[test]
fn xaddress_rejects_invalid_flags_and_reserved_bytes_even_with_valid_checksum()
-> Result<(), Box<dyn std::error::Error>> {
    let mut bytes =
        bs58::decode(XAddress::new(Address::parse(ACCOUNT)?, Some(0), false).to_string())
            .with_alphabet(bs58::Alphabet::RIPPLE)
            .with_check(None)
            .into_vec()?;
    for (position, value) in [(22, 2), (27, 1), (30, 1), (0, 3)] {
        let mut malformed = bytes.clone();
        malformed[position] = value;
        let text = bs58::encode(malformed)
            .with_alphabet(bs58::Alphabet::RIPPLE)
            .with_check()
            .into_string();
        assert!(XAddress::parse(&text).is_err());
    }
    bytes[22] = 0;
    bytes[23] = 1;
    let text = bs58::encode(bytes)
        .with_alphabet(bs58::Alphabet::RIPPLE)
        .with_check()
        .into_string();
    assert!(XAddress::parse(&text).is_err());
    Ok(())
}

#[test]
fn currency_preserves_case_punctuation_and_equivalent_protocol_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let usd = Currency::parse("USD")?;
    assert_eq!(
        usd,
        Currency::parse("0000000000000000000000005553440000000000")?
    );
    assert_eq!(usd.to_string(), "USD");
    assert_ne!(usd, Currency::parse("usd")?);
    assert_eq!(Currency::parse("$!? ".trim())?.to_string(), "$!?");
    let nonstandard = "015841551A748AD2C1F76FF6ECB0CCCD00000000";
    assert_eq!(Currency::parse(nonstandard)?.to_string(), nonstandard);
    for invalid in [
        "XRP",
        "0000000000000000000000000000000000000000",
        "0000000000000000000000005852500000000000",
        "EU",
        "EURO",
        "A B",
        "€UR",
    ] {
        assert!(Currency::parse(invalid).is_err());
    }
    Ok(())
}

#[test]
fn drops_and_issued_values_preserve_protocol_precision_without_rounding()
-> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(Drops::parse("100000000000000000")?.raw(), Drops::MAX);
    assert_eq!(
        serde_json::to_string(&Drops::new(9_007_199_254_740_993)?)?,
        "\"9007199254740993\""
    );
    for invalid in [
        "100000000000000001",
        "18446744073709551616",
        "01",
        "1.0",
        "-1",
        "+1",
        "1e3",
    ] {
        assert!(Drops::parse(invalid).is_err());
    }
    for (text, expected) in [
        ("-0.00111", "-0.00111"),
        ("9007199254740993", "9007199254740993"),
        ("1.23e11", "123000000000"),
        ("-0", "0"),
    ] {
        assert_eq!(IssuedValue::parse(text)?.value().canonical(), expected);
    }
    assert!(IssuedValue::parse("1e-81").is_ok());
    assert!(IssuedValue::parse("9.999999999999999e95").is_ok());
    for invalid in [
        "1e-82",
        "1e96",
        "12345678901234567",
        "1.2345678901234567",
        "NaN",
        "Infinity",
        "+1",
    ] {
        assert!(IssuedValue::parse(invalid).is_err());
    }
    assert_eq!(
        IssuedValue::parse("1234567890123456000")?
            .value()
            .canonical(),
        "1234567890123456000"
    );
    Ok(())
}

#[test]
fn signed_trustlines_and_page_invariants_survive_serialization()
-> Result<(), Box<dyn std::error::Error>> {
    let hash = Hash::from_bytes([1; 32]);
    let account = Address::parse(ACCOUNT)?;
    let request = PageRequest::new(10, Some(hash), None)?;
    let line = line("-0.00111")?;
    let page = TrustLinePage::new(
        account,
        request.clone(),
        vec![line.clone()],
        Some(Marker::new("next")?),
    )?;
    assert_eq!(page.lines()[0].balance().value().canonical(), "-0.00111");
    assert_eq!(
        serde_json::from_str::<TrustLinePage>(&serde_json::to_string(&page)?)?,
        page
    );
    assert!(TrustLinePage::new(account, request.clone(), vec![line.clone(), line], None).is_err());
    assert!(PageRequest::new(9, None, None).is_err());
    assert!(PageRequest::new(401, None, None).is_err());
    assert!(PageRequest::new(10, None, Some(Marker::new("next")?)).is_err());
    assert!(Marker::new("bad\nmarker").is_err());
    let continued = PageRequest::new(10, Some(hash), Some(Marker::new("next")?))?;
    assert!(TrustLinePage::new(account, continued, vec![], Some(Marker::new("next")?)).is_err());
    let mut value = serde_json::to_value(&page)?;
    value["lines"][0]["limit"] = json!("-1");
    assert!(serde_json::from_value::<TrustLinePage>(value).is_err());
    Ok(())
}

#[test]
fn observations_reject_schema_operation_hash_and_validation_mismatch()
-> Result<(), Box<dyn std::error::Error>> {
    let hash = Hash::from_bytes([1; 32]);
    let ledger = Ledger::new(1, Some(hash), true)?;
    let page = TrustLinePage::new(
        Address::parse(ACCOUNT)?,
        PageRequest::new(10, Some(hash), None)?,
        vec![],
        None,
    )?;
    let observed = Observation::trust_lines(page.clone(), context(Operation::TrustLines, ledger)?)?;
    assert_eq!(
        serde_json::from_str::<Observation<TrustLinePage>>(&serde_json::to_string(&observed)?)?,
        observed
    );
    assert!(
        Observation::trust_lines(page.clone(), context(Operation::AccountBalance, ledger)?)
            .is_err()
    );
    assert!(
        Observation::trust_lines(
            page,
            context(
                Operation::TrustLines,
                Ledger::new(1, Some(Hash::from_bytes([2; 32])), true)?
            )?
        )
        .is_err()
    );
    assert!(Ledger::new(0, Some(hash), true).is_err());
    let no_hash = Ledger::new(1, None, true)?;
    let account = AccountBalance::new(Address::parse(ACCOUNT)?, Drops::new(1)?, 1, 0, 0);
    assert!(
        Observation::account_balance(account, context(Operation::AccountBalance, no_hash)?)
            .is_err()
    );
    let page = TrustLinePage::new(
        Address::parse(ACCOUNT)?,
        PageRequest::new(10, None, None)?,
        vec![],
        None,
    )?;
    assert!(Observation::trust_lines(page, context(Operation::TrustLines, no_hash)?).is_err());
    for key in ["schema_version", "operation"] {
        let mut value = serde_json::to_value(&observed)?;
        if key == "schema_version" {
            value[key] = json!(2);
        } else {
            value["context"][key] = json!("account_balance");
        }
        assert!(serde_json::from_value::<Observation<TrustLinePage>>(value).is_err());
    }
    let duplicated = serde_json::to_string(&observed)?.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert!(serde_json::from_str::<Observation<TrustLinePage>>(&duplicated).is_err());
    Ok(())
}

#[test]
fn unsigned_payment_preserves_exact_amount_tag_and_replay_rules()
-> Result<(), Box<dyn std::error::Error>> {
    let intent = payment(
        0,
        Some(0),
        PaymentAmount::Xrp(Drops::new(9_007_199_254_740_993)?),
    )?;
    let prepared = intent.clone().prepare();
    assert_eq!(prepared.request(), &intent);
    let fields = serde_json::to_value(&prepared)?;
    assert_eq!(fields["TransactionType"], "Payment");
    assert_eq!(fields["Amount"], "9007199254740993");
    assert_eq!(fields["DestinationTag"], 0);
    assert_eq!(fields["Fee"], "12");
    for absent in [
        "NetworkID",
        "TxnSignature",
        "SigningPubKey",
        "Paths",
        "SendMax",
        "Flags",
    ] {
        assert!(fields.get(absent).is_none());
    }
    for number in [1, 2, 1024] {
        assert!(
            serde_json::to_value(
                payment(number, None, PaymentAmount::Xrp(Drops::new(1)?))?.prepare()
            )?
            .get("NetworkID")
            .is_none()
        );
    }
    assert_eq!(
        serde_json::to_value(payment(1025, None, PaymentAmount::Xrp(Drops::new(1)?))?.prepare())?["NetworkID"],
        1025
    );
    let issued = PaymentAmount::Issued {
        currency: Currency::parse("USD")?,
        issuer: Address::parse(PEER)?,
        value: IssuedValue::parse("9007199254740993")?,
    };
    assert_eq!(
        serde_json::to_value(payment(0, None, issued)?.prepare())?["Amount"]["value"],
        "9007199254740993"
    );
    assert_eq!(
        serde_json::from_str::<PaymentRequest>(&serde_json::to_string(&intent)?)?,
        intent
    );
    Ok(())
}

#[test]
fn payment_rejects_invalid_intent_and_unknown_issued_fields()
-> Result<(), Box<dyn std::error::Error>> {
    assert!(payment(0, None, PaymentAmount::Xrp(Drops::new(0)?)).is_err());
    assert!(
        payment(
            0,
            None,
            PaymentAmount::Issued {
                currency: Currency::parse("USD")?,
                issuer: Address::parse(PEER)?,
                value: IssuedValue::parse("-1")?
            }
        )
        .is_err()
    );
    let mut fields = serde_json::to_value(payment(0, None, PaymentAmount::Xrp(Drops::new(1)?))?)?;
    fields["destination"]["network"] = json!(1);
    assert!(serde_json::from_value::<PaymentRequest>(fields).is_err());
    let mut issued = json!({"currency":"USD", "issuer":PEER, "value":"1", "arbitrary":true});
    assert!(serde_json::from_value::<PaymentAmount>(issued.clone()).is_err());
    issued
        .as_object_mut()
        .ok_or("expected object")?
        .remove("arbitrary");
    assert!(serde_json::from_value::<PaymentAmount>(issued).is_ok());
    for key in ["fee", "sequence", "last_ledger_sequence"] {
        let mut fields =
            serde_json::to_value(payment(0, None, PaymentAmount::Xrp(Drops::new(1)?))?)?;
        fields[key] = if key == "fee" { json!("0") } else { json!(0) };
        assert!(serde_json::from_value::<PaymentRequest>(fields).is_err());
    }
    Ok(())
}

struct LocalReader(Rc<AccountBalance>);

fn require_send<T: Send>(value: T) -> T {
    value
}
impl XrplReader for LocalReader {
    fn get_account_balance(
        &self,
        _account: Address,
        _ledger_hash: Option<Hash>,
    ) -> impl Future<Output = Result<Observation<AccountBalance>, Error>> + Send {
        ready(
            Ledger::new(1, Some(Hash::from_bytes([1; 32])), true).and_then(|ledger| {
                Observation::account_balance(
                    (*self.0).clone(),
                    context(Operation::AccountBalance, ledger)?,
                )
            }),
        )
    }
    fn get_trust_lines(
        &self,
        _account: Address,
        _request: PageRequest,
    ) -> impl Future<Output = Result<Observation<TrustLinePage>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_fee_estimate(
        &self,
    ) -> impl Future<Output = Result<Observation<FeeEstimate>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_transaction(
        &self,
        _hash: Hash,
    ) -> impl Future<Output = Result<Observation<Transaction>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_transaction_status(
        &self,
        _hash: Hash,
    ) -> impl Future<Output = Result<Observation<TransactionStatus>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
    fn get_account_history(
        &self,
        _account: Address,
        _request: HistoryRequest,
    ) -> impl Future<Output = Result<Observation<HistoryPage>, Error>> + Send {
        ready(Err(Error::UnsupportedCapability))
    }
}

#[test]
fn actual_binary_history_blob_hash_matches_real_transaction_id_without_signature_claim()
-> Result<(), Box<dyn std::error::Error>> {
    let captured: Value = serde_json::from_str(include_str!("fixtures/xrpl_binary_history.json"))?;
    let entry = &captured["result"]["transactions"][0];
    assert!(entry.get("hash").is_none());
    assert!(entry.get("ledger_hash").is_none());
    let payload = HexData::parse(entry["tx_blob"].as_str().ok_or("missing payload")?)?;
    let hash = Hash::parse("AA422A0AF62C58042BE3A243954CC1BD2560E2345F475E577F8E21F4D455BEA5")?;
    assert_eq!(payload.transaction_hash(), hash);
    let metadata = HexData::parse(entry["meta_blob"].as_str().ok_or("missing metadata")?)?;
    let transaction = Transaction::new(
        hash,
        payload.clone(),
        Some(metadata.clone()),
        Some(Ledger::new(106_836_736, None, true)?),
    )?;
    assert_eq!(
        serde_json::from_str::<Transaction>(&serde_json::to_string(&transaction)?)?,
        transaction
    );
    assert!(
        Transaction::new(
            Hash::from_bytes([0; 32]),
            payload.clone(),
            Some(metadata),
            transaction.ledger()
        )
        .is_err()
    );
    assert!(Transaction::new(hash, payload.clone(), None, transaction.ledger()).is_err());
    assert!(Transaction::new(hash, payload, None, None).is_ok());
    for malformed in ["", "0", "0x00", "ZZ"] {
        assert!(HexData::parse(malformed).is_err());
    }
    assert!(HexData::parse(&"00".repeat(HexData::MAX_BYTES + 1)).is_err());
    Ok(())
}

#[test]
fn transaction_execution_is_separate_from_validation_and_missing_inclusion()
-> Result<(), Box<dyn std::error::Error>> {
    let hash = Hash::from_bytes([1; 32]);
    let validated = Ledger::new(100, Some(Hash::from_bytes([2; 32])), true)?;
    let failure = TransactionStatus::new(
        hash,
        Some(validated),
        Some(ResultCode::parse("tecPATH_DRY")?),
    )?;
    assert!(failure.validated());
    assert!(!failure.execution().ok_or("missing result")?.is_success());
    let pending = TransactionStatus::new(hash, None, None)?;
    assert!(!pending.validated());
    assert!(pending.ledger().is_none());
    assert!(pending.execution().is_none());
    assert!(TransactionStatus::new(hash, Some(validated), None).is_err());
    assert!(TransactionStatus::new(hash, None, Some(ResultCode::parse("tesSUCCESS")?)).is_err());
    let attribution = Context::new(
        Operation::TransactionStatus,
        network(0)?,
        None,
        Source::new("fixture", "tx", "0.1.0")?,
        Timestamp::from_unix_seconds(1),
    );
    let observation = Observation::transaction_status(pending, attribution)?;
    assert_eq!(
        serde_json::from_str::<Observation<TransactionStatus>>(&serde_json::to_string(
            &observation
        )?)?,
        observation
    );
    for malformed in [
        "SECRET_INPUT",
        "terQUEUED",
        "tesOTHER",
        "tec",
        "tecSECRET\n",
    ] {
        assert!(ResultCode::parse(malformed).is_err());
    }
    Ok(())
}

#[test]
fn history_retains_actual_range_and_structural_marker_without_invented_hash_or_order()
-> Result<(), Box<dyn std::error::Error>> {
    let captured: Value = serde_json::from_str(include_str!("fixtures/xrpl_binary_history.json"))?;
    let entry = &captured["result"]["transactions"][0];
    let payload = HexData::parse(entry["tx_blob"].as_str().ok_or("missing payload")?)?;
    let hash = payload.transaction_hash();
    let transaction = Transaction::new(
        hash,
        payload,
        Some(HexData::parse(
            entry["meta_blob"].as_str().ok_or("missing metadata")?,
        )?),
        Some(Ledger::new(106_836_736, None, true)?),
    )?;
    let requested = LedgerRange::new(32_570, 107_483_958)?;
    let request = HistoryRequest::new(requested, 2, false, None)?;
    let searched = LedgerRange::new(106_000_000, 107_483_958)?;
    let marker = HistoryMarker::new(106_836_736, 0)?;
    let page = HistoryPage::new(
        Address::parse(ACCOUNT)?,
        request.clone(),
        searched,
        vec![transaction.clone()],
        Some(marker),
    )?;
    assert_eq!(page.searched(), searched);
    assert_eq!(page.transactions()[0].ledger().and_then(Ledger::hash), None);
    assert_eq!(
        serde_json::from_str::<HistoryPage>(&serde_json::to_string(&page)?)?,
        page
    );
    assert!(
        HistoryPage::new(
            Address::parse(ACCOUNT)?,
            request.clone(),
            searched,
            vec![transaction.clone(), transaction.clone()],
            None
        )
        .is_err()
    );
    assert!(
        HistoryPage::new(
            Address::parse(ACCOUNT)?,
            request.clone(),
            LedgerRange::new(1, 107_483_958)?,
            vec![],
            None
        )
        .is_err()
    );
    assert!(
        HistoryPage::new(
            Address::parse(ACCOUNT)?,
            HistoryRequest::new(requested, 1, false, Some(marker))?,
            searched,
            vec![transaction],
            Some(marker)
        )
        .is_err()
    );
    assert!(HistoryRequest::new(requested, 0, false, None).is_err());
    assert!(HistoryRequest::new(requested, 401, false, None).is_err());
    assert!(HistoryRequest::new(requested, 2, false, Some(HistoryMarker::new(1, 0)?)).is_err());
    assert!(LedgerRange::new(100, 99).is_err());
    assert!(HistoryMarker::new(0, 0).is_err());
    assert!(serde_json::from_str::<HistoryMarker>("{\"ledger\":100,\"seq\":0,\"seq\":1}").is_err());
    Ok(())
}
#[test]
fn pure_capability_accepts_non_send_reader_and_send_ready_future_without_runtime()
-> Result<(), Box<dyn std::error::Error>> {
    let account = Address::parse(ACCOUNT)?;
    let reader = LocalReader(Rc::new(AccountBalance::new(
        account,
        Drops::new(1)?,
        1,
        0,
        0,
    )));
    let future = reader.get_account_balance(account, None);
    let mut future = std::pin::pin!(require_send(future));
    let mut context = TaskContext::from_waker(Waker::noop());
    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Ok(_))
    ));
    Ok(())
}

#[test]
fn untrusted_values_retain_fixed_diagnostics_without_input() {
    let secret = "PRIVATE_SECRET_ENDPOINT";
    let error = Address::parse(secret).unwrap_err();
    assert!(!format!("{error:?} {error}").contains(secret));
    assert_eq!(
        serde_json::to_value(error).unwrap_or(Value::Null),
        json!({"category":"validation","reason":"invalid_xrpl_address"})
    );
}
