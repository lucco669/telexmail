use super::*;
use crate::sample::sample_store;
use chrono::NaiveDate;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 10)
        .unwrap()
        .and_hms_opt(7, 0, 0)
        .unwrap()
}

#[test]
fn opened_message_risk_does_not_change_mail_or_flags() {
    let (mut store, account) = store_with_server_account();
    let raw = b"From: Sporify Billing <billing@music-account.example>\r\nSubject: Update payment details\r\nContent-Type: text/html\r\n\r\n<a href='https://spotify.com@billing.example/pay'>Update payment details</a>";
    let message = crate::mime::parse_message(raw, "", None);
    let id = store
        .add_synced(
            &account.folder_key("inbox"),
            &[SyncedMessage {
                uid: 9876,
                message,
                raw: raw.to_vec(),
            }],
        )
        .unwrap()[0];
    let before = store.get(id).unwrap();
    let report = store.inspect_message(id).unwrap();
    assert!(report.risk.score >= 70);
    assert_eq!(report.message, before);
    assert_eq!(store.get(id).unwrap(), before);
    assert_eq!(store.raw_of(id).unwrap().as_deref(), Some(raw.as_slice()));
    let json = serde_json::to_value(report).unwrap();
    assert_eq!(json["id"], id);
    assert!(json["risk"]["score"].as_u64().unwrap() >= 70);
}

fn draft(account: &str) -> OutgoingDraft {
    OutgoingDraft {
        account: account.into(),
        to: "Anna Kowalska <anna@kowalska-architekci.pl>".into(),
        cc: String::new(),
        subject: "Re: Poprawki".into(),
        body: "Dzień dobry,\n\nwdrożę do środy.\n".into(),
        ..Default::default()
    }
}

/// A store with one real (IMAP) account that has an Inbox, Sent, Archive and Trash.
fn store_with_server_account() -> (MailStore, Account) {
    let mut store = MailStore::in_memory().unwrap();
    let account = store
        .upsert_account(&AccountSetup {
            name: "Studio".into(),
            email: "biuro@twojastudio.pl".into(),
            host: "mail.twojastudio.pl".into(),
            port: 993,
            username: "biuro@twojastudio.pl".into(),
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
                folder("sent", FolderKind::Sent),
                folder("archive", FolderKind::Archive),
                folder("trash", FolderKind::Trash),
            ],
        )
        .unwrap();
    let account = store.account(&account.id).unwrap().clone();
    (store, account)
}

fn synced(uid: u32, subject: &str) -> SyncedMessage {
    SyncedMessage {
        uid,
        message: Message {
            id: 0,
            folder: String::new(),
            from_name: "Anna Kowalska".into(),
            from_email: "anna@kowalska-architekci.pl".into(),
            to: "biuro@twojastudio.pl".into(),
            cc: String::new(),
            subject: subject.into(),
            date: now(),
            size_kb: 1,
            unread: true,
            flagged: false,
            tags: Vec::new(),
            remote_images: false,
            note: None,
            attachments: vec![],
            body: "Dzień dobry, przesyłam żółwia.".into(),
            message_id: Some(format!("{uid}.{subject}@kowalska-architekci.pl")),
            bcc: String::new(),
            refs: Vec::new(),
        },
        raw: format!(
            "Message-ID: <{uid}.{subject}@kowalska-architekci.pl>\r\nSubject: {subject}\r\n\r\nx"
        )
        .into_bytes(),
    }
}

#[test]
fn a_folder_gone_from_the_server_keeps_what_only_this_pc_has() {
    let (mut store, account) = store_with_server_account();
    let sent = account.folder_key("sent");
    let server_copy = store.add_synced(&sent, &[synced(7, "Oferta")]).unwrap()[0];
    // A copy of sent mail that hasn't reached the server yet.
    let only_here = store
        .save_outgoing(draft(&account.id), None, FolderKind::Sent, now())
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
                folder("trash", FolderKind::Trash),
            ],
        )
        .unwrap();
    assert!(store.get(server_copy).is_err(), "the server's copy went");
    let kept = store.get(only_here).unwrap();
    assert_eq!(kept.folder, "local/archive");
    assert!(kept.note.unwrap().contains("sent"), "and says why");
}

#[test]
fn a_draft_replaced_only_goes_with_the_new_one_saved() {
    let (mut store, account) = store_with_server_account();
    let sent_first = store
        .save_outgoing(draft(&account.id), None, FolderKind::Sent, now())
        .unwrap();
    let mut bad = draft("nobody");
    bad.replaces = Some(sent_first);
    assert!(store
        .save_outgoing(bad, None, FolderKind::Sent, now())
        .is_err());
    assert!(
        store.get(sent_first).is_ok(),
        "nothing deleted when saving fails"
    );
}

/// One server message in the Inbox; returns its id.
fn one_in_inbox(store: &mut MailStore, account: &Account, uid: u32) -> MessageId {
    store
        .add_synced(&account.folder_key("inbox"), &[synced(uid, "Poprawki")])
        .unwrap()[0]
}

fn kinds(store: &MailStore, account: &Account) -> Vec<ChangeKind> {
    store
        .pending_changes(&account.id)
        .unwrap()
        .into_iter()
        .map(|c| c.kind)
        .collect()
}

#[test]
fn sample_mailbox_is_consistent() {
    let store = sample_store();
    assert!(store.is_sample());
    assert_eq!(store.accounts().len(), 2);
    let counts = store.folder_tree().unwrap().counts;
    assert!(counts.values().map(|c| c.total).sum::<usize>() >= 20);
    for folder in counts.keys() {
        assert!(
            store.check_folder(folder).is_ok(),
            "{folder} is a real folder"
        );
    }
}

#[test]
fn list_is_newest_first() {
    let store = sample_store();
    let rows = store.list("firma/inbox").unwrap();
    assert!(rows.len() > 3);
    assert!(rows.windows(2).all(|w| w[0].date >= w[1].date));
    assert_eq!(rows[0].from_name, "Anna Kowalska");
}

#[test]
fn polish_text_survives_the_round_trip() {
    let store = sample_store();
    let first = &store.list("firma/inbox").unwrap()[0];
    let msg = store.get(first.id).unwrap();
    assert!(msg.subject.contains("głównej"));
    assert!(msg.body.contains("Dzień dobry"));
}

#[test]
fn flags_and_read_state() {
    let mut store = sample_store();
    let id = store.list("firma/inbox").unwrap()[0].id;
    let before = store.unread_count("firma/inbox").unwrap();
    store.set_unread(id, false).unwrap();
    assert_eq!(store.unread_count("firma/inbox").unwrap(), before - 1);
    store.set_flagged(id, false).unwrap();
    assert!(!store.get(id).unwrap().flagged);
    assert_eq!(
        store.set_flagged(99_999, true),
        Err(StoreError::UnknownMessage(99_999))
    );
}

#[test]
fn delete_goes_to_trash_then_removes() {
    let mut store = sample_store();
    let id = store.list("firma/inbox").unwrap()[0].id;
    assert_eq!(store.delete(id), Ok(DeleteOutcome::MovedToTrash));
    assert_eq!(store.get(id).unwrap().folder, "firma/trash");
    assert_eq!(store.delete(id), Ok(DeleteOutcome::Removed));
    assert!(store.get(id).is_err());
}

#[test]
fn undo_is_a_move_back() {
    let mut store = sample_store();
    let id = store.list("gmail/inbox").unwrap()[0].id;
    store.delete(id).unwrap();
    store.move_to(id, "gmail/inbox").unwrap();
    assert_eq!(store.get(id).unwrap().folder, "gmail/inbox");
}

#[test]
fn moves_stay_inside_one_account() {
    let mut store = sample_store();
    let id = store.list("firma/inbox").unwrap()[0].id;
    assert!(matches!(
        store.move_to(id, "gmail/archive"),
        Err(StoreError::CrossAccountMove { .. })
    ));
    assert!(matches!(
        store.move_to(id, "firma/nope"),
        Err(StoreError::UnknownFolder(_))
    ));
    store.move_to(id, "firma/klienci").unwrap();
    assert_eq!(store.list("firma/klienci").unwrap()[0].id, id);
}

#[test]
fn outgoing_mail_is_filed_in_the_right_folder() {
    let mut store = sample_store();
    let sent = store
        .save_outgoing(draft("firma"), None, FolderKind::Sent, now())
        .unwrap();
    let msg = store.get(sent).unwrap();
    assert_eq!(msg.folder, "firma/sent");
    assert_eq!(msg.from_email, "biuro@twojastudio.pl");
    assert_eq!(msg.size_kb, 1);
    let saved = store
        .save_outgoing(draft("gmail"), None, FolderKind::Drafts, now())
        .unwrap();
    assert_eq!(store.get(saved).unwrap().folder, "gmail/drafts");
    assert!(store
        .save_outgoing(draft("nope"), None, FolderKind::Sent, now())
        .is_err());
}

#[test]
fn a_chosen_folder_takes_its_kind_and_drafts_written_here_follow() {
    let (mut store, account) = store_with_server_account();
    let folder = |id: &str, kind| Folder {
        id: id.into(),
        name: id.into(),
        kind,
        remote: Some(id.to_uppercase()),
    };
    let mut list = vec![
        folder("inbox", FolderKind::Inbox),
        folder("drafts", FolderKind::Drafts),
        folder("robocze", FolderKind::Custom),
        folder("sent", FolderKind::Sent),
    ];
    store.set_folders(&account.id, &list).unwrap();
    let written = OutgoingDraft {
        account: account.id.clone(),
        to: "anna@kowalska-architekci.pl".into(),
        subject: "Oferta".into(),
        ..OutgoingDraft::default()
    };
    let draft = store
        .save_outgoing(written, None, FolderKind::Drafts, now())
        .unwrap();
    let kind = |s: &MailStore, id: &str| s.kind_of(&account.folder_key(id)).unwrap();

    store
        .choose_folder(&account.folder_key("robocze"), FolderKind::Drafts)
        .unwrap();
    assert_eq!(kind(&store, "robocze"), FolderKind::Drafts);
    assert_eq!(kind(&store, "drafts"), FolderKind::Custom);
    assert_eq!(
        store.get(draft).unwrap().folder,
        account.folder_key("robocze")
    );
    let hints = store.folder_hints(&account.id).unwrap();
    assert_eq!(hints.chosen.get("ROBOCZE"), Some(&FolderKind::Drafts));
    assert!(hints.held.contains("ROBOCZE") && !hints.held.contains("DRAFTS"));

    // A sync's plan keeps the choice and the folder it had; were Drafts to move back,
    // the draft would go along.
    list[1].kind = FolderKind::Drafts;
    list[2].kind = FolderKind::Custom;
    store.set_folders(&account.id, &list).unwrap();
    assert_eq!(
        store.get(draft).unwrap().folder,
        account.folder_key("drafts")
    );
    assert_eq!(
        store
            .folder_hints(&account.id)
            .unwrap()
            .chosen
            .get("ROBOCZE"),
        Some(&FolderKind::Drafts),
        "the choice stays for the next plan"
    );

    // Only server folders, never the Inbox, only special kinds.
    for (key, kind) in [
        (account.folder_key("inbox"), FolderKind::Sent),
        (account.folder_key("outbox"), FolderKind::Sent),
        ("local/archive".to_owned(), FolderKind::Sent),
        (account.folder_key("sent"), FolderKind::Inbox),
    ] {
        assert!(
            store.choose_folder(&key, kind).is_err(),
            "{key} as {kind:?}"
        );
    }
}

#[test]
fn autosaves_leave_the_draft_being_edited_until_it_is_saved() {
    let mut store = sample_store();
    let draft = || OutgoingDraft {
        body: "Dzień dobry,\n\nwdrożę do piątku.\n".into(),
        ..draft("gmail")
    };
    let old = store
        .save_outgoing(draft(), None, FolderKind::Drafts, now())
        .unwrap();
    // Editing it: each autosave replaces the one before, never the draft itself.
    let first = store
        .save_outgoing(draft(), None, FolderKind::Drafts, now())
        .unwrap();
    let second = OutgoingDraft {
        autosaved: Some(first),
        ..draft()
    };
    let second = store
        .save_outgoing(second, None, FolderKind::Drafts, now())
        .unwrap();
    assert!(store.get(first).is_err());
    assert!(store.get(old).is_ok());
    // Saved for real: the draft it was edited from and the last autosave both go.
    let saved = OutgoingDraft {
        replaces: Some(old),
        autosaved: Some(second),
        ..draft()
    };
    let saved = store
        .save_outgoing(saved, None, FolderKind::Drafts, now())
        .unwrap();
    assert!(store.get(old).is_err() && store.get(second).is_err());
    assert_eq!(store.list("gmail/drafts").unwrap().len(), 1);
    // Discarding removes drafts only.
    let inbox = store.list("firma/inbox").unwrap()[0].id;
    assert!(!store.discard_draft(inbox).unwrap());
    assert!(store.get(inbox).is_ok());
    assert!(store.discard_draft(saved).unwrap());
    assert!(store.get(saved).is_err());
    assert!(!store.discard_draft(saved).unwrap());
}

#[test]
fn new_mail_arrives_once() {
    let mut store = sample_store();
    let before = store.list("firma/inbox").unwrap().len();
    let first = store.fetch_new().unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(store.list("firma/inbox").unwrap().len(), before + 1);
    assert!(store.fetch_new().unwrap().is_empty());
}

#[test]
fn sample_mail_queues_nothing() {
    let mut store = sample_store();
    let id = store.list("firma/inbox").unwrap()[0].id;
    store.set_unread(id, false).unwrap();
    store.delete(id).unwrap();
    assert!(!store.has_pending_changes("firma").unwrap());
}

#[test]
fn search_finds_words_in_bodies_without_diacritics() {
    let store = sample_store();
    let hits = store.search("dzien dob").unwrap();
    assert!(!hits.is_empty());
    for id in &hits {
        let m = store.get(*id).unwrap();
        let text = format!("{} {}", m.subject, m.body).to_lowercase();
        assert!(text.contains("dzień dob"), "{} matched", m.subject);
    }
    assert!(store.search("   ").unwrap().is_empty());
    assert!(store.search("\"unbalanced AND (").is_ok());
}

#[test]
fn flag_changes_are_queued_once_with_the_latest_value() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    store.set_unread(id, false).unwrap();
    store.set_unread(id, true).unwrap();
    store.set_unread(id, false).unwrap();
    store.set_flagged(id, true).unwrap();
    assert_eq!(
        kinds(&store, &account),
        [ChangeKind::Seen(true), ChangeKind::Flagged(true)]
    );
    let change = &store.pending_changes(&account.id).unwrap()[0];
    assert_eq!(change.folder, account.folder_key("inbox"));
    assert_eq!(change.uid, Some(7));
    assert_eq!(change.message, Some(id));
}

#[test]
fn moving_back_cancels_the_move() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    assert_eq!(store.delete(id), Ok(DeleteOutcome::MovedToTrash));
    assert_eq!(
        kinds(&store, &account),
        [ChangeKind::Move(account.folder_key("trash"))]
    );
    store.move_to(id, &account.folder_key("archive")).unwrap();
    assert_eq!(
        kinds(&store, &account),
        [ChangeKind::Move(account.folder_key("archive"))]
    );
    store.move_to(id, &account.folder_key("inbox")).unwrap();
    assert!(kinds(&store, &account).is_empty());
}

#[test]
fn spam_that_goes_to_trash_is_marked_read_first() {
    let (mut store, account) = store_with_server_account();
    let mut folders = store.account(&account.id).unwrap().folders.clone();
    folders.push(Folder {
        id: "spam".into(),
        name: "spam".into(),
        kind: FolderKind::Spam,
        remote: Some("SPAM".into()),
    });
    store.set_folders(&account.id, &folders).unwrap();
    let spam = account.folder_key("spam");
    let ids = store
        .add_synced(&spam, &[synced(7, "Wygrana"), synced(8, "Kredyt")])
        .unwrap();
    let inbox = one_in_inbox(&mut store, &account, 9);

    // Del in Spam, and a move to Trash with mail from elsewhere along.
    store.delete(ids[0]).unwrap();
    store
        .move_many(&[ids[1], inbox], &account.folder_key("trash"))
        .unwrap();
    assert!(!store.get(ids[0]).unwrap().unread);
    assert!(!store.get(ids[1]).unwrap().unread);
    assert!(store.get(inbox).unwrap().unread, "only spam is marked read");
    let trash = ChangeKind::Move(account.folder_key("trash"));
    let changes = store.pending_changes(&account.id).unwrap();
    let of = |id| -> Vec<_> {
        changes
            .iter()
            .filter(|c| c.message == Some(id))
            .map(|c| c.kind.clone())
            .collect()
    };
    // \Seen goes where the message still is, then the move.
    assert_eq!(of(ids[0]), [ChangeKind::Seen(true), trash.clone()]);
    assert_eq!(of(ids[1]), [ChangeKind::Seen(true), trash.clone()]);
    assert_eq!(of(inbox), [trash]);
    assert_eq!(changes[0].folder, spam);

    // Spam moved anywhere else keeps its state.
    let other = store.add_synced(&spam, &[synced(10, "Paczka")]).unwrap()[0];
    store
        .move_to(other, &account.folder_key("archive"))
        .unwrap();
    assert!(store.get(other).unwrap().unread);
}

#[test]
fn deleting_from_trash_queues_an_expunge_that_outlives_the_row() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    store.set_flagged(id, true).unwrap();
    store.delete(id).unwrap();
    assert_eq!(store.delete(id), Ok(DeleteOutcome::Removed));
    assert!(store.get(id).is_err());
    let changes = store.pending_changes(&account.id).unwrap();
    assert_eq!(changes.len(), 1, "the flag and the move are moot");
    assert_eq!(changes[0].kind, ChangeKind::Expunge);
    assert_eq!(
        changes[0].folder,
        account.folder_key("inbox"),
        "still there on the server"
    );
    assert_eq!(changes[0].uid, Some(7));
    assert_eq!(changes[0].subject, "Poprawki");
    assert_eq!(changes[0].message, None);
}

#[test]
fn a_done_move_updates_the_server_location() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    let archive = account.folder_key("archive");
    store.move_to(id, &archive).unwrap();
    let change = store.pending_changes(&account.id).unwrap().remove(0);
    store
        .change_moved(change.op, id, &archive, Some(31))
        .unwrap();
    assert!(kinds(&store, &account).is_empty());
    assert_eq!(store.local_uids(&archive).unwrap(), HashSet::from([31]));
    assert!(store
        .local_uids(&account.folder_key("inbox"))
        .unwrap()
        .is_empty());
    // The next download of Archive sees UID 31 and doesn't store it twice.
    assert!(store
        .add_synced(&archive, &[synced(31, "Poprawki")])
        .unwrap()
        .is_empty());
    assert_eq!(store.list(&archive).unwrap().len(), 1);
}

#[test]
fn a_moved_message_without_a_known_uid_is_recognized_by_message_id() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    let archive = account.folder_key("archive");
    store.move_to(id, &archive).unwrap();
    let change = store.pending_changes(&account.id).unwrap().remove(0);
    store.change_moved(change.op, id, &archive, None).unwrap();
    // The server gives it UID 40 in Archive; same Message-ID as before the move.
    let mut arrived = synced(7, "Poprawki");
    arrived.uid = 40;
    assert!(
        store.add_synced(&archive, &[arrived]).unwrap().is_empty(),
        "no duplicate"
    );
    assert_eq!(store.local_uids(&archive).unwrap(), HashSet::from([40]));
    assert_eq!(
        store.list(&archive).unwrap()[0].id,
        id,
        "same message, same id"
    );
}

#[test]
fn giving_up_a_move_puts_the_message_back() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    store.move_to(id, &account.folder_key("archive")).unwrap();
    let change = store.pending_changes(&account.id).unwrap().remove(0);
    assert_eq!(
        store
            .change_failed(change.op, "NO Permission denied")
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .change_failed(change.op, "NO Permission denied")
            .unwrap(),
        2
    );
    store.give_up_change(&change).unwrap();
    assert_eq!(store.get(id).unwrap().folder, account.folder_key("inbox"));
    assert!(kinds(&store, &account).is_empty());
}

#[test]
fn server_flags_win_unless_a_local_change_is_queued() {
    let (mut store, account) = store_with_server_account();
    let key = account.folder_key("inbox");
    let a = one_in_inbox(&mut store, &account, 1);
    let b = one_in_inbox(&mut store, &account, 2);
    store.set_flagged(b, true).unwrap(); // queued, not sent yet
    let flags = |uid, seen, flagged| ServerFlags {
        uid,
        seen,
        flagged,
        keywords: Vec::new(),
    };
    let changed = store
        .refresh_flags(&key, &[flags(1, true, true), flags(2, true, false)])
        .unwrap();
    assert_eq!(changed, 3, "a: read and flagged; b: read");
    let a = store.get(a).unwrap();
    assert!(!a.unread && a.flagged);
    let b = store.get(b).unwrap();
    assert!(!b.unread);
    assert!(b.flagged, "the queued local flag wins");
}

#[test]
fn synced_mail_is_stored_once_and_tracks_the_last_uid() {
    let (mut store, account) = store_with_server_account();
    let key = account.folder_key("inbox");
    assert_eq!(store.begin_folder_sync(&key, 42).unwrap().last_uid, None);
    let first = store
        .add_synced(&key, &[synced(3, "a"), synced(5, "b")])
        .unwrap();
    assert_eq!(first.len(), 2);
    assert!(store
        .add_synced(&key, &[synced(5, "b")])
        .unwrap()
        .is_empty());
    let state = store.folder_sync_state(&key).unwrap();
    assert_eq!(
        state,
        FolderSyncState {
            uid_validity: Some(42),
            last_uid: Some(5)
        }
    );
    assert_eq!(store.search("zolwia").unwrap().len(), 2);
    assert_eq!(store.search("ŻÓŁWIA").unwrap().len(), 2);

    // Message 3 was deleted on the server.
    let gone = store.forget_vanished(&key, &HashSet::from([5])).unwrap();
    assert_eq!(gone, 1);
    assert_eq!(store.list(&key).unwrap().len(), 1);

    // The server renumbered the folder: everything is downloaded again.
    let state = store.begin_folder_sync(&key, 43).unwrap();
    assert_eq!(state.last_uid, None);
    assert!(store.list(&key).unwrap().is_empty());
}

#[test]
fn a_message_gone_from_the_server_takes_its_changes_along() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    store.set_unread(id, false).unwrap();
    store.move_to(id, &account.folder_key("archive")).unwrap();
    store
        .forget_vanished(&account.folder_key("inbox"), &HashSet::new())
        .unwrap();
    assert!(store.get(id).is_err());
    assert!(!store.has_pending_changes(&account.id).unwrap());
}

#[test]
fn adding_the_same_address_updates_the_account() {
    let (mut store, account) = store_with_server_account();
    let again = store
        .upsert_account(&AccountSetup {
            name: "Biuro".into(),
            email: "BIURO@twojastudio.pl".into(),
            host: "imap.twojastudio.pl".into(),
            port: 993,
            username: "biuro".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(again.id, account.id);
    assert_eq!(again.name, "Biuro");
    assert_eq!(again.imap.unwrap().host, "imap.twojastudio.pl");
    assert_eq!(
        again.folders.len(),
        5,
        "server folders and the Outbox are kept"
    );
    let ids: Vec<&str> = store.accounts().iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, [account.id.as_str(), LOCAL_ID]);
}

#[test]
fn removing_an_account_removes_its_mail_and_changes() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 1);
    store.delete(id).unwrap();
    store.delete(id).unwrap(); // queues an expunge, which has no message row
    store.remove_account(&account.id).unwrap();
    let ids: Vec<&str> = store.accounts().iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, [LOCAL_ID], "the folders on this PC stay");
    assert!(store.folder_tree().unwrap().counts.is_empty());
    assert!(store.search("zolwia").unwrap().is_empty());
    assert!(!store.has_pending_changes(&account.id).unwrap());
    assert!(store.remove_account(LOCAL_ID).is_err());
}

#[test]
fn accounts_move_up_and_down_and_this_pc_stays_last() {
    let (mut store, first) = store_with_server_account();
    let setup = |name: &str, email: &str| AccountSetup {
        name: name.into(),
        email: email.into(),
        host: "mail.example.pl".into(),
        port: 993,
        ..Default::default()
    };
    let second = store.upsert_account(&setup("Dom", "jan@dom.pl")).unwrap();
    let third = store.upsert_account(&setup("Klub", "jan@klub.pl")).unwrap();
    let order =
        |s: &MailStore| -> Vec<String> { s.accounts().iter().map(|a| a.id.clone()).collect() };
    let ids = |list: &[&str]| -> Vec<String> { list.iter().map(|s| s.to_string()).collect() };

    store.move_account(&third.id, -1).unwrap();
    assert_eq!(
        order(&store),
        ids(&[&first.id, &third.id, &second.id, LOCAL_ID])
    );
    store.move_account(&third.id, -1).unwrap();
    store.move_account(&third.id, -1).unwrap(); // already first: nothing changes
    assert_eq!(
        order(&store),
        ids(&[&third.id, &first.id, &second.id, LOCAL_ID])
    );
    store.move_account(&second.id, 1).unwrap(); // last of the server accounts already
    store.move_account(LOCAL_ID, -1).unwrap(); // the folders on this PC don't move
    assert_eq!(
        order(&store),
        ids(&[&third.id, &first.id, &second.id, LOCAL_ID])
    );
    assert!(store.move_account("nobody", 1).is_err());

    // The order is kept, and a new account comes after the others.
    let fourth = store
        .upsert_account(&setup("Praca", "jan@praca.pl"))
        .unwrap();
    assert_eq!(
        order(&store),
        ids(&[&third.id, &first.id, &second.id, &fourth.id, LOCAL_ID])
    );
}

// ---- Folders on this PC ----

#[test]
fn the_first_account_brings_folders_on_this_pc() {
    let empty = MailStore::in_memory().unwrap();
    assert!(
        empty.this_pc().is_none(),
        "nothing before the first account"
    );
    let (mut store, account) = store_with_server_account();
    let pc = store.this_pc().unwrap();
    let kinds: Vec<_> = pc.folders.iter().map(|f| (f.id.as_str(), f.kind)).collect();
    assert_eq!(
        kinds,
        [
            ("archive", FolderKind::Archive),
            ("trash", FolderKind::Trash)
        ]
    );
    assert!(pc.imap.is_none() && pc.identities.is_empty());
    // A second account goes before them; neither can take their id.
    let second = store
        .upsert_account(&AccountSetup {
            name: "Dom".into(),
            email: "jan@local.pl".into(),
            host: "mail.local.pl".into(),
            port: 993,
            ..Default::default()
        })
        .unwrap();
    assert_ne!(second.id, LOCAL_ID);
    let ids: Vec<&str> = store.accounts().iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, [account.id.as_str(), second.id.as_str(), LOCAL_ID]);
}

#[test]
fn version_10_adds_folders_on_this_pc_to_bases_with_accounts() {
    let base = |with_account: bool| {
        let db = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..9] {
            db.execute_batch(sql).unwrap();
        }
        db.pragma_update(None, "user_version", 9).unwrap();
        if with_account {
            db.execute_batch(
                    "INSERT INTO accounts (id, position, name, email, imap_host, imap_port, imap_user)
                         VALUES ('studio', 0, 'Studio', 'biuro@x.pl', 'mail.x.pl', 993, 'biuro@x.pl');",
                )
                .unwrap();
        }
        MailStore::with_connection(db).unwrap()
    };
    let store = base(true);
    assert_eq!(store.this_pc().unwrap().folders.len(), 2);
    assert_eq!(store.accounts().last().unwrap().id, LOCAL_ID);
    assert!(base(false).this_pc().is_none());
}

#[test]
fn folders_on_this_pc_are_made_renamed_and_removed() {
    let (mut store, account) = store_with_server_account();
    let key = store.create_local_folder("  Projekty   2026 ").unwrap();
    assert_eq!(key, "local/projekty-2026");
    assert_eq!(store.folder_name(&key).unwrap(), "Projekty 2026");
    assert_eq!(store.kind_of(&key), Some(FolderKind::Custom));
    // Names are unique on this PC, case and the special folders' Polish names included.
    assert!(store.create_local_folder("projekty 2026").is_err());
    assert!(store.create_local_folder("Archive").is_err());
    assert!(store.create_local_folder("kosz").is_err());
    assert!(store.create_local_folder("   ").is_err());
    assert!(store.create_local_folder(&"x".repeat(81)).is_err());
    // Same slug, another name: another id.
    assert_eq!(
        store.create_local_folder("Projekty: 2026").unwrap(),
        "local/projekty-2026-2"
    );
    // A to Z after the special folders' rank.
    let a = store.create_local_folder("Aaa").unwrap();
    let names: Vec<String> = store
        .this_pc()
        .unwrap()
        .folders
        .iter()
        .map(|f| f.name.clone())
        .collect();
    assert_eq!(
        names,
        ["Aaa", "Projekty 2026", "Projekty: 2026", "Archive", "Trash"]
    );

    store.rename_local_folder(&key, "Klienci").unwrap();
    assert_eq!(store.folder_name(&key).unwrap(), "Klienci");
    store.rename_local_folder(&key, "klienci").unwrap(); // its own name, written otherwise
    assert!(store.rename_local_folder(&key, "Aaa").is_err());
    assert!(store.rename_local_folder("local/archive", "Stare").is_err());
    assert!(store.delete_local_folder("local/trash").is_err());
    assert!(store
        .rename_local_folder(&account.folder_key("inbox"), "X")
        .is_err());

    // Not while it holds mail; a rule that moves there loses that move, or goes.
    let id = one_in_inbox(&mut store, &account, 1);
    store.move_to(id, &key).unwrap();
    assert!(store.delete_local_folder(&key).is_err());
    store.move_to(id, &a).unwrap();
    let rule = |name: &str, actions| Rule {
        id: 0,
        name: name.into(),
        enabled: true,
        account: None,
        match_all: true,
        conditions: vec![crate::filters::Condition {
            header: None,
            field: crate::filters::Field::From,
            op: crate::filters::Op::Contains,
            value: "anna".into(),
        }],
        actions,
        stop: false,
    };
    let to_key = Action::Move {
        folder: key.clone(),
    };
    store
        .save_rule(&rule("Tylko", vec![to_key.clone()]))
        .unwrap();
    store
        .save_rule(&rule(
            "I flaga",
            vec![
                to_key,
                Action::Copy {
                    folder: key.clone(),
                },
                Action::Flag,
            ],
        ))
        .unwrap();
    assert_eq!(store.delete_local_folder(&key).unwrap(), 2);
    let rules = store.rules().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].actions, [Action::Flag]);
    assert!(store.folder_name(&key).is_none());
}

#[test]
fn server_mail_moved_to_this_pc_leaves_the_server() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    store.set_flagged(id, true).unwrap();
    store.move_many(&[id], "local/archive").unwrap();
    let m = store.get(id).unwrap();
    assert_eq!(m.folder, "local/archive");
    assert_eq!(
        store.locate(id).unwrap().1,
        None,
        "it no longer has a place on the server"
    );
    let changes = store.pending_changes(&account.id).unwrap();
    assert_eq!(changes.len(), 1, "the flag is moot now: {changes:?}");
    assert_eq!(changes[0].kind, ChangeKind::Expunge);
    assert_eq!(
        (changes[0].folder.as_str(), changes[0].uid),
        ("twojastudio/inbox", Some(7))
    );
    assert_eq!(changes[0].message, None);
    // Read and flag here touch nothing on any server.
    store.set_unread(id, false).unwrap();
    assert_eq!(store.pending_changes(&account.id).unwrap().len(), 1);
    // The next download doesn't take it back, nor drop it when the server has.
    assert!(store
        .local_uids(&account.folder_key("inbox"))
        .unwrap()
        .is_empty());
    store
        .forget_vanished(&account.folder_key("inbox"), &HashSet::new())
        .unwrap();
    assert!(store.get(id).is_ok());
    // Even after the expunge went out, it stays here when deleted from this PC: to its
    // Trash, then for good, with nothing for the server.
    let op = changes[0].op;
    store.change_done(op).unwrap();
    assert_eq!(store.delete(id).unwrap(), DeleteOutcome::MovedToTrash);
    assert_eq!(store.get(id).unwrap().folder, "local/trash");
    assert_eq!(store.delete(id).unwrap(), DeleteOutcome::Removed);
    assert!(!store.has_pending_changes(&account.id).unwrap());
}

#[test]
fn mail_back_from_this_pc_before_the_server_heard_just_moves() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    let places = store.move_many(&[id], "local/archive").unwrap();
    assert_eq!(places, [(id, account.folder_key("inbox"))]);
    store.set_unread(id, false).unwrap();
    // Z: back where it was. No expunge, no upload; what changed here goes to the server.
    store
        .move_many(&[id], &account.folder_key("inbox"))
        .unwrap();
    assert_eq!(
        store.locate(id).unwrap(),
        (
            account.folder_key("inbox"),
            Some(account.folder_key("inbox"))
        )
    );
    assert_eq!(
        store.local_uids(&account.folder_key("inbox")).unwrap(),
        HashSet::from([7])
    );
    assert_eq!(
        kinds(&store, &account),
        [ChangeKind::Seen(true), ChangeKind::Flagged(false)]
    );
    // Back to another folder of that account: a move on the server.
    store.move_many(&[id], "local/archive").unwrap();
    store
        .move_many(&[id], &account.folder_key("archive"))
        .unwrap();
    assert_eq!(
        kinds(&store, &account),
        [
            ChangeKind::Move(account.folder_key("archive")),
            ChangeKind::Seen(true),
            ChangeKind::Flagged(false)
        ]
    );
}

#[test]
fn mail_from_this_pc_is_uploaded_with_its_flags() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    store.move_many(&[id], "local/archive").unwrap();
    let expunge = store.pending_changes(&account.id).unwrap()[0].op;
    store.change_done(expunge).unwrap(); // the server removed it
    store.set_flagged(id, true).unwrap();
    store
        .move_many(&[id], &account.folder_key("archive"))
        .unwrap();
    let changes = store.pending_changes(&account.id).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].kind,
        ChangeKind::Append {
            to: account.folder_key("archive"),
            flags: Some((false, true))
        },
        "unread as it is, and flagged"
    );
    // Moved on before it went up: it goes up to the new place instead.
    store
        .move_many(&[id], &account.folder_key("inbox"))
        .unwrap();
    let changes = store.pending_changes(&account.id).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(
        matches!(&changes[0].kind, ChangeKind::Append { to, .. } if to == &account.folder_key("inbox"))
    );
    // Uploaded: the next download adopts the server's copy instead of storing it twice.
    store
        .change_appended(changes[0].op, id, &account.folder_key("inbox"))
        .unwrap();
    let mut copy = synced(9, "Poprawki");
    copy.message.message_id = Some("7.Poprawki@kowalska-architekci.pl".into());
    let new = store
        .add_synced(&account.folder_key("inbox"), &[copy])
        .unwrap();
    assert!(new.is_empty());
    assert_eq!(
        store.local_uids(&account.folder_key("inbox")).unwrap(),
        HashSet::from([9])
    );
    // Back to this PC before the upload went out: no upload at all.
    let other = one_in_inbox(&mut store, &account, 10);
    store.move_many(&[other], "local/archive").unwrap();
    let op = store.pending_changes(&account.id).unwrap()[0].op;
    store.change_done(op).unwrap();
    store
        .move_many(&[other], &account.folder_key("inbox"))
        .unwrap();
    store.move_many(&[other], "local/archive").unwrap();
    assert!(!store.has_pending_changes(&account.id).unwrap());
}

#[test]
fn mail_that_moved_on_while_it_was_uploading_leaves_the_server_again() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    store.move_many(&[id], "local/archive").unwrap();
    let op = store.pending_changes(&account.id).unwrap()[0].op;
    store.change_done(op).unwrap();
    store
        .move_many(&[id], &account.folder_key("inbox"))
        .unwrap();
    let append = store.pending_changes(&account.id).unwrap()[0].op;
    // While the upload was on its way, the message went back to this PC.
    store.move_many(&[id], "local/archive").unwrap();
    store
        .change_appended(append, id, &account.folder_key("inbox"))
        .unwrap();
    let changes = store.pending_changes(&account.id).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Expunge);
    assert_eq!(
        (
            changes[0].folder.as_str(),
            changes[0].uid,
            changes[0].message_id.as_deref()
        ),
        (
            "twojastudio/inbox",
            None,
            Some("7.Poprawki@kowalska-architekci.pl")
        ),
        "found on the server by its Message-ID"
    );
    assert_eq!(store.locate(id).unwrap(), ("local/archive".into(), None));
}

#[test]
fn mail_moved_to_this_pc_during_a_move_on_the_server_leaves_from_its_new_place() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    store.move_to(id, &account.folder_key("archive")).unwrap();
    let mv = store.pending_changes(&account.id).unwrap()[0].op;
    // The server is moving it when the user puts it on this PC.
    store.move_to(id, "local/archive").unwrap();
    store
        .change_moved(mv, id, &account.folder_key("archive"), Some(31))
        .unwrap();
    let changes = store.pending_changes(&account.id).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Expunge);
    assert_eq!(
        (changes[0].folder.as_str(), changes[0].uid),
        ("twojastudio/archive", Some(31))
    );
    assert_eq!(store.locate(id).unwrap(), ("local/archive".into(), None));
}

#[test]
fn mail_goes_between_server_accounts_by_way_of_this_pc() {
    let (mut store, account) = store_with_server_account();
    let other = store
        .upsert_account(&AccountSetup {
            name: "Dom".into(),
            email: "jan@dom.pl".into(),
            host: "mail.dom.pl".into(),
            port: 993,
            ..Default::default()
        })
        .unwrap();
    let id = one_in_inbox(&mut store, &account, 7);
    let outbox = other.folder_key(OUTBOX_ID);
    assert!(matches!(
        store.move_many(&[id], &outbox),
        Err(StoreError::CrossAccountMove { .. })
    ));
    store.move_many(&[id], "local/archive").unwrap();
    store.move_many(&[id], &outbox).unwrap();
    assert_eq!(store.get(id).unwrap().folder, outbox);
    assert!(
        !store.has_pending_changes(&other.id).unwrap(),
        "the Outbox is on no server: nothing to upload"
    );
}

#[test]
fn folders_gone_from_the_server_are_dropped() {
    let (mut store, account) = store_with_server_account();
    store
        .add_synced(&account.folder_key("sent"), &[synced(1, "a")])
        .unwrap();
    let inbox = account.folders[0].clone();
    store.set_folders(&account.id, &[inbox]).unwrap();
    let kinds: Vec<_> = store
        .account(&account.id)
        .unwrap()
        .folders
        .iter()
        .map(|f| f.kind)
        .collect();
    assert_eq!(
        kinds,
        [FolderKind::Inbox, FolderKind::Outbox],
        "the local Outbox stays"
    );
    assert!(store.folder_tree().unwrap().counts.is_empty());
}

#[test]
fn mail_survives_reopening_the_file() {
    let path = std::env::temp_dir().join(format!(
        "telex-test-{}-{}.db",
        std::process::id(),
        chrono::Local::now()
            .timestamp_nanos_opt()
            .unwrap_or_default()
    ));
    {
        let mut store = MailStore::open(&path).unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                username: String::new(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(account.id, "twojastudio");
    }
    let store = MailStore::open(&path).unwrap();
    assert!(!store.is_sample());
    assert!(!store.is_in_memory());
    assert!(MailStore::in_memory().unwrap().is_in_memory());
    assert_eq!(store.accounts()[0].email, "biuro@twojastudio.pl");
    drop(store);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}

#[test]
fn a_version_1_database_is_upgraded() {
    // A database as the first release left it, with one downloaded message.
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(MIGRATIONS[0]).unwrap();
    db.pragma_update(None, "user_version", 1).unwrap();
    db.execute_batch(
            "INSERT INTO accounts VALUES ('studio', 0, 'Studio', 'biuro@x.pl', 'mail.x.pl', 993, 'biuro@x.pl');
             INSERT INTO folders VALUES ('studio', 'inbox', 0, 'Inbox', 'inbox', 'INBOX', 7, 5);
             INSERT INTO messages (folder, uid, from_name, from_email, to_addr, cc, subject, date,
                 size_kb, unread, flagged, remote_images, attachments, body, raw)
             VALUES ('studio/inbox', 5, 'Anna', 'anna@x.pl', '', '', 'Wycena', '2026-10-09 14:30:00',
                 1, 1, 0, 0, '[]', 'Dzień dobry', CAST('Message-ID: <wycena@x.pl>' || char(13, 10) || char(13, 10) AS BLOB));",
        )
        .unwrap();

    let mut store = MailStore::with_connection(db).unwrap();
    let version: i64 = store
        .db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, MIGRATIONS.len() as i64);
    assert_eq!(
        store.local_uids("studio/inbox").unwrap(),
        HashSet::from([5])
    );
    let id = store.list("studio/inbox").unwrap()[0].id;
    assert_eq!(
        store.get(id).unwrap().message_id.as_deref(),
        Some("wycena@x.pl")
    );
    // Version 3: an Outbox, and no SMTP server until the user edits the account.
    let account = store.account("studio").unwrap();
    assert!(account.folder_of_kind(FolderKind::Outbox).is_some());
    assert!(account.smtp.is_none());
    // Version 6: one identity, sending as before.
    let me = account.identity(None);
    assert_eq!(
        (me.name.as_str(), me.email.as_str()),
        ("Studio", "biuro@x.pl")
    );
    assert_eq!(me.signature, "Pozdrawiam");
    assert!(me.forward_header.contains("\nOd: {from}\n"));
    // Changes to old mail are queued like any other.
    store.set_unread(id, false).unwrap();
    assert!(store.has_pending_changes("studio").unwrap());
    assert_eq!(store.search("dzien").unwrap(), [id]);
}

#[test]
fn version_7_moves_the_text_and_finds_the_same_mail() {
    // A version 6 database, where the text and raw message are in `messages`.
    let db = Connection::open_in_memory().unwrap();
    for sql in &MIGRATIONS[..6] {
        db.execute_batch(sql).unwrap();
    }
    db.pragma_update(None, "user_version", 6).unwrap();
    db.execute_batch(
        "INSERT INTO accounts (id, position, name, email, imap_host, imap_port, imap_user)
                 VALUES ('studio', 0, 'Studio', 'biuro@x.pl', 'mail.x.pl', 993, 'biuro@x.pl');
             INSERT INTO folders (account, id, position, name, kind, remote)
                 VALUES ('studio', 'inbox', 0, 'Inbox', 'inbox', 'INBOX');",
    )
    .unwrap();
    let words = [
        "wycena", "żółw", "Łódź", "faktura", "projekt", "dzień", "dobry",
    ];
    for i in 0..40 {
        let body: Vec<&str> = (0..i % 7 + 1).map(|j| words[(i + j) % 7]).collect();
        db.execute(
            "INSERT INTO messages (folder, srv_folder, uid, message_id, from_name, from_email,
                     to_addr, cc, subject, date, size_kb, unread, flagged, remote_images,
                     attachments, body, raw)
                 VALUES ('studio/inbox', 'studio/inbox', ?1, ?2, ?3, 'a@x.pl', 'biuro@x.pl', '',
                     ?4, '2026-10-09 14:30:00', 1, 1, 0, 0, '[]', ?5, ?6)",
            params![
                i as i64,
                format!("{i}@x.pl"),
                format!("Łucja {}", words[i % 7]),
                format!("Temat {}", words[(i * 3) % 7]),
                body.join(" "),
                format!("raw {i}").into_bytes()
            ],
        )
        .unwrap();
    }
    let queries = [
        "wycena",
        "zolw",
        "lodz",
        "faktura projekt",
        "dz",
        "łucja",
        "nic",
    ];
    let before: Vec<Vec<MessageId>> = queries
            .iter()
            .map(|q| {
                db.prepare(
                    "SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?1 ORDER BY rank LIMIT 5000",
                )
                .unwrap()
                .query_map([fts_query(q)], |r| r.get::<_, i64>(0))
                .unwrap()
                .map(|id| id.unwrap() as MessageId)
                .collect()
            })
            .collect();
    assert!(before.iter().filter(|hits| hits.len() > 5).count() >= 3);

    let mut store = MailStore::with_connection(db).unwrap();
    for (q, hits) in queries.iter().zip(&before) {
        assert_eq!(
            &store.search(q).unwrap(),
            hits,
            "same hits, same order: {q}"
        );
    }
    let columns: Vec<String> = store
        .db
        .prepare("SELECT name FROM pragma_table_info('messages')")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!columns.iter().any(|c| c == "body" || c == "raw"));
    let id = store.list("studio/inbox").unwrap()[0].id;
    assert!(!store.get(id).unwrap().body.is_empty());
    assert!(store.raw_of(id).unwrap().unwrap().starts_with(b"raw "));

    // A message that goes takes its text and its place in the index along.
    let hits = store.search("dobry").unwrap();
    let gone = hits[0];
    store.forget_message(gone).unwrap();
    assert!(!store.search("dobry").unwrap().contains(&gone));
    assert_eq!(store.raw_of(gone).unwrap(), None);
    let orphans: i64 = store
        .db
        .query_row(
            "SELECT COUNT(*) FROM message_content WHERE id NOT IN (SELECT id FROM messages)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(orphans, 0);
}

#[test]
fn many_messages_change_at_once_and_only_real_changes_are_queued() {
    let (mut store, account) = store_with_server_account();
    let ids: Vec<MessageId> = (1..=4)
        .map(|uid| one_in_inbox(&mut store, &account, uid))
        .collect();
    store.set_unread(ids[0], false).unwrap();
    let before = store.pending_changes(&account.id).unwrap().len();
    // Three are unread, one already read: three change and are queued.
    assert_eq!(store.set_unread_many(&ids, false).unwrap(), 3);
    assert_eq!(
        store.pending_changes(&account.id).unwrap().len(),
        before + 3
    );
    assert_eq!(store.unread_count(&account.folder_key("inbox")).unwrap(), 0);
    assert_eq!(store.set_flagged_many(&ids[..2], true).unwrap(), 2);
    assert_eq!(
        store.set_flagged_many(&[ids[0], 99_999], true).unwrap(),
        0,
        "gone ids skipped"
    );

    // Moving is all or nothing.
    let archive = account.folder_key("archive");
    let places = store.move_many(&ids[..2], &archive).unwrap();
    assert_eq!(
        places,
        [
            (ids[0], account.folder_key("inbox")),
            (ids[1], account.folder_key("inbox"))
        ]
    );
    assert_eq!(store.list(&archive).unwrap().len(), 2);
    assert!(store.move_many(&[ids[2], 99_999], &archive).is_err());
    assert_eq!(
        store.get(ids[2]).unwrap().folder,
        account.folder_key("inbox"),
        "untouched"
    );

    // Deleting: what is in Trash goes for good, the rest goes to Trash.
    store.delete(ids[3]).unwrap();
    let report = store.delete_many(&[ids[2], ids[3]]).unwrap();
    assert_eq!(report.trashed, [(ids[2], account.folder_key("inbox"))]);
    assert_eq!(report.removed, 1);
    assert!(store.get(ids[3]).is_err());
    assert_eq!(
        store.get(ids[2]).unwrap().folder,
        account.folder_key("trash")
    );
    assert!(kinds(&store, &account).contains(&ChangeKind::Expunge));
}

#[test]
fn a_folder_is_marked_read_in_one_go() {
    let (mut store, account) = store_with_server_account();
    for uid in 1..=5 {
        one_in_inbox(&mut store, &account, uid);
    }
    let inbox = account.folder_key("inbox");
    assert_eq!(store.mark_folder_read(&inbox).unwrap(), 5);
    assert_eq!(store.unread_count(&inbox).unwrap(), 0);
    assert_eq!(
        store.mark_folder_read(&inbox).unwrap(),
        0,
        "nothing left to do"
    );
    assert_eq!(
        kinds(&store, &account),
        vec![ChangeKind::Seen(true); 5],
        "one change per message, nothing twice"
    );
    assert!(store.mark_folder_read("nope/inbox").is_err());
}

#[test]
fn the_tree_says_how_much_older_mail_is_on_the_server() {
    let (mut store, account) = store_with_server_account();
    let inbox = account.folder_key("inbox");
    for uid in [301, 302] {
        one_in_inbox(&mut store, &account, uid);
    }
    assert_eq!(store.note_server_count(&inbox, 450).unwrap(), 448);
    assert_eq!(store.folder_tree().unwrap().counts[&inbox].older, 448);
    // A folder with nothing older says nothing.
    assert_eq!(store.note_server_count(&inbox, 2).unwrap(), 0);
    assert_eq!(store.folder_tree().unwrap().counts[&inbox].older, 0);
    let json = serde_json::to_value(store.folder_tree().unwrap()).unwrap();
    assert!(
        json["counts"][&inbox].get("older").is_none(),
        "left out when zero"
    );
}

#[test]
fn version_9_knows_what_old_replies_answer() {
    // A version 8 database with a reply in it.
    let db = Connection::open_in_memory().unwrap();
    for sql in &MIGRATIONS[..8] {
        db.execute_batch(sql).unwrap();
    }
    db.pragma_update(None, "user_version", 8).unwrap();
    db.execute_batch(
        "INSERT INTO accounts (id, position, name, email, imap_host, imap_port, imap_user)
                 VALUES ('studio', 0, 'Studio', 'biuro@x.pl', 'mail.x.pl', 993, 'biuro@x.pl');
             INSERT INTO folders (account, id, position, name, kind, remote)
                 VALUES ('studio', 'inbox', 0, 'Inbox', 'inbox', 'INBOX');
             INSERT INTO messages (id, folder, srv_folder, uid, message_id, from_name, from_email,
                 to_addr, cc, subject, date, size_kb, unread, flagged, remote_images, attachments)
                 VALUES (1, 'studio/inbox', 'studio/inbox', 1, 'b@x.pl', 'Anna', 'a@x.pl', '', '',
                     'Re: Wycena', '2026-10-09 14:30:00', 1, 1, 0, 0, '[]');",
    )
    .unwrap();
    db.execute(
        "INSERT INTO message_content (id, body, raw) VALUES (1, 'OK', ?1)",
        [
            b"Message-ID: <b@x.pl>\r\nIn-Reply-To: <a@x.pl>\r\nSubject: Re: Wycena\r\n\r\nOK\r\n"
                .to_vec(),
        ],
    )
    .unwrap();
    let store = MailStore::with_connection(db).unwrap();
    let m = store.get(1).unwrap();
    assert_eq!(m.refs, ["a@x.pl"]);
    assert_eq!(m.bcc, "");
    let account = store.account("studio").unwrap();
    assert_eq!(
        account.imap.as_ref().unwrap().security,
        SmtpSecurity::Tls,
        "as before"
    );
}

#[test]
fn imap_over_starttls_is_kept() {
    let mut store = MailStore::in_memory().unwrap();
    let account = store
        .upsert_account(&AccountSetup {
            name: "Studio".into(),
            email: "biuro@x.pl".into(),
            host: "mail.x.pl".into(),
            port: 143,
            imap_security: SmtpSecurity::StartTls,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(account.imap.unwrap().security, SmtpSecurity::StartTls);
}

#[test]
fn slugs_are_safe_ids() {
    assert_eq!(slug("INBOX.Faktury 2026"), "inbox-faktury-2026");
    assert_eq!(slug("  --Wysłane--  "), "wysłane");
    assert_eq!(slug("..."), "");
}

#[test]
fn identities_have_one_default_and_never_run_out() {
    let (mut store, account) = store_with_server_account();
    let first = account.identity(None);
    assert_eq!(account.identities.len(), 1);
    assert!(first.is_default && first.signature == "Pozdrawiam");

    let jan = store
        .save_identity(&Identity::new(
            &account.id,
            "Jan Kowalski",
            "jan@twojastudio.pl",
        ))
        .unwrap();
    let a = store.account(&account.id).unwrap();
    assert_eq!(a.identities.iter().filter(|i| i.is_default).count(), 1);
    assert_eq!(a.identity(None).id, jan.id, "the new default");

    store.delete_identity(jan.id).unwrap();
    let a = store.account(&account.id).unwrap();
    assert_eq!(a.identities.len(), 1);
    assert!(a.identities[0].is_default, "the other one took over");
    assert!(matches!(
        store.delete_identity(first.id),
        Err(StoreError::Invalid(_))
    ));
    let mut bad = first;
    bad.email = "jan".into();
    assert!(store.save_identity(&bad).is_err());
}

#[test]
fn trusted_senders_load_images_except_in_spam() {
    let mut store = sample_store();
    let first = &store.list("firma/inbox").unwrap()[0];
    let (id, sender) = (first.id, first.from_email.clone());
    assert!(!store.image_trust(id).unwrap().auto);
    store
        .set_sender_trusted(&sender.to_uppercase(), true)
        .unwrap();
    assert!(
        store.is_sender_trusted(&sender).unwrap(),
        "addresses ignore case"
    );
    let trust = store.image_trust(id).unwrap();
    assert!(trust.auto && trust.trusted && trust.sender == sender);
    assert_eq!(store.trusted_senders().unwrap(), [sender.to_uppercase()]);
    store.move_to(id, "firma/spam").unwrap();
    let trust = store.image_trust(id).unwrap();
    assert!(trust.trusted && !trust.auto, "never in Spam");
    store.move_to(id, "firma/inbox").unwrap();
    store.set_sender_trusted(&sender, false).unwrap();
    assert!(!store.image_trust(id).unwrap().auto);
    assert!(store.trusted_senders().unwrap().is_empty());
}

#[test]
fn errors_speak_the_chosen_language() {
    use crate::i18n::{self, Lang};
    let missing = StoreError::MissingFolder {
        account: "firma".into(),
        kind: FolderKind::Trash,
    };
    assert_eq!(missing.to_string(), "account \"firma\" has no Trash folder");
    assert_eq!(
        i18n::with(Lang::Pl, || missing.to_string()),
        "konto „firma” nie ma folderu Kosz"
    );
    assert_eq!(
        i18n::with(Lang::Pl, || StoreError::Database("disk full".into())
            .to_string()),
        "błąd bazy poczty: disk full",
        "SQLite's own words stay as they are"
    );
    let mut store = sample_store();
    let gone = i18n::with(Lang::Pl, || store.get(999_999).unwrap_err().to_string());
    assert_eq!(gone, "wiadomość 999999 nie istnieje");
    let only = store.account("firma").unwrap().identities[0].id;
    let refused = i18n::with(Lang::Pl, || store.delete_identity(only).unwrap_err());
    assert_eq!(
        refused.to_string(),
        "Konto potrzebuje co najmniej jednej tożsamości. Zmień tę zamiast ją usuwać."
    );
}

fn tag(store: &mut MailStore, ids: &[MessageId], keyword: &str, on: bool) -> usize {
    store.set_tag_many(ids, keyword, on).unwrap()
}

fn keyword(k: &str, on: bool) -> ChangeKind {
    ChangeKind::Keyword {
        keyword: k.into(),
        on,
    }
}

#[test]
fn tags_change_here_at_once_and_queue_one_change_per_keyword() {
    let (mut store, account) = store_with_server_account();
    let a = one_in_inbox(&mut store, &account, 1);
    let b = one_in_inbox(&mut store, &account, 2);
    assert_eq!(tag(&mut store, &[a, b], "$label2", true), 2);
    assert_eq!(tag(&mut store, &[a, b], "$label2", true), 0, "already so");
    assert_eq!(tag(&mut store, &[a], "$label4", true), 1);
    assert_eq!(store.get(a).unwrap().tags, ["$label2", "$label4"]);
    let rows = store.list(&account.folder_key("inbox")).unwrap();
    assert!(rows.iter().any(|r| r.tags == ["$label2", "$label4"]));
    assert_eq!(
        kinds(&store, &account),
        [
            keyword("$label2", true),
            keyword("$label2", true),
            keyword("$label4", true)
        ]
    );
    // Taken off again before the server heard: one change per message and keyword.
    tag(&mut store, &[a], "$label2", false);
    assert_eq!(
        kinds(&store, &account),
        [
            keyword("$label2", true),
            keyword("$label4", true),
            keyword("$label2", false)
        ]
    );
    assert!(store.set_tag_many(&[a], "nosuchtag", true).is_err());
    // Mail on this PC keeps them in the mail base only.
    store.move_many(&[b], "local/archive").unwrap();
    assert_eq!(store.get(b).unwrap().tags, ["$label2"], "a move keeps tags");
    let before = store.pending_changes(&account.id).unwrap().len();
    tag(&mut store, &[b], "$label1", true);
    assert_eq!(store.pending_changes(&account.id).unwrap().len(), before);
    assert_eq!(store.get(b).unwrap().tags, ["$label1", "$label2"]);
}

#[test]
fn the_server_s_tags_come_down_unless_a_change_is_queued() {
    let (mut store, account) = store_with_server_account();
    let key = account.folder_key("inbox");
    let a = one_in_inbox(&mut store, &account, 1);
    let b = one_in_inbox(&mut store, &account, 2);
    tag(&mut store, &[b], "$label1", true); // queued, not sent yet
    let server = |uid, keywords: &[&str]| ServerFlags {
        uid,
        seen: false,
        flagged: false,
        keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
    };
    let changed = store
        .refresh_flags(
            &key,
            &[
                server(1, &["$Label3", "$Forwarded"]),
                server(2, &["$label5"]),
            ],
        )
        .unwrap();
    assert_eq!(changed, 2);
    assert_eq!(
        store.get(a).unwrap().tags,
        ["$label3"],
        "only tags, lowercase"
    );
    assert_eq!(
        store.get(b).unwrap().tags,
        ["$label1", "$label5"],
        "the queued tag stays"
    );
    // Nothing changed: nothing written.
    assert_eq!(
        store
            .refresh_flags(&key, &[server(1, &["$label3"])])
            .unwrap(),
        0
    );
    // A server that can't keep tags never takes them away.
    assert!(store.keep_tags_here(&account.id).unwrap());
    assert!(!store.keep_tags_here(&account.id).unwrap(), "news once");
    store.refresh_flags(&key, &[server(1, &[])]).unwrap();
    assert_eq!(store.get(a).unwrap().tags, ["$label3"]);
    assert_eq!(
        store.accounts_keeping_tags_here().unwrap(),
        std::slice::from_ref(&account.id)
    );
    // New mail keeps only what are tags here.
    let mut tagged = synced(9, "Nowa");
    tagged.message.tags = vec!["$label2".into(), "$mdnsent".into()];
    let id = store.add_synced(&key, &[tagged]).unwrap()[0];
    assert_eq!(store.get(id).unwrap().tags, ["$label2"]);
}

#[test]
fn mail_back_from_this_pc_says_only_the_tags_that_changed() {
    let (mut store, account) = store_with_server_account();
    let id = one_in_inbox(&mut store, &account, 7);
    tag(&mut store, &[id], "$label1", true);
    tag(&mut store, &[id], "$label2", true);
    for c in store.pending_changes(&account.id).unwrap() {
        store.change_done(c.op).unwrap(); // the server has both
    }
    store.move_many(&[id], "local/archive").unwrap();
    tag(&mut store, &[id], "$label1", false);
    tag(&mut store, &[id], "$label3", true);
    store
        .move_many(&[id], &account.folder_key("inbox"))
        .unwrap();
    let tags: Vec<ChangeKind> = kinds(&store, &account)
        .into_iter()
        .filter(|k| matches!(k, ChangeKind::Keyword { .. }))
        .collect();
    assert_eq!(tags, [keyword("$label3", true), keyword("$label1", false)]);
}

#[test]
fn deleting_a_tag_takes_it_off_every_message_here_and_on_the_server() {
    let (mut store, account) = store_with_server_account();
    let a = one_in_inbox(&mut store, &account, 1);
    let b = one_in_inbox(&mut store, &account, 2);
    tag(&mut store, &[a, b], "$label5", true);
    tag(&mut store, &[a], "$label1", true);
    for c in store.pending_changes(&account.id).unwrap() {
        store.change_done(c.op).unwrap();
    }
    let later = store
        .tags()
        .unwrap()
        .into_iter()
        .find(|t| t.keyword == "$label5");
    assert_eq!(later.unwrap().count, 2);
    assert_eq!(store.delete_tag("$label5").unwrap(), 2);
    assert!(store.tags().unwrap().iter().all(|t| t.keyword != "$label5"));
    assert_eq!(store.get(a).unwrap().tags, ["$label1"]);
    assert!(store.get(b).unwrap().tags.is_empty());
    assert_eq!(kinds(&store, &account), vec![keyword("$label5", false); 2]);
}

#[test]
fn rules_find_tags_and_add_or_take_them_off_saying_so() {
    use crate::filters::{run, Condition, Field, Op, Rule, Trigger};
    let (mut store, account) = store_with_server_account();
    let a = one_in_inbox(&mut store, &account, 1);
    let b = one_in_inbox(&mut store, &account, 2);
    tag(&mut store, &[a], "$label4", true);
    let rule = |name: &str, op, value: &str, actions| Rule {
        id: 0,
        name: name.into(),
        enabled: true,
        account: None,
        match_all: true,
        conditions: vec![Condition {
            field: Field::Tag,
            op,
            value: value.into(),
            header: None,
        }],
        actions,
        stop: false,
    };
    let done = rule(
        "Zrobione",
        Op::Is,
        "$label4",
        vec![
            Action::RemoveTag {
                tag: "$label4".into(),
            },
            Action::AddTag {
                tag: "$label5".into(),
            },
        ],
    );
    let untagged = rule(
        "Bez etykiet",
        Op::NotExists,
        "",
        vec![Action::AddTag {
            tag: "$label2".into(),
        }],
    );
    let mut missing = untagged.clone();
    missing.actions = vec![Action::AddTag {
        tag: "nosuchtag".into(),
    }];
    assert!(matches!(
        store.save_rule(&missing),
        Err(StoreError::Invalid(_))
    ));
    store.save_rule(&done).unwrap();
    store.save_rule(&untagged).unwrap();
    let report = crate::i18n::with(crate::i18n::Lang::En, || {
        run(&mut store, &[a, b], Trigger::OnDemand, chrono::Local::now()).unwrap()
    });
    assert_eq!(report.matched, 2);
    let a = store.get(a).unwrap();
    assert_eq!(a.tags, ["$label5"]);
    assert_eq!(
        a.note.as_deref(),
        Some("Rule \"Zrobione\" took the tag To do off it and tagged it Later.")
    );
    let b = store.get(b).unwrap();
    assert_eq!(b.tags, ["$label2"]);
    assert!(!untagged.matches(&b), "b has a tag now");
    // In Polish.
    let c = one_in_inbox(&mut store, &account, 3);
    crate::i18n::with(crate::i18n::Lang::Pl, || {
        run(&mut store, &[c], Trigger::OnDemand, chrono::Local::now()).unwrap()
    });
    assert_eq!(
        store.get(c).unwrap().note.as_deref(),
        Some("Reguła „Bez etykiet” oznaczyła ją etykietą Praca.")
    );
    // A rule whose tag was deleted says so when it runs.
    store.delete_tag("$label2").unwrap();
    let d = one_in_inbox(&mut store, &account, 4);
    let report = run(&mut store, &[d], Trigger::OnDemand, chrono::Local::now()).unwrap();
    assert_eq!(report.problems.len(), 1, "{:?}", report.problems);
}
