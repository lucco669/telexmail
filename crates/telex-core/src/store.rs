use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use chrono::NaiveDateTime;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::model::{
    split_folder_key, Account, AccountSetup, Folder, FolderKind, ImapSettings, Message, MessageId,
    MessageSummary, OutgoingDraft,
};

const SCHEMA: &str = include_str!("schema.sql");
/// Bump together with schema.sql, and add a migration in `MailStore::with_connection`.
const SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    UnknownMessage(MessageId),
    UnknownAccount(String),
    UnknownFolder(String),
    /// Moving a message between accounts needs an upload to the other server, which isn't supported yet.
    CrossAccountMove {
        from: String,
        to: String,
    },
    /// The account has no folder of the kind the operation needs (e.g. no Trash).
    MissingFolder {
        account: String,
        kind: FolderKind,
    },
    /// The message is a copy of mail on a server, and Telex doesn't change the server yet.
    ReadOnly {
        account: String,
    },
    /// SQLite failed: disk full, file locked or damaged.
    Database(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMessage(id) => write!(f, "message {id} does not exist"),
            Self::UnknownAccount(a) => write!(f, "account \"{a}\" does not exist"),
            Self::UnknownFolder(k) => write!(f, "folder \"{k}\" does not exist"),
            Self::CrossAccountMove { from, to } => {
                write!(
                    f,
                    "can't move a message from {from} to {to}: they belong to different accounts"
                )
            }
            Self::MissingFolder { account, kind } => {
                write!(f, "account \"{account}\" has no {kind:?} folder")
            }
            Self::ReadOnly { account } => write!(
                f,
                "Telex only reads {account} for now. Moving and deleting server mail comes later."
            ),
            Self::Database(e) => write!(f, "the mail database failed: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteOutcome {
    /// The message is now in the account's Trash and can be moved back.
    MovedToTrash,
    /// The message was already in Trash and is gone for good.
    Removed,
}

/// Everything the UI needs to draw the folder tree and message lists.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub accounts: Vec<Account>,
    pub messages: Vec<MessageSummary>,
    /// This is the built-in sample mailbox, not the user's mail.
    pub sample: bool,
}

/// Where the last sync of one folder stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FolderSyncState {
    pub uid_validity: Option<u32>,
    /// Highest UID downloaded so far; `None` before the first sync.
    pub last_uid: Option<u32>,
}

/// A message downloaded from the server, ready to be stored.
#[derive(Debug, Clone)]
pub struct SyncedMessage {
    pub uid: u32,
    pub message: Message,
    /// The message exactly as the server sent it.
    pub raw: Vec<u8>,
}

/// The local mail base: accounts, folders and messages in SQLite.
///
/// The public API is what the UI talks to. Accounts and folders are few, so they are
/// also kept in memory; messages are always read from the database.
#[derive(Debug)]
pub struct MailStore {
    db: Connection,
    accounts: Vec<Account>,
    sample: bool,
    /// Sample messages waiting for "Get mail". Always empty for real accounts.
    incoming: Vec<Message>,
}

impl MailStore {
    /// Open the mail database at `path`, creating it if needed.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let db = Connection::open(path)?;
        // WAL lets the UI read while a sync writes, and survives crashes well.
        db.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        Self::with_connection(db, false)
    }

    /// An empty store that lives only in memory.
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::with_connection(Connection::open_in_memory()?, false)
    }

    fn with_connection(db: Connection, sample: bool) -> Result<Self, StoreError> {
        db.pragma_update(None, "foreign_keys", true)?;
        let version: i32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        match version {
            0 => {
                let tx = db.unchecked_transaction()?;
                tx.execute_batch(SCHEMA)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            SCHEMA_VERSION => {}
            v => {
                return Err(StoreError::Database(format!(
                    "it was made by a newer version of Telex (schema {v})"
                )))
            }
        }
        let accounts = load_accounts(&db)?;
        Ok(Self {
            db,
            accounts,
            sample,
            incoming: Vec::new(),
        })
    }

    /// The sample mailbox: built in memory from fixed data and never saved.
    pub fn new(
        accounts: Vec<Account>,
        messages: Vec<Message>,
        incoming: Vec<Message>,
    ) -> Result<Self, StoreError> {
        let mut store = Self::with_connection(Connection::open_in_memory()?, true)?;
        let tx = store.db.transaction()?;
        for (pos, a) in accounts.iter().enumerate() {
            insert_account(&tx, a, pos)?;
            for (fpos, f) in a.folders.iter().enumerate() {
                insert_folder(&tx, &a.id, f, fpos)?;
            }
        }
        for m in &messages {
            insert_message(&tx, &m.folder, m, None, None)?;
        }
        tx.commit()?;
        store.reload_accounts()?;
        store.incoming = incoming;
        Ok(store)
    }

    /// True for the built-in sample mailbox.
    pub fn is_sample(&self) -> bool {
        self.sample
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    pub fn account(&self, id: &str) -> Result<&Account, StoreError> {
        self.accounts
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| StoreError::UnknownAccount(id.to_owned()))
    }

    fn reload_accounts(&mut self) -> Result<(), StoreError> {
        self.accounts = load_accounts(&self.db)?;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<Snapshot, StoreError> {
        let mut stmt = self
            .db
            .prepare_cached(&format!("SELECT {SUMMARY_COLUMNS} FROM messages"))?;
        let messages = stmt
            .query_map([], summary_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Snapshot {
            accounts: self.accounts.clone(),
            messages,
            sample: self.sample,
        })
    }

    /// Messages in one folder, newest first.
    pub fn list(&self, folder: &str) -> Result<Vec<MessageSummary>, StoreError> {
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM messages WHERE folder = ?1 ORDER BY date DESC, id DESC"
        ))?;
        let rows = stmt
            .query_map([folder], summary_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn unread_count(&self, folder: &str) -> Result<usize, StoreError> {
        let n: i64 = self.db.query_row(
            "SELECT COUNT(*) FROM messages WHERE folder = ?1 AND unread",
            [folder],
            |r| r.get(0),
        )?;
        Ok(usize::try_from(n).unwrap_or_default())
    }

    pub fn get(&self, id: MessageId) -> Result<Message, StoreError> {
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT {MESSAGE_COLUMNS} FROM messages WHERE id = ?1"
        ))?;
        stmt.query_row([id as i64], message_from_row)
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))
    }

    /// Folder key and IMAP UID of a message.
    fn locate(&self, id: MessageId) -> Result<(String, Option<u32>), StoreError> {
        self.db
            .query_row(
                "SELECT folder, uid FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))
    }

    /// Server mail can't be moved or deleted until Telex learns to change the server.
    fn check_writable(&self, folder: &str, uid: Option<u32>) -> Result<(), StoreError> {
        if uid.is_none() {
            return Ok(());
        }
        let account = split_folder_key(folder).map_or(folder, |(a, _)| a);
        let email = self
            .account(account)
            .map_or_else(|_| account.to_owned(), |a| a.email.clone());
        Err(StoreError::ReadOnly { account: email })
    }

    /// Check that "<account>/<folder>" exists and return the account id.
    fn check_folder<'k>(&self, key: &'k str) -> Result<&'k str, StoreError> {
        let unknown = || StoreError::UnknownFolder(key.to_owned());
        let (acct, folder) = split_folder_key(key).ok_or_else(unknown)?;
        let account = self.account(acct).map_err(|_| unknown())?;
        if account.folders.iter().any(|f| f.id == folder) {
            Ok(acct)
        } else {
            Err(unknown())
        }
    }

    fn folder_kind(&self, key: &str) -> Option<FolderKind> {
        let (acct, folder) = split_folder_key(key)?;
        let account = self.accounts.iter().find(|a| a.id == acct)?;
        account
            .folders
            .iter()
            .find(|f| f.id == folder)
            .map(|f| f.kind)
    }

    fn folder_key_of_kind(&self, account: &str, kind: FolderKind) -> Result<String, StoreError> {
        let a = self.account(account)?;
        a.folder_of_kind(kind)
            .map(|f| a.folder_key(&f.id))
            .ok_or_else(|| StoreError::MissingFolder {
                account: account.to_owned(),
                kind,
            })
    }

    /// Read state and flags change only the local copy for now; the server keeps its own.
    pub fn set_unread(&mut self, id: MessageId, unread: bool) -> Result<(), StoreError> {
        self.set_column(id, "unread", unread)
    }

    pub fn set_flagged(&mut self, id: MessageId, flagged: bool) -> Result<(), StoreError> {
        self.set_column(id, "flagged", flagged)
    }

    fn set_column(&mut self, id: MessageId, column: &str, value: bool) -> Result<(), StoreError> {
        let changed = self.db.execute(
            &format!("UPDATE messages SET {column} = ?1 WHERE id = ?2"),
            params![value, id as i64],
        )?;
        if changed == 0 {
            Err(StoreError::UnknownMessage(id))
        } else {
            Ok(())
        }
    }

    /// Move a message to another folder of the same account.
    pub fn move_to(&mut self, id: MessageId, folder: &str) -> Result<(), StoreError> {
        let target_acct = self.check_folder(folder)?.to_owned();
        let (source, uid) = self.locate(id)?;
        let source_acct = split_folder_key(&source)
            .map(|(a, _)| a)
            .unwrap_or_default();
        if source_acct != target_acct {
            return Err(StoreError::CrossAccountMove {
                from: source,
                to: folder.to_owned(),
            });
        }
        self.check_writable(&source, uid)?;
        self.db.execute(
            "UPDATE messages SET folder = ?1 WHERE id = ?2",
            params![folder, id as i64],
        )?;
        Ok(())
    }

    /// Delete like a mail client does: first to Trash, and from Trash for good.
    pub fn delete(&mut self, id: MessageId) -> Result<DeleteOutcome, StoreError> {
        let (folder, uid) = self.locate(id)?;
        self.check_writable(&folder, uid)?;
        if self.folder_kind(&folder) == Some(FolderKind::Trash) {
            self.db
                .execute("DELETE FROM messages WHERE id = ?1", [id as i64])?;
            return Ok(DeleteOutcome::Removed);
        }
        let account = split_folder_key(&folder)
            .map(|(a, _)| a.to_owned())
            .unwrap_or_default();
        let trash = self.folder_key_of_kind(&account, FolderKind::Trash)?;
        self.db.execute(
            "UPDATE messages SET folder = ?1 WHERE id = ?2",
            params![trash, id as i64],
        )?;
        Ok(DeleteOutcome::MovedToTrash)
    }

    /// Store a message the user wrote, in Sent or Drafts of the sending account.
    /// Sending over SMTP is not wired up yet, so "Sent" only files the message.
    pub fn save_outgoing(
        &mut self,
        draft: OutgoingDraft,
        kind: FolderKind,
        now: NaiveDateTime,
    ) -> Result<MessageId, StoreError> {
        debug_assert!(kind.is_outgoing(), "outgoing mail goes to Sent or Drafts");
        let account = self.account(&draft.account)?.clone();
        let folder = self.folder_key_of_kind(&account.id, kind)?;
        let attachments_kb: u32 = draft.attachments.iter().map(|a| a.size_kb).sum();
        let body_kb = u32::try_from(draft.body.len().div_ceil(1024)).unwrap_or(u32::MAX);
        let message = Message {
            id: 0,
            folder: folder.clone(),
            from_name: account.name.clone(),
            from_email: account.email.clone(),
            to: draft.to,
            cc: draft.cc,
            subject: draft.subject,
            date: now,
            size_kb: (body_kb + attachments_kb).max(1),
            unread: false,
            flagged: false,
            remote_images: false,
            note: None,
            attachments: draft.attachments,
            body: draft.body,
        };
        insert_message(&self.db, &folder, &message, None, None)
    }

    /// Deliver waiting sample messages and return their ids.
    /// Real accounts get new mail through `imap::sync_account` instead.
    pub fn fetch_new(&mut self) -> Result<Vec<MessageId>, StoreError> {
        let arrived = std::mem::take(&mut self.incoming);
        let tx = self.db.transaction()?;
        let ids = arrived
            .iter()
            .map(|m| insert_message(&tx, &m.folder, m, None, None))
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit()?;
        Ok(ids)
    }

    /// Full-text search over subject, people and body. Every word must match, as a
    /// prefix, ignoring case and Polish diacritics. Best matches first.
    pub fn search(&self, query: &str) -> Result<Vec<MessageId>, StoreError> {
        let q = fts_query(query);
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self.db.prepare_cached(
            "SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?1 ORDER BY rank LIMIT 5000",
        )?;
        let ids = stmt
            .query_map([q], |r| r.get::<_, i64>(0))?
            .map(|r| r.map(|id| id as MessageId))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    // ---- Accounts and sync ----

    /// Add an account from the setup dialog, or update the one with the same address.
    /// Updating is how the user fixes a server name or a changed password.
    pub fn upsert_account(&mut self, setup: &AccountSetup) -> Result<Account, StoreError> {
        let existing = self
            .accounts
            .iter()
            .find(|a| a.email.eq_ignore_ascii_case(&setup.email))
            .map(|a| a.id.clone());
        let id = if let Some(id) = existing {
            self.db.execute(
                "UPDATE accounts SET name = ?2, email = ?3, imap_host = ?4, imap_port = ?5, imap_user = ?6
                 WHERE id = ?1",
                params![id, setup.name, setup.email, setup.host, setup.port, setup.username],
            )?;
            id
        } else {
            let id = self.unique_account_id(&setup.email);
            self.db.execute(
                "INSERT INTO accounts (id, position, name, email, imap_host, imap_port, imap_user)
                 VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM accounts), ?2, ?3, ?4, ?5, ?6)",
                params![id, setup.name, setup.email, setup.host, setup.port, setup.username],
            )?;
            id
        };
        self.reload_accounts()?;
        self.account(&id).cloned()
    }

    /// An id for folder keys, from the email's domain: "biuro@twojastudio.pl" → "twojastudio".
    fn unique_account_id(&self, email: &str) -> String {
        let domain = email.rsplit_once('@').map_or(email, |(_, d)| d);
        let base = match slug(domain.split('.').next().unwrap_or(domain)) {
            s if s.is_empty() => "account".to_owned(),
            s => s,
        };
        let mut id = base.clone();
        let mut n = 2;
        while self.accounts.iter().any(|a| a.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    /// Remove an account and every local copy of its mail. The server is not touched.
    pub fn remove_account(&mut self, id: &str) -> Result<(), StoreError> {
        self.account(id)?;
        let tx = self.db.transaction()?;
        tx.execute(
            "DELETE FROM messages WHERE substr(folder, 1, length(?1) + 1) = ?1 || '/'",
            [id],
        )?;
        tx.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        tx.commit()?;
        self.reload_accounts()
    }

    /// Replace an account's folder list with what the server has now. Folders that
    /// disappeared from the server are dropped together with their local copies.
    pub fn set_folders(&mut self, account: &str, folders: &[Folder]) -> Result<(), StoreError> {
        self.account(account)?;
        let keep: HashSet<&str> = folders.iter().map(|f| f.id.as_str()).collect();
        let tx = self.db.transaction()?;
        let existing: Vec<String> = {
            let mut stmt = tx.prepare("SELECT id FROM folders WHERE account = ?1")?;
            let ids = stmt
                .query_map([account], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            ids
        };
        for gone in existing.iter().filter(|id| !keep.contains(id.as_str())) {
            tx.execute(
                "DELETE FROM messages WHERE folder = ?1",
                [format!("{account}/{gone}")],
            )?;
            tx.execute(
                "DELETE FROM folders WHERE account = ?1 AND id = ?2",
                [account, gone],
            )?;
        }
        for (pos, f) in folders.iter().enumerate() {
            tx.execute(
                "INSERT INTO folders (account, id, position, name, kind, remote)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (account, id) DO UPDATE SET
                   position = excluded.position, name = excluded.name,
                   kind = excluded.kind, remote = excluded.remote",
                params![account, f.id, pos as i64, f.name, f.kind.as_str(), f.remote],
            )?;
        }
        tx.commit()?;
        self.reload_accounts()
    }

    pub fn folder_sync_state(&self, folder_key: &str) -> Result<FolderSyncState, StoreError> {
        let (account, folder) = split_folder_key(folder_key)
            .ok_or_else(|| StoreError::UnknownFolder(folder_key.to_owned()))?;
        self.db
            .query_row(
                "SELECT uidvalidity, last_uid FROM folders WHERE account = ?1 AND id = ?2",
                [account, folder],
                |r| {
                    Ok(FolderSyncState {
                        uid_validity: r.get(0)?,
                        last_uid: r.get(1)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| StoreError::UnknownFolder(folder_key.to_owned()))
    }

    /// Start syncing a folder whose UIDVALIDITY on the server is `uid_validity`. If that
    /// changed since last time, the server has renumbered the folder: the old copies are
    /// dropped so they get downloaded again.
    pub fn begin_folder_sync(
        &mut self,
        folder_key: &str,
        uid_validity: u32,
    ) -> Result<FolderSyncState, StoreError> {
        let state = self.folder_sync_state(folder_key)?;
        if state.uid_validity == Some(uid_validity) {
            return Ok(state);
        }
        let (account, folder) = split_folder_key(folder_key).unwrap_or_default();
        let tx = self.db.transaction()?;
        tx.execute(
            "DELETE FROM messages WHERE folder = ?1 AND uid IS NOT NULL",
            [folder_key],
        )?;
        tx.execute(
            "UPDATE folders SET uidvalidity = ?3, last_uid = NULL WHERE account = ?1 AND id = ?2",
            params![account, folder, uid_validity],
        )?;
        tx.commit()?;
        Ok(FolderSyncState {
            uid_validity: Some(uid_validity),
            last_uid: None,
        })
    }

    /// Drop local copies of messages that are no longer on the server. Returns how many.
    pub fn forget_vanished(
        &mut self,
        folder_key: &str,
        on_server: &HashSet<u32>,
    ) -> Result<usize, StoreError> {
        let local: Vec<(i64, u32)> = {
            let mut stmt = self
                .db
                .prepare("SELECT id, uid FROM messages WHERE folder = ?1 AND uid IS NOT NULL")?;
            let rows = stmt
                .query_map([folder_key], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?;
            rows
        };
        let gone: Vec<i64> = local
            .into_iter()
            .filter(|(_, uid)| !on_server.contains(uid))
            .map(|(id, _)| id)
            .collect();
        if !gone.is_empty() {
            let tx = self.db.transaction()?;
            for id in &gone {
                tx.execute("DELETE FROM messages WHERE id = ?1", [id])?;
            }
            tx.commit()?;
        }
        Ok(gone.len())
    }

    /// Store messages downloaded from one folder and remember the highest UID seen.
    /// A message already stored (same folder and UID) is skipped. Returns the new ids.
    pub fn add_synced(
        &mut self,
        folder_key: &str,
        batch: &[SyncedMessage],
    ) -> Result<Vec<MessageId>, StoreError> {
        let account = self.check_folder(folder_key)?;
        let folder = &folder_key[account.len() + 1..];
        let tx = self.db.transaction()?;
        let mut ids = Vec::new();
        for item in batch {
            let stored: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM messages WHERE folder = ?1 AND uid = ?2)",
                params![folder_key, item.uid],
                |r| r.get(0),
            )?;
            if !stored {
                ids.push(insert_message(
                    &tx,
                    folder_key,
                    &item.message,
                    Some(item.uid),
                    Some(&item.raw),
                )?);
            }
        }
        if let Some(max) = batch.iter().map(|m| m.uid).max() {
            tx.execute(
                "UPDATE folders SET last_uid = MAX(COALESCE(last_uid, 0), ?3)
                 WHERE account = ?1 AND id = ?2",
                params![account, folder, max],
            )?;
        }
        tx.commit()?;
        Ok(ids)
    }
}

/// Lowercase, letters and digits only, words joined by "-": "Faktury 2026" → "faktury-2026".
pub(crate) fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

/// Turn what the user typed into a safe FTS5 query: each word quoted, matched as a prefix.
/// ł is folded to l, as the index triggers in schema.sql do.
fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|w| w.replace('"', "").replace('ł', "l").replace('Ł', "L"))
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

const SUMMARY_COLUMNS: &str = "id, folder, from_name, from_email, to_addr, subject, date, \
     size_kb, unread, flagged, attachments <> '[]'";

fn summary_from_row(r: &Row<'_>) -> rusqlite::Result<MessageSummary> {
    Ok(MessageSummary {
        id: r.get::<_, i64>(0)? as MessageId,
        folder: r.get(1)?,
        from_name: r.get(2)?,
        from_email: r.get(3)?,
        to: r.get(4)?,
        subject: r.get(5)?,
        date: r.get(6)?,
        size_kb: r.get(7)?,
        unread: r.get(8)?,
        flagged: r.get(9)?,
        has_attachments: r.get(10)?,
    })
}

const MESSAGE_COLUMNS: &str = "id, folder, from_name, from_email, to_addr, cc, subject, date, \
     size_kb, unread, flagged, remote_images, note, attachments, body";

fn message_from_row(r: &Row<'_>) -> rusqlite::Result<Message> {
    let attachments: String = r.get(13)?;
    Ok(Message {
        id: r.get::<_, i64>(0)? as MessageId,
        folder: r.get(1)?,
        from_name: r.get(2)?,
        from_email: r.get(3)?,
        to: r.get(4)?,
        cc: r.get(5)?,
        subject: r.get(6)?,
        date: r.get(7)?,
        size_kb: r.get(8)?,
        unread: r.get(9)?,
        flagged: r.get(10)?,
        remote_images: r.get(11)?,
        note: r.get(12)?,
        attachments: serde_json::from_str(&attachments).unwrap_or_default(),
        body: r.get(14)?,
    })
}

fn load_accounts(db: &Connection) -> Result<Vec<Account>, StoreError> {
    let mut stmt = db.prepare(
        "SELECT id, name, email, imap_host, imap_port, imap_user FROM accounts ORDER BY position, id",
    )?;
    let mut accounts = stmt
        .query_map([], |r| {
            let imap = match r.get::<_, Option<String>>(3)? {
                Some(host) => Some(ImapSettings {
                    host,
                    port: r.get(4)?,
                    username: r.get(5)?,
                }),
                None => None,
            };
            Ok(Account {
                id: r.get(0)?,
                name: r.get(1)?,
                email: r.get(2)?,
                folders: Vec::new(),
                imap,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut stmt =
        db.prepare("SELECT account, id, name, kind, remote FROM folders ORDER BY position, id")?;
    let rows = stmt.query_map([], |r| {
        let kind: String = r.get(3)?;
        Ok((
            r.get::<_, String>(0)?,
            Folder {
                id: r.get(1)?,
                name: r.get(2)?,
                kind: FolderKind::from_name(&kind),
                remote: r.get(4)?,
            },
        ))
    })?;
    for row in rows {
        let (account, folder) = row?;
        if let Some(a) = accounts.iter_mut().find(|a| a.id == account) {
            a.folders.push(folder);
        }
    }
    Ok(accounts)
}

fn insert_account(db: &Connection, a: &Account, position: usize) -> Result<(), StoreError> {
    let imap = a.imap.as_ref();
    db.execute(
        "INSERT INTO accounts (id, position, name, email, imap_host, imap_port, imap_user)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            a.id,
            position as i64,
            a.name,
            a.email,
            imap.map(|i| &i.host),
            imap.map(|i| i.port),
            imap.map(|i| &i.username),
        ],
    )?;
    Ok(())
}

fn insert_folder(
    db: &Connection,
    account: &str,
    f: &Folder,
    position: usize,
) -> Result<(), StoreError> {
    db.execute(
        "INSERT INTO folders (account, id, position, name, kind, remote) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![account, f.id, position as i64, f.name, f.kind.as_str(), f.remote],
    )?;
    Ok(())
}

fn insert_message(
    db: &Connection,
    folder: &str,
    m: &Message,
    uid: Option<u32>,
    raw: Option<&[u8]>,
) -> Result<MessageId, StoreError> {
    let attachments =
        serde_json::to_string(&m.attachments).map_err(|e| StoreError::Database(e.to_string()))?;
    db.execute(
        "INSERT INTO messages (folder, uid, from_name, from_email, to_addr, cc, subject, date,
             size_kb, unread, flagged, remote_images, note, attachments, body, raw)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            folder,
            uid,
            m.from_name,
            m.from_email,
            m.to,
            m.cc,
            m.subject,
            m.date,
            m.size_kb,
            m.unread,
            m.flagged,
            m.remote_images,
            m.note,
            attachments,
            m.body,
            raw,
        ],
    )?;
    Ok(db.last_insert_rowid() as MessageId)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::sample_store;
    use chrono::NaiveDate;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 10)
            .unwrap()
            .and_hms_opt(7, 0, 0)
            .unwrap()
    }

    fn draft(account: &str) -> OutgoingDraft {
        OutgoingDraft {
            account: account.into(),
            to: "Anna Kowalska <anna@kowalska-architekci.pl>".into(),
            cc: String::new(),
            subject: "Re: Poprawki".into(),
            body: "Dzień dobry,\n\nwdrożę do środy.\n".into(),
            attachments: vec![],
        }
    }

    /// A store with one real (IMAP) account that has an Inbox, Sent and Trash.
    fn store_with_server_account() -> (MailStore, Account) {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                username: "biuro@twojastudio.pl".into(),
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
                remote_images: false,
                note: None,
                attachments: vec![],
                body: "Dzień dobry, przesyłam żółwia.".into(),
            },
            raw: b"Subject: x\r\n\r\nx".to_vec(),
        }
    }

    #[test]
    fn sample_mailbox_is_consistent() {
        let store = sample_store();
        assert!(store.is_sample());
        assert_eq!(store.accounts().len(), 2);
        let snap = store.snapshot().unwrap();
        assert!(snap.messages.len() >= 20);
        let ids: HashSet<_> = snap.messages.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), snap.messages.len(), "ids are unique");
        for m in &snap.messages {
            assert!(
                store.check_folder(&m.folder).is_ok(),
                "{} points to a real folder",
                m.folder
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
            .save_outgoing(draft("firma"), FolderKind::Sent, now())
            .unwrap();
        let msg = store.get(sent).unwrap();
        assert_eq!(msg.folder, "firma/sent");
        assert_eq!(msg.from_email, "biuro@twojastudio.pl");
        assert_eq!(msg.size_kb, 1);
        let saved = store
            .save_outgoing(draft("gmail"), FolderKind::Drafts, now())
            .unwrap();
        assert_eq!(store.get(saved).unwrap().folder, "gmail/drafts");
        assert!(store
            .save_outgoing(draft("nope"), FolderKind::Sent, now())
            .is_err());
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
    fn server_mail_is_read_only() {
        let (mut store, account) = store_with_server_account();
        let key = account.folder_key("inbox");
        let ids = store.add_synced(&key, &[synced(7, "Poprawki")]).unwrap();
        let id = ids[0];
        assert!(matches!(store.delete(id), Err(StoreError::ReadOnly { .. })));
        assert!(matches!(
            store.move_to(id, &account.folder_key("sent")),
            Err(StoreError::ReadOnly { .. })
        ));
        // Read state and flags are local, so they still work.
        store.set_unread(id, false).unwrap();
        store.set_flagged(id, true).unwrap();
        // Mail written here exists only here, so it can be deleted.
        let draft = store
            .save_outgoing(draft(&account.id), FolderKind::Sent, now())
            .unwrap();
        assert_eq!(store.delete(draft), Ok(DeleteOutcome::MovedToTrash));
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
    fn adding_the_same_address_updates_the_account() {
        let (mut store, account) = store_with_server_account();
        let again = store
            .upsert_account(&AccountSetup {
                name: "Biuro".into(),
                email: "BIURO@twojastudio.pl".into(),
                host: "imap.twojastudio.pl".into(),
                port: 993,
                username: "biuro".into(),
            })
            .unwrap();
        assert_eq!(again.id, account.id);
        assert_eq!(again.name, "Biuro");
        assert_eq!(again.imap.unwrap().host, "imap.twojastudio.pl");
        assert_eq!(again.folders.len(), 3, "folders are kept");
        assert_eq!(store.accounts().len(), 1);
    }

    #[test]
    fn removing_an_account_removes_its_mail() {
        let (mut store, account) = store_with_server_account();
        store
            .add_synced(&account.folder_key("inbox"), &[synced(1, "a")])
            .unwrap();
        store.remove_account(&account.id).unwrap();
        assert!(store.accounts().is_empty());
        assert!(store.snapshot().unwrap().messages.is_empty());
        assert!(store.search("zolwia").unwrap().is_empty());
    }

    #[test]
    fn folders_gone_from_the_server_are_dropped() {
        let (mut store, account) = store_with_server_account();
        store
            .add_synced(&account.folder_key("sent"), &[synced(1, "a")])
            .unwrap();
        let inbox = account.folders[0].clone();
        store.set_folders(&account.id, &[inbox]).unwrap();
        assert_eq!(store.account(&account.id).unwrap().folders.len(), 1);
        assert!(store.snapshot().unwrap().messages.is_empty());
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
                })
                .unwrap();
            assert_eq!(account.id, "twojastudio");
        }
        let store = MailStore::open(&path).unwrap();
        assert!(!store.is_sample());
        assert_eq!(store.accounts()[0].email, "biuro@twojastudio.pl");
        drop(store);
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
    }

    #[test]
    fn slugs_are_safe_ids() {
        assert_eq!(slug("INBOX.Faktury 2026"), "inbox-faktury-2026");
        assert_eq!(slug("  --Wysłane--  "), "wysłane");
        assert_eq!(slug("..."), "");
    }
}
