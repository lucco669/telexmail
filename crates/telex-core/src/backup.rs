//! Backups of the mail base: one consistent copy of the whole file, made while Telex runs on a
//! connection of its own (SQLite's `VACUUM INTO`, which also leaves the free space out), into
//! a folder the user picks; by hand, or every day or week keeping the newest few. A backup
//! holds the mail, folders, rules, identities, trusted senders and OpenPGP keys (their secret
//! parts only when `backup_secret_keys` says so); not passwords, which stay
//! in the system's password store, nor settings.json, which the settings file exports.
//!
//! Restoring puts the mail base as it is aside, next to it, and the copy in its place, and then
//! makes it safe to meet the servers again ([`MailStore::after_restore`]). Compacting gives the
//! space of removed mail back to the disk.

use std::path::{Path, PathBuf};

use chrono::{NaiveDateTime, TimeDelta};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use crate::model::FolderKind;
use crate::store::{MailStore, StoreError, LOCAL_ID, MIGRATIONS};
use crate::tr;

/// Backup files are called "telex-backup-2026-10-11-153000.db": sorted by name, they are in
/// order, and nothing else in the folder looks like one.
const PREFIX: &str = "telex-backup-";
const EXTENSION: &str = ".db";
const STAMP: &str = "%Y-%m-%d-%H%M%S";

/// A backup in the backup folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupFile {
    pub path: String,
    /// When it was made, from its name.
    pub made: NaiveDateTime,
    pub bytes: u64,
}

/// What a backup holds, read without changing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupInfo {
    pub path: String,
    /// When it was made: from its name, else the file's own date.
    pub made: Option<NaiveDateTime>,
    pub bytes: u64,
    /// The server accounts' addresses.
    pub accounts: Vec<String>,
    pub messages: usize,
    /// Messages in folders on this PC.
    pub on_this_pc: usize,
}

fn file_error(e: std::io::Error) -> StoreError {
    StoreError::Invalid(tr!(
        "the file couldn't be written or read: {e}",
        "nie udało się zapisać ani odczytać pliku: {e}"
    ))
}

/// When a backup file of this name was made, or `None` when the name isn't a backup's.
fn made_of(name: &str) -> Option<NaiveDateTime> {
    let stamp = name.strip_prefix(PREFIX)?.strip_suffix(EXTENSION)?;
    NaiveDateTime::parse_from_str(stamp, STAMP).ok()
}

/// The backups in `dir`, newest first. Nothing but Telex's own backup files.
pub fn list(dir: &Path) -> Vec<BackupFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<BackupFile> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let made = made_of(&e.file_name().to_string_lossy())?;
            let meta = e.metadata().ok().filter(std::fs::Metadata::is_file)?;
            Some(BackupFile {
                path: e.path().display().to_string(),
                made,
                bytes: meta.len(),
            })
        })
        .collect();
    found.sort_by_key(|b| std::cmp::Reverse(b.made));
    found
}

/// A backup is due when `every` is set and the newest in `dir` is older than that, or there
/// is none. A few minutes' slack, so a daily backup at the same time of day isn't skipped.
pub fn due(dir: &Path, every: Option<TimeDelta>, now: NaiveDateTime) -> bool {
    let Some(every) = every else { return false };
    match list(dir).first() {
        Some(newest) => now - newest.made >= every - TimeDelta::minutes(10),
        None => true,
    }
}

/// Write a backup of the mail base at `db` into `dir`, made now (`at`). It appears under its
/// name only when it is complete and opens. Returns it.
pub fn write(db: &Path, dir: &Path, at: NaiveDateTime) -> Result<BackupFile, StoreError> {
    write_keys(db, dir, at, true)
}

/// [`write`], with or without the secret parts of the user's OpenPGP keys (the setting
/// `backup_secret_keys`). Without them the backup keeps the keys' public parts, and the
/// removed bytes are overwritten (`secure_delete`) and compacted away, not left in free pages.
pub fn write_keys(
    db: &Path,
    dir: &Path,
    at: NaiveDateTime,
    secret_keys: bool,
) -> Result<BackupFile, StoreError> {
    std::fs::create_dir_all(dir).map_err(file_error)?;
    let mut at = at;
    let mut dest = dir.join(format!("{PREFIX}{}{EXTENSION}", at.format(STAMP)));
    while dest.exists() {
        at += TimeDelta::seconds(1);
        dest = dir.join(format!("{PREFIX}{}{EXTENSION}", at.format(STAMP)));
    }
    let part = dir.join(format!("{PREFIX}{}{EXTENSION}.part", at.format(STAMP)));
    let _ = std::fs::remove_file(&part);
    let made = (|| {
        let source = Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        source.execute("VACUUM INTO ?1", [part.display().to_string()])?;
        drop(source);
        if !secret_keys {
            let copy = Connection::open(&part)?;
            copy.pragma_update(None, "secure_delete", true)?;
            let has_keys: bool = copy.query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE name = 'pgp_keys')",
                [],
                |r| r.get(0),
            )?;
            let has_certs: bool = copy.query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE name = 'smime_certs')",
                [],
                |r| r.get(0),
            )?;
            if has_keys {
                copy.execute("UPDATE pgp_keys SET secret = NULL", [])?;
            }
            if has_certs {
                copy.execute("UPDATE smime_certs SET secret = NULL", [])?;
            }
            if has_keys || has_certs {
                copy.execute_batch("VACUUM")?;
            }
        }
        inspect(&part)?; // it opens, and it is a mail base
        std::fs::rename(&part, &dest).map_err(file_error)
    })();
    if let Err(e) = made {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let bytes = std::fs::metadata(&dest).map_err(file_error)?.len();
    Ok(BackupFile {
        path: dest.display().to_string(),
        made: at,
        bytes,
    })
}

/// Remove all but the newest `keep` backups in `dir`. Only Telex's own backup files go.
/// Returns how many were removed.
pub fn prune(dir: &Path, keep: usize) -> usize {
    list(dir)
        .into_iter()
        .skip(keep.max(1))
        .filter(|b| std::fs::remove_file(&b.path).is_ok())
        .count()
}

/// What the file at `path` holds, opened read-only. Errors when it isn't a Telex mail base, or
/// is one from a newer Telex.
pub fn inspect(path: &Path) -> Result<BackupInfo, StoreError> {
    let not_ours = || {
        StoreError::Invalid(tr!(
            "This file isn't a Telex backup.",
            "Ten plik nie jest kopią zapasową Telexa."
        ))
    };
    let meta = std::fs::metadata(path).map_err(file_error)?;
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| not_ours())?;
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| not_ours())?;
    if version > MIGRATIONS.len() as i64 {
        return Err(StoreError::Invalid(tr!(
            "This backup comes from a newer Telex. Update Telex to restore it.",
            "Ta kopia pochodzi z nowszego Telexa. Zaktualizuj Telexa, żeby ją przywrócić."
        )));
    }
    let messages: i64 = db
        .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
        .map_err(|_| not_ours())?;
    let mut stmt = db
        .prepare("SELECT email FROM accounts WHERE imap_host IS NOT NULL ORDER BY position, id")
        .map_err(|_| not_ours())?;
    let accounts = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    let on_this_pc: i64 = db.query_row(
        "SELECT COUNT(*) FROM messages WHERE folder >= ?1 || '/' AND folder < ?1 || '0'",
        [LOCAL_ID],
        |r| r.get(0),
    )?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let made = made_of(&name).or_else(|| {
        meta.modified()
            .ok()
            .map(|t| chrono::DateTime::<chrono::Local>::from(t).naive_local())
    });
    Ok(BackupInfo {
        path: path.display().to_string(),
        made,
        bytes: meta.len(),
        accounts,
        messages: usize::try_from(messages).unwrap_or_default(),
        on_this_pc: usize::try_from(on_this_pc).unwrap_or_default(),
    })
}

/// "telex.db" → "telex.db-wal": SQLite's journal and shared memory next to a mail base.
fn sidecar(db: &Path, suffix: &str) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Put the backup `from` in place of the mail base `db`, which must be closed meanwhile (no
/// connection to it open). The mail base as it was is kept next to it as
/// "telex-before-restore-<time>.db"; the copy goes in only once it opens, is brought up to
/// this Telex's schema and is made safe to sync. Returns where the old one went.
pub fn restore(db: &Path, from: &Path, at: NaiveDateTime) -> Result<PathBuf, StoreError> {
    inspect(from)?;
    let dir = db.parent().unwrap_or(Path::new("."));
    let incoming = dir.join("telex-restoring.db");
    for path in [
        incoming.clone(),
        sidecar(&incoming, "-wal"),
        sidecar(&incoming, "-shm"),
    ] {
        let _ = std::fs::remove_file(path);
    }
    std::fs::copy(from, &incoming).map_err(file_error)?;
    let ready = (|| {
        let mut store = MailStore::open(&incoming)?;
        store.after_restore()?;
        store.checkpoint()
    })();
    if let Err(e) = ready {
        let _ = std::fs::remove_file(&incoming);
        return Err(e);
    }
    let aside = dir.join(format!("telex-before-restore-{}.db", at.format(STAMP)));
    if db.exists() {
        std::fs::rename(db, &aside).map_err(file_error)?;
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::rename(sidecar(db, suffix), sidecar(&aside, suffix));
        }
    }
    if let Err(e) = std::fs::rename(&incoming, db) {
        // Put the old one back rather than leave no mail base at all.
        let _ = std::fs::rename(&aside, db);
        return Err(file_error(e));
    }
    Ok(aside)
}

impl MailStore {
    /// Make a restored mail base safe to meet the servers again. What it was going to tell
    /// them is old news by now: its queued changes are dropped, so at the next check the
    /// servers' state wins (flags, deletions, moves made elsewhere since); mail whose move
    /// hadn't reached the server goes back where the server has it; and mail waiting in an
    /// Outbox moves to Drafts, since it may have gone out after the backup was made. A POP3
    /// account's next check is like its first: what came after the backup and is still on the
    /// server comes down again as old mail, without rules, replies or notifications.
    pub fn after_restore(&mut self) -> Result<usize, StoreError> {
        let outboxes: Vec<(String, Option<String>)> = self
            .accounts()
            .iter()
            .filter_map(|a| {
                let outbox = a.folder_of_kind(FolderKind::Outbox)?;
                let drafts = a
                    .folder_of_kind(FolderKind::Drafts)
                    .map(|f| a.folder_key(&f.id));
                Some((a.folder_key(&outbox.id), drafts))
            })
            .collect();
        let note = tr!(
            "Restored from a backup, in which this was waiting to be sent. It may have gone out \
             since: check Sent before you send it again.",
            "Przywrócono z kopii zapasowej, w której ta wiadomość czekała na wysłanie. Mogła już \
             wyjść: sprawdź Wysłane, zanim wyślesz ją ponownie."
        );
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM pending_ops", [])?;
        tx.execute(
            "UPDATE messages SET folder = srv_folder
             WHERE srv_folder IS NOT NULL AND folder <> srv_folder",
            [],
        )?;
        tx.execute("DELETE FROM outbox", [])?;
        tx.execute(
            "UPDATE folders SET last_uid = NULL WHERE kind = 'inbox'
               AND account IN (SELECT id FROM accounts WHERE incoming = 'pop3')",
            [],
        )?;
        let mut held = 0;
        for (outbox, drafts) in &outboxes {
            if let Some(drafts) = drafts {
                held += tx.execute(
                    "UPDATE messages SET folder = ?2, note = ?3 WHERE folder = ?1",
                    params![outbox, drafts, note],
                )?;
            } else {
                tx.execute(
                    "UPDATE messages SET note = ?2 WHERE folder = ?1",
                    params![outbox, note],
                )?;
            }
        }
        tx.commit()?;
        self.reload_accounts()?;
        Ok(held)
    }

    /// The mail base's pages: how many, how many of them are free, and how large one is.
    pub fn page_counts(&self) -> Result<(u64, u64, u64), StoreError> {
        let get = |name: &str| -> Result<u64, StoreError> {
            let v: i64 = self.db.pragma_query_value(None, name, |r| r.get(0))?;
            Ok(u64::try_from(v).unwrap_or_default())
        };
        Ok((
            get("page_count")?,
            get("freelist_count")?,
            get("page_size")?,
        ))
    }

    /// Write everything in the journal into the file itself, so the file alone is the whole
    /// mail base.
    pub fn checkpoint(&self) -> Result<(), StoreError> {
        self.db
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .optional()?;
        Ok(())
    }

    /// Give the space of removed mail back to the disk (SQLite's VACUUM). Takes a while on a
    /// large mail base, and the store is busy meanwhile. Returns the file's size before and
    /// after, in bytes.
    pub fn compact(&self) -> Result<(u64, u64), StoreError> {
        let size = |db: &Connection| -> Result<u64, StoreError> {
            let pages: i64 = db.pragma_query_value(None, "page_count", |r| r.get(0))?;
            let page: i64 = db.pragma_query_value(None, "page_size", |r| r.get(0))?;
            Ok(u64::try_from(pages * page).unwrap_or_default())
        };
        let before = size(&self.db)?;
        self.db.execute_batch("VACUUM")?;
        self.checkpoint()?;
        Ok((before, size(&self.db)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSetup, Folder, OutgoingDraft};
    use crate::store::SyncedMessage;
    use chrono::NaiveDate;

    fn at(day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, 30, 0)
            .unwrap()
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "telex-backup-test-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A mail base on disk with one server account, an Inbox and a message in it.
    fn base(dir: &Path) -> (PathBuf, MailStore) {
        let path = dir.join("telex.db");
        let mut store = MailStore::open(&path).unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
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
                    folder("drafts", FolderKind::Drafts),
                    folder("archive", FolderKind::Archive),
                ],
            )
            .unwrap();
        let raw =
            b"From: Anna <anna@x.pl>\r\nSubject: Wycena\r\nMessage-ID: <w@x.pl>\r\n\r\nTekst\r\n"
                .to_vec();
        let message = crate::mime::parse_message(&raw, "twojastudio/inbox", None);
        store
            .add_synced(
                "twojastudio/inbox",
                &[SyncedMessage {
                    uid: 4,
                    message,
                    raw,
                }],
            )
            .unwrap();
        (path, store)
    }

    #[test]
    fn backups_are_whole_files_named_by_time_and_the_oldest_go() {
        let dir = temp_dir();
        let (db, mut store) = base(&dir);
        let id = store.list("twojastudio/inbox").unwrap()[0].id;
        store.move_to(id, "local/archive").unwrap(); // mail only this PC has
        let backups = dir.join("Kopie");
        assert!(
            due(&backups, Some(TimeDelta::days(1)), at(1, 9)),
            "none yet"
        );
        assert!(!due(&backups, None, at(1, 9)), "off");
        let first = write(&db, &backups, at(1, 9)).unwrap();
        assert!(
            first.path.ends_with("telex-backup-2026-10-01-093000.db"),
            "{}",
            first.path
        );
        // Mail written while Telex runs: the store's own connection keeps going.
        store.set_flagged(id, true).unwrap();
        let info = inspect(Path::new(&first.path)).unwrap();
        assert_eq!(info.accounts, ["biuro@twojastudio.pl"]);
        assert_eq!((info.messages, info.on_this_pc), (1, 1));
        assert_eq!(info.made, Some(at(1, 9)));
        // A second one in the same second gets the next second's name.
        let again = write(&db, &backups, at(1, 9)).unwrap();
        assert!(again.path.ends_with("093001.db"));
        assert!(
            !due(&backups, Some(TimeDelta::days(1)), at(1, 23)),
            "not yet a day"
        );
        assert!(
            due(&backups, Some(TimeDelta::days(1)), at(2, 9)),
            "a day later"
        );
        for day in 2..=7 {
            write(&db, &backups, at(day, 9)).unwrap();
        }
        std::fs::write(backups.join("notatki.txt"), "mine").unwrap();
        std::fs::write(backups.join("telex-backup-zly.db"), "not a date").unwrap();
        assert_eq!(list(&backups).len(), 8);
        assert_eq!(prune(&backups, 5), 3);
        let kept = list(&backups);
        assert_eq!(kept.len(), 5);
        assert_eq!(kept[0].made, at(7, 9), "newest first");
        assert!(
            backups.join("notatki.txt").exists() && backups.join("telex-backup-zly.db").exists(),
            "only Telex's own backups go"
        );
        assert!(!list(&backups).iter().any(|b| b.path.ends_with(".part")));
        drop(store);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn secret_keys_go_into_backups_only_when_the_user_wants() {
        let dir = temp_dir();
        let (db, mut store) = base(&dir);
        let key = store
            .generate_key(
                "Studio",
                "biuro@twojastudio.pl",
                "hasło",
                crate::openpgp::KeyKind::Ed25519,
                at(1, 8),
            )
            .unwrap();
        let (public, secret): (Vec<u8>, Vec<u8>) = store
            .db
            .query_row("SELECT public, secret FROM pgp_keys", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        // Pieces of the secret part that its public part doesn't have (the row may be split
        // over pages, so pieces, not the whole).
        let has = |hay: &[u8], piece: &[u8]| hay.windows(piece.len()).any(|w| w == piece);
        let pieces: Vec<&[u8]> = secret
            .chunks(24)
            .filter(|p| p.len() == 24 && !has(&public, p))
            .collect();
        assert!(pieces.len() > 4);
        let contains = |hay: &[u8]| pieces.iter().any(|p| has(hay, p));
        let with = write_keys(&db, &dir.join("z"), at(1, 9), true).unwrap();
        assert!(contains(&std::fs::read(&with.path).unwrap()));
        let without = write_keys(&db, &dir.join("bez"), at(1, 9), false).unwrap();
        let copy = std::fs::read(&without.path).unwrap();
        assert!(!contains(&copy), "not even in free pages");
        // The public part stays, so signatures still verify after a restore.
        let backup = Connection::open(&without.path).unwrap();
        let (public, secret): (i64, i64) = backup
            .query_row(
                "SELECT COUNT(public), COUNT(secret) FROM pgp_keys WHERE fingerprint = ?1",
                [&key.fingerprint],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((public, secret), (1, 0));
        assert!(
            store.export_secret_key(&key.fingerprint).is_ok(),
            "the base keeps it"
        );
        drop(backup);
        drop(store);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_restored_base_lets_the_servers_win_and_sends_nothing_twice() {
        let dir = temp_dir();
        let (db, mut store) = base(&dir);
        let id = store.list("twojastudio/inbox").unwrap()[0].id;
        // In the backup: a move the server hasn't heard of, a flag, and mail waiting to go.
        store.move_to(id, "twojastudio/archive").unwrap();
        store.set_flagged(id, true).unwrap();
        let draft = OutgoingDraft {
            account: "twojastudio".into(),
            to: "anna@x.pl".into(),
            subject: "Oferta".into(),
            body: "Treść".into(),
            ..Default::default()
        };
        let account = store.account("twojastudio").unwrap().clone();
        let composed =
            crate::smtp::compose(&account, &draft, &Default::default(), chrono::Local::now())
                .unwrap();
        store
            .queue_outgoing(
                "twojastudio",
                &composed.message,
                &composed.raw,
                &composed.env_from,
                &composed.env_to,
                None,
                None,
            )
            .unwrap();
        let backup = write(&db, &dir.join("kopie"), at(3, 8)).unwrap();
        // After the backup: more mail, kept only in the base that is replaced.
        store.create_local_folder("Po kopii").unwrap();
        drop(store); // Telex lets go of the file first

        let aside = restore(&db, Path::new(&backup.path), at(5, 10)).unwrap();
        assert!(aside.ends_with("telex-before-restore-2026-10-05-103000.db"));
        assert!(aside.exists(), "the mail base as it was is kept");
        let mut store = MailStore::open(&db).unwrap();
        assert!(store
            .this_pc()
            .unwrap()
            .folders
            .iter()
            .all(|f| f.name != "Po kopii"));
        assert!(
            !store.has_pending_changes("twojastudio").unwrap(),
            "nothing old goes out"
        );
        assert_eq!(
            store.get(id).unwrap().folder,
            "twojastudio/inbox",
            "where the server has it"
        );
        assert!(store.list("twojastudio/outbox").unwrap().is_empty());
        let drafts = store.list("twojastudio/drafts").unwrap();
        assert_eq!(drafts.len(), 1);
        assert!(store
            .get(drafts[0].id)
            .unwrap()
            .note
            .unwrap()
            .contains("check Sent"));
        assert!(!store.has_outgoing("twojastudio", at(30, 23)).unwrap());
        // It is a normal mail base again.
        store.set_unread(id, false).unwrap();
        assert!(store.has_pending_changes("twojastudio").unwrap());
        drop(store);

        // Not a backup: nothing changes.
        let other = dir.join("notatka.db");
        std::fs::write(&other, "hello").unwrap();
        assert!(restore(&db, &other, at(6, 10)).is_err());
        assert!(MailStore::open(&db).unwrap().get(id).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn compacting_gives_the_space_back() {
        let dir = temp_dir();
        let (_, mut store) = base(&dir);
        let big = vec![b'x'; 2_000_000];
        let mut message =
            crate::mime::parse_message(b"Subject: Big\r\n\r\nx", "twojastudio/inbox", None);
        message.body = String::from_utf8(big.clone()).unwrap();
        store
            .add_synced(
                "twojastudio/inbox",
                &[SyncedMessage {
                    uid: 9,
                    message,
                    raw: big,
                }],
            )
            .unwrap();
        let id = store
            .list("twojastudio/inbox")
            .unwrap()
            .iter()
            .find(|m| m.subject == "Big")
            .unwrap()
            .id;
        store.move_to(id, "local/archive").unwrap();
        store.delete(id).unwrap();
        store.delete(id).unwrap();
        let (before, after) = store.compact().unwrap();
        assert!(before > after + 1_000_000, "{before} → {after}");
        drop(store);
        let _ = std::fs::remove_dir_all(dir);
    }
}
