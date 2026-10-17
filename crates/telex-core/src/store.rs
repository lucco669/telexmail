use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

use chrono::NaiveDateTime;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::filters::{Action, Rule};
use crate::html;
use crate::imap;
use crate::list;
use crate::mime::parse_message_id;
use crate::model::{
    account_of, split_folder_key, Account, AccountSetup, Attachment, Folder, FolderKind, Identity,
    ImapSettings, Message, MessageId, MessageSummary, OutgoingDraft, Pop3Keep, SmtpSecurity,
    SmtpSettings,
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
    include_str!("migrations/011_folder_choices.sql"),
    include_str!("migrations/012_away.sql"),
    include_str!("migrations/013_saved_searches_and_rule_actions.sql"),
    include_str!("migrations/014_spam_filter.sql"),
    include_str!("migrations/015_pop3.sql"),
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

/// The folders a POP3 account starts with besides the Outbox, by id (also their kind) and
/// name. They are all here, like the folders on this PC; the user adds more.
const POP3_FOLDERS: [(&str, &str); 6] = [
    ("inbox", "Inbox"),
    ("drafts", "Drafts"),
    ("sent", "Sent"),
    ("archive", "Archive"),
    ("spam", "Spam"),
    ("trash", "Trash"),
];

/// Delete a message that exists only here (a draft, an unsent message). Server mail and
/// unknown ids are left alone: replacing never deletes anything on a server.
const REMOVE_LOCAL: &str = "DELETE FROM messages WHERE id = ?1 AND srv_folder IS NULL";
/// Delete a draft that exists only here, in an account's Drafts.
const DISCARD_DRAFT: &str =
    "DELETE FROM messages WHERE id = ?1 AND srv_folder IS NULL AND folder IN
     (SELECT account || '/' || id FROM folders WHERE kind = 'drafts')";

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
/// What `MailStore::image_trust` says about a message's remote images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageTrust {
    pub sender: String,
    pub trusted: bool,
    /// They load without asking.
    pub auto: bool,
}

#[derive(Debug)]
pub struct MailStore {
    pub(crate) db: Connection,
    accounts: Vec<Account>,
    /// The tests' sample mailbox (`sample.rs`), and its messages waiting for "Get mail".
    #[cfg(test)]
    sample: bool,
    #[cfg(test)]
    incoming: Vec<Message>,
    /// The last filtered list the UI paged through (see `list`).
    pub(crate) found: RefCell<Option<list::Found>>,
    pub(crate) grouped: RefCell<Option<list::Grouped>>,
    /// The reply graph thread rows are made from (see `list::Graph`).
    pub(crate) graph: RefCell<Option<list::Graph>>,
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
        Self::with_connection(db)
    }

    /// An empty store that lives only in memory.
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(db: Connection) -> Result<Self, StoreError> {
        db.pragma_update(None, "foreign_keys", true)?;
        // A sync uses about twenty statements over and over; the default keeps 16.
        db.set_prepared_statement_cache_capacity(64);
        list::register_functions(&db)?;
        migrate(&db)?;
        list::watch_shape(&db)?;
        let accounts = load_accounts(&db)?;
        Ok(Self {
            db,
            accounts,
            #[cfg(test)]
            sample: false,
            #[cfg(test)]
            incoming: Vec::new(),
            found: RefCell::new(None),
            grouped: RefCell::new(None),
            graph: RefCell::new(None),
            thread: RefCell::new(None),
        })
    }

    /// The sample mailbox: built in memory from fixed data and never saved.
    #[cfg(test)]
    pub(crate) fn new(
        accounts: Vec<Account>,
        messages: Vec<Message>,
        incoming: Vec<Message>,
    ) -> Result<Self, StoreError> {
        let mut store = Self::with_connection(Connection::open_in_memory()?)?;
        store.sample = true;
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
    #[cfg(test)]
    pub(crate) fn is_sample(&self) -> bool {
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

    #[cfg(test)]
    pub(crate) fn unread_count(&self, folder: &str) -> Result<usize, StoreError> {
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

    /// The opened message with an offline, advisory risk score. No mail or flags change.
    pub fn inspect_message(
        &self,
        id: MessageId,
    ) -> Result<crate::risk::InspectedMessage, StoreError> {
        let message = self.get(id)?;
        let risk = if self
            .kind_of(&message.folder)
            .is_some_and(FolderKind::is_outgoing)
        {
            crate::risk::Report::default()
        } else {
            let raw: Option<Vec<u8>> = self.db.query_row(
                "SELECT CASE WHEN length(raw) <= ?2 THEN raw END FROM message_content WHERE id = ?1",
                params![id as i64, crate::risk::MAX_RAW as i64], |r| r.get(0),
            ).optional()?.flatten();
            crate::risk::analyse(&message, raw.as_deref())
        };
        Ok(crate::risk::InspectedMessage { message, risk })
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
    ///
    /// Moving mail into Spam or out of it teaches the spam filter (`spam.rs`), in the same
    /// transaction.
    pub fn move_many(
        &mut self,
        ids: &[MessageId],
        folder: &str,
    ) -> Result<Vec<(MessageId, String)>, StoreError> {
        self.move_and_learn(ids, folder, true)
    }

    /// A move the spam filter doesn't learn from: its own, to Spam.
    pub(crate) fn move_unlearned(
        &mut self,
        ids: &[MessageId],
        folder: &str,
    ) -> Result<Vec<(MessageId, String)>, StoreError> {
        self.move_and_learn(ids, folder, false)
    }

    fn move_and_learn(
        &mut self,
        ids: &[MessageId],
        folder: &str,
        learn: bool,
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
        let lessons = if learn {
            let from: Vec<(MessageId, &str)> =
                places.iter().map(|(id, f, _)| (*id, f.as_str())).collect();
            crate::spam::lessons_for_move(self, &from, folder)?
        } else {
            Vec::new()
        };
        let read = if self.folder_kind(folder) == Some(FolderKind::Trash) {
            self.spam_among(places.iter().map(|(id, f, _)| (*id, f.as_str())))
        } else {
            Vec::new()
        };
        let tx = self.db.transaction()?;
        crate::spam::learn(&tx, &lessons)?;
        // Before the move, so the server hears \Seen where the message still is.
        set_flags_in(&tx, &read, Flag::Unread, false, true)?;
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

    /// Of messages and the folders they are in, those in a Spam folder. Spam that goes to
    /// Trash is marked read on the way: nobody means to read it, and it shouldn't count.
    fn spam_among<'a>(&self, places: impl Iterator<Item = (MessageId, &'a str)>) -> Vec<MessageId> {
        places
            .filter(|(_, f)| self.folder_kind(f) == Some(FolderKind::Spam))
            .map(|(id, _)| id)
            .collect()
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
        let read = self.spam_among(plan.iter().map(|(id, f, _, _)| (*id, f.as_str())));
        let mut report = DeleteReport::default();
        let tx = self.db.transaction()?;
        set_flags_in(&tx, &read, Flag::Unread, false, true)?;
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
        let account = self.account(&draft.account)?.clone();
        let folder = self.folder_key_of_kind(&account.id, kind)?;
        let (replaces, autosaved) = (draft.replaces, draft.autosaved);
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
        // The old version goes only together with the new one coming in.
        let tx = self.db.transaction()?;
        if let Some(old) = replaces {
            tx.execute(REMOVE_LOCAL, [old as i64])?;
        }
        if let Some(old) = autosaved {
            tx.execute(DISCARD_DRAFT, [old as i64])?;
        }
        let id = insert_message(&tx, &folder, None, &message, None, raw)?;
        tx.commit()?;
        Ok(id)
    }

    /// Delete a draft for good: an autosave the user discarded or saved over. Only a draft
    /// that exists only here, in an account's Drafts; anything else is left alone. True when
    /// one was removed.
    pub fn discard_draft(&mut self, id: MessageId) -> Result<bool, StoreError> {
        Ok(self.db.execute(DISCARD_DRAFT, [id as i64])? > 0)
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
        let env_to =
            serde_json::to_string(env_to).map_err(|e| StoreError::Database(e.to_string()))?;
        let tx = self.db.transaction()?;
        if let Some(old) = replaces {
            tx.execute(REMOVE_LOCAL, [old as i64])?;
        }
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
                let note = tr!(
                    "Sent. This account has no Sent folder, so it stays here.",
                    "Wysłano. To konto nie ma folderu Wysłane, więc wiadomość zostaje tutaj."
                );
                tx.execute(
                    "UPDATE messages SET note = ?1 WHERE id = ?2",
                    params![note, id as i64],
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
        let outbox = format!("{account}/{OUTBOX_ID}");
        Ok(self
            .db
            .prepare_cached(
                "SELECT EXISTS (SELECT 1 FROM outbox o JOIN messages m ON m.id = o.message
                 JOIN message_content c ON c.id = o.message
             WHERE o.account = ?1 AND m.folder = ?2 AND NOT o.refused
               AND c.raw IS NOT NULL AND (o.send_after IS NULL OR o.send_after <= ?3))",
            )?
            .query_row(params![account, outbox, now], |r| r.get(0))?)
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

    /// Messages as Markdown, oldest first (`markdown.rs`): what Copy as Markdown puts on the
    /// clipboard. Changes nothing, not even the read state.
    pub fn markdown(&self, ids: &[MessageId]) -> Result<String, StoreError> {
        let mut all = ids
            .iter()
            .map(|&id| Ok((self.get(id)?, self.raw_of(id)?)))
            .collect::<Result<Vec<_>, StoreError>>()?;
        all.sort_by_key(|(m, _)| (m.date, m.id));
        Ok(crate::markdown::messages(
            all.iter().map(|(m, raw)| (m, raw.as_deref())),
        ))
    }

    /// Copy and remember this rule action in one transaction. The original stays put.
    pub fn rule_copy(
        &mut self,
        rule: i64,
        id: MessageId,
        folder: &str,
    ) -> Result<bool, StoreError> {
        let message = self.get(id)?;
        if message.folder == folder {
            return Ok(false);
        }
        let owner = self.check_folder(folder)?.to_owned();
        if self.kind_of(folder) == Some(FolderKind::Outbox) {
            return Err(StoreError::Invalid(crate::tr!(
                "A rule cannot copy into the Outbox.",
                "Reguła nie może kopiować do kolejki wysyłania."
            )));
        }
        let raw = self.raw_of(id)?;
        let on_server = self.folder(folder).is_some_and(|f| f.remote.is_some());
        if on_server && raw.is_none() {
            return Err(StoreError::Invalid(crate::tr!(
                "The original message is needed to copy it to a server.",
                "Kopiowanie na serwer wymaga oryginalnej wiadomości."
            )));
        }
        let action = format!("copy:{folder}");
        let tx = self.db.transaction()?;
        let recorded = tx.execute(
            "INSERT OR IGNORE INTO rule_actions_done (rule,message,action) VALUES (?1,?2,?3)",
            params![rule, id as i64, action],
        )?;
        if recorded == 0 {
            return Ok(false);
        }
        let copy = insert_message(&tx, folder, None, &message, None, raw.as_deref())?;
        if on_server {
            from_this_pc(&tx, copy, folder, &owner, true)?;
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn rule_action_done(
        &self,
        rule: i64,
        id: MessageId,
        action: &str,
    ) -> Result<bool, StoreError> {
        Ok(self.db.query_row("SELECT EXISTS(SELECT 1 FROM rule_actions_done WHERE rule=?1 AND message=?2 AND action=?3)", params![rule,id as i64,action], |r| r.get(0))?)
    }

    pub fn queue_rule_forward(
        &mut self,
        rule: i64,
        original: MessageId,
        action: &str,
        account: &str,
        composed: &crate::smtp::Composed,
    ) -> Result<bool, StoreError> {
        let outbox = self.folder_key_of_kind(account, FolderKind::Outbox)?;
        let env_to = serde_json::to_string(&composed.env_to)
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let tx = self.db.transaction()?;
        if tx.execute(
            "INSERT OR IGNORE INTO rule_actions_done (rule,message,action) VALUES (?1,?2,?3)",
            params![rule, original as i64, action],
        )? == 0
        {
            return Ok(false);
        }
        let id = insert_message(
            &tx,
            &outbox,
            None,
            &composed.message,
            None,
            Some(&composed.raw),
        )?;
        tx.execute(
            "INSERT INTO outbox (message,account,env_from,env_to) VALUES (?1,?2,?3,?4)",
            params![id as i64, account, composed.env_from, env_to],
        )?;
        tx.commit()?;
        Ok(true)
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

    /// A message's sender, whether the user trusts them, and so whether its remote images
    /// load without asking: a trusted sender, and not in Spam.
    pub fn image_trust(&self, id: MessageId) -> Result<ImageTrust, StoreError> {
        let (folder, sender): (String, String) = self
            .db
            .query_row(
                "SELECT folder, from_email FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))?;
        let trusted = self.is_sender_trusted(&sender)?;
        let auto = trusted && self.folder_kind(&folder) != Some(FolderKind::Spam);
        Ok(ImageTrust {
            sender,
            trusted,
            auto,
        })
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
    #[cfg(test)]
    pub(crate) fn fetch_new(&mut self) -> Result<Vec<MessageId>, StoreError> {
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
            .map(|a| {
                (
                    a.id.clone(),
                    a.imap.as_ref().is_some_and(|i| i.pop3.is_some()),
                )
            });
        let keep = setup.pop3.unwrap_or_default();
        let incoming = if setup.pop3.is_some() { "pop3" } else { "imap" };
        let id = if let Some((id, was_pop3)) = existing {
            // The folders of the two are different things: IMAP's are the server's, POP3's
            // are here. Switching would lose one or the other.
            if was_pop3 != setup.pop3.is_some() && id != LOCAL_ID {
                let email = &setup.email;
                return Err(StoreError::Invalid(if was_pop3 {
                    tr!(
                        "{email} gets its mail over POP3. To use IMAP instead, remove the account and add it again.",
                        "{email} odbiera pocztę przez POP3. Żeby używać IMAP, usuń konto i dodaj je ponownie."
                    )
                } else {
                    tr!(
                        "{email} gets its mail over IMAP. To use POP3 instead, remove the account and add it again.",
                        "{email} odbiera pocztę przez IMAP. Żeby używać POP3, usuń konto i dodaj je ponownie."
                    )
                }));
            }
            self.db.execute(
                "UPDATE accounts SET name = ?2, email = ?3, imap_host = ?4, imap_port = ?5,
                     imap_user = ?6, smtp_host = ?7, smtp_port = ?8, smtp_security = ?9,
                     imap_security = ?10, pop3_leave = ?11, pop3_days = ?12,
                     pop3_until_deleted = ?13
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
                    setup.imap_security.as_str(),
                    keep.leave,
                    keep.days,
                    keep.until_deleted
                ],
            )?;
            id
        } else {
            let id = self.unique_account_id(&setup.email);
            let tx = self.db.transaction()?;
            tx.execute(
                "INSERT INTO accounts (id, position, name, email, imap_host, imap_port, imap_user,
                     smtp_host, smtp_port, smtp_security, imap_security, incoming, pop3_leave,
                     pop3_days, pop3_until_deleted)
                 VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM accounts WHERE id <> 'local'),
                     ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
                    setup.imap_security.as_str(),
                    incoming,
                    keep.leave,
                    keep.days,
                    keep.until_deleted
                ],
            )?;
            tx.execute(
                "INSERT INTO folders (account, id, position, name, kind) VALUES (?1, ?2, 1, 'Outbox', 'outbox')",
                params![id, OUTBOX_ID],
            )?;
            // A POP3 server has only an Inbox: every folder of the account is here.
            if setup.pop3.is_some() {
                for (pos, (folder, name)) in POP3_FOLDERS.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO folders (account, id, position, name, kind) VALUES (?1, ?2, ?3, ?4, ?2)",
                        params![id, folder, pos as i64 + 2, name],
                    )?;
                }
            }
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

    /// What `imap::plan_folders` needs to know besides the server's list: the kinds the user
    /// chose, the kinds the folders have now, and which hold mail. Empty for a new account.
    pub fn folder_hints(&self, account: &str) -> Result<imap::FolderHints, StoreError> {
        let mut hints = imap::FolderHints::default();
        let mut stmt = self.db.prepare_cached(
            "SELECT f.remote, f.kind, f.chosen,
                    EXISTS (SELECT 1 FROM messages WHERE folder = f.account || '/' || f.id)
             FROM folders f WHERE f.account = ?1 AND f.remote IS NOT NULL",
        )?;
        let rows = stmt.query_map([account], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, bool>(3)?,
            ))
        })?;
        for row in rows {
            let (remote, kind, chosen, held) = row?;
            if let Some(chosen) = chosen {
                hints
                    .chosen
                    .insert(remote.clone(), FolderKind::from_name(&chosen));
            }
            hints
                .was
                .insert(remote.clone(), FolderKind::from_name(&kind));
            if held {
                hints.held.insert(remote);
            }
        }
        Ok(hints)
    }

    /// Use a server folder for sent mail (or drafts, archive, spam, trash) from now on, as
    /// the user chose; the folder that had that kind becomes one of the user's own. The next
    /// sync keeps the choice while the folder exists.
    pub fn choose_folder(&mut self, key: &str, kind: FolderKind) -> Result<(), StoreError> {
        let (account, id) =
            split_folder_key(key).ok_or_else(|| StoreError::UnknownFolder(key.to_owned()))?;
        self.folder(key)
            .filter(|f| f.remote.is_some() && f.kind != FolderKind::Inbox)
            .ok_or_else(|| StoreError::UnknownFolder(key.to_owned()))?;
        if !imap::SPECIAL_KINDS.contains(&kind) {
            let kind = kind.as_str();
            return Err(StoreError::Invalid(tr!(
                "A folder can't be chosen for {kind}.",
                "Nie można wybrać folderu dla: {kind}."
            )));
        }
        let tx = self.db.transaction()?;
        let drafts_before = drafts_folder(&tx, account)?;
        tx.execute(
            "UPDATE folders SET chosen = NULL WHERE account = ?1 AND chosen = ?2",
            params![account, kind.as_str()],
        )?;
        tx.execute(
            "UPDATE folders SET kind = 'custom' WHERE account = ?1 AND kind = ?2",
            params![account, kind.as_str()],
        )?;
        tx.execute(
            "UPDATE folders SET kind = ?3, chosen = ?3 WHERE account = ?1 AND id = ?2",
            params![account, id, kind.as_str()],
        )?;
        let drafts_after = drafts_folder(&tx, account)?;
        move_local_drafts(&tx, account, drafts_before, drafts_after.as_deref())?;
        tx.commit()?;
        self.reload_accounts()
    }

    /// Replace an account's server folders with what the server has now. Folders that
    /// disappeared from the server are dropped together with their local copies; local
    /// folders (the Outbox) stay. Drafts written here follow Drafts to another folder.
    pub fn set_folders(&mut self, account: &str, folders: &[Folder]) -> Result<(), StoreError> {
        let names: HashMap<String, String> = self
            .account(account)?
            .folders
            .iter()
            .map(|f| (f.id.clone(), f.name.clone()))
            .collect();
        let names: HashMap<&str, &str> = names
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let keep: HashSet<&str> = folders.iter().map(|f| f.id.as_str()).collect();
        let tx = self.db.transaction()?;
        // Before a vanished folder takes its messages along.
        let drafts = folders.iter().find(|f| f.kind == FolderKind::Drafts);
        let drafts_before = drafts_folder(&tx, account)?;
        move_local_drafts(&tx, account, drafts_before, drafts.map(|f| f.id.as_str()))?;
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
            // Mail that exists only here goes to Archive on this PC instead of with the folder:
            // what waits to be uploaded or for a move to reach the server, and copies of sent
            // mail not uploaded yet. The server's copies go.
            let name = names.get(gone.as_str()).copied().unwrap_or(gone.as_str());
            let note = tr!(
                "The folder {name} is gone from the server; this message, kept only on this PC, moved to Archive here.",
                "Folderu {name} nie ma już na serwerze; ta wiadomość, zapisana tylko na tym komputerze, trafiła tu do Archiwum."
            );
            let kept_here =
                "(folder = ?1 AND srv_folder IS NULL) OR (srv_folder = ?1 AND folder <> ?1)";
            let rescued: i64 = tx.query_row(
                &format!("SELECT COUNT(*) FROM messages WHERE {kept_here}"),
                [&key],
                |r| r.get(0),
            )?;
            if rescued > 0 {
                tx.execute_batch(ENSURE_LOCAL)?;
                tx.execute(
                    &format!(
                        "UPDATE messages SET folder = ?2, srv_folder = NULL, uid = NULL,
                             note = CASE WHEN note IS NULL OR note = '' THEN ?3 ELSE note || ' ' || ?3 END
                         WHERE {kept_here}"
                    ),
                    params![key, format!("{LOCAL_ID}/archive"), note],
                )?;
            }
            tx.execute("DELETE FROM messages WHERE folder = ?1", [&key])?;
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

    /// Make the folders on this PC now, before any server account: mail moved in from
    /// another program can live there alone.
    pub fn ensure_this_pc(&mut self) -> Result<(), StoreError> {
        if self.this_pc().is_none() {
            self.db.execute_batch(ENSURE_LOCAL)?;
            self.reload_accounts()?;
        }
        Ok(())
    }

    /// The folders on this PC, once the mail base has them: from the first server account on.
    pub fn this_pc(&self) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|a| a.id == LOCAL_ID && a.imap.is_none())
    }

    /// An account whose folders are all here, so the user makes, renames and removes them:
    /// the folders on this PC, and a POP3 account (its server has only an Inbox).
    fn own_folders(&self, account: &str) -> Result<&Account, StoreError> {
        match self.accounts.iter().find(|a| a.id == account) {
            Some(a) if a.id == LOCAL_ID && a.imap.is_none() => Ok(a),
            Some(a) if a.imap.as_ref().is_some_and(|i| i.pop3.is_some()) => Ok(a),
            Some(a) => {
                let email = &a.email;
                Err(StoreError::Invalid(tr!(
                    "The folders of {email} are on its server: make them there, or on this PC.",
                    "Foldery konta {email} są na jego serwerze: utwórz je tam albo na tym komputerze."
                )))
            }
            None if account == LOCAL_ID => Err(no_local_folders()),
            None => Err(StoreError::UnknownAccount(account.to_owned())),
        }
    }

    /// A name for a folder of an account that keeps its folders here (the folders on this PC,
    /// a POP3 account), trimmed, or what is wrong with it. `except` is the folder being
    /// renamed, which may keep its own name.
    fn check_folder_name(
        &self,
        account: &str,
        name: &str,
        except: Option<&str>,
    ) -> Result<String, StoreError> {
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
        let pc = self.own_folders(account)?;
        let taken = pc.folders.iter().any(|f| {
            Some(f.id.as_str()) != except
                && (f.name.to_lowercase() == name.to_lowercase()
                    || f.kind
                        .pl_name()
                        .is_some_and(|pl| pl.to_lowercase() == name.to_lowercase()))
        });
        if taken {
            return Err(StoreError::Invalid(if account == LOCAL_ID {
                tr!(
                    "There is a folder called \"{name}\" on this PC already.",
                    "Na tym komputerze jest już folder „{name}”."
                )
            } else {
                tr!(
                    "This account has a folder called \"{name}\" already.",
                    "To konto ma już folder „{name}”."
                )
            }));
        }
        Ok(name)
    }

    /// Make a folder on this PC. Returns its key.
    pub fn create_local_folder(&mut self, name: &str) -> Result<String, StoreError> {
        self.create_folder(LOCAL_ID, name)
    }

    /// Make a folder of an account that keeps its folders here: the folders on this PC, or a
    /// POP3 account. Returns its key.
    pub fn create_folder(&mut self, account: &str, name: &str) -> Result<String, StoreError> {
        let name = self.check_folder_name(account, name, None)?;
        let pc = self.own_folders(account)?;
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
            params![account, id, name],
        )?;
        self.reload_accounts()?;
        Ok(format!("{account}/{id}"))
    }

    /// The user's own folder on this PC (or of a POP3 account), by key: Archive and Trash
    /// there are for good.
    fn own_local_folder(&self, key: &str) -> Result<(String, Folder), StoreError> {
        let unknown = || StoreError::UnknownFolder(key.to_owned());
        let (account, _) = split_folder_key(key).ok_or_else(unknown)?;
        let local = account == LOCAL_ID;
        self.own_folders(account)?;
        let folder = self.folder(key).cloned().ok_or_else(unknown)?;
        if folder.kind != FolderKind::Custom {
            return Err(StoreError::Invalid(if local {
                tr!(
                    "Archive and Trash on this PC stay as they are.",
                    "Archiwum i Kosz na tym komputerze zostają takie, jakie są."
                )
            } else {
                tr!(
                    "The account's own folders (Inbox, Sent, Trash and the others) stay as they are.",
                    "Stałe foldery konta (Odebrane, Wysłane, Kosz i pozostałe) zostają takie, jakie są."
                )
            }));
        }
        Ok((account.to_owned(), folder))
    }

    /// Give a folder on this PC (or of a POP3 account) another name. Its key stays, so rules
    /// keep working.
    pub fn rename_local_folder(&mut self, key: &str, name: &str) -> Result<(), StoreError> {
        let (account, folder) = self.own_local_folder(key)?;
        let name = self.check_folder_name(&account, name, Some(&folder.id))?;
        self.db.execute(
            "UPDATE folders SET name = ?3 WHERE account = ?1 AND id = ?2",
            params![account, folder.id, name],
        )?;
        self.reload_accounts()
    }

    /// Remove an empty folder on this PC (or of a POP3 account), and the rules' moves/copies
    /// to it with it: a rule that would have nothing left to do goes too. Returns how many
    /// rules changed.
    pub fn delete_local_folder(&mut self, key: &str) -> Result<usize, StoreError> {
        let (account, folder) = self.own_local_folder(key)?;
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
                .retain(|a| !matches!(a, Action::Move { folder } | Action::Copy { folder } if folder == key));
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
            params![account, folder.id],
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
            {
                let mut delete =
                    tx.prepare_cached("DELETE FROM messages WHERE srv_folder = ?1 AND uid = ?2")?;
                for uid in &gone {
                    delete.execute(params![folder_key, uid])?;
                }
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
        let (Some(lo), Some(hi)) = (
            flags.iter().map(|f| f.0).min(),
            flags.iter().map(|f| f.0).max(),
        ) else {
            return Ok(0);
        };
        // What this PC has for those UIDs, read once; only what differs is written.
        type Local = (i64, bool, bool, bool, bool);
        let here: HashMap<u32, Local> = {
            let mut stmt = self.db.prepare_cached(
                "SELECT uid, id, unread, flagged,
                    EXISTS (SELECT 1 FROM pending_ops WHERE message = m.id AND kind = 'seen'),
                    EXISTS (SELECT 1 FROM pending_ops WHERE message = m.id AND kind = 'flagged')
                 FROM messages m WHERE srv_folder = ?1 AND uid BETWEEN ?2 AND ?3",
            )?;
            let rows = stmt.query_map(params![folder_key, lo, hi], |r| {
                Ok((
                    r.get(0)?,
                    (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?),
                ))
            })?;
            rows.collect::<Result<_, _>>()?
        };
        let tx = self.db.transaction()?;
        let mut changed = 0;
        {
            let mut set_unread =
                tx.prepare_cached("UPDATE messages SET unread = ?2 WHERE id = ?1")?;
            let mut set_flagged =
                tx.prepare_cached("UPDATE messages SET flagged = ?2 WHERE id = ?1")?;
            for &(uid, seen, flagged) in flags {
                let Some(&(id, unread, was_flagged, seen_queued, flag_queued)) = here.get(&uid)
                else {
                    continue;
                };
                // A local change still queued wins over the server's.
                if unread == seen && !seen_queued {
                    changed += set_unread.execute(params![id, !seen])?;
                }
                if was_flagged != flagged && !flag_queued {
                    changed += set_flagged.execute(params![id, flagged])?;
                }
            }
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
    let tx = db.transaction()?;
    let done = set_flags_in(&tx, ids, flag, value, only_changed)?;
    tx.commit()?;
    Ok(done)
}

/// [`set_flags`] inside a transaction the caller holds.
fn set_flags_in(
    tx: &Connection,
    ids: &[MessageId],
    flag: Flag,
    value: bool,
    only_changed: bool,
) -> Result<usize, StoreError> {
    let (column, kind, server_value) = match flag {
        Flag::Unread => ("unread", "seen", !value),
        Flag::Flagged => ("flagged", "flagged", value),
    };
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
        thread: None,
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
             imap_security, incoming, pop3_leave, pop3_days, pop3_until_deleted
         FROM accounts ORDER BY id = 'local', position, id",
    )?;
    let mut accounts = stmt
        .query_map([], |r| {
            let pop3 = match r.get::<_, String>(10)?.as_str() {
                "pop3" => Some(Pop3Keep {
                    leave: r.get(11)?,
                    days: r.get(12)?,
                    until_deleted: r.get(13)?,
                }),
                _ => None,
            };
            let imap = match r.get::<_, Option<String>>(3)? {
                Some(host) => Some(ImapSettings {
                    host,
                    port: r.get(4)?,
                    username: r.get(5)?,
                    security: SmtpSecurity::from_name(
                        &r.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    ),
                    pop3,
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
    // user's own folders on this PC (and a POP3 account's) go A to Z.
    for a in &mut accounts {
        let by_name = a.id == LOCAL_ID || a.imap.as_ref().is_some_and(|i| i.pop3.is_some());
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

#[cfg(test)]
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

#[cfg(test)]
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

/// The id of an account's Drafts on the server, if it has one.
fn drafts_folder(db: &Connection, account: &str) -> Result<Option<String>, StoreError> {
    Ok(db
        .query_row(
            "SELECT id FROM folders WHERE account = ?1 AND kind = 'drafts' AND remote IS NOT NULL",
            [account],
            |r| r.get(0),
        )
        .optional()?)
}

/// Drafts written here are on no server: when another server folder becomes Drafts, they
/// go there too (not what waits for an upload, which knows where it goes).
fn move_local_drafts(
    db: &Connection,
    account: &str,
    from: Option<String>,
    to: Option<&str>,
) -> Result<(), StoreError> {
    let (Some(from), Some(to)) = (from, to) else {
        return Ok(());
    };
    if from != to {
        db.execute(
            "UPDATE messages SET folder = ?2 WHERE folder = ?1 AND srv_folder IS NULL
               AND id NOT IN (SELECT message FROM pending_ops WHERE message IS NOT NULL)",
            params![format!("{account}/{from}"), format!("{account}/{to}")],
        )?;
    }
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
mod tests;
