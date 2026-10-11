use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use chrono::NaiveDateTime;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::filters::{Action, Rule};
use crate::html;
use crate::list;
use crate::mime::parse_message_id;
use crate::model::{
    split_folder_key, Account, AccountSetup, Attachment, Folder, FolderKind, Identity,
    ImapSettings, Message, MessageId, MessageSummary, OutgoingDraft, SmtpSecurity, SmtpSettings,
};
use crate::tr;

/// Schema steps, oldest first: step n brings a database to version n. A released step is
/// never edited; a change is a new file in migrations/ added here.
pub(crate) const MIGRATIONS: &[&str] = &[
    include_str!("migrations/001_base.sql"),
    include_str!("migrations/002_write_back.sql"),
    include_str!("migrations/003_sending.sql"),
    include_str!("migrations/004_trusted_senders.sql"),
    include_str!("migrations/005_rules.sql"),
    include_str!("migrations/006_identities.sql"),
    include_str!("migrations/007_message_content.sql"),
    include_str!("migrations/008_usage.sql"),
    include_str!("migrations/009_conversations_and_sending.sql"),
    include_str!("migrations/010_local_folders.sql"),
];

/// Every server account's local Outbox has this id. `imap::plan_folders` never gives it
/// to a server folder.
pub const OUTBOX_ID: &str = "outbox";

/// The folders on this PC ("On this PC"): mail that is kept only here. They are stored as an
/// account without servers that has this id, so their keys are "local/<folder>".
pub const LOCAL_ID: &str = "local";

/// The folders on this PC with their Archive and Trash, unless they are there already, as
/// migrations/010_local_folders.sql makes them for mail bases that have server accounts.
const ENSURE_LOCAL: &str = "
    INSERT OR IGNORE INTO accounts (id, position, name, email) VALUES ('local', 1000000, 'On this PC', '');
    INSERT OR IGNORE INTO folders (account, id, position, name, kind) VALUES ('local', 'archive', 0, 'Archive', 'archive');
    INSERT OR IGNORE INTO folders (account, id, position, name, kind) VALUES ('local', 'trash', 1, 'Trash', 'trash');";

/// The longest name a folder on this PC can have.
const FOLDER_NAME_MAX: usize = 80;

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
    /// Something the user filled in doesn't make sense yet (a rule without actions, say).
    /// The text is a sentence for the UI.
    Invalid(String),
    /// SQLite failed: disk full, file locked or damaged.
    Database(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::UnknownMessage(id) => {
                tr!("message {id} does not exist", "wiadomość {id} nie istnieje")
            }
            Self::UnknownAccount(a) => {
                tr!("account \"{a}\" does not exist", "konto „{a}” nie istnieje")
            }
            Self::UnknownFolder(k) => {
                tr!("folder \"{k}\" does not exist", "folder „{k}” nie istnieje")
            }
            Self::CrossAccountMove { from, to } => tr!(
                "can't move a message from {from} to {to}: they belong to different accounts",
                "nie można przenieść wiadomości z {from} do {to}: należą do różnych kont"
            ),
            Self::MissingFolder { account, kind } => {
                let pl = kind.pl_name().unwrap_or(kind.as_str());
                tr!(
                    "account \"{account}\" has no {kind:?} folder",
                    "konto „{account}” nie ma folderu {pl}"
                )
            }
            Self::Invalid(why) => return f.write_str(why),
            Self::Database(e) => tr!("the mail database failed: {e}", "błąd bazy poczty: {e}"),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e.to_string())
    }
}

impl StoreError {
    /// The store's lock is broken: a thread panicked while holding it.
    pub(crate) fn poisoned() -> Self {
        Self::Database(tr!(
            "another task crashed while using it; restart Telex",
            "inne zadanie uległo awarii, gdy z niej korzystało; uruchom Telexa ponownie"
        ))
    }
}

/// Saving an identity or a rule that is gone meanwhile.
fn identity_gone() -> StoreError {
    StoreError::Invalid(tr!(
        "That identity no longer exists.",
        "Tej tożsamości już nie ma."
    ))
}

fn rule_gone() -> StoreError {
    StoreError::Invalid(tr!("That rule no longer exists.", "Tej reguły już nie ma."))
}

/// A mail base without server accounts has no folders on this PC yet.
fn no_local_folders() -> StoreError {
    StoreError::Invalid(tr!(
        "Folders on this PC come with your first mail account.",
        "Foldery na tym komputerze pojawiają się z pierwszym kontem pocztowym."
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteOutcome {
    /// The message is now in the account's Trash and can be moved back.
    MovedToTrash,
    /// The message was already in Trash and is gone for good.
    Removed,
}

/// What deleting several messages did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DeleteReport {
    /// Messages now in Trash, each with the folder it was in, for undo.
    pub trashed: Vec<(MessageId, String)>,
    /// Messages that were in Trash and are gone for good.
    pub removed: usize,
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
    /// Upload the message to this server folder (IMAP APPEND): a copy of sent mail for Sent
    /// (`flags` None: always read), or mail moved there from this PC with its read state and
    /// flag, `(seen, flagged)`.
    Append {
        to: String,
        flags: Option<(bool, bool)>,
    },
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
    pub(crate) db: Connection,
    accounts: Vec<Account>,
    sample: bool,
    /// Sample messages waiting for "Get mail". Always empty for real accounts.
    incoming: Vec<Message>,
    /// The last filtered list the UI paged through (see `list`).
    pub(crate) found: RefCell<Option<list::Found>>,
    /// The last conversation worked out: for which message, at which `total_changes`.
    pub(crate) thread: RefCell<Option<(MessageId, u64, Vec<MessageId>)>>,
}

impl MailStore {
    /// Open the mail database at `path`, creating or upgrading it if needed.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let db = Connection::open(path)?;
        // WAL lets the UI read while a sync writes, and survives crashes well. With WAL,
        // synchronous=NORMAL is still safe: a power cut can lose the last commits, never
        // damage the file. A larger cache, memory-mapped reads and temporary tables in
        // memory keep lists and searches off the disk.
        db.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        db.pragma_update(None, "cache_size", -32_768)?; // KiB: 32 MiB
        db.pragma_update_and_check(None, "mmap_size", 256 * 1024 * 1024, |_| Ok(()))?;
        db.pragma_update(None, "temp_store", "MEMORY")?;
        Self::with_connection(db, false)
    }

    /// An empty store that lives only in memory.
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::with_connection(Connection::open_in_memory()?, false)
    }

    fn with_connection(db: Connection, sample: bool) -> Result<Self, StoreError> {
        db.pragma_update(None, "foreign_keys", true)?;
        list::register_functions(&db)?;
        migrate(&db)?;
        let accounts = load_accounts(&db)?;
        Ok(Self {
            db,
            accounts,
            sample,
            incoming: Vec::new(),
            found: RefCell::new(None),
            thread: RefCell::new(None),
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
            insert_identity(&tx, &Identity::new(&a.id, &a.name, &a.email), 0)?;
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

    /// Lives only in memory: nothing is saved until it is swapped for a file. The app starts
    /// this way until the first account is added.
    pub fn is_in_memory(&self) -> bool {
        self.db.path().is_none_or(str::is_empty)
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

    pub(crate) fn reload_accounts(&mut self) -> Result<(), StoreError> {
        self.accounts = load_accounts(&self.db)?;
        Ok(())
    }

    /// Let SQLite update its statistics for the query planner. Runs when the store closes;
    /// the app also calls it when its window closes, because the process may end without
    /// dropping the store.
    pub fn optimize(&self) {
        let _ = self.db.execute_batch("PRAGMA optimize");
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
            "SELECT {MESSAGE_COLUMNS} FROM messages m LEFT JOIN message_content c ON c.id = m.id
             WHERE m.id = ?1"
        ))?;
        stmt.query_row([id as i64], message_from_row)
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))
    }

    /// One row of the message list.
    pub fn summary(&self, id: MessageId) -> Result<MessageSummary, StoreError> {
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM messages WHERE id = ?1"
        ))?;
        stmt.query_row([id as i64], summary_from_row)
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))
    }

    /// The folder a message is in now, or `None` when it no longer exists.
    pub fn folder_of(&self, id: MessageId) -> Result<Option<String>, StoreError> {
        match self.locate(id) {
            Ok((folder, _)) => Ok(Some(folder)),
            Err(StoreError::UnknownMessage(_)) => Ok(None),
            Err(e) => Err(e),
        }
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

    fn folder(&self, key: &str) -> Option<&Folder> {
        let (acct, folder) = split_folder_key(key)?;
        let account = self.accounts.iter().find(|a| a.id == acct)?;
        account.folders.iter().find(|f| f.id == folder)
    }

    fn folder_kind(&self, key: &str) -> Option<FolderKind> {
        self.folder(key).map(|f| f.kind)
    }

    /// What a folder is for, by its key.
    pub fn kind_of(&self, key: &str) -> Option<FolderKind> {
        self.folder_kind(key)
    }

    /// A folder's display name, by its key; `None` when it doesn't exist.
    pub fn folder_name(&self, key: &str) -> Option<String> {
        self.folder(key).map(|f| f.name.clone())
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
        self.locate(id)?;
        set_flags(&mut self.db, &[id], Flag::Unread, unread, false).map(|_| ())
    }

    pub fn set_flagged(&mut self, id: MessageId, flagged: bool) -> Result<(), StoreError> {
        self.locate(id)?;
        set_flags(&mut self.db, &[id], Flag::Flagged, flagged, false).map(|_| ())
    }

    /// Mark many messages read or unread at once. Messages already so, or gone, are left
    /// alone. Returns how many changed.
    pub fn set_unread_many(
        &mut self,
        ids: &[MessageId],
        unread: bool,
    ) -> Result<usize, StoreError> {
        set_flags(&mut self.db, ids, Flag::Unread, unread, true)
    }

    /// Flag or unflag many messages at once. Returns how many changed.
    pub fn set_flagged_many(
        &mut self,
        ids: &[MessageId],
        flagged: bool,
    ) -> Result<usize, StoreError> {
        set_flags(&mut self.db, ids, Flag::Flagged, flagged, true)
    }

    /// Mark every unread message in a folder read. Returns how many.
    pub fn mark_folder_read(&mut self, folder: &str) -> Result<usize, StoreError> {
        self.check_folder(folder)?;
        let ids: Vec<MessageId> = {
            let mut stmt = self
                .db
                .prepare_cached("SELECT id FROM messages WHERE folder = ?1 AND unread")?;
            let ids = stmt
                .query_map([folder], |r| r.get::<_, i64>(0))?
                .map(|id| id.map(|id| id as MessageId))
                .collect::<Result<_, _>>()?;
            ids
        };
        self.set_unread_many(&ids, false)
    }

    /// Move a message to another folder (see [`MailStore::move_many`]).
    pub fn move_to(&mut self, id: MessageId, folder: &str) -> Result<(), StoreError> {
        self.move_many(&[id], folder).map(|_| ())
    }

    /// Move messages to another folder, all or none: a message that is gone, or one of
    /// another server account, stops the whole move. Returns where each one was.
    ///
    /// Within an account, server mail moves on the server too. Server mail moved to a folder
    /// on this PC leaves the server (an expunge, taken back when the message returns before
    /// the server heard of it); mail from this PC moved to a server folder is uploaded there.
    /// Between two server accounts mail goes by way of this PC, in two moves.
    pub fn move_many(
        &mut self,
        ids: &[MessageId],
        folder: &str,
    ) -> Result<Vec<(MessageId, String)>, StoreError> {
        let target_acct = self.check_folder(folder)?.to_owned();
        let on_server = self.folder(folder).is_some_and(|f| f.remote.is_some());
        let mut places = Vec::with_capacity(ids.len());
        for &id in ids {
            let (source, srv_folder) = self.locate(id)?;
            let from = account_of(&source);
            if from != target_acct && from != LOCAL_ID && target_acct != LOCAL_ID {
                return Err(StoreError::CrossAccountMove {
                    from: source,
                    to: folder.to_owned(),
                });
            }
            places.push((id, source, srv_folder));
        }
        let tx = self.db.transaction()?;
        for (id, source, srv_folder) in &places {
            if account_of(source) == target_acct {
                move_row(
                    &tx,
                    *id,
                    srv_folder.as_deref(),
                    folder,
                    &target_acct,
                    on_server,
                )?;
            } else if target_acct == LOCAL_ID {
                to_this_pc(&tx, *id, srv_folder.as_deref(), folder)?;
            } else {
                from_this_pc(&tx, *id, folder, &target_acct, on_server)?;
            }
        }
        tx.commit()?;
        Ok(places.into_iter().map(|(id, from, _)| (id, from)).collect())
    }

    /// Delete like a mail client does: first to Trash, and from Trash for good.
    pub fn delete(&mut self, id: MessageId) -> Result<DeleteOutcome, StoreError> {
        let report = self.delete_many(&[id])?;
        Ok(if report.trashed.is_empty() {
            DeleteOutcome::Removed
        } else {
            DeleteOutcome::MovedToTrash
        })
    }

    /// Delete many messages: to their account's Trash, or for good when they are in Trash
    /// already. Every message is checked first; nothing changes when one can't go.
    pub fn delete_many(&mut self, ids: &[MessageId]) -> Result<DeleteReport, StoreError> {
        let mut plan = Vec::with_capacity(ids.len());
        for &id in ids {
            let (folder, srv_folder) = self.locate(id)?;
            let to_trash = if self.folder_kind(&folder) == Some(FolderKind::Trash) {
                None
            } else {
                let trash = self.folder_key_of_kind(account_of(&folder), FolderKind::Trash)?;
                let on_server = self.folder(&trash).is_some_and(|f| f.remote.is_some());
                Some((trash, on_server))
            };
            plan.push((id, folder, srv_folder, to_trash));
        }
        let mut report = DeleteReport::default();
        let tx = self.db.transaction()?;
        for (id, folder, srv_folder, to_trash) in plan {
            match to_trash {
                Some((trash, on_server)) => {
                    let account = account_of(&trash);
                    move_row(&tx, id, srv_folder.as_deref(), &trash, account, on_server)?;
                    report.trashed.push((id, folder));
                }
                None => {
                    remove_row(&tx, id, srv_folder.as_deref())?;
                    report.removed += 1;
                }
            }
        }
        tx.commit()?;
        Ok(report)
    }

    /// Store a message the user wrote, in Drafts (or, for the sample mailbox, in Sent:
    /// it can't really send). A draft edited from another replaces it. `raw` is the draft
    /// as a whole message (`smtp::compose_draft`), which keeps its HTML and forwarded files.
    pub fn save_outgoing(
        &mut self,
        draft: OutgoingDraft,
        raw: Option<&[u8]>,
        kind: FolderKind,
        now: NaiveDateTime,
    ) -> Result<MessageId, StoreError> {
        debug_assert!(kind.is_outgoing(), "outgoing mail goes to Sent or Drafts");
        if let Some(old) = draft.replaces {
            self.remove_local(old)?;
        }
        let account = self.account(&draft.account)?.clone();
        let folder = self.folder_key_of_kind(&account.id, kind)?;
        let identity = account.identity(draft.identity);
        let body = match &draft.html {
            Some(h) => html::to_text(&html::compose_clean(h)),
            None => draft.body,
        };
        let attachments_kb: u32 = draft.attachments.iter().map(|a| a.size_kb).sum();
        let size_kb = match raw {
            Some(raw) => u32::try_from(raw.len().div_ceil(1024)).unwrap_or(u32::MAX),
            None => u32::try_from(body.len().div_ceil(1024)).unwrap_or(u32::MAX) + attachments_kb,
        };
        // Files from this PC are inside `raw` now; the paths they came from don't matter.
        let attachments = draft
            .attachments
            .into_iter()
            .map(|a| Attachment { path: None, ..a })
            .collect();
        let message = Message {
            id: 0,
            folder: folder.clone(),
            from_name: identity.name,
            from_email: identity.email,
            to: draft.to,
            cc: draft.cc,
            bcc: draft.bcc,
            subject: draft.subject,
            date: now,
            size_kb: size_kb.max(1),
            unread: false,
            flagged: false,
            remote_images: false,
            note: None,
            attachments,
            body,
            message_id: None,
            refs: Vec::new(),
        };
        insert_message(&self.db, &folder, None, &message, None, raw)
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
    /// will be kept (Bcc included; sending leaves that header out); `message` is its parsed
    /// form for the list and preview. With `send_after`, it waits until then, so the user
    /// can still take it back ([`MailStore::hold_outgoing`]).
    #[allow(clippy::too_many_arguments)]
    pub fn queue_outgoing(
        &mut self,
        account: &str,
        message: &Message,
        raw: &[u8],
        env_from: &str,
        env_to: &[String],
        replaces: Option<MessageId>,
        send_after: Option<NaiveDateTime>,
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
            "INSERT INTO outbox (message, account, env_from, env_to, send_after)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id as i64, account, env_from, env_to, send_after],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Take back a message that is still waiting out its moment in the Outbox: it stays
    /// there, held, until the user edits it (E) or drops it. Too late once it is due.
    pub fn hold_outgoing(&mut self, id: MessageId, now: NaiveDateTime) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        let held = tx.execute(
            "UPDATE outbox SET refused = 1, send_after = NULL
             WHERE message = ?1 AND NOT refused AND send_after IS NOT NULL AND send_after > ?2",
            params![id as i64, now],
        )?;
        if held == 0 {
            return Err(StoreError::Invalid(tr!(
                "Too late to undo: the message is on its way.",
                "Za późno, żeby cofnąć: wiadomość już wychodzi."
            )));
        }
        let note = tr!(
            "Not sent: you took it back. Press E to change it and send it, or Del to drop it.",
            "Nie wysłano: cofnięto wysyłanie. Naciśnij E, żeby ją zmienić i wysłać, albo Del, \
             żeby ją usunąć."
        );
        tx.execute(
            "UPDATE messages SET note = ?2 WHERE id = ?1",
            params![id as i64, note],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The oldest message after `after` waiting in the account's Outbox that is due at
    /// `now`, skipping ones the server refused for good or the user took back. Go through
    /// the Outbox by passing back the last id.
    pub fn next_outgoing(
        &self,
        account: &str,
        after: MessageId,
        now: NaiveDateTime,
    ) -> Result<Option<OutgoingMail>, StoreError> {
        let outbox = format!("{account}/{OUTBOX_ID}");
        self.db
            .query_row(
                "SELECT o.message, o.env_from, o.env_to, c.raw, m.subject, m.to_addr, o.attempts
                 FROM outbox o JOIN messages m ON m.id = o.message
                     JOIN message_content c ON c.id = o.message
                 WHERE o.account = ?1 AND m.folder = ?2 AND NOT o.refused AND o.message > ?3
                   AND c.raw IS NOT NULL AND (o.send_after IS NULL OR o.send_after <= ?4)
                 ORDER BY o.message LIMIT 1",
                params![account, outbox, after as i64, now],
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
            tr!(
                "Not sent: {error}. Press E to fix it and send it again, or Del to drop it.",
                "Nie wysłano: {error}. Naciśnij E, żeby poprawić i wysłać ponownie, albo Del, \
                 żeby usunąć."
            )
        } else {
            tr!(
                "Not sent yet: {error}. Telex tries again at every check.",
                "Jeszcze nie wysłano: {error}. Telex ponawia próbę przy każdym sprawdzaniu poczty."
            )
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

    /// True when the account has mail due to be sent at `now` (refused and held mail
    /// doesn't count, nor mail still waiting out its moment to undo).
    pub fn has_outgoing(&self, account: &str, now: NaiveDateTime) -> Result<bool, StoreError> {
        Ok(self.next_outgoing(account, 0, now)?.is_some())
    }

    /// The message as it was received or sent, when Telex has it.
    pub fn raw_of(&self, id: MessageId) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .db
            .query_row(
                "SELECT raw FROM message_content WHERE id = ?1",
                [id as i64],
                |r| r.get(0),
            )
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

    /// Every sender whose images load without asking, A to Z, for Settings.
    pub fn trusted_senders(&self) -> Result<Vec<String>, StoreError> {
        let mut stmt = self
            .db
            .prepare("SELECT email FROM trusted_senders ORDER BY email COLLATE NOCASE")?;
        let senders = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(senders)
    }

    pub fn is_sender_trusted(&self, email: &str) -> Result<bool, StoreError> {
        Ok(self.db.query_row(
            "SELECT EXISTS (SELECT 1 FROM trusted_senders WHERE email = ?1)",
            [email.trim()],
            |r| r.get(0),
        )?)
    }

    /// Load the message's remote images without asking: the sender is trusted and the
    /// message isn't in Spam.
    pub fn auto_load_images(&self, id: MessageId) -> Result<bool, StoreError> {
        let (folder, from): (String, String) = self
            .db
            .query_row(
                "SELECT folder, from_email FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))?;
        Ok(self.folder_kind(&folder) != Some(FolderKind::Spam) && self.is_sender_trusted(&from)?)
    }

    /// Add a line to the note shown above a message (why a rule or check touched it),
    /// unless it's already there.
    pub fn add_note(&mut self, id: MessageId, text: &str) -> Result<(), StoreError> {
        self.db.execute(
            "UPDATE messages SET note = CASE
                 WHEN note IS NULL OR note = '' THEN ?2
                 WHEN instr(note, ?2) > 0 THEN note
                 ELSE note || ' ' || ?2 END
             WHERE id = ?1",
            params![id as i64, text],
        )?;
        Ok(())
    }

    // ---- Identities ----

    /// Add an identity (id 0) or update one. Making it the default takes that from the
    /// account's other identities.
    pub fn save_identity(&mut self, identity: &Identity) -> Result<Identity, StoreError> {
        identity.check().map_err(StoreError::Invalid)?;
        self.account(&identity.account)?;
        let tx = self.db.transaction()?;
        let id = if identity.id == 0 {
            let position: i64 = tx.query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM identities WHERE account = ?1",
                [&identity.account],
                |r| r.get(0),
            )?;
            insert_identity(&tx, identity, usize::try_from(position).unwrap_or_default())?
        } else {
            let changed = tx.execute(
                "UPDATE identities SET name = ?2, email = ?3, signature = ?4, reply_header = ?5,
                     forward_header = ?6, is_default = ?7
                 WHERE id = ?1 AND account = ?8",
                params![
                    identity.id,
                    identity.name.trim(),
                    identity.email.trim(),
                    identity.signature,
                    identity.reply_header,
                    identity.forward_header,
                    identity.is_default,
                    identity.account
                ],
            )?;
            if changed == 0 {
                return Err(identity_gone());
            }
            identity.id
        };
        if identity.is_default {
            tx.execute(
                "UPDATE identities SET is_default = 0 WHERE account = ?1 AND id <> ?2",
                params![identity.account, id],
            )?;
        }
        ensure_a_default(&tx, &identity.account)?;
        tx.commit()?;
        self.reload_accounts()?;
        self.account(&identity.account)?
            .identities
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .ok_or_else(identity_gone)
    }

    /// Remove an identity. The last one of an account stays: mail has to be from someone.
    pub fn delete_identity(&mut self, id: i64) -> Result<(), StoreError> {
        let account: Option<String> = self
            .db
            .query_row("SELECT account FROM identities WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        let Some(account) = account else {
            return Ok(());
        };
        if self.account(&account)?.identities.len() <= 1 {
            return Err(StoreError::Invalid(tr!(
                "An account needs at least one identity. Change this one instead.",
                "Konto potrzebuje co najmniej jednej tożsamości. Zmień tę zamiast ją usuwać."
            )));
        }
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM identities WHERE id = ?1", [id])?;
        ensure_a_default(&tx, &account)?;
        tx.commit()?;
        self.reload_accounts()
    }

    // ---- Rules ----

    /// Every rule, in the order they run.
    pub fn rules(&self) -> Result<Vec<Rule>, StoreError> {
        let mut stmt = self.db.prepare_cached(
            "SELECT id, name, enabled, account, match_all, conditions, actions, stop
             FROM rules ORDER BY position, id",
        )?;
        let rows = stmt.query_map([], |r| {
            let conditions: String = r.get(5)?;
            let actions: String = r.get(6)?;
            Ok(Rule {
                id: r.get(0)?,
                name: r.get(1)?,
                enabled: r.get(2)?,
                account: r.get(3)?,
                match_all: r.get(4)?,
                conditions: serde_json::from_str(&conditions).unwrap_or_default(),
                actions: serde_json::from_str(&actions).unwrap_or_default(),
                stop: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Add a rule (id 0, at the end) or update one. It is checked first.
    pub fn save_rule(&mut self, rule: &Rule) -> Result<Rule, StoreError> {
        rule.check(&self.accounts).map_err(StoreError::Invalid)?;
        let not_json = |e: serde_json::Error| StoreError::Database(e.to_string());
        let conditions = serde_json::to_string(&rule.conditions).map_err(not_json)?;
        let actions = serde_json::to_string(&rule.actions).map_err(not_json)?;
        let name = rule.name.trim();
        let id = if rule.id == 0 {
            self.db.execute(
                "INSERT INTO rules (position, name, enabled, account, match_all, conditions, actions, stop)
                 VALUES ((SELECT COALESCE(MAX(position) + 1, 0) FROM rules), ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![name, rule.enabled, rule.account, rule.match_all, conditions, actions, rule.stop],
            )?;
            self.db.last_insert_rowid()
        } else {
            let changed = self.db.execute(
                "UPDATE rules SET name = ?2, enabled = ?3, account = ?4, match_all = ?5,
                     conditions = ?6, actions = ?7, stop = ?8
                 WHERE id = ?1",
                params![
                    rule.id,
                    name,
                    rule.enabled,
                    rule.account,
                    rule.match_all,
                    conditions,
                    actions,
                    rule.stop
                ],
            )?;
            if changed == 0 {
                return Err(rule_gone());
            }
            rule.id
        };
        self.rules()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(rule_gone)
    }

    pub fn delete_rule(&mut self, id: i64) -> Result<(), StoreError> {
        self.db.execute("DELETE FROM rules WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Move a rule up (`-1`) or down (`1`) in the order.
    pub fn move_rule(&mut self, id: i64, delta: i64) -> Result<(), StoreError> {
        let ids: Vec<i64> = self.rules()?.iter().map(|r| r.id).collect();
        let Some(from) = ids.iter().position(|&r| r == id) else {
            return Ok(());
        };
        let to = (from as i64 + delta).clamp(0, ids.len() as i64 - 1) as usize;
        let mut order = ids;
        let moved = order.remove(from);
        order.insert(to, moved);
        let tx = self.db.transaction()?;
        for (pos, rule) in order.iter().enumerate() {
            tx.execute(
                "UPDATE rules SET position = ?2 WHERE id = ?1",
                params![rule, pos as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// When a rule last auto-replied to this sender.
    pub fn last_auto_reply(
        &self,
        rule: i64,
        sender: &str,
    ) -> Result<Option<NaiveDateTime>, StoreError> {
        Ok(self
            .db
            .query_row(
                "SELECT sent_at FROM auto_replies WHERE rule = ?1 AND sender = ?2",
                params![rule, sender],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn record_auto_reply(
        &mut self,
        rule: i64,
        sender: &str,
        at: NaiveDateTime,
    ) -> Result<(), StoreError> {
        self.db.execute(
            "INSERT INTO auto_replies (rule, sender, sent_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (rule, sender) DO UPDATE SET sent_at = excluded.sent_at",
            params![rule, sender, at],
        )?;
        Ok(())
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
                     imap_user = ?6, smtp_host = ?7, smtp_port = ?8, smtp_security = ?9,
                     imap_security = ?10
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
                    setup.smtp_security.as_str(),
                    setup.imap_security.as_str()
                ],
            )?;
            id
        } else {
            let id = self.unique_account_id(&setup.email);
            let tx = self.db.transaction()?;
            tx.execute(
                "INSERT INTO accounts (id, position, name, email, imap_host, imap_port, imap_user,
                     smtp_host, smtp_port, smtp_security, imap_security)
                 VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM accounts WHERE id <> 'local'),
                     ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    id,
                    setup.name,
                    setup.email,
                    setup.host,
                    setup.port,
                    setup.username,
                    setup.smtp_host,
                    setup.smtp_port,
                    setup.smtp_security.as_str(),
                    setup.imap_security.as_str()
                ],
            )?;
            tx.execute(
                "INSERT INTO folders (account, id, position, name, kind) VALUES (?1, ?2, 1, 'Outbox', 'outbox')",
                params![id, OUTBOX_ID],
            )?;
            insert_identity(&tx, &Identity::new(&id, &setup.name, &setup.email), 0)?;
            // The first server account brings the folders on this PC.
            tx.execute_batch(ENSURE_LOCAL)?;
            tx.commit()?;
            id
        };
        self.reload_accounts()?;
        self.account(&id).cloned()
    }

    /// Move a server account up (`-1`) or down (`1`) in the folder tree. The folders on this
    /// PC stay last.
    pub fn move_account(&mut self, id: &str, delta: i64) -> Result<(), StoreError> {
        self.account(id)?;
        let mut order: Vec<String> = self
            .accounts
            .iter()
            .filter(|a| a.id != LOCAL_ID)
            .map(|a| a.id.clone())
            .collect();
        let Some(from) = order.iter().position(|a| a == id) else {
            return Ok(()); // the folders on this PC
        };
        let to = (from as i64 + delta).clamp(0, order.len() as i64 - 1) as usize;
        if to == from {
            return Ok(());
        }
        let moved = order.remove(from);
        order.insert(to, moved);
        let tx = self.db.transaction()?;
        for (pos, account) in order.iter().enumerate() {
            tx.execute(
                "UPDATE accounts SET position = ?2 WHERE id = ?1",
                params![account, pos as i64],
            )?;
        }
        tx.commit()?;
        self.reload_accounts()
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
        while id == LOCAL_ID || self.accounts.iter().any(|a| a.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    /// Remove an account, every local copy of its mail and its queued changes.
    /// The server is not touched. The folders on this PC can't be removed this way: their
    /// mail is nowhere else, and it stays when the accounts it came from go.
    pub fn remove_account(&mut self, id: &str) -> Result<(), StoreError> {
        if id == LOCAL_ID {
            return Err(StoreError::Invalid(tr!(
                "The folders on this PC stay: their mail is nowhere else.",
                "Foldery na tym komputerze zostają: ich poczty nie ma nigdzie indziej."
            )));
        }
        self.account(id)?;
        let tx = self.db.transaction()?;
        tx.execute(
            "DELETE FROM messages WHERE substr(folder, 1, length(?1) + 1) = ?1 || '/'",
            [id],
        )?;
        tx.execute("DELETE FROM pending_ops WHERE account = ?1", [id])?;
        tx.execute("DELETE FROM outbox WHERE account = ?1", [id])?;
        tx.execute("DELETE FROM rules WHERE account = ?1", [id])?;
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

    // ---- Folders on this PC ----

    /// The folders on this PC, once the mail base has them: from the first server account on.
    pub fn this_pc(&self) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|a| a.id == LOCAL_ID && a.imap.is_none())
    }

    /// A name for a folder on this PC, trimmed, or what is wrong with it. `except` is the
    /// folder being renamed, which may keep its own name.
    fn check_local_name(&self, name: &str, except: Option<&str>) -> Result<String, StoreError> {
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            return Err(StoreError::Invalid(tr!(
                "Type a name for the folder.",
                "Wpisz nazwę folderu."
            )));
        }
        if name.chars().count() > FOLDER_NAME_MAX {
            return Err(StoreError::Invalid(tr!(
                "That name is too long: {FOLDER_NAME_MAX} characters at most.",
                "Ta nazwa jest za długa: najwyżej {FOLDER_NAME_MAX} znaków."
            )));
        }
        let pc = self.this_pc().ok_or_else(no_local_folders)?;
        let taken = pc.folders.iter().any(|f| {
            Some(f.id.as_str()) != except
                && (f.name.to_lowercase() == name.to_lowercase()
                    || f.kind
                        .pl_name()
                        .is_some_and(|pl| pl.to_lowercase() == name.to_lowercase()))
        });
        if taken {
            return Err(StoreError::Invalid(tr!(
                "There is a folder called \"{name}\" on this PC already.",
                "Na tym komputerze jest już folder „{name}”."
            )));
        }
        Ok(name)
    }

    /// Make a folder on this PC. Returns its key.
    pub fn create_local_folder(&mut self, name: &str) -> Result<String, StoreError> {
        let name = self.check_local_name(name, None)?;
        let pc = self.this_pc().ok_or_else(no_local_folders)?;
        let base = match slug(&name) {
            s if s.is_empty() => "folder".to_owned(),
            s => s,
        };
        let mut id = base.clone();
        let mut n = 2;
        while id == OUTBOX_ID || pc.folders.iter().any(|f| f.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        self.db.execute(
            "INSERT INTO folders (account, id, position, name, kind) VALUES (?1, ?2, 2, ?3, 'custom')",
            params![LOCAL_ID, id, name],
        )?;
        self.reload_accounts()?;
        Ok(format!("{LOCAL_ID}/{id}"))
    }

    /// The user's own folder on this PC, by key: Archive and Trash there are for good.
    fn own_local_folder(&self, key: &str) -> Result<Folder, StoreError> {
        let folder = match split_folder_key(key) {
            Some((LOCAL_ID, _)) => self.folder(key).cloned(),
            _ => None,
        }
        .ok_or_else(|| StoreError::UnknownFolder(key.to_owned()))?;
        if folder.kind != FolderKind::Custom {
            return Err(StoreError::Invalid(tr!(
                "Archive and Trash on this PC stay as they are.",
                "Archiwum i Kosz na tym komputerze zostają takie, jakie są."
            )));
        }
        Ok(folder)
    }

    /// Give a folder on this PC another name. Its key stays, so rules keep working.
    pub fn rename_local_folder(&mut self, key: &str, name: &str) -> Result<(), StoreError> {
        let folder = self.own_local_folder(key)?;
        let name = self.check_local_name(name, Some(&folder.id))?;
        self.db.execute(
            "UPDATE folders SET name = ?3 WHERE account = ?1 AND id = ?2",
            params![LOCAL_ID, folder.id, name],
        )?;
        self.reload_accounts()
    }

    /// Remove an empty folder on this PC, and the rules' moves to it with it: a rule that
    /// would have nothing left to do goes too. Returns how many rules changed.
    pub fn delete_local_folder(&mut self, key: &str) -> Result<usize, StoreError> {
        let folder = self.own_local_folder(key)?;
        let held: i64 = self.db.query_row(
            "SELECT COUNT(*) FROM messages WHERE folder = ?1",
            [key],
            |r| r.get(0),
        )?;
        if held > 0 {
            let name = &folder.name;
            return Err(StoreError::Invalid(tr!(
                "Move or delete the messages in \"{name}\" first.",
                "Najpierw przenieś albo usuń wiadomości z folderu „{name}”."
            )));
        }
        let mut changed = 0;
        for mut rule in self.rules()? {
            let before = rule.actions.len();
            rule.actions
                .retain(|a| !matches!(a, Action::Move { folder } if folder == key));
            if rule.actions.len() == before {
                continue;
            }
            changed += 1;
            if rule.actions.is_empty() {
                self.delete_rule(rule.id)?;
            } else {
                self.db.execute(
                    "UPDATE rules SET actions = ?2 WHERE id = ?1",
                    params![
                        rule.id,
                        serde_json::to_string(&rule.actions)
                            .map_err(|e| StoreError::Database(e.to_string()))?
                    ],
                )?;
            }
        }
        self.db.execute(
            "DELETE FROM folders WHERE account = ?1 AND id = ?2",
            params![LOCAL_ID, folder.id],
        )?;
        self.reload_accounts()?;
        Ok(changed)
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

    /// Remember how many messages of a server folder are on the server, so the window can
    /// say how many older ones weren't downloaded. Returns that number.
    pub fn note_server_count(
        &mut self,
        folder_key: &str,
        on_server: usize,
    ) -> Result<usize, StoreError> {
        let (account, folder) = split_folder_key(folder_key)
            .ok_or_else(|| StoreError::UnknownFolder(folder_key.to_owned()))?;
        let here: i64 = self.db.query_row(
            "SELECT COUNT(*) FROM messages WHERE srv_folder = ?1 AND uid IS NOT NULL",
            [folder_key],
            |r| r.get(0),
        )?;
        let missing = on_server.saturating_sub(usize::try_from(here).unwrap_or_default());
        self.db.execute(
            "UPDATE folders SET missing = ?3 WHERE account = ?1 AND id = ?2",
            params![account, folder, missing as i64],
        )?;
        Ok(missing)
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
                "append" => {
                    let flags = r
                        .get::<_, Option<i64>>(3)?
                        .map(|bits| (bits & 1 != 0, bits & 2 != 0));
                    r.get::<_, Option<String>>(4)?
                        .map(|to| ChangeKind::Append { to, flags })
                }
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

    /// The server saved the message in `folder_key` (a copy of sent mail, or mail from this
    /// PC). Its UID comes with the next download, which recognizes it by Message-ID instead of
    /// storing it twice. When the message moved on (or went) while it was being uploaded, the
    /// copy on the server is removed again.
    pub fn change_appended(
        &mut self,
        op: i64,
        id: MessageId,
        folder_key: &str,
    ) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM pending_ops WHERE id = ?1", [op])?;
        let now: Option<(String, Option<String>, String)> = tx
            .query_row(
                "SELECT folder, message_id, subject FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        match now {
            Some((folder, ..)) if folder == folder_key => {
                tx.execute(
                    "UPDATE messages SET srv_folder = ?1 WHERE id = ?2 AND srv_folder IS NULL",
                    params![folder_key, id as i64],
                )?;
            }
            Some((_, Some(message_id), subject)) => {
                tx.execute(
                    "INSERT INTO pending_ops (account, kind, srv_folder, message_id, subject, origin)
                     VALUES (?1, 'expunge', ?2, ?3, ?4, ?5)",
                    params![account_of(folder_key), folder_key, message_id, subject, id as i64],
                )?;
            }
            _ => {}
        }
        tx.commit()?;
        Ok(())
    }

    /// The server moved the message to `folder_key`, where its UID is `uid` if known. When it
    /// was moved to this PC meanwhile, it leaves the server from its new place instead.
    pub fn change_moved(
        &mut self,
        op: i64,
        id: MessageId,
        folder_key: &str,
        uid: Option<u32>,
    ) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM pending_ops WHERE id = ?1", [op])?;
        let here: Option<(Option<String>, Option<String>, String)> = tx
            .query_row(
                "SELECT srv_folder, message_id, subject FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        match here {
            Some((None, message_id, subject)) => {
                tx.execute(
                    "UPDATE pending_ops SET srv_folder = ?2, uid = ?3
                     WHERE kind = 'expunge' AND origin = ?1",
                    params![id as i64, folder_key, uid],
                )?;
                if tx.changes() == 0 {
                    tx.execute(
                        "INSERT INTO pending_ops (account, kind, srv_folder, uid, message_id, subject, origin)
                         VALUES (?1, 'expunge', ?2, ?3, ?4, ?5, ?6)",
                        params![account_of(folder_key), folder_key, uid, message_id, subject, id as i64],
                    )?;
                }
            }
            Some(_) => set_server_location(&tx, id, folder_key, uid)?,
            None => {}
        }
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

impl Drop for MailStore {
    fn drop(&mut self) {
        self.optimize();
    }
}

/// Bring the database up to the newest schema, one step at a time.
fn migrate(db: &Connection) -> Result<(), StoreError> {
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let version = usize::try_from(version).unwrap_or_default();
    if version > MIGRATIONS.len() {
        return Err(StoreError::Database(tr!(
            "it was made by a newer version of Telex (schema {version})",
            "utworzyła ją nowsza wersja Telexa (schemat {version})"
        )));
    }
    for (step, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        let reached = step + 1;
        let tx = db.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        if reached == 2 {
            backfill_message_ids(&tx)?;
        }
        if reached == 9 {
            backfill_refs(&tx)?;
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

/// Version 9 added `refs`; fill it in from the headers of mail stored before, one message
/// at a time so a large mail base isn't read into memory at once.
fn backfill_refs(db: &Connection) -> Result<(), StoreError> {
    let ids: Vec<i64> = {
        let mut stmt = db.prepare("SELECT id FROM message_content WHERE raw IS NOT NULL")?;
        let ids = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        ids
    };
    // Only the start of each message: the headers are there, attachments aren't needed.
    let mut read = db.prepare("SELECT substr(raw, 1, 65536) FROM message_content WHERE id = ?1")?;
    let mut write = db.prepare("UPDATE messages SET refs = ?1 WHERE id = ?2")?;
    for id in ids {
        let raw: Vec<u8> = read.query_row([id], |r| r.get(0))?;
        let refs = crate::mime::answered_ids_of(&raw);
        if !refs.is_empty() {
            write.execute(params![refs.join(" "), id])?;
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

/// The two flags the user changes, with their column and their kind in `pending_ops`.
#[derive(Debug, Clone, Copy)]
enum Flag {
    /// The column is `unread`; the server's flag is \Seen, the other way round.
    Unread,
    Flagged,
}

/// Change a flag on messages here and, for server mail, queue the change, all in one
/// transaction. Only the latest value of each flag is kept in the queue. With
/// `only_changed`, messages that already have the value (or are gone) are skipped; without,
/// every message is queued, so an explicit toggle always reaches the server. Returns how
/// many messages were changed or queued.
fn set_flags(
    db: &mut Connection,
    ids: &[MessageId],
    flag: Flag,
    value: bool,
    only_changed: bool,
) -> Result<usize, StoreError> {
    let (column, kind, server_value) = match flag {
        Flag::Unread => ("unread", "seen", !value),
        Flag::Flagged => ("flagged", "flagged", value),
    };
    let tx = db.transaction()?;
    let mut done = 0;
    {
        let mut update = tx.prepare_cached(&format!(
            "UPDATE messages SET {column} = ?1 WHERE id = ?2 AND ({column} <> ?1 OR NOT ?3)
             RETURNING srv_folder"
        ))?;
        let mut forget =
            tx.prepare_cached("DELETE FROM pending_ops WHERE message = ?1 AND kind = ?2")?;
        let mut queue = tx.prepare_cached(
            "INSERT INTO pending_ops (account, message, kind, value) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for &id in ids {
            let changed: Option<Option<String>> = update
                .query_row(params![value, id as i64, only_changed], |r| r.get(0))
                .optional()?;
            let Some(srv) = changed else { continue };
            done += 1;
            if let Some(srv) = srv {
                forget.execute(params![id as i64, kind])?;
                queue.execute(params![account_of(&srv), id as i64, kind, server_value])?;
            }
        }
    }
    tx.commit()?;
    Ok(done)
}

/// File one message in `folder` of its own account, and queue the move for server mail. A
/// message that is still to be uploaded (a copy of sent mail) goes up to its new folder
/// instead, or stays here when that folder is on no server (`on_server`).
fn move_row(
    db: &Connection,
    id: MessageId,
    srv_folder: Option<&str>,
    folder: &str,
    account: &str,
    on_server: bool,
) -> Result<(), StoreError> {
    db.prepare_cached("UPDATE messages SET folder = ?1 WHERE id = ?2")?
        .execute(params![folder, id as i64])?;
    // Leaving the Outbox means it's no longer to be sent.
    db.prepare_cached("DELETE FROM outbox WHERE message = ?1")?
        .execute([id as i64])?;
    match srv_folder {
        Some(srv) => {
            // One move per message: moving again replaces it, moving back cancels it.
            db.prepare_cached("DELETE FROM pending_ops WHERE message = ?1 AND kind = 'move'")?
                .execute([id as i64])?;
            if srv != folder {
                db.prepare_cached(
                    "INSERT INTO pending_ops (account, message, kind, target) VALUES (?1, ?2, 'move', ?3)",
                )?
                .execute(params![account, id as i64, folder])?;
            }
        }
        None if on_server => {
            db.prepare_cached(
                "UPDATE pending_ops SET target = ?2 WHERE message = ?1 AND kind = 'append'",
            )?
            .execute(params![id as i64, folder])?;
        }
        None => {
            db.prepare_cached("DELETE FROM pending_ops WHERE message = ?1 AND kind = 'append'")?
                .execute([id as i64])?;
        }
    }
    Ok(())
}

/// File a message on this PC. Server mail leaves the server: its queued changes are moot,
/// and an expunge that remembers where it was, and which message it came from, goes out
/// instead (see [`from_this_pc`]).
fn to_this_pc(
    db: &Connection,
    id: MessageId,
    srv_folder: Option<&str>,
    folder: &str,
) -> Result<(), StoreError> {
    let (uid, message_id, subject): (Option<u32>, Option<String>, String) = db.query_row(
        "SELECT uid, message_id, subject FROM messages WHERE id = ?1",
        [id as i64],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    db.execute("DELETE FROM pending_ops WHERE message = ?1", [id as i64])?;
    db.execute("DELETE FROM outbox WHERE message = ?1", [id as i64])?;
    db.execute(
        "UPDATE messages SET folder = ?1, srv_folder = NULL, uid = NULL WHERE id = ?2",
        params![folder, id as i64],
    )?;
    if let Some(srv) = srv_folder {
        if uid.is_some() || message_id.is_some() {
            db.execute(
                "INSERT INTO pending_ops (account, kind, srv_folder, uid, message_id, subject, origin)
                 VALUES (?1, 'expunge', ?2, ?3, ?4, ?5, ?6)",
                params![account_of(srv), srv, uid, message_id, subject, id as i64],
            )?;
        }
    }
    Ok(())
}

/// Bring a message from this PC to `folder` of the server account `account`. When it left that
/// account so recently that the server hasn't removed it yet, the expunge is taken back and it
/// simply moves to `folder` there, with the read state and flag it has now. Otherwise it is
/// uploaded, with them; to a folder on no server (`on_server` false) it just moves.
fn from_this_pc(
    db: &Connection,
    id: MessageId,
    folder: &str,
    account: &str,
    on_server: bool,
) -> Result<(), StoreError> {
    let queued: Option<(i64, String, Option<u32>)> = db
        .query_row(
            "SELECT id, srv_folder, uid FROM pending_ops
             WHERE kind = 'expunge' AND origin = ?1 AND account = ?2 ORDER BY id DESC LIMIT 1",
            params![id as i64, account],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (unread, flagged): (bool, bool) = db.query_row(
        "SELECT unread, flagged FROM messages WHERE id = ?1",
        [id as i64],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    db.execute(
        "DELETE FROM pending_ops WHERE message = ?1 AND kind = 'append'",
        [id as i64],
    )?;
    db.execute(
        "UPDATE messages SET folder = ?1 WHERE id = ?2",
        params![folder, id as i64],
    )?;
    let queue = "INSERT INTO pending_ops (account, message, kind, value, target) VALUES (?1, ?2, ?3, ?4, ?5)";
    if let Some((op, srv, uid)) = queued {
        db.execute("DELETE FROM pending_ops WHERE id = ?1", [op])?;
        set_server_location(db, id, &srv, uid)?;
        if srv != folder {
            db.execute(
                queue,
                params![account, id as i64, "move", None::<i64>, folder],
            )?;
        }
        db.execute(
            queue,
            params![account, id as i64, "seen", !unread, None::<String>],
        )?;
        db.execute(
            queue,
            params![account, id as i64, "flagged", flagged, None::<String>],
        )?;
    } else if on_server {
        let flags = i64::from(!unread) | i64::from(flagged) << 1;
        db.execute(queue, params![account, id as i64, "append", flags, folder])?;
    }
    Ok(())
}

/// Remove one message for good; for server mail, queue the expunge, which remembers where
/// the message is on the server once the row is gone.
fn remove_row(db: &Connection, id: MessageId, srv_folder: Option<&str>) -> Result<(), StoreError> {
    if let Some(srv) = srv_folder {
        let (uid, message_id, subject): (Option<u32>, Option<String>, String) = db.query_row(
            "SELECT uid, message_id, subject FROM messages WHERE id = ?1",
            [id as i64],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        db.execute("DELETE FROM messages WHERE id = ?1", [id as i64])?;
        if uid.is_some() || message_id.is_some() {
            db.execute(
                "INSERT INTO pending_ops (account, kind, srv_folder, uid, message_id, subject)
                 VALUES (?1, 'expunge', ?2, ?3, ?4, ?5)",
                params![account_of(srv), srv, uid, message_id, subject],
            )?;
        }
    } else {
        db.execute("DELETE FROM messages WHERE id = ?1", [id as i64])?;
    }
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
/// ł is folded to l, as the index triggers in migrations/007_message_content.sql do.
pub(crate) fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|w| w.replace('"', "").replace('ł', "l").replace('Ł', "L"))
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) const SUMMARY_COLUMNS: &str =
    "id, folder, from_name, from_email, to_addr, subject, date, \
     size_kb, unread, flagged, attachments <> '[]'";

pub(crate) fn summary_from_row(r: &Row<'_>) -> rusqlite::Result<MessageSummary> {
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

const MESSAGE_COLUMNS: &str = "m.id, m.folder, m.from_name, m.from_email, m.to_addr, m.cc, \
     m.subject, m.date, m.size_kb, m.unread, m.flagged, m.remote_images, m.note, m.attachments, \
     COALESCE(c.body, ''), m.message_id, m.bcc, m.refs";

fn message_from_row(r: &Row<'_>) -> rusqlite::Result<Message> {
    let attachments: String = r.get(13)?;
    let refs: String = r.get(17)?;
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
        bcc: r.get(16)?,
        refs: refs.split_whitespace().map(str::to_owned).collect(),
    })
}

fn load_accounts(db: &Connection) -> Result<Vec<Account>, StoreError> {
    let mut stmt = db.prepare(
        "SELECT id, name, email, imap_host, imap_port, imap_user, smtp_host, smtp_port, smtp_security,
             imap_security
         FROM accounts ORDER BY id = 'local', position, id",
    )?;
    let mut accounts = stmt
        .query_map([], |r| {
            let imap = match r.get::<_, Option<String>>(3)? {
                Some(host) => Some(ImapSettings {
                    host,
                    port: r.get(4)?,
                    username: r.get(5)?,
                    security: SmtpSecurity::from_name(
                        &r.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    ),
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
                identities: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut stmt = db.prepare(
        "SELECT id, account, name, email, signature, reply_header, forward_header, is_default
         FROM identities ORDER BY position, id",
    )?;
    let identities = stmt.query_map([], |r| {
        Ok(Identity {
            id: r.get(0)?,
            account: r.get(1)?,
            name: r.get(2)?,
            email: r.get(3)?,
            signature: r.get(4)?,
            reply_header: r.get(5)?,
            forward_header: r.get(6)?,
            is_default: r.get(7)?,
        })
    })?;
    for identity in identities {
        let identity = identity?;
        if let Some(a) = accounts.iter_mut().find(|a| a.id == identity.account) {
            a.identities.push(identity);
        }
    }
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
    // Server folders arrive in plan order and the Outbox is local; rank puts it in place. The
    // user's own folders on this PC go A to Z.
    for a in &mut accounts {
        let by_name = a.id == LOCAL_ID;
        a.folders.sort_by(|x, y| {
            x.kind.rank().cmp(&y.kind.rank()).then_with(|| {
                if by_name && x.kind == FolderKind::Custom && y.kind == FolderKind::Custom {
                    x.name.to_lowercase().cmp(&y.name.to_lowercase())
                } else {
                    std::cmp::Ordering::Equal
                }
            })
        });
    }
    Ok(accounts)
}

/// If none of the account's identities is the default, the first one becomes it.
fn ensure_a_default(db: &Connection, account: &str) -> Result<(), StoreError> {
    db.execute(
        "UPDATE identities SET is_default = 1
         WHERE id = (SELECT id FROM identities WHERE account = ?1 ORDER BY position, id LIMIT 1)
           AND NOT EXISTS (SELECT 1 FROM identities WHERE account = ?1 AND is_default)",
        [account],
    )?;
    Ok(())
}

fn insert_identity(db: &Connection, i: &Identity, position: usize) -> Result<i64, StoreError> {
    db.execute(
        "INSERT INTO identities (account, position, name, email, signature, reply_header,
             forward_header, is_default)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            i.account,
            position as i64,
            i.name.trim(),
            i.email.trim(),
            i.signature,
            i.reply_header,
            i.forward_header,
            i.is_default
        ],
    )?;
    Ok(db.last_insert_rowid())
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
pub(crate) fn insert_message(
    db: &Connection,
    folder: &str,
    srv_folder: Option<&str>,
    m: &Message,
    uid: Option<u32>,
    raw: Option<&[u8]>,
) -> Result<MessageId, StoreError> {
    let attachments =
        serde_json::to_string(&m.attachments).map_err(|e| StoreError::Database(e.to_string()))?;
    db.prepare_cached(
        "INSERT INTO messages (folder, srv_folder, uid, message_id, from_name, from_email, to_addr,
             cc, subject, date, size_kb, unread, flagged, remote_images, note, attachments, bcc, refs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
    )?
    .execute(params![
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
        m.bcc,
        m.refs.join(" "),
    ])?;
    let id = db.last_insert_rowid();
    // The text goes in second: its trigger indexes it together with the summary.
    db.prepare_cached("INSERT INTO message_content (id, body, raw) VALUES (?1, ?2, ?3)")?
        .execute(params![id, m.body, raw])?;
    Ok(id as MessageId)
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
                bcc: String::new(),
                refs: Vec::new(),
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
            MailStore::with_connection(db, false).unwrap()
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
            .save_rule(&rule("I flaga", vec![to_key, Action::Flag]))
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

        let mut store = MailStore::with_connection(db, false).unwrap();
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
            [b"Message-ID: <b@x.pl>\r\nIn-Reply-To: <a@x.pl>\r\nSubject: Re: Wycena\r\n\r\nOK\r\n".to_vec()],
        )
        .unwrap();
        let store = MailStore::with_connection(db, false).unwrap();
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
        assert!(!store.auto_load_images(id).unwrap());
        store
            .set_sender_trusted(&sender.to_uppercase(), true)
            .unwrap();
        assert!(
            store.is_sender_trusted(&sender).unwrap(),
            "addresses ignore case"
        );
        assert!(store.auto_load_images(id).unwrap());
        assert_eq!(store.trusted_senders().unwrap(), [sender.to_uppercase()]);
        store.move_to(id, "firma/spam").unwrap();
        assert!(!store.auto_load_images(id).unwrap(), "never in Spam");
        store.move_to(id, "firma/inbox").unwrap();
        store.set_sender_trusted(&sender, false).unwrap();
        assert!(!store.auto_load_images(id).unwrap());
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
}
