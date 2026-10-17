//! Compose's Sign and Encrypt: what Telex sends, read back by Telex (and by GnuPG, with
//! `write_mail_for_gnupg`).

use super::tests::{add, alice, import, store, view, ALICE_FP, BOB_FP, BOB_PUB, GNUPG_ENCRYPTED};
use super::*;

fn draft_to(to: &str, bcc: &str) -> crate::model::OutgoingDraft {
    crate::model::OutgoingDraft {
        account: "x".into(),
        to: to.into(),
        bcc: bcc.into(),
        subject: "Umowa na październik".into(),
        body: "Dzień dobry,\nw załączniku poprawki.\n\n-- \nAlice\n".into(),
        ..Default::default()
    }
}

fn composed(s: &MailStore, draft: &crate::model::OutgoingDraft, choice: Choice) -> Vec<u8> {
    let account = s.accounts()[0].clone();
    let to = crate::smtp::recipient_emails(&draft.to).unwrap();
    let bcc = crate::smtp::recipient_emails(&draft.bcc).unwrap_or_default();
    let p = s
        .protection("alice@telex.test", &to, &bcc, choice, true, false)
        .unwrap();
    crate::smtp::compose_protected(
        &account,
        draft,
        &Default::default(),
        chrono::Local::now(),
        &p,
    )
    .unwrap()
    .raw
}

fn recipients_in(raw: &[u8]) -> Vec<String> {
    let payload = match find(raw).unwrap() {
        Found::MimeEncrypted { payload, .. } => payload,
        _ => panic!("not encrypted"),
    };
    let (msg, _) = PgpMessage::from_reader(BufReader::new(Cursor::new(&payload[..]))).unwrap();
    recipients_of(&msg)
}

#[test]
fn a_signed_message_verifies_and_survives_servers_that_trim_lines() {
    let mut s = alice();
    let sign = Choice {
        sign: true,
        ..Choice::default()
    };
    let raw = composed(&s, &draft_to("bob@gnupg.test", ""), sign);
    let text = String::from_utf8(raw.clone()).unwrap();
    assert!(text.contains("multipart/signed"));
    assert!(text.contains("Autocrypt: addr=alice@telex.test; keydata="));
    // Quoted-printable: no line ends in a space for a server to trim ("-- " is "--=20").
    assert!(text
        .split("\r\n")
        .all(|l| !l.ends_with(' ') && !l.ends_with('\t')));
    let id = add(&mut s, &raw);
    let (shown, v) = view(&s, id);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert!(shown.message.body.contains("w załączniku poprawki"));
    assert!(shown.message.attachments.is_empty());
}

#[test]
fn an_encrypted_message_hides_its_text_and_subject_and_reads_back() {
    let mut s = alice();
    import(&mut s, BOB_PUB);
    let both = Choice {
        sign: true,
        encrypt: true,
        attach_key: true,
        ..Choice::default()
    };
    let raw = composed(&s, &draft_to("Bob <bob@gnupg.test>", ""), both);
    let text = String::from_utf8(raw.clone()).unwrap();
    assert!(text.contains("multipart/encrypted"));
    assert!(text.contains("\r\nSubject: ...\r\n"));
    assert!(!text.contains("październik") && !text.contains("poprawki"));
    assert!(
        text.contains("Autocrypt: "),
        "the Autocrypt header stays outside"
    );
    // Alice reads her own copy: subject, text, signature, her key attached.
    let id = add(&mut s, &raw);
    let (shown, v) = view(&s, id);
    assert!(v.decrypted);
    assert_eq!(v.signature.unwrap().status, SigStatus::Good);
    assert_eq!(shown.message.subject, "Umowa na październik");
    assert!(shown.message.body.contains("w załączniku poprawki"));
    assert_eq!(shown.message.attachments.len(), 1);
    assert!(shown.message.attachments[0].name.starts_with("OpenPGP_0x"));
    // Encrypted to Bob too: his encryption subkey is among the recipients.
    let bob = s.public_key(BOB_FP).unwrap();
    assert!(recipients_in(&raw).contains(&key_id_of(&bob.public_subkeys[0].key)));
}

#[test]
fn bcc_recipients_are_encrypted_to_without_being_named() {
    let mut s = alice();
    import(&mut s, BOB_PUB);
    let encrypt = Choice {
        encrypt: true,
        ..Choice::default()
    };
    let raw = composed(&s, &draft_to("alice@telex.test", "bob@gnupg.test"), encrypt);
    let recipients = recipients_in(&raw);
    let bob = s.public_key(BOB_FP).unwrap();
    assert!(!recipients.contains(&key_id_of(&bob.public_subkeys[0].key)));
    assert!(
        recipients.iter().any(|r| r.chars().all(|c| c == '0')),
        "{recipients:?}"
    );
    // Alice still reads it (her key is named; the anonymous one is tried too).
    let id = add(&mut s, &raw);
    assert!(view(&s, id).1.decrypted);
}

#[test]
fn encrypting_refuses_with_the_addresses_that_have_no_key() {
    let s = alice();
    let encrypt = Choice {
        encrypt: true,
        ..Choice::default()
    };
    let e = s
        .protection(
            "alice@telex.test",
            &["bob@gnupg.test".into(), "carol@telex.test".into()],
            &[],
            encrypt,
            true,
            false,
        )
        .err()
        .unwrap();
    assert_eq!(
        e,
        WriteProblem::NoKeyFor {
            emails: vec!["bob@gnupg.test".into(), "carol@telex.test".into()]
        }
    );
    assert!(crate::i18n::with(crate::i18n::Lang::En, || e.to_string())
        .contains("no key for bob@gnupg.test, carol@telex.test"));
    // No own key: nothing to sign with or to encrypt one's copy to.
    let bare = store();
    let sign = Choice {
        sign: true,
        ..Choice::default()
    };
    assert!(matches!(
        bare.protection("alice@telex.test", &[], &[], sign, true, false),
        Err(WriteProblem::NoOwnKey { .. })
    ));
}

#[test]
fn a_draft_is_encrypted_to_its_writer_only() {
    let mut s = alice();
    import(&mut s, BOB_PUB);
    let all = Choice {
        sign: true,
        encrypt: true,
        attach_key: true,
        ..Choice::default()
    };
    let p = s
        .protection(
            "alice@telex.test",
            &["bob@gnupg.test".into()],
            &[],
            all,
            true,
            true,
        )
        .unwrap();
    assert!(p.sign.is_none() && p.autocrypt.is_none() && p.key_file.is_none());
    assert_eq!(p.encrypt.len(), 1);
    let account = s.accounts()[0].clone();
    let raw = crate::smtp::compose_draft_protected(
        &account,
        &draft_to("bob@gnupg.test", ""),
        &Default::default(),
        chrono::Local::now(),
        &p,
    )
    .unwrap();
    assert!(!String::from_utf8_lossy(&raw).contains("poprawki"));
    assert_eq!(recipients_in(&raw).len(), 1);
    let id = add(&mut s, &raw);
    assert!(view(&s, id).0.message.body.contains("poprawki"));
}

#[test]
fn compose_starts_encrypted_for_replies_to_encrypted_mail_and_as_last_chosen() {
    let mut s = alice();
    import(&mut s, BOB_PUB);
    let bob = vec!["bob@gnupg.test".to_owned()];
    let state = s.compose_state("alice@telex.test", &bob, None, false);
    assert_eq!(state.own.as_ref().unwrap().fingerprint, ALICE_FP);
    assert!(state.unlocked && state.can_encrypt && state.missing.is_empty());
    assert!(!state.encrypt && !state.sign);
    let encrypted = add(&mut s, GNUPG_ENCRYPTED);
    let reply = s.compose_state("alice@telex.test", &bob, Some(encrypted), false);
    assert!(reply.encrypt && reply.sign);
    let both = Choice {
        sign: true,
        encrypt: true,
        attach_key: false,
        ..Choice::default()
    };
    s.remember_choice(&bob, both).unwrap();
    let next = s.compose_state("alice@telex.test", &bob, None, false);
    assert!(next.encrypt && next.sign);
    let carol = s.compose_state("alice@telex.test", &["carol@telex.test".into()], None, true);
    assert_eq!(carol.missing, vec!["carol@telex.test".to_owned()]);
    assert!(!carol.can_encrypt && !carol.encrypt && carol.sign);
    s.pgp_lock(ALICE_FP);
    assert!(
        !s.compose_state("alice@telex.test", &bob, None, false)
            .unlocked
    );
}

#[test]
fn autocrypt_carries_a_minimal_key_other_readers_collect() {
    let s = alice();
    let (_, key) = own_public(&s.db, "alice@telex.test").unwrap();
    let value = autocrypt_value("Alice@Telex.test", &key).unwrap();
    let header = parse_autocrypt(&value.replace("\r\n", "")).unwrap();
    assert_eq!(header.addr, "alice@telex.test");
    let small = SignedPublicKey::from_bytes(Cursor::new(&header.keydata[..])).unwrap();
    assert_eq!(fingerprint_of(&small.primary_key), ALICE_FP);
    assert!(encryption_key(&small).is_some());
    assert!(value.split("\r\n").all(|l| l.len() <= 78));
}

/// Not a test: writes what Telex sends to Bob, for checking with GnuPG (`gpg --decrypt`,
/// `gpg --verify`); TELEX_PGP_OUT is the folder.
#[test]
#[ignore]
fn write_mail_for_gnupg() {
    let mut s = alice();
    import(&mut s, BOB_PUB);
    let dir = std::env::var("TELEX_PGP_OUT").unwrap_or_else(|_| ".".into());
    let sign = Choice {
        sign: true,
        ..Choice::default()
    };
    let signed = composed(&s, &draft_to("bob@gnupg.test", ""), sign);
    std::fs::write(format!("{dir}/telex-signed.eml"), signed).unwrap();
    let both = Choice {
        sign: true,
        encrypt: true,
        attach_key: false,
        ..Choice::default()
    };
    let encrypted = composed(&s, &draft_to("bob@gnupg.test", ""), both);
    std::fs::write(format!("{dir}/telex-encrypted.eml"), encrypted).unwrap();
}

#[test]
fn an_encrypted_draft_keeps_no_text_in_the_mail_base() {
    use crate::model::{Folder, FolderKind};
    let mut s = alice();
    let account = s.accounts()[0].clone();
    let folder = |id: &str, kind| Folder {
        id: id.into(),
        name: id.into(),
        kind,
        remote: Some(id.to_uppercase()),
    };
    s.set_folders(
        &account.id,
        &[
            folder("inbox", FolderKind::Inbox),
            folder("drafts", FolderKind::Drafts),
        ],
    )
    .unwrap();
    let account = s.accounts()[0].clone();
    let encrypt = Choice {
        encrypt: true,
        ..Choice::default()
    };
    let p = s
        .protection("alice@telex.test", &[], &[], encrypt, false, true)
        .unwrap();
    let mut draft = draft_to("bob@gnupg.test", "");
    draft.account = account.id.clone();
    let raw = crate::smtp::compose_draft_protected(
        &account,
        &draft,
        &Default::default(),
        chrono::Local::now(),
        &p,
    )
    .unwrap();
    let id = s
        .save_outgoing(
            draft,
            Some(&raw),
            FolderKind::Drafts,
            chrono::Local::now().naive_local(),
        )
        .unwrap();
    assert_eq!(s.get(id).unwrap().body, "");
    assert!(s.search("poprawki").unwrap().is_empty());
    // Opened again (E), it reads as written.
    assert!(view(&s, id).0.message.body.contains("poprawki"));
}
