//! S/MIME: known vectors made with OpenSSL 3.5.5 (fixtures/README.md), certificates made in
//! the tests, and what Telex writes read back by Telex (and by OpenSSL, with
//! `write_mail_for_openssl`).

use super::*;
use crate::model::{AccountSetup, Folder, FolderKind, MessageId};
use crate::openpgp::Choice;
use crate::store::SyncedMessage;
use std::str::FromStr;

const CA: &[u8] = include_bytes!("fixtures/ca.pem");
const ALICE_CERT: &[u8] = include_bytes!("fixtures/alice.pem");
const ALICE_KEY: &str = include_str!("fixtures/alice.key");
const BOB_CERT: &[u8] = include_bytes!("fixtures/bob.pem");
const ALICE_P12: &[u8] = include_bytes!("fixtures/alice.p12");
const ALICE_LEGACY_P12: &[u8] = include_bytes!("fixtures/alice-legacy.p12");
const BOB_P12: &[u8] = include_bytes!("fixtures/bob.p12");
const SIGNED: &[u8] = include_bytes!("fixtures/signed.eml");
const OPAQUE: &[u8] = include_bytes!("fixtures/opaque.eml");
const OPAQUE_BER: &[u8] = include_bytes!("fixtures/opaque-ber.eml");
const ENCRYPTED: &[u8] = include_bytes!("fixtures/encrypted.eml");
const ENCRYPTED_BER: &[u8] = include_bytes!("fixtures/encrypted-ber.eml");
const ENCRYPTED_3DES: &[u8] = include_bytes!("fixtures/encrypted-3des.eml");
const ENCRYPTED_OAEP: &[u8] = include_bytes!("fixtures/encrypted-oaep.eml");
const SIGNED_ENCRYPTED: &[u8] = include_bytes!("fixtures/signed-encrypted.eml");
/// The password of alice.p12 (UTF-8 for PBES2, UTF-16 for the MAC: both must be right).
const ALICE_PASS: &str = "zażółć";
const ALICE_FP: &str = "5D72EDF7989391A1828D85A0EE8917FA9A5382657559851CA61E8769A747A68B";

fn now() -> NaiveDateTime {
    chrono::Local::now().naive_local()
}

/// A mail base for `email`: one IMAP account with an Inbox.
fn base(email: &str) -> MailStore {
    let mut store = MailStore::in_memory().unwrap();
    let account = store
        .upsert_account(&AccountSetup {
            name: "Test".into(),
            email: email.into(),
            host: "imap.example.com".into(),
            port: 993,
            username: email.into(),
            ..Default::default()
        })
        .unwrap();
    store
        .set_folders(
            &account.id,
            &[Folder {
                id: "inbox".into(),
                name: "Inbox".into(),
                kind: FolderKind::Inbox,
                remote: Some("INBOX".into()),
            }],
        )
        .unwrap();
    store
}

fn add(store: &mut MailStore, raw: &[u8]) -> MessageId {
    let folder = store.accounts()[0].folder_key("inbox");
    let uid = store
        .db
        .query_row("SELECT COUNT(*) + 1 FROM messages", [], |r| r.get(0))
        .unwrap();
    let message = crate::mime::parse_message(raw, "", None);
    store
        .add_synced(
            &folder,
            &[SyncedMessage {
                uid,
                message,
                raw: raw.to_vec(),
            }],
        )
        .unwrap()[0]
}

fn view(store: &MailStore, id: MessageId) -> (crate::risk::InspectedMessage, View) {
    let inspected = store.inspect_message(id).unwrap();
    let view = inspected.pgp.clone().expect("S/MIME found");
    assert!(view.smime);
    (inspected, view)
}

fn status(store: &MailStore, id: MessageId) -> SigStatus {
    view(store, id).1.signature.expect("a signature").status
}

/// The test authority counts as one the system trusts, on this thread.
fn trust_test_ca(on: bool) {
    let anchors = on.then(|| vec![read_certificates(CA).remove(0)]);
    trust::TEST_ANCHORS.with(|a| *a.borrow_mut() = anchors);
}

fn bob() -> MailStore {
    let mut s = base("bob@example.com");
    assert!(s.import_p12(BOB_P12, "bobpass", now()).unwrap().is_some());
    s
}

fn alice() -> MailStore {
    let mut s = base("alice@example.com");
    assert!(s
        .import_p12(ALICE_P12, ALICE_PASS, now())
        .unwrap()
        .is_some());
    s
}

// ---- PKCS #12 ----

#[test]
fn modern_and_legacy_p12_files_open_with_their_passwords() {
    let alice_cert = read_certificates(ALICE_CERT).remove(0);
    let ca = read_certificates(CA).remove(0);
    let file = p12::read(ALICE_P12, ALICE_PASS).unwrap();
    assert_eq!(file.cert, alice_cert);
    assert_eq!(file.chain, vec![ca.clone()]);
    assert!(matches!(private_key(&file.key), Some(PrivateKey::Rsa(_))));
    // OpenSSL's -legacy: 3DES for the key, 40-bit RC2 for the certificates, a SHA-1 MAC.
    let legacy = p12::read(ALICE_LEGACY_P12, "test1234").unwrap();
    assert_eq!(legacy.cert, alice_cert);
    assert_eq!(legacy.chain, vec![ca]);
    assert_eq!(*legacy.key, *file.key);
    for wrong in ["zazolc", "", "test1234"] {
        assert_eq!(
            p12::read(ALICE_P12, wrong).err(),
            Some(p12::P12Error::WrongPassword)
        );
    }
    assert_eq!(p12::read(SIGNED, "x").err(), Some(p12::P12Error::NotP12));
    assert!(p12::is_p12(ALICE_P12) && p12::is_p12(ALICE_LEGACY_P12));
    assert!(!p12::is_p12(ALICE_CERT));
}

#[test]
fn an_imported_p12_is_kept_encrypted_and_asks_for_its_password_next_time() {
    let mut s = base("alice@example.com");
    assert_eq!(s.import_p12(ALICE_P12, "nope", now()).unwrap(), None);
    let report = s.import_p12(ALICE_P12, ALICE_PASS, now()).unwrap().unwrap();
    assert_eq!(report.added[0].fingerprint, ALICE_FP);
    assert_eq!(report.added[0].name, "Alice Example <alice@example.com>");
    // The private key isn't stored as it is: not even a piece of it shows in the row.
    let file = p12::read(ALICE_P12, ALICE_PASS).unwrap();
    let sealed: Vec<u8> =
        s.db.query_row("SELECT secret FROM smime_certs", [], |r| r.get(0))
            .unwrap();
    assert!(!file
        .key
        .chunks(16)
        .filter(|c| c.len() == 16)
        .any(|c| sealed.windows(16).any(|w| w == c)));
    let certs = s.smime_certs().unwrap();
    assert_eq!(certs.len(), 1);
    let c = &certs[0];
    assert!(c.secret && c.protected && c.can_sign && c.can_encrypt && !c.expired);
    assert_eq!(c.emails, ["alice@example.com"]);
    assert_eq!(c.issuer, "Telex Test CA");
    assert_eq!(c.algorithm, "RSA 2048");
    assert_eq!(s.smime_unlocked(), [ALICE_FP]);
    // A new session: locked until the password is given again.
    s.smime_lock(ALICE_FP);
    assert!(matches!(
        s.own_cert_for("alice@example.com"),
        Err(WriteProblem::CertLocked { .. })
    ));
    assert!(!s.smime_unlock(ALICE_FP, "wrong").unwrap());
    assert!(s.smime_unlock(ALICE_FP, ALICE_PASS).unwrap());
    assert!(s.own_cert_for("Alice@Example.com").is_ok());
    // Importing again changes nothing.
    let again = s.import_p12(ALICE_P12, ALICE_PASS, now()).unwrap().unwrap();
    assert_eq!(again.unchanged.len(), 1);
}

#[test]
fn certificate_files_import_as_confirmed_and_authorities_are_skipped() {
    let mut s = base("alice@example.com");
    let both = [BOB_CERT, CA].concat();
    let report = s.import_certificates(&both, now()).unwrap();
    assert_eq!(report.added.len(), 1);
    assert_eq!(report.skipped, 1);
    let c = &s.smime_certs().unwrap()[0];
    assert!(c.verified && !c.secret);
    assert_eq!(c.source, "file");
    assert!(s
        .export_cert(&c.fingerprint)
        .unwrap()
        .starts_with("-----BEGIN CERTIFICATE-----"));
    assert!(s.import_certificates(b"not a certificate", now()).is_err());
    s.delete_cert(&c.fingerprint).unwrap();
    assert!(s.smime_certs().unwrap().is_empty());
}

// ---- Signed mail (OpenSSL) ----

#[test]
fn openssl_signed_mail_is_unconfirmed_until_its_authority_is_trusted() {
    let mut s = base("bob@example.com");
    let id = add(&mut s, SIGNED);
    let (shown, v) = view(&s, id);
    let sig = v.signature.clone().unwrap();
    // The test authority isn't one Windows (or the system) trusts.
    assert_eq!(sig.status, SigStatus::Unconfirmed);
    assert_eq!(sig.key.as_ref().unwrap().fingerprint, ALICE_FP);
    assert_eq!(sig.issuer.as_deref(), Some("Telex Test CA"));
    assert!(sig.created.is_some());
    assert!(!v.encrypted && !v.partial);
    // The signature isn't an attachment; the text is the signed text.
    assert!(shown.message.attachments.is_empty());
    assert!(shown.message.body.contains("Cześć Bob"));
    // Its certificate was learned when the message came, to encrypt to Alice later.
    let learned = s.smime_certs().unwrap();
    assert_eq!(learned.len(), 1);
    assert_eq!(learned[0].source, "message");
    assert!(!learned[0].verified);
    trust_test_ca(true);
    s.pgp.borrow_mut().forget_opened();
    assert_eq!(status(&s, id), SigStatus::Good);
    trust_test_ca(false);
    // Or the user confirms the certificate after comparing its fingerprint.
    s.set_cert_verified(ALICE_FP, true).unwrap();
    assert_eq!(status(&s, id), SigStatus::Good);
}

#[test]
fn a_changed_or_misaddressed_signed_message_says_so() {
    trust_test_ca(true);
    let text = String::from_utf8(SIGNED.to_vec()).unwrap();
    let mut s = base("bob@example.com");
    let changed = add(&mut s, text.replace("in spirit", "in person").as_bytes());
    assert_eq!(status(&s, changed), SigStatus::Bad);
    let other = add(
        &mut s,
        text.replace("From: alice@example.com", "From: mallory@example.com")
            .as_bytes(),
    );
    assert_eq!(status(&s, other), SigStatus::OtherSender);
    // A broken signature part.
    let broken = text.replacen("MIIH", "MIIA", 1);
    let id = add(&mut s, broken.as_bytes());
    assert_eq!(status(&s, id), SigStatus::Bad);
    trust_test_ca(false);
}

#[test]
fn opaque_signed_mail_shows_what_it_carries() {
    trust_test_ca(true);
    let mut s = base("bob@example.com");
    for raw in [OPAQUE, OPAQUE_BER] {
        let id = add(&mut s, raw);
        let (shown, v) = view(&s, id);
        assert_eq!(v.signature.unwrap().status, SigStatus::Good);
        assert!(shown
            .message
            .body
            .contains("the contract is attached in spirit"));
        assert!(
            shown.message.attachments.is_empty(),
            "smime.p7m isn't listed"
        );
    }
    trust_test_ca(false);
}

// ---- Encrypted mail (OpenSSL) ----

#[test]
fn openssl_encrypted_mail_decrypts_with_bobs_certificate() {
    let mut s = bob();
    // AES-256, streamed BER (as Thunderbird writes), 3DES, RSA-OAEP with AES-128.
    for raw in [ENCRYPTED, ENCRYPTED_BER, ENCRYPTED_3DES, ENCRYPTED_OAEP] {
        let id = add(&mut s, raw);
        let (shown, v) = view(&s, id);
        assert!(v.encrypted && v.decrypted, "{:?}", v.problem);
        assert!(v.signature.is_none());
        assert!(shown.message.body.contains("Cześć Bob"));
        assert!(shown.message.attachments.is_empty());
        // Nothing decrypted reached the mail base or the search index.
        let stored: String =
            s.db.query_row(
                "SELECT body FROM message_content WHERE id = ?1",
                [id as i64],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!stored.contains("Cześć"));
    }
    let found: i64 =
        s.db.query_row(
            "SELECT COUNT(*) FROM messages_fts WHERE messages_fts MATCH 'contract'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(found, 0, "not in the search index");
}

#[test]
fn signed_then_encrypted_mail_shows_both() {
    trust_test_ca(true);
    let mut s = bob();
    let id = add(&mut s, SIGNED_ENCRYPTED);
    let (shown, v) = view(&s, id);
    assert!(v.decrypted);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert!(shown.message.body.contains("Cześć Bob"));
    assert!(shown.message.attachments.is_empty());
    trust_test_ca(false);
}

#[test]
fn locked_and_someone_elses_mail_say_why() {
    let mut s = bob();
    let bob_fp = s.smime_certs().unwrap()[0].fingerprint.clone();
    s.smime_lock(&bob_fp);
    let id = add(&mut s, ENCRYPTED);
    let (shown, v) = view(&s, id);
    assert_eq!(v.problem, Some(Problem::Locked));
    assert_eq!(v.locked[0].fingerprint, bob_fp);
    assert_eq!(shown.message.body, "");
    assert!(s.smime_unlock(&bob_fp, "bobpass").unwrap());
    assert!(view(&s, id).1.decrypted);
    // Alice has no key for mail encrypted to Bob only.
    let mut a = alice();
    let id = add(&mut a, ENCRYPTED);
    let v = view(&a, id).1;
    assert_eq!(v.problem, Some(Problem::NotForYou));
    assert_eq!(v.recipients.len(), 1);
    assert!(v.recipients[0].starts_with("Telex Test CA · "));
    // Damaged: the encrypted part cut short.
    let text = String::from_utf8(ENCRYPTED.to_vec()).unwrap();
    let cut: String = text
        .lines()
        .take(text.lines().count() - 3)
        .collect::<Vec<_>>()
        .join("\r\n");
    let id = add(&mut s, cut.as_bytes());
    assert!(!view(&s, id).1.decrypted);
}

// ---- Writing ----

fn draft(to: &str, bcc: &str) -> crate::model::OutgoingDraft {
    crate::model::OutgoingDraft {
        account: "x".into(),
        to: to.into(),
        bcc: bcc.into(),
        subject: "Umowa na październik".into(),
        body: "Dzień dobry,\nw załączniku poprawki.\n\n-- \nAlice\n".into(),
        ..Default::default()
    }
}

fn composed(s: &MailStore, d: &crate::model::OutgoingDraft, choice: Choice) -> Vec<u8> {
    let account = s.accounts()[0].clone();
    let to = crate::smtp::recipient_emails(&d.to).unwrap();
    let bcc = crate::smtp::recipient_emails(&d.bcc).unwrap_or_default();
    let p = s
        .protection(&account.email, &to, &bcc, choice, true, false)
        .unwrap();
    crate::smtp::compose_protected(&account, d, &Default::default(), chrono::Local::now(), &p)
        .unwrap()
        .raw
}

const SIGN: Choice = Choice {
    sign: true,
    encrypt: false,
    attach_key: false,
    smime: true,
};
const BOTH: Choice = Choice {
    sign: true,
    encrypt: true,
    attach_key: false,
    smime: true,
};

#[test]
fn a_signed_message_verifies_at_the_recipient() {
    trust_test_ca(true);
    let a = alice();
    let raw = composed(&a, &draft("Bob <bob@example.com>", ""), SIGN);
    let text = String::from_utf8(raw.clone()).unwrap();
    assert!(text.contains("multipart/signed; protocol=\"application/pkcs7-signature\""));
    assert!(text.contains("micalg=sha-256"));
    assert!(text.contains("Subject: Umowa na"));
    assert!(!text.contains("Autocrypt"));
    // Quoted-printable: no line ends in a space for a server to trim.
    assert!(text.split("\r\n").all(|l| !l.ends_with(' ')));
    let mut b = base("bob@example.com");
    let id = add(&mut b, &raw);
    let (shown, v) = view(&b, id);
    let sig = v.signature.unwrap();
    assert_eq!(sig.status, SigStatus::Good);
    assert_eq!(sig.key.unwrap().fingerprint, ALICE_FP);
    assert!(shown.message.body.contains("w załączniku poprawki"));
    assert!(shown.message.attachments.is_empty());
    // Bob now has Alice's certificate to encrypt his answer to.
    assert!(b.cert_for("alice@example.com").is_some());
    trust_test_ca(false);
}

#[test]
fn an_encrypted_message_reads_back_for_the_sender_and_the_recipient() {
    trust_test_ca(true);
    let mut a = alice();
    a.import_certificates(BOB_CERT, now()).unwrap();
    let raw = composed(&a, &draft("bob@example.com", ""), BOTH);
    let text = String::from_utf8(raw.clone()).unwrap();
    assert!(text.contains("application/pkcs7-mime; smime-type=enveloped-data"));
    assert!(!text.contains("poprawki"));
    // S/MIME programs show the outer subject: it stays.
    assert!(text.contains("Subject: Umowa na"));
    // Alice reads her own copy.
    let id = add(&mut a, &raw);
    let (shown, v) = view(&a, id);
    assert!(v.decrypted);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert!(shown.message.body.contains("w załączniku poprawki"));
    // So does Bob.
    let mut b = bob();
    let id = add(&mut b, &raw);
    let (shown, v) = view(&b, id);
    assert!(v.decrypted, "{:?}", v.problem);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert!(shown.message.body.contains("w załączniku poprawki"));
    trust_test_ca(false);
}

#[test]
fn writing_says_what_is_missing() {
    let mut a = alice();
    let account = a.accounts()[0].email.clone();
    let to = |s: &str| vec![s.to_owned()];
    // Nobody's certificate yet.
    assert_eq!(
        a.protection(&account, &to("bob@example.com"), &[], BOTH, false, false)
            .err(),
        Some(WriteProblem::NoCertFor {
            emails: vec!["bob@example.com".into()]
        })
    );
    a.import_certificates(BOB_CERT, now()).unwrap();
    // Blind copies would be named in the message.
    assert_eq!(
        a.protection(
            &account,
            &to("bob@example.com"),
            &to("carol@example.com"),
            BOTH,
            false,
            false
        )
        .err(),
        Some(WriteProblem::BlindCopies)
    );
    // Signing without the password.
    a.smime_lock(ALICE_FP);
    assert!(matches!(
        a.protection(&account, &to("bob@example.com"), &[], SIGN, false, false),
        Err(WriteProblem::CertLocked { .. })
    ));
    // A draft is only encrypted to the sender, and needs no password.
    let p = a
        .protection(&account, &to("carol@example.com"), &[], BOTH, false, true)
        .unwrap();
    let s = p.smime.unwrap();
    assert!(s.sign.is_none());
    assert_eq!(s.encrypt.len(), 1);
    // Someone without a certificate of their own.
    let c = base("carol@example.com");
    assert_eq!(
        c.protection(
            "carol@example.com",
            &to("bob@example.com"),
            &[],
            SIGN,
            false,
            false
        )
        .err(),
        Some(WriteProblem::NoOwnCert {
            email: "carol@example.com".into()
        })
    );
}

#[test]
fn compose_starts_with_smime_where_it_fits() {
    let mut a = alice();
    let st = a.compose_state(
        "alice@example.com",
        &["bob@example.com".into()],
        None,
        false,
    );
    assert!(st.smime, "Alice has a certificate and no OpenPGP key");
    assert_eq!(st.smime_own.as_ref().unwrap().fingerprint, ALICE_FP);
    assert!(st.smime_unlocked);
    assert_eq!(st.smime_missing, ["bob@example.com"]);
    assert!(!st.smime_can_encrypt && !st.encrypt);
    // Bob's signed mail brings his certificate; his authority is trusted.
    trust_test_ca(true);
    let id = add(
        &mut a,
        &composed(
            &bob(),
            &{
                let mut d = draft("alice@example.com", "");
                d.account = "x".into();
                d
            },
            SIGN,
        ),
    );
    let st = a.compose_state(
        "alice@example.com",
        &["bob@example.com".into()],
        Some(id),
        false,
    );
    assert!(st.smime_can_encrypt && st.sign);
    trust_test_ca(false);
}

// ---- Certificates made in the tests ----

struct Authority {
    key: p256::ecdsa::SigningKey,
    name: x509_cert::name::Name,
    cert: Vec<u8>,
}

fn time(secs: i64) -> x509_cert::time::Time {
    x509_cert::time::Time::UtcTime(
        der::asn1::UtcTime::from_unix_duration(std::time::Duration::from_secs(secs as u64))
            .unwrap(),
    )
}

fn authority() -> Authority {
    use x509_cert::builder::{Builder, CertificateBuilder, Profile};
    let key = p256::ecdsa::SigningKey::random(&mut rand::rngs::OsRng);
    let name = x509_cert::name::Name::from_str("CN=Made In Test CA").unwrap();
    let spki = SubjectPublicKeyInfoOwned::from_key(*key.verifying_key()).unwrap();
    let validity = x509_cert::time::Validity {
        not_before: time(now_secs() - 86_400 * 3650),
        not_after: time(now_secs() + 86_400 * 3650),
    };
    let builder = CertificateBuilder::new(
        Profile::Root,
        x509_cert::serial_number::SerialNumber::from(1u32),
        validity,
        name.clone(),
        spki,
        &key,
    )
    .unwrap();
    let cert = builder
        .build::<p256::ecdsa::DerSignature>()
        .unwrap()
        .to_der()
        .unwrap();
    Authority { key, name, cert }
}

/// A certificate for `email` with Alice's RSA key, valid from `from` to `to` (Unix seconds).
fn leaf(ca: &Authority, email: &str, from: i64, to: i64, serial: u32) -> Vec<u8> {
    use pkcs8::DecodePrivateKey;
    use x509_cert::builder::{Builder, CertificateBuilder, Profile};
    use x509_cert::ext::pkix::name::GeneralName;
    let key = rsa::RsaPrivateKey::from_pkcs8_pem(ALICE_KEY).unwrap();
    let spki = SubjectPublicKeyInfoOwned::from_key(key.to_public_key()).unwrap();
    let mut builder = CertificateBuilder::new(
        Profile::Leaf {
            issuer: ca.name.clone(),
            enable_key_agreement: false,
            enable_key_encipherment: true,
        },
        x509_cert::serial_number::SerialNumber::from(serial),
        x509_cert::time::Validity {
            not_before: time(from),
            not_after: time(to),
        },
        x509_cert::name::Name::from_str("CN=Made In Test").unwrap(),
        spki,
        &ca.key,
    )
    .unwrap();
    builder
        .add_extension(&x509_cert::ext::pkix::SubjectAltName(vec![
            GeneralName::Rfc822Name(der::asn1::Ia5String::new(email).unwrap()),
        ]))
        .unwrap();
    builder
        .add_extension(&x509_cert::ext::pkix::ExtendedKeyUsage(vec![EKU_EMAIL]))
        .unwrap();
    builder
        .build::<p256::ecdsa::DerSignature>()
        .unwrap()
        .to_der()
        .unwrap()
}

fn own(cert: Vec<u8>, ca: &Authority) -> OwnCert {
    use pkcs8::DecodePrivateKey;
    OwnCert {
        fingerprint: hex(&sha2::Sha256::digest(&cert)),
        cert,
        chain: vec![ca.cert.clone()],
        key: PrivateKey::Rsa(Box::new(
            rsa::RsaPrivateKey::from_pkcs8_pem(ALICE_KEY).unwrap(),
        )),
    }
}

/// A message from `from` signed with `signer`, as Telex writes it.
fn signed_by(signer: OwnCert, from: &str) -> Vec<u8> {
    let raw = format!(
        "From: {from}\r\nTo: bob@example.com\r\nSubject: Test\r\nDate: Sat, 17 Oct 2026 10:00:00 +0200\r\n\
         Message-ID: <{}@example.com>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello.\r\n",
        signer.fingerprint
    );
    protect(
        raw.as_bytes(),
        &Protect {
            sign: Some(signer),
            encrypt: Vec::new(),
        },
    )
    .unwrap()
}

#[test]
fn made_certificates_check_dates_addresses_and_chains() {
    let ca = authority();
    let now = now_secs();
    let year = 86_400 * 365;
    let good = leaf(&ca, "anna@example.com", now - year, now + year, 10);
    let expired = leaf(&ca, "anna@example.com", now - 2 * year, now - year, 11);
    let elsewhere = leaf(&ca, "carol@example.com", now - year, now + year, 12);
    assert_eq!(
        trust::with_anchors(&good, &[], std::slice::from_ref(&ca.cert), now),
        trust::Trust::Trusted
    );
    assert_eq!(
        trust::with_anchors(&good, &[], &[], now),
        trust::Trust::Unknown
    );
    let other_ca = authority();
    assert_eq!(
        trust::with_anchors(&good, &[], std::slice::from_ref(&other_ca.cert), now),
        trust::Trust::Untrusted
    );
    // Windows doesn't know an authority made a moment ago.
    #[cfg(windows)]
    assert_eq!(
        trust::chain(&good, std::slice::from_ref(&ca.cert), now),
        trust::Trust::Untrusted
    );

    trust::TEST_ANCHORS.with(|a| *a.borrow_mut() = Some(vec![ca.cert.clone()]));
    let mut s = base("bob@example.com");
    let id = add(
        &mut s,
        &signed_by(own(good.clone(), &ca), "anna@example.com"),
    );
    assert_eq!(status(&s, id), SigStatus::Good);
    let id = add(&mut s, &signed_by(own(expired, &ca), "anna@example.com"));
    assert_eq!(status(&s, id), SigStatus::Expired);
    let id = add(&mut s, &signed_by(own(elsewhere, &ca), "anna@example.com"));
    assert_eq!(status(&s, id), SigStatus::OtherSender);
    // Without the certificate in the message, and none stored: no key.
    let mut bare = own(good, &ca);
    bare.chain.clear();
    let raw = signed_by(bare, "nobody@example.com");
    let found = find(&raw).unwrap();
    let Found::Signed {
        signature, signed, ..
    } = found
    else {
        panic!()
    };
    let mut sd = signed_data(&signature).unwrap();
    sd.certificates = None;
    let checked = check(&base("x@example.com").db, &sd, &signed);
    assert!(matches!(checked[0], Checked::NoCert { .. }));
    assert_eq!(
        verdict(&checked[0], "nobody@example.com", now).status,
        SigStatus::NoKey
    );
    trust_test_ca(false);
}

#[test]
fn hostile_cms_is_refused_quietly() {
    let mut s = bob();
    // A pkcs7-mime part that is no CMS, one nested far too deep, one far too large.
    let deep: Vec<u8> = std::iter::repeat_n([0x30u8, 0x80], 5_000)
        .flatten()
        .collect();
    for body in [b"garbage".to_vec(), deep] {
        let raw = format!(
            "From: x@example.com\r\nTo: bob@example.com\r\nSubject: x\r\nMIME-Version: 1.0\r\n\
             Content-Type: application/pkcs7-mime; smime-type=enveloped-data; name=smime.p7m\r\n\
             Content-Transfer-Encoding: base64\r\n\r\n{}\r\n",
            base64_lines(&body)
        );
        assert!(matches!(
            find(raw.as_bytes()),
            Some(Found::Enveloped { .. })
        ));
        let id = add(&mut s, raw.as_bytes());
        let (shown, v) = view(&s, id);
        assert_eq!(v.problem, Some(Problem::Failed));
        assert!(shown.message.attachments.is_empty());
    }
    // An authEnveloped message (AES-GCM) isn't read: said so.
    let auth = ContentInfo {
        content_type: ID_AUTH_ENVELOPED_DATA,
        content: der::Any::new(der::Tag::Sequence, Vec::<u8>::new()).unwrap(),
    }
    .to_der()
    .unwrap();
    let raw = format!(
        "From: x@example.com\r\nTo: bob@example.com\r\nSubject: x\r\nMIME-Version: 1.0\r\n\
         Content-Type: application/pkcs7-mime; smime-type=authEnveloped-data; name=smime.p7m\r\n\
         Content-Transfer-Encoding: base64\r\n\r\n{}\r\n",
        base64_lines(&auth)
    );
    let id = add(&mut s, raw.as_bytes());
    assert_eq!(view(&s, id).1.problem, Some(Problem::Unsupported));
}

#[test]
fn the_polish_says_what_is_missing() {
    crate::i18n::with(crate::i18n::Lang::Pl, || {
        assert!(WriteProblem::BlindCopies
            .to_string()
            .starts_with("Szyfrowanie S/MIME"));
        assert!(WriteProblem::NoCertFor {
            emails: vec!["bob@example.com".into()]
        }
        .to_string()
        .contains("brak certyfikatu dla bob@example.com"));
    });
}

/// Not a test: writes what Telex sends into fixtures/out/ for OpenSSL to check
/// (`cargo test -p telex-core write_mail_for_openssl -- --ignored`, then fixtures/README.md).
#[test]
#[ignore]
fn write_mail_for_openssl() {
    let mut a = alice();
    a.import_certificates(BOB_CERT, now()).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/smime/fixtures/out");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("telex-signed.eml"),
        composed(&a, &draft("bob@example.com", ""), SIGN),
    )
    .unwrap();
    std::fs::write(
        dir.join("telex-encrypted.eml"),
        composed(&a, &draft("bob@example.com", ""), BOTH),
    )
    .unwrap();
}
