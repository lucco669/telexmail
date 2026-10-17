use super::*;
use crate::model::{AccountSetup, Folder, FolderKind};
use crate::store::SyncedMessage;
use chrono::NaiveDate;

// Known vectors made with GnuPG 2.4.9 (fixtures/): Bob's key is GnuPG's, Alice's was made
// by Telex (`make_fixture_key`), passphrase "alice pass". Bob signs, and encrypts to Alice.
const ALICE_SEC: &str = include_str!("fixtures/alice.sec.asc");
const ALICE_PUB: &str = include_str!("fixtures/alice.pub.asc");
pub(super) const BOB_PUB: &str = include_str!("fixtures/bob.pub.asc");
const GNUPG_SIGNED: &[u8] = include_bytes!("fixtures/gnupg-signed.eml");
pub(super) const GNUPG_ENCRYPTED: &[u8] = include_bytes!("fixtures/gnupg-encrypted.eml");
const GNUPG_CLEARSIGNED: &str = include_str!("fixtures/gnupg-clearsigned.asc");
const GNUPG_INLINE: &str = include_str!("fixtures/gnupg-inline-encrypted.asc");
pub(super) const ALICE_FP: &str = "797602095C0061C9BE60579E731B4FA16994A65D";
pub(super) const BOB_FP: &str = "53DA690420621651DD27594E6B84029E1A9EE8CA";

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 17)
        .unwrap()
        .and_hms_opt(9, 0, 0)
        .unwrap()
}

/// Not a test: prints a fresh key for the fixtures (`cargo test -p telex-core make_fixture_key
/// -- --ignored --nocapture`).
#[test]
#[ignore]
fn make_fixture_key() {
    let key = generate(
        "Alice Telex",
        "alice@telex.test",
        "alice pass",
        KeyKind::Ed25519,
    )
    .unwrap();
    println!("{}", key.to_armored_string(None.into()).unwrap());
    println!(
        "{}",
        key.to_public_key().to_armored_string(None.into()).unwrap()
    );
}

/// Alice's mail base: one IMAP account with an Inbox and Spam.
pub(super) fn store() -> MailStore {
    let mut store = MailStore::in_memory().unwrap();
    let account = store
        .upsert_account(&AccountSetup {
            name: "Alice Telex".into(),
            email: "alice@telex.test".into(),
            host: "imap.telex.test".into(),
            port: 993,
            username: "alice@telex.test".into(),
            ..Default::default()
        })
        .unwrap();
    let folder = |id: &str, kind| Folder {
        id: id.into(),
        name: id.into(),
        kind,
        remote: Some(id.to_uppercase()),
    };
    store
        .set_folders(
            &account.id,
            &[
                folder("inbox", FolderKind::Inbox),
                folder("spam", FolderKind::Spam),
            ],
        )
        .unwrap();
    store
}

fn inbox(store: &MailStore) -> String {
    store.accounts()[0].folder_key("inbox")
}

fn add_to(store: &mut MailStore, folder: &str, raw: &[u8]) -> MessageId {
    let uid = store
        .db
        .query_row("SELECT COUNT(*) + 1 FROM messages", [], |r| r.get(0))
        .unwrap();
    let message = crate::mime::parse_message(raw, "", None);
    store
        .add_synced(
            folder,
            &[SyncedMessage {
                uid,
                message,
                raw: raw.to_vec(),
            }],
        )
        .unwrap()[0]
}

pub(super) fn add(store: &mut MailStore, raw: &[u8]) -> MessageId {
    let folder = inbox(store);
    add_to(store, &folder, raw)
}

pub(super) fn import(store: &mut MailStore, key: &str) {
    store
        .import_keys(key.as_bytes(), Source::File, now())
        .unwrap();
}

/// Alice with her own key, unlocked.
pub(super) fn alice() -> MailStore {
    let mut s = store();
    import(&mut s, ALICE_SEC);
    assert!(s.pgp_unlock(ALICE_FP, "alice pass").unwrap());
    s
}

pub(super) fn view(store: &MailStore, id: MessageId) -> (crate::risk::InspectedMessage, View) {
    let inspected = store.inspect_message(id).unwrap();
    let view = inspected.pgp.clone().expect("OpenPGP found");
    (inspected, view)
}

fn status(store: &MailStore, id: MessageId) -> SigStatus {
    view(store, id).1.signature.expect("a signature").status
}

// ---- Signed mail ----

#[test]
fn gnupg_signed_mail_is_good_once_bobs_key_is_here() {
    let mut s = store();
    let id = add(&mut s, GNUPG_SIGNED);
    let (shown, v) = view(&s, id);
    let sig = v.signature.unwrap();
    assert_eq!(sig.status, SigStatus::NoKey);
    assert_eq!(sig.key_id, "6B84029E1A9EE8CA");
    assert!(!v.encrypted && !v.partial);
    // The signature isn't an attachment; the text is the signed text.
    assert!(shown.message.attachments.is_empty());
    assert!(shown.message.body.contains("Zażółć gęślą jaźń."));

    import(&mut s, BOB_PUB);
    let sig = view(&s, id).1.signature.unwrap();
    assert_eq!(sig.status, SigStatus::Good);
    let key = sig.key.unwrap();
    assert_eq!(key.fingerprint, BOB_FP);
    assert_eq!(key.name, "Bob GnuPG <bob@gnupg.test>");
    assert!(sig.created.is_some());
}

#[test]
fn a_changed_signed_message_is_bad() {
    let mut s = store();
    import(&mut s, BOB_PUB);
    let raw = String::from_utf8(GNUPG_SIGNED.to_vec())
        .unwrap()
        .replace("Podpisane w GnuPG.", "Podpisane w GnuPG!");
    let id = add(&mut s, raw.as_bytes());
    assert_eq!(status(&s, id), SigStatus::Bad);
}

#[test]
fn a_signature_by_someone_else_says_so() {
    let mut s = store();
    import(&mut s, BOB_PUB);
    let raw = String::from_utf8(GNUPG_SIGNED.to_vec()).unwrap().replace(
        "From: Bob GnuPG <bob@gnupg.test>",
        "From: Bob <bob@bank.test>",
    );
    let id = add(&mut s, raw.as_bytes());
    assert_eq!(status(&s, id), SigStatus::OtherSender);
}

#[test]
fn signed_mail_stored_with_bare_line_feeds_still_verifies() {
    let mut s = store();
    import(&mut s, BOB_PUB);
    let raw = String::from_utf8(GNUPG_SIGNED.to_vec())
        .unwrap()
        .replace("\r\n", "\n");
    let id = add(&mut s, raw.as_bytes());
    assert_eq!(status(&s, id), SigStatus::Good);
}

#[test]
fn a_signed_part_inside_a_list_footer_is_partial() {
    let mut s = store();
    import(&mut s, BOB_PUB);
    let signed = String::from_utf8(GNUPG_SIGNED.to_vec()).unwrap();
    let (head, body) = signed.split_once("\r\n\r\n").unwrap();
    let content_type = head
        .lines()
        .find(|l| l.starts_with("Content-Type"))
        .unwrap();
    let raw = format!(
        "From: Bob GnuPG <bob@gnupg.test>\r\nSubject: [lista] Podpisane\r\nMIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"list\"\r\n\r\n--list\r\n{content_type}\r\n\r\n{body}\r\n\
         --list\r\nContent-Type: text/plain\r\n\r\nWypisz się: https://lista.test/\r\n--list--\r\n"
    );
    let id = add(&mut s, raw.as_bytes());
    let v = view(&s, id).1;
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert!(v.partial);
}

#[test]
fn mail_without_openpgp_has_none() {
    let mut s = alice();
    let id = add(
        &mut s,
        b"From: a@b.test\r\nSubject: Hej\r\n\r\nZwykly list, bez podpisu.\r\n",
    );
    assert_eq!(s.inspect_message(id).unwrap().pgp, None);
    let json = serde_json::to_value(s.inspect_message(id).unwrap()).unwrap();
    assert!(json.get("pgp").is_none());
}

// ---- Encrypted mail ----

#[test]
fn gnupg_mail_encrypted_to_alice_waits_for_her_passphrase() {
    let mut s = store();
    import(&mut s, ALICE_SEC);
    import(&mut s, BOB_PUB);
    let id = add(&mut s, GNUPG_ENCRYPTED);
    let v = view(&s, id).1;
    assert!(v.encrypted && !v.decrypted);
    assert_eq!(v.problem, Some(Problem::Locked));
    assert_eq!(v.locked[0].fingerprint, ALICE_FP);
    assert_eq!(view(&s, id).0.message.body, "");

    assert!(!s.pgp_unlock(ALICE_FP, "wrong").unwrap());
    assert!(s.pgp_unlock(ALICE_FP, "alice pass").unwrap());
    let (shown, v) = view(&s, id);
    assert!(v.decrypted);
    assert_eq!(v.problem, None);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    // The protected subject replaces "...", the text and the file come from inside.
    assert_eq!(shown.message.subject, "Tajny projekt");
    assert!(shown.message.body.contains("Hasło do sejfu: kalafior42."));
    assert_eq!(shown.message.attachments.len(), 1);
    assert_eq!(shown.message.attachments[0].name, "plan.pdf");
    assert!(s.attachment(id, 0).unwrap().data.starts_with(b"%PDF-1.4"));
}

#[test]
fn decrypted_text_is_never_stored_or_indexed() {
    let mut s = alice();
    let id = add(&mut s, GNUPG_ENCRYPTED);
    assert!(view(&s, id).0.message.body.contains("kalafior42"));
    // The mail base has the message as it came, encrypted; its text and index have nothing.
    assert_eq!(s.raw_of(id).unwrap().unwrap(), GNUPG_ENCRYPTED);
    assert_eq!(s.get(id).unwrap().body, "");
    assert!(s.get(id).unwrap().attachments.is_empty());
    assert!(s.search("kalafior42").unwrap().is_empty());
    let any_plain: bool =
        s.db.query_row(
            "SELECT EXISTS (SELECT 1 FROM message_content WHERE instr(body, 'kalafior') > 0
               OR instr(raw, CAST('kalafior' AS BLOB)) > 0)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!any_plain);
    // In memory, the readable message is the decrypted one: for HTML, attachments, replies.
    let readable = s.readable_raw(id).unwrap().unwrap();
    assert!(String::from_utf8_lossy(&readable).contains("kalafior42"));
    assert_eq!(s.get_readable(id).unwrap().subject, "Tajny projekt");
    // A key change forgets it.
    import(&mut s, BOB_PUB);
    assert_eq!(s.readable_raw(id).unwrap().unwrap(), GNUPG_ENCRYPTED);
}

#[test]
fn only_the_last_opened_messages_stay_decrypted() {
    let mut s = alice();
    let ids: Vec<MessageId> = (0..KEEP_OPENED + 2)
        .map(|_| add(&mut s, GNUPG_ENCRYPTED))
        .collect();
    for &id in &ids {
        s.inspect_message(id).unwrap();
    }
    assert_eq!(s.pgp.borrow().opened.len(), KEEP_OPENED);
    assert_eq!(s.readable_raw(ids[0]).unwrap().unwrap(), GNUPG_ENCRYPTED);
    assert_ne!(
        s.readable_raw(ids[KEEP_OPENED + 1]).unwrap().unwrap(),
        GNUPG_ENCRYPTED
    );
}

#[test]
fn mail_for_someone_else_names_its_recipients() {
    let mut s = alice();
    import(&mut s, BOB_PUB);
    let bob = s.public_key(BOB_FP).unwrap();
    let armored = encrypt(
        b"Content-Type: text/plain\r\n\r\nTylko dla Boba\r\n",
        &[bob],
        None,
    )
    .unwrap();
    let id = add(&mut s, &pgp_mime_encrypted("carol@telex.test", &armored));
    let v = view(&s, id).1;
    assert_eq!(v.problem, Some(Problem::NotForYou));
    assert_eq!(v.recipients.len(), 1);
    assert_ne!(
        v.recipients[0],
        key_id_of(&s.public_key(ALICE_FP).unwrap().primary_key)
    );
}

#[test]
fn damaged_ciphertext_fails_and_shows_nothing() {
    let mut s = alice();
    let text = String::from_utf8(GNUPG_ENCRYPTED.to_vec()).unwrap();
    // Change one letter of the armored payload, well inside it.
    let start = text.find("-----BEGIN PGP MESSAGE-----").unwrap() + 120;
    let mut bytes = text.into_bytes();
    bytes[start] = if bytes[start] == b'A' { b'B' } else { b'A' };
    let id = add(&mut s, &bytes);
    let (shown, v) = view(&s, id);
    assert!(!v.decrypted);
    assert_eq!(v.problem, Some(Problem::Failed));
    assert_eq!(shown.message.body, "");
}

#[test]
fn a_compression_bomb_stops_at_the_limit() {
    use pgp::composed::MessageBuilder;
    use pgp::crypto::sym::SymmetricKeyAlgorithm;
    use pgp::types::CompressionAlgorithm;
    let mut s = alice();
    let alice_key = s.public_key(ALICE_FP).unwrap();
    let mut rng = rand::thread_rng();
    let zeros = vec![b'0'; MAX_PLAIN * 3];
    let mut builder =
        MessageBuilder::from_bytes("", zeros).seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES256);
    builder.compression(CompressionAlgorithm::ZLIB);
    match encryption_key(&alice_key).unwrap() {
        EncryptTo::Sub(k) => builder.encrypt_to_key(&mut rng, k).unwrap(),
        EncryptTo::Primary(k) => builder.encrypt_to_key(&mut rng, k).unwrap(),
    };
    let armored = builder.to_armored_string(&mut rng, None.into()).unwrap();
    assert!(armored.len() < MAX_PLAIN / 10, "compressed small");
    let id = add(&mut s, &pgp_mime_encrypted("bob@gnupg.test", &armored));
    let v = view(&s, id).1;
    assert_eq!(v.problem, Some(Problem::TooLarge));
}

#[test]
fn encrypted_mail_lists_no_attachments_and_no_text_when_stored() {
    let m = crate::mime::parse_message(GNUPG_ENCRYPTED, "x/inbox", None);
    assert!(m.attachments.is_empty());
    assert_eq!(m.body.trim(), "");
    let m = crate::mime::parse_message(GNUPG_SIGNED, "x/inbox", None);
    assert!(m.attachments.is_empty());
    assert!(crate::mime::attachments_of(GNUPG_SIGNED).is_empty());
}

// ---- Inline PGP ----

fn plain_mail(from: &str, text: &str) -> Vec<u8> {
    format!(
        "From: {from}\r\nTo: alice@telex.test\r\nSubject: Inline\r\n\
         Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{text}"
    )
    .into_bytes()
}

#[test]
fn gnupg_clearsigned_text_shows_its_text_and_signature() {
    let mut s = store();
    import(&mut s, BOB_PUB);
    let id = add(
        &mut s,
        &plain_mail("Bob GnuPG <bob@gnupg.test>", GNUPG_CLEARSIGNED),
    );
    let (shown, v) = view(&s, id);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert!(!v.partial);
    assert_eq!(
        shown.message.body,
        "Cześć Alice,\n- to jest podpisane.\nBob\n"
    );
    // Stored and searched without the armor.
    let stored = s.get(id).unwrap().body;
    assert!(!stored.contains("BEGIN PGP") && stored.contains("to jest podpisane"));
}

#[test]
fn text_around_a_clearsigned_block_is_partial() {
    let mut s = store();
    import(&mut s, BOB_PUB);
    let text = format!("Dopisek bez podpisu.\n\n{GNUPG_CLEARSIGNED}");
    let id = add(&mut s, &plain_mail("bob@gnupg.test", &text));
    let v = view(&s, id).1;
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert!(v.partial);
}

#[test]
fn gnupg_inline_encrypted_text_is_decrypted() {
    let mut s = alice();
    import(&mut s, BOB_PUB);
    let id = add(
        &mut s,
        &plain_mail("Bob GnuPG <bob@gnupg.test>", GNUPG_INLINE),
    );
    assert_eq!(s.get(id).unwrap().body.trim(), "");
    let (shown, v) = view(&s, id);
    assert!(v.decrypted);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert_eq!(shown.message.body.trim(), "Spotkanie w czwartek o 9.");
    assert!(s.search("czwartek").unwrap().is_empty());
}

// ---- Telex's own signatures and encryption, read back ----

/// A PGP/MIME encrypted message (RFC 3156 §4) around an armored payload.
fn pgp_mime_encrypted(from: &str, armored: &str) -> Vec<u8> {
    format!(
        "From: {from}\r\nTo: alice@telex.test\r\nSubject: ...\r\nMIME-Version: 1.0\r\n\
         Content-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=\"e\"\r\n\r\n\
         --e\r\nContent-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n\r\n\
         --e\r\nContent-Type: application/octet-stream; name=\"encrypted.asc\"\r\n\r\n{armored}\r\n--e--\r\n"
    )
    .into_bytes()
}

#[test]
fn telex_signs_and_encrypts_what_it_reads_back() {
    let mut s = alice();
    import(&mut s, BOB_PUB);
    let own = s.own_key_for("Alice@Telex.test").unwrap();
    assert_eq!(own.fingerprint, ALICE_FP);
    // Detached, over a part.
    let part = b"Content-Type: text/plain; charset=utf-8\r\n\r\nPodpisane w Telexie.\r\n";
    let sig = sign_detached(&own, part).unwrap();
    let raw = format!(
        "From: Alice Telex <alice@telex.test>\r\nSubject: Sig\r\nMIME-Version: 1.0\r\n\
         Content-Type: multipart/signed; micalg=pgp-sha256; protocol=\"application/pgp-signature\"; boundary=\"s\"\r\n\r\n\
         --s\r\n{}\r\n--s\r\nContent-Type: application/pgp-signature\r\n\r\n{sig}\r\n--s--\r\n",
        std::str::from_utf8(part).unwrap()
    );
    let id = add(&mut s, raw.as_bytes());
    assert_eq!(status(&s, id), SigStatus::Good);
    // Signed and encrypted to Bob and herself: she reads her own copy.
    let to = s
        .keys_for(&["bob@gnupg.test".into(), "alice@telex.test".into()])
        .unwrap();
    let armored = encrypt(part, &to, Some(&own)).unwrap();
    let id = add(
        &mut s,
        &pgp_mime_encrypted("Alice Telex <alice@telex.test>", &armored),
    );
    let (shown, v) = view(&s, id);
    assert!(v.decrypted);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert!(shown.message.body.contains("Podpisane w Telexie."));
}

#[test]
fn encrypting_needs_a_key_for_everyone() {
    let mut s = alice();
    let missing = s
        .keys_for(&["bob@gnupg.test".into(), "alice@telex.test".into()])
        .unwrap_err();
    assert_eq!(
        missing,
        WriteProblem::NoKeyFor {
            emails: vec!["bob@gnupg.test".into()]
        }
    );
    assert!(
        crate::i18n::with(crate::i18n::Lang::Pl, || missing.to_string())
            .contains("brak klucza dla bob@gnupg.test")
    );
    assert_eq!(
        s.own_key_for("bob@gnupg.test").err(),
        Some(WriteProblem::NoOwnKey {
            email: "bob@gnupg.test".into()
        })
    );
    s.pgp_lock(ALICE_FP);
    assert!(matches!(
        s.own_key_for("alice@telex.test"),
        Err(WriteProblem::Locked { .. })
    ));
}

// ---- Keys ----

#[test]
fn keys_import_once_and_list_with_their_facts() {
    let mut s = store();
    let both = format!("{ALICE_PUB}\n{BOB_PUB}");
    let report = s.import_keys(both.as_bytes(), Source::File, now()).unwrap();
    assert_eq!(report.added.len(), 2);
    let again = s.import_keys(both.as_bytes(), Source::File, now()).unwrap();
    assert_eq!((again.added.len(), again.unchanged.len()), (0, 2));
    let keys = s.pgp_keys().unwrap();
    let bob = keys.iter().find(|k| k.fingerprint == BOB_FP).unwrap();
    assert_eq!(bob.emails, vec!["bob@gnupg.test".to_owned()]);
    assert_eq!(bob.algorithm, "Ed25519");
    assert!(bob.can_encrypt && bob.verified && !bob.secret && !bob.revoked);
    assert_eq!(bob.source, "file");
    // Its secret part comes later: the same key, now own.
    let report = s
        .import_keys(ALICE_SEC.as_bytes(), Source::File, now())
        .unwrap();
    assert_eq!(report.updated.len(), 1);
    let alice = s.pgp_keys().unwrap().into_iter().next().unwrap();
    assert_eq!(alice.fingerprint, ALICE_FP);
    assert!(alice.secret && alice.protected);
    // Exported and read back: the same certificate.
    let exported = s.export_public_key(BOB_FP).unwrap();
    assert!(exported.starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----"));
    assert_eq!(
        s.import_keys(exported.as_bytes(), Source::File, now())
            .unwrap()
            .unchanged
            .len(),
        1
    );
    let secret = s.export_secret_key(ALICE_FP).unwrap();
    assert!(secret
        .lines()
        .next()
        .is_some_and(|l| l.contains("PGP PRIVATE KEY BLOCK")));
    // Removed with its ids.
    s.delete_key(BOB_FP).unwrap();
    let ids: i64 =
        s.db.query_row(
            "SELECT COUNT(*) FROM pgp_key_ids WHERE fingerprint = ?1",
            [BOB_FP],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ids, 0);
    assert_eq!(s.pgp_keys().unwrap().len(), 1);
}

#[test]
fn what_isnt_a_key_is_refused() {
    let mut s = store();
    let e = s
        .import_keys(
            b"-----BEGIN PGP PUBLIC KEY BLOCK-----\n\nnope\n-----END PGP PUBLIC KEY BLOCK-----\n",
            Source::File,
            now(),
        )
        .unwrap_err();
    assert!(matches!(e, StoreError::Invalid(_)));
    assert!(s.import_keys(&[0xff; 4000], Source::File, now()).is_err());
    assert!(s.pgp_keys().unwrap().is_empty());
}

#[test]
fn a_generated_key_signs_encrypts_and_is_protected() {
    let mut s = store();
    let made = s
        .generate_key(
            "Alicja Nowak",
            "alicja@telex.test",
            "długie hasło",
            KeyKind::Ed25519,
            now(),
        )
        .unwrap();
    assert_eq!(made.name, "Alicja Nowak <alicja@telex.test>");
    let info = s.pgp_keys().unwrap().into_iter().next().unwrap();
    assert!(info.secret && info.protected && info.can_encrypt && info.verified);
    assert_eq!(info.source, "own");
    // Known for this session; not after a lock, until the passphrase is given again.
    assert!(s.own_key_for("alicja@telex.test").is_ok());
    s.pgp_lock(&made.fingerprint);
    assert!(s.own_key_for("alicja@telex.test").is_err());
    assert!(!s.pgp_unlock(&made.fingerprint, "inne").unwrap());
    assert!(s.pgp_unlock(&made.fingerprint, "długie hasło").unwrap());
    // Its exported secret part stays protected by the passphrase.
    let exported = s.export_secret_key(&made.fingerprint).unwrap();
    let (key, _) = SignedSecretKey::from_string(&exported).unwrap();
    assert!(is_protected(&key));
    assert!(generate("A", "not an address", "x", KeyKind::Ed25519).is_err());
    assert!(generate("A", "a@b.test", "", KeyKind::Ed25519).is_err());
}

// ---- Autocrypt ----

fn bob_keydata() -> String {
    let (key, _) = SignedPublicKey::from_string(BOB_PUB).unwrap();
    base64::engine::general_purpose::STANDARD.encode(key.to_bytes().unwrap())
}

fn with_autocrypt(header: &str) -> Vec<u8> {
    format!(
        "From: Bob GnuPG <bob@gnupg.test>\r\nTo: alice@telex.test\r\nSubject: Hej\r\n\
         Date: Fri, 16 Oct 2026 12:00:00 +0200\r\nAutocrypt: {header}\r\n\r\nCześć!\r\n"
    )
    .into_bytes()
}

#[test]
fn autocrypt_headers_are_read_as_level_1_says() {
    let h =
        parse_autocrypt("addr=Bob@GnuPG.test; prefer-encrypt=mutual; keydata=AAEC AwQ=").unwrap();
    assert_eq!(h.addr, "bob@gnupg.test");
    assert!(h.prefer_encrypt);
    assert_eq!(h.keydata, vec![0, 1, 2, 3, 4]);
    assert!(parse_autocrypt("addr=a@b.test; _note=x; keydata=AAEC").is_some());
    // Unknown attributes, a missing part, bad base64: not a header.
    assert!(parse_autocrypt("addr=a@b.test; color=red; keydata=AAEC").is_none());
    assert!(parse_autocrypt("addr=a@b.test").is_none());
    assert!(parse_autocrypt("addr=a@b.test; keydata=!!").is_none());
}

#[test]
fn autocrypt_keys_are_collected_from_incoming_mail_unconfirmed() {
    let mut s = store();
    add(
        &mut s,
        &with_autocrypt(&format!(
            "addr=bob@gnupg.test; prefer-encrypt=mutual; keydata={}",
            bob_keydata()
        )),
    );
    let keys = s.pgp_keys().unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(
        (keys[0].source.as_str(), keys[0].verified),
        ("autocrypt", false)
    );
    let peer: (String, bool) = s
        .db
        .query_row("SELECT fingerprint, prefer_encrypt FROM autocrypt_peers WHERE email = 'bob@gnupg.test'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(peer, (BOB_FP.to_owned(), true));
    assert!(s.key_for("bob@gnupg.test").is_some());
    // His signature reads as valid from an unconfirmed key, until Alice confirms it.
    let id = add(&mut s, GNUPG_SIGNED);
    assert_eq!(status(&s, id), SigStatus::Unconfirmed);
    s.set_key_verified(BOB_FP, true).unwrap();
    assert_eq!(status(&s, id), SigStatus::Good);
}

#[test]
fn autocrypt_ignores_other_addresses_spam_and_two_headers() {
    let mut s = store();
    let keydata = bob_keydata();
    add(
        &mut s,
        &with_autocrypt(&format!("addr=mallory@x.test; keydata={keydata}")),
    );
    let two = String::from_utf8(with_autocrypt(&format!(
        "addr=bob@gnupg.test; keydata={keydata}"
    )))
    .unwrap()
    .replace(
        "Autocrypt:",
        &format!("Autocrypt: addr=bob@gnupg.test; keydata={keydata}\r\nAutocrypt:"),
    );
    add(&mut s, two.as_bytes());
    let spam = s.accounts()[0].folder_key("spam");
    add_to(
        &mut s,
        &spam,
        &with_autocrypt(&format!("addr=bob@gnupg.test; keydata={keydata}")),
    );
    assert!(s.pgp_keys().unwrap().is_empty());
}

#[test]
fn a_key_attached_to_mail_is_offered_and_imported_on_request() {
    let mut s = store();
    // The signed part and Bob's key next to it, in a multipart/mixed.
    let signed = String::from_utf8(GNUPG_SIGNED.to_vec()).unwrap();
    let raw = format!(
        "From: Bob GnuPG <bob@gnupg.test>\r\nSubject: Klucz\r\nMIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"m\"\r\n\r\n--m\r\n{}\r\n--m\r\n\
         Content-Type: application/pgp-keys; name=\"bob.asc\"\r\n\r\n{}\r\n--m--\r\n",
        signed.split_once("MIME-Version: 1.0\r\n").unwrap().1,
        BOB_PUB.replace('\n', "\r\n")
    );
    let id = add(&mut s, raw.as_bytes());
    let v = view(&s, id).1;
    assert_eq!(v.offered.len(), 1);
    assert_eq!(v.offered[0].fingerprint, BOB_FP);
    assert_eq!(v.signature.unwrap().status, SigStatus::Unconfirmed);
    assert!(
        s.pgp_keys().unwrap().is_empty(),
        "nothing stored by looking"
    );
    let report = s.import_offered_keys(id, now()).unwrap();
    assert_eq!(report.added.len(), 1);
    let keys = s.pgp_keys().unwrap();
    assert_eq!(
        (keys[0].source.as_str(), keys[0].verified),
        ("message", false)
    );
    assert!(view(&s, id).1.offered.is_empty());
}

// ---- Pieces ----

#[test]
fn the_signed_part_is_cut_as_rfc_3156_says() {
    let body =
        b"preamble\r\n--b\r\nContent-Type: text/plain\r\n\r\nline\r\n\r\n--b\r\nsig\r\n--b--\r\n";
    assert_eq!(
        first_part(body, b"b").unwrap(),
        b"Content-Type: text/plain\r\n\r\nline\r\n"
    );
    // "--bb" isn't the delimiter "--b" at a line's start… but it starts with it; boundaries
    // are chosen not to be prefixes of the content, as RFC 2046 asks.
    assert_eq!(canonical(b"a\nb\r\nc\n"), b"a\r\nb\r\nc\r\n");
}

#[test]
fn the_decrypted_part_keeps_the_outer_sender() {
    let outer =
        b"From: Bob <bob@gnupg.test>\r\nSubject: ...\r\nContent-Type: multipart/encrypted\r\n\r\nx";
    let inner =
        b"Content-Type: text/plain\r\nSubject: Tajne\r\nFrom: Mallory <m@x.test>\r\n\r\nTekst";
    let made = String::from_utf8(with_outer_headers(outer, inner)).unwrap();
    let m = crate::mime::parse_message(made.as_bytes(), "x/inbox", None);
    assert_eq!(m.from_email, "bob@gnupg.test");
    assert_eq!(m.subject, "Tajne");
    assert_eq!(m.body.trim(), "Tekst");
    assert!(!made.contains("multipart/encrypted"));
}

#[test]
fn index_text_drops_encrypted_blocks_and_keeps_signed_text() {
    assert_eq!(index_text("Hej\n"), "Hej\n");
    let text = format!("Przed\n{GNUPG_INLINE}Po\n");
    assert_eq!(index_text(&text), "Przed\nPo\n");
    assert_eq!(
        index_text(GNUPG_CLEARSIGNED),
        "Cześć Alice,\n- to jest podpisane.\nBob\n"
    );
}

#[test]
fn user_ids_give_their_addresses() {
    assert_eq!(
        email_of_user_id("Anna <Anna@Studio.pl>").as_deref(),
        Some("anna@studio.pl")
    );
    assert_eq!(
        email_of_user_id("anna@studio.pl").as_deref(),
        Some("anna@studio.pl")
    );
    assert_eq!(email_of_user_id("Anna Kowalska"), None);
}

#[test]
fn a_key_that_comes_by_itself_never_replaces_one_telex_has() {
    let mut s = store();
    import(&mut s, BOB_PUB);
    // Bob's certificate without its encryption subkey, as anyone could send it.
    let (mut stripped, _) = SignedPublicKey::from_string(BOB_PUB).unwrap();
    stripped.public_subkeys.clear();
    let mut db_tx = s.db.transaction().unwrap();
    let (_, saved) = save_key(
        &db_tx,
        &PublicOrSecret::Public(stripped.clone()),
        Source::Autocrypt,
        now(),
    )
    .unwrap();
    assert_eq!(saved, Saved::Unchanged);
    db_tx.commit().unwrap();
    assert!(
        s.key_for("bob@gnupg.test").is_some(),
        "still encrypts to Bob"
    );
    // The user's own import of a newer copy does replace it.
    db_tx = s.db.transaction().unwrap();
    let (_, saved) = save_key(
        &db_tx,
        &PublicOrSecret::Public(stripped),
        Source::File,
        now(),
    )
    .unwrap();
    assert_eq!(saved, Saved::Updated);
    db_tx.commit().unwrap();
}
