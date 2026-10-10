use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use chrono::NaiveDateTime;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::mime::parse_message_id;
use crate::model::{
    split_folder_key, Account, AccountSetup, Folder, FolderKind, ImapSettings, Message, MessageId,
    MessageSummary, OutgoingDraft, SmtpSecurity, SmtpSettings,
};

/// Schema steps, oldest first: step n brings a database to version n. A released step is
/// never edited; a change is a new file in migrations/ added here.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/001_base.sql"),
    include_str!("migrations/002_write_back.sql"),
    include_str!("migrations/003_sending.sql"),
    include_str!("migrations/004_trusted_senders.sql"),
];

/// Every server account's local Outbox has this id. `imap::plan_folders` never gives it
/// to a server folder.
pub const OUTBOX_ID: &str = "outbox";

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

/// What a queued change does on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Seen(bool),
    Flagged(bool),
    /// Move to this folder key of the same account.
    Move(String),
    /// Remove for good (Del in Trash).
    Expunge,
    /// Save a copy of a sent message in this server folder (IMAP APPEND).
    Append(String),
}

/// A message waiting in an Outbox, with what SMTP needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingMail {
    pub message: MessageId,
    pub account: String,
    /// The SMTP envelope: who it's from, and every recipient (To and Cc).
    pub env_from: String,
    pub env_to: Vec<String>,
    /// The message exactly as it will be sent.
    pub raw: Vec<u8>,
    pub subject: String,
    /// The To header, for "Sent to …".
    pub to: String,
    pub attempts: u32,
}

/// A change the user made that the server hasn't heard about yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingChange {
    pub op: i64,
    /// The local message; `None` for an expunge, whose message is already gone here.
    pub message: Option<MessageId>,
    /// Where the message is on the server: folder key and UID. The UID is `None` for a
    /// moved message whose new UID isn't known yet.
    pub folder: String,
    pub uid: Option<u32>,
    pub message_id: Option<String>,
    /// For error messages.
    pub subject: String,
    pub kind: ChangeKind,
    pub attempts: u32,
}

/// The local mail base: accounts, folders and messages in SQLite.
///
/// The public API is what the UI talks to. Accounts and folders are few, so they are
/// also kept in memory; messages are always read from the database.
///
/// A message from a server has a server location (`srv_folder`, `uid`) besides the folder
/// the UI shows. Changes to server mail are applied here at once and queued in
/// `pending_ops`; `imap::push_changes` sends them and reports back.
#[derive(Debug)]
pub struct MailStore {
    db: Connection,
    accounts: Vec<Account>,
    sample: bool,
    /// Sample messages waiting for "Get mail". Always empty for real accounts.
    incoming: Vec<Message>,
}

impl MailStore {
    /// Open the mail database at `path`, creating or upgrading it if needed.
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
        migrate(&db)?;
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
            insert_message(&tx, &m.folder, None, m, None, None)?;
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

    /// The folder the UI shows, and the server folder if the message lives on a server.
    fn locate(&self, id: MessageId) -> Result<(String, Option<String>), StoreError> {
        self.db
            .query_row(
                "SELECT folder, srv_folder FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))
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

    pub fn set_unread(&mut self, id: MessageId, unread: bool) -> Result<(), StoreError> {
        self.set_flag(id, "unread", "seen", unread, !unread)
    }

    pub fn set_flagged(&mut self, id: MessageId, flagged: bool) -> Result<(), StoreError> {
        self.set_flag(id, "flagged", "flagged", flagged, flagged)
    }

    /// Change a flag here and, for server mail, queue the change. Only the latest value of
    /// each flag is kept in the queue.
    fn set_flag(
        &mut self,
        id: MessageId,
        column: &str,
        op_kind: &str,
        value: bool,
        server_value: bool,
    ) -> Result<(), StoreError> {
        let (_, srv_folder) = self.locate(id)?;
        let tx = self.db.transaction()?;
        tx.execute(
            &format!("UPDATE messages SET {column} = ?1 WHERE id = ?2"),
            params![value, id as i64],
        )?;
        if let Some(srv) = srv_folder {
            tx.execute(
                "DELETE FROM pending_ops WHERE message = ?1 AND kind = ?2",
                params![id as i64, op_kind],
            )?;
            tx.execute(
                "INSERT INTO pending_ops (account, message, kind, value) VALUES (?1, ?2, ?3, ?4)",
                params![account_of(&srv), id as i64, op_kind, server_value],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Move a message to another folder of the same account.
    pub fn move_to(&mut self, id: MessageId, folder: &str) -> Result<(), StoreError> {
        let target_acct = self.check_folder(folder)?.to_owned();
        let (source, srv_folder) = self.locate(id)?;
        if account_of(&source) != target_acct {
            return Err(StoreError::CrossAccountMove {
                from: source,
                to: folder.to_owned(),
            });
        }
        let tx = self.db.transaction()?;
        tx.execute(
            "UPDATE messages SET folder = ?1 WHERE id = ?2",
            params![folder, id as i64],
        )?;
        // Leaving the Outbox means it's no longer to be sent.
        tx.execute("DELETE FROM outbox WHERE message = ?1", [id as i64])?;
        if let Some(srv) = srv_folder {
            // One move per message: moving again replaces it, moving back cancels it.
            tx.execute(
                "DELETE FROM pending_ops WHERE message = ?1 AND kind = 'move'",
                [id as i64],
            )?;
            if srv != folder {
                tx.execute(
                    "INSERT INTO pending_ops (account, message, kind, target) VALUES (?1, ?2, 'move', ?3)",
                    params![target_acct, id as i64, folder],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Delete like a mail client does: first to Trash, and from Trash for good.
    pub fn delete(&mut self, id: MessageId) -> Result<DeleteOutcome, StoreError> {
        let (folder, srv_folder) = self.locate(id)?;
        if self.folder_kind(&folder) != Some(FolderKind::Trash) {
            let trash = self.folder_key_of_kind(account_of(&folder), FolderKind::Trash)?;
            self.move_to(id, &trash)?;
            return Ok(DeleteOutcome::MovedToTrash);
        }
        let tx = self.db.transaction()?;
        if let Some(srv) = srv_folder {
            // The row goes now; the queued expunge remembers where the message is on the server.
            let (uid, message_id, subject): (Option<u32>, Option<String>, String) = tx.query_row(
                "SELECT uid, message_id, subject FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            tx.execute("DELETE FROM messages WHERE id = ?1", [id as i64])?;
            if uid.is_some() || message_id.is_some() {
                tx.execute(
                    "INSERT INTO pending_ops (account, kind, srv_folder, uid, message_id, subject)
                     VALUES (?1, 'expunge', ?2, ?3, ?4, ?5)",
                    params![account_of(&srv), srv, uid, message_id, subject],
                )?;
            }
        } else {
            tx.execute("DELETE FROM messages WHERE id = ?1", [id as i64])?;
        }
        tx.commit()?;
        Ok(DeleteOutcome::Removed)
    }

    /// Store a message the user wrote, in Drafts (or, for the sample mailbox, in Sent:
    /// it can't really send). A draft edited from another replaces it.
    pub fn save_outgoing(
        &mut self,
        draft: OutgoingDraft,
        kind: FolderKind,
        now: NaiveDateTime,
    ) -> Result<MessageId, StoreError> {
        debug_assert!(kind.is_outgoing(), "outgoing mail goes to Sent or Drafts");
        if let Some(old) = draft.replaces {
            self.remove_local(old)?;
        }
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
            message_id: None,
        };
        insert_message(&self.db, &folder, None, &message, None, None)
    }

    /// Delete a message that exists only here (a draft, an unsent message). Server mail
    /// and unknown ids are left alone: replacing never deletes anything on a server.
    fn remove_local(&mut self, id: MessageId) -> Result<(), StoreError> {
        self.db.execute(
            "DELETE FROM messages WHERE id = ?1 AND srv_folder IS NULL",
            [id as i64],
        )?;
        Ok(())
    }

    // ---- Sending ----

    /// Put a composed message in the account's Outbox. `raw` is the message exactly as it
    /// will be sent; `message` is its parsed form for the list and preview.
    pub fn queue_outgoing(
        &mut self,
        account: &str,
        message: &Message,
        raw: &[u8],
        env_from: &str,
        env_to: &[String],
        replaces: Option<MessageId>,
    ) -> Result<MessageId, StoreError> {
        let outbox = self.folder_key_of_kind(account, FolderKind::Outbox)?;
        if let Some(old) = replaces {
            self.remove_local(old)?;
        }
        let env_to =
            serde_json::to_string(env_to).map_err(|e| StoreError::Database(e.to_string()))?;
        let tx = self.db.transaction()?;
        let id = insert_message(&tx, &outbox, None, message, None, Some(raw))?;
        tx.execute(
            "INSERT INTO outbox (message, account, env_from, env_to) VALUES (?1, ?2, ?3, ?4)",
            params![id as i64, account, env_from, env_to],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// The oldest message after `after` waiting in the account's Outbox, skipping ones the
    /// server refused for good. Go through the Outbox by passing back the last id.
    pub fn next_outgoing(
        &self,
        account: &str,
        after: MessageId,
    ) -> Result<Option<OutgoingMail>, StoreError> {
        let outbox = format!("{account}/{OUTBOX_ID}");
        self.db
            .query_row(
                "SELECT o.message, o.env_from, o.env_to, m.raw, m.subject, m.to_addr, o.attempts
                 FROM outbox o JOIN messages m ON m.id = o.message
                 WHERE o.account = ?1 AND m.folder = ?2 AND NOT o.refused AND o.message > ?3
                   AND m.raw IS NOT NULL
                 ORDER BY o.message LIMIT 1",
                params![account, outbox, after as i64],
                |r| {
                    let env_to: String = r.get(2)?;
                    Ok(OutgoingMail {
                        message: r.get::<_, i64>(0)? as MessageId,
                        account: account.to_owned(),
                        env_from: r.get(1)?,
                        env_to: serde_json::from_str(&env_to).unwrap_or_default(),
                        raw: r.get(3)?,
                        subject: r.get(4)?,
                        to: r.get(5)?,
                        attempts: r.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// The SMTP server took the message: file it in Sent and queue a copy for the server's
    /// Sent folder. Without a Sent folder it stays in the Outbox, marked as sent.
    pub fn outgoing_sent(&mut self, id: MessageId, at: NaiveDateTime) -> Result<(), StoreError> {
        let (folder, _) = self.locate(id)?;
        let account = account_of(&folder).to_owned();
        let sent = self
            .account(&account)?
            .folder_of_kind(FolderKind::Sent)
            .map(|f| (f.id.clone(), f.remote.is_some()));
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM outbox WHERE message = ?1", [id as i64])?;
        match sent {
            Some((sent_id, on_server)) => {
                let key = format!("{account}/{sent_id}");
                tx.execute(
                    "UPDATE messages SET folder = ?1, date = ?2, note = NULL WHERE id = ?3",
                    params![key, at, id as i64],
                )?;
                if on_server {
                    tx.execute(
                        "INSERT INTO pending_ops (account, message, kind, target) VALUES (?1, ?2, 'append', ?3)",
                        params![account, id as i64, key],
                    )?;
                }
            }
            None => {
                tx.execute(
                    "UPDATE messages SET note = 'Sent. This account has no Sent folder, so it stays here.'
                     WHERE id = ?1",
                    [id as i64],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Sending failed. `refused`: the server said no for good (a bad address, say), so it
    /// isn't retried; otherwise it is, at every check. The reason shows above the message.
    pub fn outgoing_failed(
        &mut self,
        id: MessageId,
        error: &str,
        refused: bool,
    ) -> Result<(), StoreError> {
        let note = if refused {
            format!("Not sent: {error}. Press E to fix it and send it again, or Del to drop it.")
        } else {
            format!("Not sent yet: {error}. Telex tries again at every check.")
        };
        let tx = self.db.transaction()?;
        tx.execute(
            "UPDATE outbox SET attempts = attempts + 1, last_error = ?2, refused = ?3 WHERE message = ?1",
            params![id as i64, error, refused],
        )?;
        tx.execute(
            "UPDATE messages SET note = ?2 WHERE id = ?1",
            params![id as i64, note],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// True when the account has mail waiting to be sent (refused mail doesn't count).
    pub fn has_outgoing(&self, account: &str) -> Result<bool, StoreError> {
        Ok(self.next_outgoing(account, 0)?.is_some())
    }

    /// The message as it was received or sent, when Telex has it.
    pub fn raw_of(&self, id: MessageId) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .db
            .query_row("SELECT raw FROM messages WHERE id = ?1", [id as i64], |r| {
                r.get(0)
            })
            .optional()?
            .flatten())
    }

    // ---- Remote images ----

    /// Let a sender's remote images load without asking, or stop that.
    pub fn set_sender_trusted(&mut self, email: &str, trusted: bool) -> Result<(), StoreError> {
        let email = email.trim();
        if trusted {
            self.db.execute(
                "INSERT OR IGNORE INTO trusted_senders (email, added) VALUES (?1, datetime('now'))",
                [email],
            )?;
        } else {
            self.db
                .execute("DELETE FROM trusted_senders WHERE email = ?1", [email])?;
        }
        Ok(())
    }

    pub fn is_sender_trusted(&self, email: &str) -> Result<bool, StoreError> {
        Ok(self.db.query_row(
            "SELECT EXISTS (SELECT 1 FROM trusted_senders WHERE email = ?1)",
            [email.trim()],
            |r| r.get(0),
        )?)
    }

    /// Load the message's remote images without asking: the sender is trusted, and the
    /// message is neither in Spam nor marked by a filter (`note`).
    pub fn auto_load_images(&self, id: MessageId) -> Result<bool, StoreError> {
        let (folder, from, note): (String, String, Option<String>) = self
            .db
            .query_row(
                "SELECT folder, from_email, note FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))?;
        Ok(note.is_none()
            && self.folder_kind(&folder) != Some(FolderKind::Spam)
            && self.is_sender_trusted(&from)?)
    }

    /// Deliver waiting sample messages and return their ids.
    /// Real accounts get new mail through `imap::sync_account` instead.
    pub fn fetch_new(&mut self) -> Result<Vec<MessageId>, StoreError> {
        let arrived = std::mem::take(&mut self.incoming);
        let tx = self.db.transaction()?;
        let ids = arrived
            .iter()
            .map(|m| insert_message(&tx, &m.folder, None, m, None, None))
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

    // ---- Accounts and folders ----

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
                "UPDATE accounts SET name = ?2, email = ?3, imap_host = ?4, imap_port = ?5,
                     imap_user = ?6, smtp_host = ?7, smtp_port = ?8, smtp_security = ?9
                 WHERE id = ?1",
                params![
                    id,
                    setup.name,
                    setup.email,
                    setup.host,
                    setup.port,
                    setup.username,
                    setup.smtp_host,
                    setup.smtp_port,
                    setup.smtp_security.as_str()
                ],
            )?;
            id
        } else {
            let id = self.unique_account_id(&setup.email);
            let tx = self.db.transaction()?;
            tx.execute(
                "INSERT INTO accounts (id, position, name, email, imap_host, imap_port, imap_user,
                     smtp_host, smtp_port, smtp_security)
                 VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM accounts),
                     ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    id,
                    setup.name,
                    setup.email,
                    setup.host,
                    setup.port,
                    setup.username,
                    setup.smtp_host,
                    setup.smtp_port,
                    setup.smtp_security.as_str()
                ],
            )?;
            tx.execute(
                "INSERT INTO folders (account, id, position, name, kind) VALUES (?1, ?2, 1, 'Outbox', 'outbox')",
                params![id, OUTBOX_ID],
            )?;
            tx.commit()?;
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

    /// Remove an account, every local copy of its mail and its queued changes.
    /// The server is not touched.
    pub fn remove_account(&mut self, id: &str) -> Result<(), StoreError> {
        self.account(id)?;
        let tx = self.db.transaction()?;
        tx.execute(
            "DELETE FROM messages WHERE substr(folder, 1, length(?1) + 1) = ?1 || '/'",
            [id],
        )?;
        tx.execute("DELETE FROM pending_ops WHERE account = ?1", [id])?;
        tx.execute("DELETE FROM outbox WHERE account = ?1", [id])?;
        tx.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        tx.commit()?;
        self.reload_accounts()
    }

    /// Replace an account's server folders with what the server has now. Folders that
    /// disappeared from the server are dropped together with their local copies; local
    /// folders (the Outbox) stay.
    pub fn set_folders(&mut self, account: &str, folders: &[Folder]) -> Result<(), StoreError> {
        self.account(account)?;
        let keep: HashSet<&str> = folders.iter().map(|f| f.id.as_str()).collect();
        let tx = self.db.transaction()?;
        let existing: Vec<String> = {
            let mut stmt =
                tx.prepare("SELECT id FROM folders WHERE account = ?1 AND remote IS NOT NULL")?;
            let ids = stmt
                .query_map([account], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            ids
        };
        for gone in existing.iter().filter(|id| !keep.contains(id.as_str())) {
            let key = format!("{account}/{gone}");
            tx.execute(
                "DELETE FROM messages WHERE folder = ?1 OR srv_folder = ?1",
                [&key],
            )?;
            tx.execute(
                "DELETE FROM pending_ops WHERE srv_folder = ?1 OR target = ?1",
                [&key],
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

    // ---- Downloading (server → here) ----

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
            "DELETE FROM messages WHERE srv_folder = ?1 AND uid IS NOT NULL",
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

    /// UIDs of the messages stored from one server folder.
    pub fn local_uids(&self, folder_key: &str) -> Result<HashSet<u32>, StoreError> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT uid FROM messages WHERE srv_folder = ?1 AND uid IS NOT NULL")?;
        let uids = stmt
            .query_map([folder_key], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(uids)
    }

    /// Drop local copies of messages that are no longer on the server. Returns how many.
    pub fn forget_vanished(
        &mut self,
        folder_key: &str,
        on_server: &HashSet<u32>,
    ) -> Result<usize, StoreError> {
        let gone: Vec<u32> = self
            .local_uids(folder_key)?
            .into_iter()
            .filter(|uid| !on_server.contains(uid))
            .collect();
        if !gone.is_empty() {
            let tx = self.db.transaction()?;
            for uid in &gone {
                tx.execute(
                    "DELETE FROM messages WHERE srv_folder = ?1 AND uid = ?2",
                    params![folder_key, uid],
                )?;
            }
            tx.commit()?;
        }
        Ok(gone.len())
    }

    /// Take read state and flags from the server, `(uid, seen, flagged)` per message,
    /// except where a local change is still queued: that one wins. Returns how many changed.
    pub fn refresh_flags(
        &mut self,
        folder_key: &str,
        flags: &[(u32, bool, bool)],
    ) -> Result<usize, StoreError> {
        let tx = self.db.transaction()?;
        let mut changed = 0;
        for &(uid, seen, flagged) in flags {
            changed += tx.execute(
                "UPDATE messages SET unread = ?3 WHERE srv_folder = ?1 AND uid = ?2 AND unread <> ?3
                 AND NOT EXISTS (SELECT 1 FROM pending_ops WHERE message = messages.id AND kind = 'seen')",
                params![folder_key, uid, !seen],
            )?;
            changed += tx.execute(
                "UPDATE messages SET flagged = ?3 WHERE srv_folder = ?1 AND uid = ?2 AND flagged <> ?3
                 AND NOT EXISTS (SELECT 1 FROM pending_ops WHERE message = messages.id AND kind = 'flagged')",
                params![folder_key, uid, flagged],
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Store messages downloaded from one folder and remember the highest UID seen.
    /// A message already stored is skipped, and one that Telex moved here itself (same
    /// Message-ID, UID not yet known) gets its UID instead of a duplicate. Returns new ids.
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
                "SELECT EXISTS (SELECT 1 FROM messages WHERE srv_folder = ?1 AND uid = ?2)",
                params![folder_key, item.uid],
                |r| r.get(0),
            )?;
            if stored {
                continue;
            }
            let moved_here: Option<i64> = match &item.message.message_id {
                Some(mid) => tx
                    .query_row(
                        "SELECT id FROM messages WHERE srv_folder = ?1 AND uid IS NULL AND message_id = ?2
                         ORDER BY id LIMIT 1",
                        params![folder_key, mid],
                        |r| r.get(0),
                    )
                    .optional()?,
                None => None,
            };
            match moved_here {
                Some(id) => {
                    tx.execute(
                        "UPDATE messages SET uid = ?1 WHERE id = ?2",
                        params![item.uid, id],
                    )?;
                }
                None => ids.push(insert_message(
                    &tx,
                    folder_key,
                    Some(folder_key),
                    &item.message,
                    Some(item.uid),
                    Some(&item.raw),
                )?),
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

    // ---- Uploading (here → server) ----

    /// True when the account has changes the server hasn't heard about.
    pub fn has_pending_changes(&self, account: &str) -> Result<bool, StoreError> {
        Ok(self.db.query_row(
            "SELECT EXISTS (SELECT 1 FROM pending_ops WHERE account = ?1)",
            [account],
            |r| r.get(0),
        )?)
    }

    /// Position of the newest queued change (0 when there is none). A change made later
    /// gets a higher number, also when it replaces an older one.
    pub fn last_change_op(&self, account: &str) -> Result<i64, StoreError> {
        Ok(self.db.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM pending_ops WHERE account = ?1",
            [account],
            |r| r.get(0),
        )?)
    }

    /// The account's queued changes, oldest first, with each message's current server location.
    pub fn pending_changes(&self, account: &str) -> Result<Vec<PendingChange>, StoreError> {
        let mut stmt = self.db.prepare_cached(
            "SELECT p.id, p.message, p.kind, p.value, p.target, p.attempts,
                    CASE p.kind WHEN 'expunge' THEN p.srv_folder WHEN 'append' THEN p.target
                         ELSE m.srv_folder END,
                    CASE p.kind WHEN 'expunge' THEN p.uid WHEN 'append' THEN NULL ELSE m.uid END,
                    COALESCE(m.message_id, p.message_id), COALESCE(m.subject, p.subject, '')
             FROM pending_ops p LEFT JOIN messages m ON m.id = p.message
             WHERE p.account = ?1 ORDER BY p.id",
        )?;
        let rows = stmt.query_map([account], |r| {
            let value: Option<bool> = r.get(3)?;
            let kind = match r.get::<_, String>(2)?.as_str() {
                "seen" => Some(ChangeKind::Seen(value.unwrap_or_default())),
                "flagged" => Some(ChangeKind::Flagged(value.unwrap_or_default())),
                "move" => r.get::<_, Option<String>>(4)?.map(ChangeKind::Move),
                "expunge" => Some(ChangeKind::Expunge),
                "append" => r.get::<_, Option<String>>(4)?.map(ChangeKind::Append),
                _ => None,
            };
            let (op, message, attempts): (i64, Option<i64>, u32) =
                (r.get(0)?, r.get(1)?, r.get(5)?);
            let (folder, uid, message_id, subject): (Option<String>, _, _, _) =
                (r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?);
            // A row without a server folder can't be sent anywhere; skip it.
            Ok(kind.zip(folder).map(|(kind, folder)| PendingChange {
                op,
                message: message.map(|id| id as MessageId),
                folder,
                uid,
                message_id,
                subject,
                kind,
                attempts,
            }))
        })?;
        let mut changes = Vec::new();
        for row in rows {
            changes.extend(row?);
        }
        Ok(changes)
    }

    /// The server has applied the change.
    pub fn change_done(&mut self, op: i64) -> Result<(), StoreError> {
        self.db
            .execute("DELETE FROM pending_ops WHERE id = ?1", [op])?;
        Ok(())
    }

    /// The server refused the change. Returns how many times it has failed so far.
    pub fn change_failed(&mut self, op: i64, error: &str) -> Result<u32, StoreError> {
        self.db.execute(
            "UPDATE pending_ops SET attempts = attempts + 1, last_error = ?2 WHERE id = ?1",
            params![op, error],
        )?;
        Ok(self
            .db
            .query_row(
                "SELECT attempts FROM pending_ops WHERE id = ?1",
                [op],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default())
    }

    /// Stop trying a change the server keeps refusing. A refused move puts the message
    /// back where it really is; flags are corrected by the next download.
    pub fn give_up_change(&mut self, change: &PendingChange) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM pending_ops WHERE id = ?1", [change.op])?;
        if let (ChangeKind::Move(_), Some(id)) = (&change.kind, change.message) {
            tx.execute(
                "UPDATE messages SET folder = srv_folder WHERE id = ?1 AND srv_folder IS NOT NULL",
                [id as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The server saved a copy of a sent message in `folder_key`. Its UID comes with the
    /// next download, which recognizes it by Message-ID instead of storing it twice.
    pub fn change_appended(
        &mut self,
        op: i64,
        id: MessageId,
        folder_key: &str,
    ) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM pending_ops WHERE id = ?1", [op])?;
        tx.execute(
            "UPDATE messages SET srv_folder = ?1 WHERE id = ?2 AND srv_folder IS NULL",
            params![folder_key, id as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The server moved the message to `folder_key`, where its UID is `uid` if known.
    pub fn change_moved(
        &mut self,
        op: i64,
        id: MessageId,
        folder_key: &str,
        uid: Option<u32>,
    ) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM pending_ops WHERE id = ?1", [op])?;
        set_server_location(&tx, id, folder_key, uid)?;
        tx.commit()?;
        Ok(())
    }

    /// The UID of a moved message, found later by its Message-ID.
    pub fn set_server_uid(
        &mut self,
        id: MessageId,
        folder_key: &str,
        uid: u32,
    ) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        set_server_location(&tx, id, folder_key, Some(uid))?;
        tx.commit()?;
        Ok(())
    }

    /// The message is gone from the server, so its local copy and queued changes go too.
    pub fn forget_message(&mut self, id: MessageId) -> Result<(), StoreError> {
        self.db
            .execute("DELETE FROM messages WHERE id = ?1", [id as i64])?;
        Ok(())
    }
}

/// Bring the database up to the newest schema, one step at a time.
fn migrate(db: &Connection) -> Result<(), StoreError> {
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let version = usize::try_from(version).unwrap_or_default();
    if version > MIGRATIONS.len() {
        return Err(StoreError::Database(format!(
            "it was made by a newer version of Telex (schema {version})"
        )));
    }
    for (step, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        let reached = step + 1;
        let tx = db.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        if reached == 2 {
            backfill_message_ids(&tx)?;
        }
        tx.pragma_update(None, "user_version", reached as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// Version 2 added `message_id`; fill it in for mail downloaded before.
fn backfill_message_ids(db: &Connection) -> Result<(), StoreError> {
    let rows: Vec<(i64, Vec<u8>)> = {
        let mut stmt = db.prepare("SELECT id, raw FROM messages WHERE raw IS NOT NULL")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        rows
    };
    for (id, raw) in rows {
        if let Some(mid) = parse_message_id(&raw) {
            db.execute(
                "UPDATE messages SET message_id = ?1 WHERE id = ?2",
                params![mid, id],
            )?;
        }
    }
    Ok(())
}

/// Point a message at its new server location. A second copy already stored there
/// (downloaded before Telex learned the UID) is dropped.
fn set_server_location(
    db: &Connection,
    id: MessageId,
    folder_key: &str,
    uid: Option<u32>,
) -> Result<(), StoreError> {
    if let Some(uid) = uid {
        db.execute(
            "DELETE FROM messages WHERE srv_folder = ?1 AND uid = ?2 AND id <> ?3",
            params![folder_key, uid, id as i64],
        )?;
    }
    db.execute(
        "UPDATE messages SET srv_folder = ?1, uid = ?2 WHERE id = ?3",
        params![folder_key, uid, id as i64],
    )?;
    Ok(())
}

/// "firma/inbox" → "firma".
fn account_of(folder_key: &str) -> &str {
    split_folder_key(folder_key).map_or(folder_key, |(a, _)| a)
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
/// ł is folded to l, as the index triggers in migrations/001_base.sql do.
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
     size_kb, unread, flagged, remote_images, note, attachments, body, message_id";

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
        message_id: r.get(15)?,
    })
}

fn load_accounts(db: &Connection) -> Result<Vec<Account>, StoreError> {
    let mut stmt = db.prepare(
        "SELECT id, name, email, imap_host, imap_port, imap_user, smtp_host, smtp_port, smtp_security
         FROM accounts ORDER BY position, id",
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
            let smtp = match r.get::<_, Option<String>>(6)? {
                Some(host) => Some(SmtpSettings {
                    host,
                    port: r.get(7)?,
                    security: SmtpSecurity::from_name(
                        &r.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    ),
                }),
                None => None,
            };
            Ok(Account {
                id: r.get(0)?,
                name: r.get(1)?,
                email: r.get(2)?,
                folders: Vec::new(),
                imap,
                smtp,
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
    // Server folders arrive in plan order and the Outbox is local; rank puts it in place.
    for a in &mut accounts {
        a.folders.sort_by_key(|f| f.kind.rank());
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

/// `srv_folder` and `uid` are set for mail from a server, `None` for mail that exists only here.
fn insert_message(
    db: &Connection,
    folder: &str,
    srv_folder: Option<&str>,
    m: &Message,
    uid: Option<u32>,
    raw: Option<&[u8]>,
) -> Result<MessageId, StoreError> {
    let attachments =
        serde_json::to_string(&m.attachments).map_err(|e| StoreError::Database(e.to_string()))?;
    db.execute(
        "INSERT INTO messages (folder, srv_folder, uid, message_id, from_name, from_email, to_addr,
             cc, subject, date, size_kb, unread, flagged, remote_images, note, attachments, body, raw)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            folder,
            srv_folder,
            uid,
            m.message_id,
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
                remote_images: false,
                note: None,
                attachments: vec![],
                body: "Dzień dobry, przesyłam żółwia.".into(),
                message_id: Some(format!("{uid}.{subject}@kowalska-architekci.pl")),
            },
            raw: format!("Message-ID: <{uid}.{subject}@kowalska-architekci.pl>\r\nSubject: {subject}\r\n\r\nx")
                .into_bytes(),
        }
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
        let changed = store
            .refresh_flags(&key, &[(1, true, true), (2, true, false)])
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
        assert_eq!(store.accounts().len(), 1);
    }

    #[test]
    fn removing_an_account_removes_its_mail_and_changes() {
        let (mut store, account) = store_with_server_account();
        let id = one_in_inbox(&mut store, &account, 1);
        store.delete(id).unwrap();
        store.delete(id).unwrap(); // queues an expunge, which has no message row
        store.remove_account(&account.id).unwrap();
        assert!(store.accounts().is_empty());
        assert!(store.snapshot().unwrap().messages.is_empty());
        assert!(store.search("zolwia").unwrap().is_empty());
        assert!(!store.has_pending_changes(&account.id).unwrap());
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
                    ..Default::default()
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

        let mut store = MailStore::with_connection(db, false).unwrap();
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
        // Changes to old mail are queued like any other.
        store.set_unread(id, false).unwrap();
        assert!(store.has_pending_changes("studio").unwrap());
        assert_eq!(store.search("dzien").unwrap(), [id]);
    }

    #[test]
    fn slugs_are_safe_ids() {
        assert_eq!(slug("INBOX.Faktury 2026"), "inbox-faktury-2026");
        assert_eq!(slug("  --Wysłane--  "), "wysłane");
        assert_eq!(slug("..."), "");
    }

    #[test]
    fn trusted_senders_load_images_except_in_spam() {
        let mut store = sample_store();
        let first = &store.list("firma/inbox").unwrap()[0];
        let (id, sender) = (first.id, first.from_email.clone());
        assert!(!store.auto_load_images(id).unwrap());
        store
            .set_sender_trusted(&sender.to_uppercase(), true)
            .unwrap();
        assert!(
            store.is_sender_trusted(&sender).unwrap(),
            "addresses ignore case"
        );
        assert!(store.auto_load_images(id).unwrap());
        store.move_to(id, "firma/spam").unwrap();
        assert!(!store.auto_load_images(id).unwrap(), "never in Spam");
        store.move_to(id, "firma/inbox").unwrap();
        store.set_sender_trusted(&sender, false).unwrap();
        assert!(!store.auto_load_images(id).unwrap());
    }
}
