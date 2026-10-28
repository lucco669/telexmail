//! Two-way sync with an IMAP server.
//!
//! Downloading opens folders with EXAMINE (read-only) and fetches with BODY.PEEK, so
//! looking never changes anything. Changes the user makes are queued by `MailStore` and
//! sent by [`push_changes`]: flags with UID STORE, moves with UID MOVE (or COPY, then
//! delete and EXPUNGE on servers without MOVE), deletes from Trash with EXPUNGE.
//! The tests run all of it against a small fake server.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_imap::error::Error as ImapError;
use async_imap::extensions::idle::IdleResponse;
use async_imap::imap_proto::{Response, ResponseCode, Status, UidSetMember};
use async_imap::types::{
    Capability, Fetch, Flag, Name, NameAttribute, QuotaResourceName, UnsolicitedResponse,
};
use async_imap::{Client, Session};
use chrono::{DateTime, Local, TimeDelta};
use futures_util::TryStreamExt;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

use crate::filters::{self, Trigger};
use crate::mime::parse_message;
use crate::model::{Account, Folder, FolderKind, ImapSettings, MessageId, SmtpSecurity};
use crate::servers::{self, Purpose, Step, Trace, Turn, SERVERS};
use crate::store::{
    slug, ChangeKind, MailStore, PendingChange, ServerFlags, StoreError, SyncedMessage, OUTBOX_ID,
};
use crate::tr;
use crate::usage::Quota;

/// How many of the newest messages to download per folder the first time.
pub const FIRST_SYNC_LIMIT: usize = 300;
/// A change the server refuses this many times is dropped and reported.
pub const MAX_ATTEMPTS: u32 = 3;
/// Messages per FETCH command: small enough to report progress often.
const FETCH_BATCH: usize = 20;
/// Messages per flag check (UID FETCH … FLAGS); the answers are tiny.
const FLAGS_BATCH: usize = 500;
/// What Telex asks for. BODY.PEEK leaves the \Seen flag alone.
const FETCH_ITEMS: &str = "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
/// A batch of big attachments on a slow line can take a while.
const FETCH_TIMEOUT: Duration = Duration::from_secs(300);

/// What async-imap needs from a connection: TLS in the app, plain TCP in the tests.
pub trait ImapStream: AsyncRead + AsyncWrite + Unpin + fmt::Debug + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + fmt::Debug + Send> ImapStream for T {}

#[derive(Debug)]
pub enum SyncError {
    /// DNS or TCP failed: wrong server name, no network, blocked port.
    Connect {
        host: String,
        reason: String,
    },
    /// TLS failed, usually a certificate that doesn't match the server name.
    Tls {
        host: String,
        reason: String,
    },
    /// The server said no to the user name or password.
    Login(String),
    Timeout,
    /// The server stopped answering a moment ago; nothing connects to it until then.
    Resting(DateTime<Local>),
    /// The server answered with an error or closed the connection.
    Server(String),
    Store(StoreError),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Connect { host, reason } => {
                tr!(
                    "can't reach {host}: {reason}",
                    "brak połączenia z {host}: {reason}"
                )
            }
            Self::Tls { host, reason } => tr!(
                "no secure connection to {host}: {reason}",
                "brak bezpiecznego połączenia z {host}: {reason}"
            ),
            Self::Login(reason) => tr!(
                "the server rejected the user name or password ({reason})",
                "serwer odrzucił nazwę użytkownika lub hasło ({reason})"
            ),
            Self::Timeout => tr!("the server stopped answering", "serwer przestał odpowiadać"),
            Self::Resting(until) => {
                let time = until.format("%H:%M");
                tr!(
                    "the server stopped answering; Telex tries again after {time}",
                    "serwer przestał odpowiadać; Telex spróbuje ponownie po {time}"
                )
            }
            Self::Server(reason) => tr!(
                "the server reported a problem: {reason}",
                "serwer zgłosił problem: {reason}"
            ),
            Self::Store(e) => return e.fmt(f),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for SyncError {}

impl From<StoreError> for SyncError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

impl From<ImapError> for SyncError {
    fn from(e: ImapError) -> Self {
        match e {
            ImapError::No(m) | ImapError::Bad(m) => Self::Server(m.trim().to_owned()),
            ImapError::ConnectionLost => Self::Server(tr!(
                "the connection was closed",
                "połączenie zostało zamknięte"
            )),
            ImapError::Io(e) => Self::Server(connection_error(&e)),
            other => Self::Server(other.to_string()),
        }
    }
}

/// Sent to the UI while a folder downloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncProgress {
    pub account: String,
    /// The folder's name on the server; `key` lets the UI name a special folder its way.
    pub folder: String,
    pub key: String,
    pub done: usize,
    pub total: usize,
}

/// What one sync of an account did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Ids of messages downloaded, in every folder.
    pub new: Vec<MessageId>,
    /// The new Inbox mail that really arrived (not a folder's first download), for a
    /// notification; not what the spam filter moved to Spam.
    pub arrived: Vec<MessageId>,
    /// New Inbox mail the spam filter moved to Spam.
    pub spam: usize,
    /// New Inbox mail the rules changed, and how many automatic replies they queued.
    pub sorted: usize,
    pub answered: usize,
    /// Changes the server refused for good (and rule problems), as sentences.
    pub refused: Vec<String>,
}

/// What a download brought.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Downloaded {
    /// Every message stored this time, in every folder.
    pub new: Vec<MessageId>,
    /// The new Inbox mail that really arrived: not the first download of a folder, which
    /// is old mail. Rules run on these.
    pub arrived: Vec<MessageId>,
}

/// A folder as the server lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFolder {
    /// The exact name, in modified UTF-7.
    pub name: String,
    pub delimiter: Option<String>,
    /// The special use the server announced (RFC 6154), if any.
    pub special: Option<FolderKind>,
    /// False for containers (\Noselect) and virtual folders such as Gmail's All Mail.
    pub syncable: bool,
}

impl RemoteFolder {
    fn from_name(n: &Name) -> Self {
        let attrs = n.attributes();
        let special = attrs.iter().find_map(|a| match a {
            NameAttribute::Sent => Some(FolderKind::Sent),
            NameAttribute::Drafts => Some(FolderKind::Drafts),
            NameAttribute::Trash => Some(FolderKind::Trash),
            NameAttribute::Junk => Some(FolderKind::Spam),
            NameAttribute::Archive => Some(FolderKind::Archive),
            _ => None,
        });
        let syncable = !attrs.iter().any(|a| match a {
            NameAttribute::NoSelect | NameAttribute::All | NameAttribute::Flagged => true,
            NameAttribute::Extension(x) => x.eq_ignore_ascii_case("\\NonExistent"),
            _ => false,
        });
        Self {
            name: n.name().to_owned(),
            delimiter: n.delimiter().map(str::to_owned),
            special,
            syncable,
        }
    }

    /// The name split into its levels and decoded: "INBOX.Wys&AUI-ane" → ["INBOX", "Wysłane"].
    fn path(&self) -> Vec<String> {
        let decoded = decode_folder_name(&self.name);
        match self.delimiter.as_deref().filter(|d| !d.is_empty()) {
            Some(d) => decoded.split(d).map(str::to_owned).collect(),
            None => vec![decoded],
        }
    }
}

/// Server features that change how Telex writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServerCaps {
    /// RFC 6851 MOVE. Without it a move is COPY, then delete and EXPUNGE.
    pub can_move: bool,
    /// RFC 4315 UIDPLUS: UID EXPUNGE, and the new UID after a copy or move (COPYUID).
    pub uidplus: bool,
    /// RFC 9208 QUOTA: the server says how much storage the account uses and may use.
    pub quota: bool,
}

// ---- Public entry points ----

/// Log in once to check the settings, and return the server's folders. A POP3 server has
/// none: the account's folders are all here (`pop3::check_account`).
pub async fn check_account(
    settings: &ImapSettings,
    password: &str,
) -> Result<Vec<RemoteFolder>, SyncError> {
    if settings.pop3.is_some() {
        return crate::pop3::check_account(settings, password)
            .await
            .map(|()| Vec::new());
    }
    let (mut session, mut turn) = connect_and_login(settings, password, Purpose::Test).await?;
    let folders = list_folders(&mut session).await;
    logout(&mut session).await;
    outcome(&mut turn, &folders);
    folders
}

/// Which folders a sync downloads.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Scope {
    /// Every folder, after asking the server for its folder list.
    #[default]
    All,
    /// Every folder, starting with the one selected in the window (its local id).
    AllFirst(String),
    /// Only the Inbox: when IDLE says something changed there.
    Inbox,
}

/// Full sync of one account: send queued changes, download what's new, then send
/// whatever the user changed meanwhile.
///
/// The store is locked only between network calls, so the UI stays responsive.
pub async fn sync_account(
    account: &Account,
    password: &str,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
    scope: Scope,
) -> Result<SyncReport, SyncError> {
    let Some(settings) = &account.imap else {
        return Ok(SyncReport::default());
    };
    if settings.pop3.is_some() {
        return crate::pop3::sync_account(account, password, store, progress).await;
    }
    let (mut session, mut turn) = connect_and_login(settings, password, Purpose::Check).await?;
    let result = sync_all(&mut session, account, store, progress, scope).await;
    logout(&mut session).await;
    outcome(&mut turn, &result);
    result
}

/// Download up to `limit` of the newest messages of a folder that are on the server but not
/// here: older than what the first download took. Rules don't run on them; they are old
/// mail. Returns the new ids; the folder's count of missing mail is updated.
pub async fn download_older(
    account: &Account,
    password: &str,
    store: &Mutex<MailStore>,
    folder_key: &str,
    limit: usize,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<Vec<MessageId>, SyncError> {
    let Some(settings) = &account.imap else {
        return Ok(Vec::new());
    };
    if settings.pop3.is_some() {
        return crate::pop3::download_older(account, password, store, limit, progress).await;
    }
    let (mut session, mut turn) = connect_and_login(settings, password, Purpose::Older).await?;
    let result = download_older_in(&mut session, account, store, folder_key, limit, progress).await;
    logout(&mut session).await;
    outcome(&mut turn, &result);
    result
}

/// [`download_older`] over a session that is already logged in.
pub async fn download_older_in<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    folder_key: &str,
    limit: usize,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<Vec<MessageId>, SyncError> {
    let folder = account
        .folders
        .iter()
        .find(|f| account.folder_key(&f.id) == folder_key)
        .ok_or_else(|| SyncError::Store(StoreError::UnknownFolder(folder_key.to_owned())))?;
    let Some(remote) = folder.remote.as_deref() else {
        return Ok(Vec::new());
    };
    let mailbox = timeout(COMMAND_TIMEOUT, session.examine(remote))
        .await
        .map_err(|_| SyncError::Timeout)??;
    let state = lock(store)?.begin_folder_sync(folder_key, mailbox.uid_validity.unwrap_or(0))?;
    let on_server = if mailbox.exists == 0 {
        HashSet::new()
    } else {
        timeout(COMMAND_TIMEOUT, session.uid_search("ALL"))
            .await
            .map_err(|_| SyncError::Timeout)??
    };
    let local = lock(store)?.local_uids(folder_key)?;
    let wanted = older_to_fetch(&on_server, &local, state.last_uid, limit);
    let total = wanted.len();
    let mut ids = Vec::new();
    for chunk in wanted.chunks(FETCH_BATCH) {
        progress(SyncProgress {
            account: account.name.clone(),
            folder: folder.name.clone(),
            key: folder_key.to_owned(),
            done: ids.len(),
            total,
        });
        let (batch, answered) = fetch_batch(session, chunk, folder_key).await?;
        let mut s = lock(store)?;
        ids.extend(s.add_synced(folder_key, &batch)?);
        s.refresh_flags(folder_key, &answered)?;
    }
    progress(SyncProgress {
        account: account.name.clone(),
        folder: folder.name.clone(),
        key: folder_key.to_owned(),
        done: total,
        total,
    });
    lock(store)?.note_server_count(folder_key, on_server.len())?;
    Ok(ids)
}

/// Only send queued changes. Doesn't connect when there is nothing to send.
/// Returns the changes the server refused for good.
pub async fn push_account(
    account: &Account,
    password: &str,
    store: &Mutex<MailStore>,
) -> Result<Vec<String>, SyncError> {
    // A POP3 account's changes are all here; what leaves its server goes at a check.
    let Some(settings) = account.imap.as_ref().filter(|i| i.pop3.is_none()) else {
        return Ok(Vec::new());
    };
    if !lock(store)?.has_pending_changes(&account.id)? {
        return Ok(Vec::new());
    }
    let (mut session, mut turn) = connect_and_login(settings, password, Purpose::Push).await?;
    let result = async {
        let caps = server_caps(&mut session).await?;
        push_changes(&mut session, account, store, caps).await
    }
    .await;
    logout(&mut session).await;
    outcome(&mut turn, &result);
    result
}

/// [`sync_account`] over a session that is already logged in.
pub async fn sync_all<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
    scope: Scope,
) -> Result<SyncReport, SyncError> {
    let caps = server_caps(session).await?;
    let queued_before = lock(store)?.last_change_op(&account.id)?;
    let refused = push_queued_after(session, account, store, caps, 0).await?;
    let downloaded = sync_session(session, account, store, progress, scope).await?;
    let quota = if caps.quota {
        storage_quota(session).await
    } else {
        None
    };
    lock(store)?.set_quota(&account.id, quota)?;
    let mut report = SyncReport {
        new: downloaded.new,
        refused,
        ..SyncReport::default()
    };
    sort_arrived(store, &account.id, downloaded.arrived, &mut report)?;
    // Send what the user did during the download, but don't retry what was just refused.
    // The folder list may have changed, so read the account again.
    let account = lock(store)?.account(&account.id)?.clone();
    report
        .refused
        .extend(push_queued_after(session, &account, store, caps, queued_before).await?);
    Ok(report)
}

/// New Inbox mail that arrived, through the spam filter, the rules and the away reply, as
/// every check does it (IMAP and POP3). Fills in the report's `arrived`, `spam`, `sorted`,
/// `answered`, and the rules' and replies' problems in `refused`.
pub(crate) fn sort_arrived(
    store: &Mutex<MailStore>,
    account: &str,
    mut arrived: Vec<MessageId>,
    report: &mut SyncReport,
) -> Result<(), SyncError> {
    // The spam filter first, so the rules and the away reply never see what it caught; then
    // it learns from the mail sorted since the last check.
    let caught = {
        let mut s = lock(store)?;
        s.note_arrivals(&arrived, Local::now().date_naive())?;
        let caught = crate::spam::sort_arrivals(&mut s, &arrived, crate::spam::level())?;
        crate::spam::learn_pending(&mut s, crate::spam::LEARN_BATCH)?;
        caught
    };
    arrived.retain(|id| !caught.contains(id));
    report.arrived = arrived.clone();
    report.spam = caught.len();
    if !arrived.is_empty() {
        let run = {
            let mut s = lock(store)?;
            filters::run(&mut s, &arrived, Trigger::Arrival, Local::now())?
        };
        report.sorted = run.matched;
        report.answered = run.replied;
        report.refused.extend(run.problems);
        // The away reply, where Telex sends it, after the rules have filed the mail.
        let away = crate::away::answer(&mut *lock(store)?, account, &arrived, Local::now());
        report.answered += away.replied;
        report.refused.extend(away.problems);
    }
    Ok(())
}

/// Download: refresh the folder list, then for every folder drop what was deleted on
/// the server, take flag changes made elsewhere, and fetch new messages. With
/// [`Scope::Inbox`] only the Inbox, from the folder list Telex already has.
pub async fn sync_session<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
    scope: Scope,
) -> Result<Downloaded, SyncError> {
    let first = match &scope {
        Scope::AllFirst(id) => Some(id.clone()),
        _ => None,
    };
    let (mut folders, remote) = match scope {
        Scope::All | Scope::AllFirst(_) => {
            let remote = list_folders(session).await?;
            let folders = {
                let mut s = lock(store)?;
                let folders = plan_folders(&remote, &s.folder_hints(&account.id)?);
                s.set_folders(&account.id, &folders)?;
                folders
            };
            (folders, Some(remote))
        }
        Scope::Inbox => {
            let inbox = account
                .folders
                .iter()
                .filter(|f| f.kind == FolderKind::Inbox)
                .cloned()
                .collect();
            (inbox, None)
        }
    };

    if let Some(id) = first {
        // Keep the normal order of all other folders, and the stored tree order.
        if let Some(index) = folders.iter().position(|f| f.id == id) {
            let folder = folders.remove(index);
            folders.insert(0, folder);
        }
    }

    let mut downloaded = Downloaded::default();
    for folder in &folders {
        let Some(remote_name) = folder.remote.as_deref() else {
            continue;
        };
        let key = account.folder_key(&folder.id);
        let mailbox = match timeout(COMMAND_TIMEOUT, session.examine(remote_name))
            .await
            .map_err(|_| SyncError::Timeout)?
        {
            Ok(m) => m,
            // A folder the server refuses to open (permissions, just deleted): skip it.
            Err(ImapError::No(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let state = lock(store)?.begin_folder_sync(&key, mailbox.uid_validity.unwrap_or(0))?;
        let arriving = folder.kind == FolderKind::Inbox && state.last_uid.is_some();
        let on_server = if mailbox.exists == 0 {
            HashSet::new()
        } else {
            timeout(COMMAND_TIMEOUT, session.uid_search("ALL"))
                .await
                .map_err(|_| SyncError::Timeout)??
        };
        lock(store)?.forget_vanished(&key, &on_server)?;

        let mut known: Vec<u32> = lock(store)?.local_uids(&key)?.into_iter().collect();
        known.sort_unstable();
        for chunk in known.chunks(FLAGS_BATCH) {
            let flags = fetch_flags(session, chunk).await?;
            lock(store)?.refresh_flags(&key, &flags)?;
        }

        let wanted = uids_to_fetch(&on_server, state.last_uid, FIRST_SYNC_LIMIT);
        let total = wanted.len();
        let report = |done| {
            progress(SyncProgress {
                account: account.name.clone(),
                folder: folder.name.clone(),
                key: key.clone(),
                done,
                total,
            });
        };
        if total > 0 {
            report(0);
        }
        let mut done = 0;
        for chunk in wanted.chunks(FETCH_BATCH) {
            let (batch, answered) = fetch_batch(session, chunk, &key).await?;
            let ids = {
                let mut s = lock(store)?;
                let ids = s.add_synced(&key, &batch)?;
                if state.last_uid.is_some()
                    && !folder.kind.is_outgoing()
                    && !matches!(folder.kind, FolderKind::Spam | FolderKind::Trash)
                {
                    s.note_arrivals(&ids, Local::now().date_naive())?;
                }
                s.refresh_flags(&key, &answered)?;
                ids
            };
            if arriving {
                downloaded.arrived.extend(&ids);
            }
            downloaded.new.extend(ids);
            done += chunk.len();
            report(done);
        }
        lock(store)?.note_server_count(&key, on_server.len())?;
    }
    // Which folder holds mail is known now: a first download can settle which is Sent.
    if let Some(remote) = remote {
        let mut s = lock(store)?;
        let settled = plan_folders(&remote, &s.folder_hints(&account.id)?);
        if settled != s.account(&account.id)?.folders {
            s.set_folders(&account.id, &settled)?;
        }
    }
    Ok(downloaded)
}

/// Upload: send the account's queued changes. A change the server refuses stays queued
/// and is retried next time, up to [`MAX_ATTEMPTS`]; the refusals given up on are returned.
/// A lost connection stops the push, and everything not yet sent stays queued.
pub async fn push_changes<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    caps: ServerCaps,
) -> Result<Vec<String>, SyncError> {
    push_queued_after(session, account, store, caps, 0).await
}

/// How long one IDLE lasts before Telex renews it: servers may end one after 30 minutes of
/// silence (RFC 2177), and renewing also finds out when a connection quietly died.
pub const IDLE_RENEW: Duration = Duration::from_secs(25 * 60);

/// Why watching an Inbox ended without an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watched {
    /// The server doesn't support IDLE; regular checks will have to do.
    NoIdle,
}

/// Watch an account's Inbox with IDLE (RFC 2177) on a connection of its own, and call
/// `changed` whenever the server says something happened there: new mail, mail deleted,
/// flags changed elsewhere. The caller then syncs the Inbox. Runs until the connection
/// breaks (an error: the caller tries again after a pause) or the server turns out not to
/// support IDLE. The Inbox is opened read-only, so watching never changes anything.
pub async fn watch_inbox(
    settings: &ImapSettings,
    password: &str,
    changed: &(dyn Fn() + Send + Sync),
) -> Result<Watched, SyncError> {
    // POP3 has nothing like IDLE: the regular checks bring its mail.
    if settings.pop3.is_some() {
        return Ok(Watched::NoIdle);
    }
    let (session, mut turn) = connect_and_login(settings, password, Purpose::Watch).await?;
    let result = watch_session(session, changed, IDLE_RENEW).await;
    outcome(&mut turn, &result);
    result
}

/// [`watch_inbox`] over a session that is already logged in. IDLE takes the session and
/// gives it back after DONE, so this owns it.
async fn watch_session<T: ImapStream>(
    mut session: Session<T>,
    changed: &(dyn Fn() + Send + Sync),
    renew: Duration,
) -> Result<Watched, SyncError> {
    let caps = timeout(COMMAND_TIMEOUT, session.capabilities())
        .await
        .map_err(|_| SyncError::Timeout)??;
    if !caps.has_str("IDLE") {
        logout(&mut session).await;
        return Ok(Watched::NoIdle);
    }
    timeout(COMMAND_TIMEOUT, session.examine("INBOX"))
        .await
        .map_err(|_| SyncError::Timeout)??;
    loop {
        let mut idle = session.idle();
        timeout(COMMAND_TIMEOUT, idle.init())
            .await
            .map_err(|_| SyncError::Timeout)??;
        let (wait, _stop) = idle.wait_with_timeout(renew);
        let response = wait.await?;
        session = timeout(COMMAND_TIMEOUT, idle.done())
            .await
            .map_err(|_| SyncError::Timeout)??;
        let mut news = matches!(response, IdleResponse::NewData(_));
        // Whatever the server said around DONE waits in the session's queue.
        while let Ok(r) = session.unsolicited_responses.try_recv() {
            news |= !matches!(r, UnsolicitedResponse::Status { .. });
        }
        if news {
            changed();
        }
    }
}

/// [`push_changes`] for the changes queued after `after_op` only.
async fn push_queued_after<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    caps: ServerCaps,
    after_op: i64,
) -> Result<Vec<String>, SyncError> {
    let (appends, changes): (Vec<_>, Vec<_>) = lock(store)?
        .pending_changes(&account.id)?
        .into_iter()
        .filter(|c| c.op > after_op)
        .partition(|c| matches!(c.kind, ChangeKind::Append { .. }));
    let mut refused = Vec::new();

    // Copies of sent mail for the Sent folder, mail moved here from this PC, and drafts saved
    // here. APPEND needs no selected folder.
    for change in &appends {
        let (ChangeKind::Append { to, flags, draft }, Some(id)) = (&change.kind, change.message)
        else {
            continue;
        };
        let raw = lock(store)?.raw_of(id)?;
        let (Some(raw), Some(remote)) = (raw, remote_name(account, to)) else {
            lock(store)?.change_done(change.op)?; // nothing to save, or nowhere to save it
            continue;
        };
        if flags.is_none()
            && !sent_copy_needed(session, account, store, change, id, to, &remote).await?
        {
            continue;
        }
        // Mail from this PC takes its tags along, where the server keeps them, and whether it
        // was answered ($Forwarded is a keyword, so it goes as they do).
        let (answered, forwarded) = match flags {
            Some(_) => lock(store)?.answered_of(id)?,
            None => (false, false),
        };
        let mut keywords = match flags {
            Some(_) if !lock(store)?.tags_kept_here(&account.id)? => {
                lock(store)?.keywords_of(id)?
            }
            _ => Vec::new(),
        };
        if forwarded && flags.is_some() && !lock(store)?.tags_kept_here(&account.id)? {
            keywords.push(FORWARDED.to_owned());
        }
        let mut list: Vec<&str> = match (flags, draft) {
            (_, true) => vec!["\\Seen", "\\Draft"],
            (None | Some((true, false)), _) => vec!["\\Seen"],
            (Some((true, true)), _) => vec!["\\Seen", "\\Flagged"],
            (Some((false, true)), _) => vec!["\\Flagged"],
            (Some((false, false)), _) => vec![],
        };
        if answered {
            list.push("\\Answered");
        }
        let plain = (!list.is_empty()).then(|| format!("({})", list.join(" ")));
        list.extend(keywords.iter().map(String::as_str));
        let tagged = (!list.is_empty()).then(|| format!("({})", list.join(" ")));
        let mut result = timeout(
            FETCH_TIMEOUT,
            session.append(&remote, tagged.as_deref(), None, &raw),
        )
        .await
        .map_err(|_| SyncError::Timeout)?;
        // A server that minds the keywords still takes the message without them.
        if !keywords.is_empty()
            && matches!(
                result,
                Err(ImapError::No(_) | ImapError::Bad(_) | ImapError::Append)
            )
        {
            result = timeout(
                FETCH_TIMEOUT,
                session.append(&remote, plain.as_deref(), None, &raw),
            )
            .await
            .map_err(|_| SyncError::Timeout)?;
        }
        match result {
            Ok(()) => {
                // A draft is replaced again and again: its UID is looked up now, so the
                // next version can expunge exactly this one (they share a Message-ID).
                let uid = if *draft {
                    newest_unknown(session, store, to, &remote, change.message_id.as_deref())
                        .await?
                } else {
                    None
                };
                lock(store)?.change_appended(change, id, to, uid)?;
            }
            Err(ImapError::No(reason) | ImapError::Bad(reason)) => {
                note_refusal(store, change, &reason, &mut refused)?;
            }
            Err(ImapError::Append) => {
                let reason = tr!("the server didn't accept it", "serwer jej nie przyjął");
                note_refusal(store, change, &reason, &mut refused)?;
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Mail that left for this PC before Telex knew its UID on the server (just moved or
    // uploaded there): find it by its Message-ID, never taking a message that is still here.
    // When it isn't there, there is nothing left to remove.
    let mut changes = changes;
    for change in changes
        .iter_mut()
        .filter(|c| c.kind == ChangeKind::Expunge && c.uid.is_none())
    {
        let (Some(mid), Some(remote)) = (&change.message_id, remote_name(account, &change.folder))
        else {
            lock(store)?.change_done(change.op)?;
            continue;
        };
        match timeout(COMMAND_TIMEOUT, session.examine(&remote))
            .await
            .map_err(|_| SyncError::Timeout)?
        {
            Ok(_) => {}
            Err(ImapError::No(_)) => continue,
            Err(e) => return Err(e.into()),
        }
        let found = find_by_message_id(session, mid).await?;
        let known = lock(store)?.local_uids(&change.folder)?;
        match found.into_iter().filter(|u| !known.contains(u)).max() {
            Some(uid) => change.uid = Some(uid),
            None => lock(store)?.change_done(change.op)?,
        }
    }

    // Grouped by server folder, so each folder is selected once. A moved message whose
    // new UID isn't known yet waits until a download finds it.
    let mut by_folder: BTreeMap<String, Vec<PendingChange>> = BTreeMap::new();
    for change in changes.into_iter().filter(|c| c.uid.is_some()) {
        by_folder
            .entry(change.folder.clone())
            .or_default()
            .push(change);
    }

    let mut moved_without_uid = Vec::new();
    for (key, group) in by_folder {
        let Some(remote) = remote_name(account, &key) else {
            continue; // the folder is gone; the next download drops its mail and changes
        };
        let mailbox = match timeout(COMMAND_TIMEOUT, session.select(&remote))
            .await
            .map_err(|_| SyncError::Timeout)?
        {
            Ok(mailbox) => mailbox,
            Err(ImapError::No(reason)) => {
                for change in &group {
                    note_refusal(store, change, &reason, &mut refused)?;
                }
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        // Tags the server can't keep stay on this PC; the user hears it once. $Forwarded is a
        // keyword too: where tags can't go, it stays here with them.
        let mut group = group;
        if group.iter().any(|c| {
            matches!(
                c.kind,
                ChangeKind::Keyword { .. } | ChangeKind::Forwarded(_)
            )
        }) {
            let mut s = lock(store)?;
            let mut here = s.tags_kept_here(&account.id)?;
            let mut kept = Vec::with_capacity(group.len());
            for change in group {
                let keyword = match &change.kind {
                    ChangeKind::Keyword { keyword, .. } => keyword.as_str(),
                    ChangeKind::Forwarded(_) => FORWARDED,
                    _ => {
                        kept.push(change);
                        continue;
                    }
                };
                if !here && !keeps_keyword(&mailbox.permanent_flags, keyword) {
                    here = true;
                    if s.keep_tags_here(&account.id)? {
                        refused.push(tr!(
                            "the server can't keep tags, so they stay on this PC",
                            "serwer nie przechowuje etykiet, więc zostają na tym komputerze"
                        ));
                    }
                }
                if here {
                    s.change_done(change.op)?;
                } else {
                    kept.push(change);
                }
            }
            group = kept;
        }
        let mut uids: Vec<u32> = group.iter().filter_map(|c| c.uid).collect();
        uids.sort_unstable();
        uids.dedup();
        let mut present = HashSet::new();
        for chunk in uids.chunks(BATCH) {
            present.extend(
                timeout(
                    COMMAND_TIMEOUT,
                    session.uid_search(format!("UID {}", uid_set(chunk))),
                )
                .await
                .map_err(|_| SyncError::Timeout)??,
            );
        }

        let mut live = Vec::new();
        for change in group {
            if change.uid.is_some_and(|u| present.contains(&u)) {
                live.push(change);
                continue;
            }
            // Already gone from the server (deleted elsewhere): nothing left to change.
            match change.message {
                Some(id) => lock(store)?.forget_message(id)?,
                None => lock(store)?.change_done(change.op)?,
            }
        }
        // The same change to many messages is one command: a flag for all of them, a move to
        // one folder, an expunge. Flags first; then moves and expunges, which end the UIDs.
        for batch in batches(live) {
            for chunk in batch.chunks(BATCH) {
                let outcome = timeout(FETCH_TIMEOUT, apply_batch(session, account, chunk, caps))
                    .await
                    .map_err(|_| SyncError::Timeout)?;
                match outcome {
                    Ok(new_uids) => {
                        for (change, new_uid) in chunk.iter().zip(new_uids) {
                            match (&change.kind, change.message) {
                                (ChangeKind::Move(to), Some(id)) => {
                                    lock(store)?.change_moved(change.op, id, to, new_uid)?;
                                    if new_uid.is_none() {
                                        moved_without_uid.push((
                                            id,
                                            to.clone(),
                                            change.message_id.clone(),
                                        ));
                                    }
                                }
                                _ => lock(store)?.change_done(change.op)?,
                            }
                        }
                    }
                    // A refused batch goes again one message at a time, so only the message
                    // the server minds is held back.
                    Err(ImapError::No(reason) | ImapError::Bad(reason)) if chunk.len() == 1 => {
                        note_refusal(store, &chunk[0], &reason, &mut refused)?;
                    }
                    Err(ImapError::No(_) | ImapError::Bad(_)) => {
                        for change in chunk {
                            let one = std::slice::from_ref(change);
                            let result =
                                timeout(COMMAND_TIMEOUT, apply_batch(session, account, one, caps))
                                    .await
                                    .map_err(|_| SyncError::Timeout)?;
                            match (result, &change.kind, change.message) {
                                (Ok(new_uid), ChangeKind::Move(to), Some(id)) => {
                                    let new_uid = new_uid.first().copied().flatten();
                                    lock(store)?.change_moved(change.op, id, to, new_uid)?;
                                    if new_uid.is_none() {
                                        moved_without_uid.push((
                                            id,
                                            to.clone(),
                                            change.message_id.clone(),
                                        ));
                                    }
                                }
                                (Ok(_), _, _) => lock(store)?.change_done(change.op)?,
                                (Err(ImapError::No(reason) | ImapError::Bad(reason)), _, _) => {
                                    note_refusal(store, change, &reason, &mut refused)?;
                                }
                                (Err(e), _, _) => return Err(e.into()),
                            }
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }

    // The server didn't say where moved messages ended up: look them up by Message-ID.
    for (id, key, message_id) in moved_without_uid {
        let (Some(mid), Some(remote)) = (message_id, remote_name(account, &key)) else {
            continue;
        };
        if timeout(COMMAND_TIMEOUT, session.examine(&remote))
            .await
            .map_err(|_| SyncError::Timeout)?
            .is_err()
        {
            continue;
        }
        let found = find_by_message_id(session, &mid).await?;
        let known = lock(store)?.local_uids(&key)?;
        if let Some(uid) = found.into_iter().filter(|u| !known.contains(u)).max() {
            lock(store)?.set_server_uid(id, &key, uid)?;
        }
    }
    Ok(refused)
}

/// Before Telex uploads a copy of sent mail to Sent: is one needed, or does the server keep
/// its own? The account's choice or the provider list may say; else Telex looks for the
/// message in Sent and remembers what it found. When the server has its own, the upload goes
/// and the message points at the server's copy. False: no upload (now). A server Telex
/// doesn't know yet gets a moment after the send to file its copy before Telex decides.
async fn sent_copy_needed<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    change: &PendingChange,
    id: MessageId,
    to: &str,
    remote: &str,
) -> Result<bool, SyncError> {
    let into_sent = account
        .folder_of_kind(FolderKind::Sent)
        .is_some_and(|f| account.folder_key(&f.id) == to);
    if !into_sent {
        return Ok(true); // moved on from Sent before it went up: it goes where it is now
    }
    let keeps = lock(store)?.account(&account.id)?.server_keeps_sent();
    match keeps {
        Some(false) => return Ok(true),
        Some(true) => {
            lock(store)?.server_filed_sent(change.op, id, to, None)?;
            return Ok(false);
        }
        None => {}
    }
    let Some(mid) = change.message_id.as_deref() else {
        return Ok(true);
    };
    match timeout(COMMAND_TIMEOUT, session.examine(remote))
        .await
        .map_err(|_| SyncError::Timeout)?
    {
        Ok(_) => {}
        Err(ImapError::No(_)) => return Ok(true),
        Err(e) => return Err(e.into()),
    }
    let found = find_by_message_id(session, mid).await?.into_iter().max();
    let mut s = lock(store)?;
    if let Some(uid) = found {
        s.found_sent_copy(&account.id, true)?;
        s.server_filed_sent(change.op, id, to, Some(uid))?;
        return Ok(false);
    }
    if s.sent_just_now(id, Local::now().naive_local())? {
        return Ok(false);
    }
    s.found_sent_copy(&account.id, false)?;
    Ok(true)
}

/// The UID of the newest message with this Message-ID in a server folder that this PC
/// doesn't know yet: the one just uploaded there (UIDs only grow).
async fn newest_unknown<T: ImapStream>(
    session: &mut Session<T>,
    store: &Mutex<MailStore>,
    folder_key: &str,
    remote: &str,
    mid: Option<&str>,
) -> Result<Option<u32>, SyncError> {
    let Some(mid) = mid else { return Ok(None) };
    match timeout(COMMAND_TIMEOUT, session.examine(remote))
        .await
        .map_err(|_| SyncError::Timeout)?
    {
        Ok(_) => {}
        Err(ImapError::No(_)) => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let found = find_by_message_id(session, mid).await?;
    let known = lock(store)?.local_uids(folder_key)?;
    Ok(found.into_iter().filter(|u| !known.contains(u)).max())
}

// ---- Connection ----

/// One TLS setup for every connection, so its session cache lets a server resume a session
/// instead of a full handshake on each check.
pub(crate) fn tls_connector() -> TlsConnector {
    static CONFIG: std::sync::LazyLock<Arc<ClientConfig>> = std::sync::LazyLock::new(|| {
        let roots = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
        let config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("the ring provider supports TLS 1.2 and 1.3")
            .with_root_certificates(roots)
            .with_no_client_auth();
        Arc::new(config)
    });
    TlsConnector::from(CONFIG.clone())
}

/// Connect to the server, encrypted from the first byte (SSL/TLS) or after STARTTLS, read
/// its greeting and log in. Nothing ever goes over the network unencrypted except the
/// greeting and the STARTTLS command itself.
///
/// First it waits for one of the server's places (`servers`); the turn goes with the session
/// and gives the place back when the caller drops it, after logging out. Every step goes into
/// the connection log (Settings › Connections), timed, with the error where one failed.
async fn connect_and_login(
    settings: &ImapSettings,
    password: &str,
    purpose: Purpose,
) -> Result<(Session<TlsStream<TcpStream>>, Turn), SyncError> {
    let host = &settings.host;
    let mut trace = Trace::new(purpose, "imap", &settings.username, host, settings.port);
    let key = servers::key(host, settings.port).await;
    trace.address(&key);
    trace.done();
    trace.begin(Step::Wait);
    let mut turn = match SERVERS.turn(&key, host, purpose.places()).await {
        Ok(turn) => turn,
        Err(left) => {
            let until = Local::now() + TimeDelta::from_std(left).unwrap_or_default();
            return Err(failed(trace, SyncError::Resting(until)));
        }
    };
    trace.done();
    let client = match connect(settings, turn.key(), &mut trace).await {
        Ok(client) => client,
        Err(e) => return Err(failed(trace, e)),
    };
    trace.begin(Step::Login);
    let session = match login(client, settings, password).await {
        Ok(session) => session,
        Err(e) => return Err(failed(trace, e)),
    };
    trace.done();
    turn.opened(trace);
    Ok((session, turn))
}

/// Into the log with the step it failed at.
fn failed(trace: Trace, e: SyncError) -> SyncError {
    trace.fail(&e.to_string());
    e
}

/// How the work of a logged-in connection ended, for its line in the log.
fn outcome<T>(turn: &mut Turn, result: &Result<T, SyncError>) {
    if let Err(e) = result {
        turn.failed(e);
    }
}

/// A connection with the greeting read, ready to log in. A server that takes the connection
/// but doesn't greet gets a rest (`Servers::silent`); one that greets, its rests start over.
async fn connect(
    settings: &ImapSettings,
    key: &str,
    trace: &mut Trace,
) -> Result<Client<TlsStream<TcpStream>>, SyncError> {
    let host = settings.host.as_str();
    let silent = |e: SyncError| {
        SERVERS.silent(key);
        e
    };
    let unreachable = |reason: String| SyncError::Connect {
        host: host.to_owned(),
        reason,
    };
    trace.begin(Step::Connect);
    let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect((host, settings.port)))
        .await
        .map_err(|_| {
            silent(unreachable(tr!(
                "no answer within 20 seconds",
                "brak odpowiedzi w ciągu 20 sekund"
            )))
        })?
        .map_err(|e| unreachable(e.to_string()))?;
    trace.done();
    // async-imap writes a command in four small pieces (tag, space, command, line end). With
    // Nagle's algorithm on, the rest waits for the server to acknowledge the first piece, and
    // servers delay that: every command cost the round trip plus up to 200 ms.
    let _ = tcp.set_nodelay(true);
    let name = ServerName::try_from(host)
        .map_err(|_| {
            unreachable(tr!(
                "that isn't a valid server name",
                "to nie jest poprawna nazwa serwera"
            ))
        })?
        .to_owned();
    let starttls = settings.security == SmtpSecurity::StartTls;
    let tcp = if starttls {
        trace.begin(Step::Starttls);
        let mut plain = Client::new(tcp);
        greet(&mut plain).await.map_err(silent)?;
        match timeout(
            COMMAND_TIMEOUT,
            plain.run_command_and_check_ok("STARTTLS", None),
        )
        .await
        {
            Err(_) => return Err(SyncError::Timeout),
            Ok(Err(e)) => {
                return Err(SyncError::Tls {
                    host: host.to_owned(),
                    reason: tr!(
                        "it doesn't offer STARTTLS ({e}). Try SSL/TLS on port 993",
                        "nie obsługuje STARTTLS ({e}). Spróbuj SSL/TLS na porcie 993"
                    ),
                })
            }
            Ok(Ok(())) => plain.into_inner(),
        }
    } else {
        tcp
    };
    if starttls {
        trace.done();
    }
    trace.begin(Step::Tls);
    let tls = timeout(CONNECT_TIMEOUT, tls_connector().connect(name, tcp))
        .await
        .map_err(|_| silent(SyncError::Timeout))?
        .map_err(|e| SyncError::Tls {
            host: host.to_owned(),
            reason: tls_reason(&e),
        })?;
    trace.done();
    let mut client = Client::new(tls);
    // After STARTTLS the server says nothing more; the greeting came before it.
    if !starttls {
        trace.begin(Step::Greeting);
        greet(&mut client).await.map_err(silent)?;
        trace.done();
    }
    SERVERS.answered(key);
    Ok(client)
}

/// Plain words for the certificate errors people actually hit.
fn tls_reason(e: &std::io::Error) -> String {
    let text = e.to_string();
    if text.contains("NotValidForName") {
        tr!(
            "its certificate is for a different name. Use the server name from your provider's setup guide",
            "certyfikat serwera jest wystawiony na inną nazwę. Użyj nazwy serwera z instrukcji \
             Twojego dostawcy poczty"
        )
    } else if text.contains("UnknownIssuer") {
        tr!(
            "its certificate isn't from a trusted authority",
            "certyfikat serwera nie pochodzi od zaufanego wystawcy"
        )
    } else if text.contains("Expired") {
        tr!("its certificate has expired", "certyfikat serwera wygasł")
    } else {
        text
    }
}

/// A broken connection, in words; the system's own reason stays as it is.
fn connection_error(e: &impl fmt::Display) -> String {
    tr!("connection error: {e}", "błąd połączenia: {e}")
}

/// Read the server's greeting and log in.
#[cfg(test)]
async fn open_session<T: ImapStream>(
    mut client: Client<T>,
    settings: &ImapSettings,
    password: &str,
) -> Result<Session<T>, SyncError> {
    greet(&mut client).await?;
    login(client, settings, password).await
}

/// Read the server's greeting.
async fn greet<T: ImapStream>(client: &mut Client<T>) -> Result<(), SyncError> {
    match timeout(CONNECT_TIMEOUT, client.read_response()).await {
        Err(_) => Err(SyncError::Timeout),
        Ok(Err(e)) => Err(SyncError::Server(connection_error(&e))),
        Ok(Ok(None)) => Err(SyncError::Server(tr!(
            "it hung up before saying hello",
            "rozłączył się, zanim się przywitał"
        ))),
        Ok(Ok(Some(greeting))) => match greeting.parsed() {
            Response::Data {
                status: Status::Bye,
                information,
                ..
            } => {
                let reason = information.as_deref().unwrap_or_default().trim().to_owned();
                Err(SyncError::Server(tr!(
                    "it said goodbye before logging in ({reason})",
                    "pożegnał się przed logowaniem ({reason})"
                )))
            }
            _ => Ok(()),
        },
    }
}

/// Log in on a connection whose greeting was read.
async fn login<T: ImapStream>(
    client: Client<T>,
    settings: &ImapSettings,
    password: &str,
) -> Result<Session<T>, SyncError> {
    match timeout(COMMAND_TIMEOUT, client.login(&settings.username, password)).await {
        Err(_) => Err(SyncError::Timeout),
        Ok(Ok(session)) => Ok(session),
        Ok(Err((ImapError::No(m) | ImapError::Bad(m), _))) => {
            Err(SyncError::Login(m.trim().to_owned()))
        }
        Ok(Err((e, _))) => Err(e.into()),
    }
}

async fn logout<T: ImapStream>(session: &mut Session<T>) {
    let _ = timeout(Duration::from_secs(5), session.logout()).await;
}

async fn server_caps<T: ImapStream>(session: &mut Session<T>) -> Result<ServerCaps, SyncError> {
    let caps = timeout(COMMAND_TIMEOUT, session.capabilities())
        .await
        .map_err(|_| SyncError::Timeout)??;
    Ok(ServerCaps {
        can_move: caps.has_str("MOVE"),
        uidplus: caps.has_str("UIDPLUS"),
        // RFC 2087 says QUOTA; RFC 9208 servers may name only QUOTA=RES-STORAGE and the like.
        quota: caps.has_str("QUOTA")
            || caps
                .iter()
                .any(|c| matches!(c, Capability::Atom(a) if a.starts_with("QUOTA="))),
    })
}

/// The account's storage on the server, from the quota root of its Inbox. `None` when the
/// server sets no storage limit or doesn't answer; a quota is a nicety, never an error.
async fn storage_quota<T: ImapStream>(session: &mut Session<T>) -> Option<Quota> {
    let (_, quotas) = timeout(COMMAND_TIMEOUT, session.get_quota_root("INBOX"))
        .await
        .ok()?
        .ok()?;
    quotas
        .iter()
        .flat_map(|q| &q.resources)
        .find(|r| r.name == QuotaResourceName::Storage && r.limit > 0)
        .map(|r| Quota {
            used_kb: r.usage,
            limit_kb: r.limit,
        })
}

/// The store is shared with the UI thread; lock it only for quick database work.
fn lock(store: &Mutex<MailStore>) -> Result<MutexGuard<'_, MailStore>, SyncError> {
    store
        .lock()
        .map_err(|_| SyncError::Store(StoreError::poisoned()))
}

// ---- Downloading ----

async fn list_folders<T: ImapStream>(
    session: &mut Session<T>,
) -> Result<Vec<RemoteFolder>, SyncError> {
    let names = timeout(COMMAND_TIMEOUT, async {
        let stream = session.list(Some(""), Some("*")).await?;
        stream.try_collect::<Vec<Name>>().await
    })
    .await
    .map_err(|_| SyncError::Timeout)??;
    Ok(names.iter().map(RemoteFolder::from_name).collect())
}

/// The messages, and the flags of those already answered or forwarded (on another device):
/// the store takes those after storing them (`refresh_flags`).
async fn fetch_batch<T: ImapStream>(
    session: &mut Session<T>,
    uids: &[u32],
    folder_key: &str,
) -> Result<(Vec<SyncedMessage>, Vec<ServerFlags>), SyncError> {
    let fetches = timeout(FETCH_TIMEOUT, async {
        let stream = session.uid_fetch(uid_set(uids), FETCH_ITEMS).await?;
        stream.try_collect::<Vec<Fetch>>().await
    })
    .await
    .map_err(|_| SyncError::Timeout)??;
    let wanted: HashSet<u32> = uids.iter().copied().collect();
    let mut answered = Vec::new();
    let messages = fetches
        .iter()
        .filter_map(|f| {
            // Servers may add FETCH lines for other messages; keep only what was asked for.
            let uid = f.uid.filter(|u| wanted.contains(u))?;
            let raw = f.body()?;
            let received = f
                .internal_date()
                .map(|d| d.with_timezone(&Local).naive_local());
            let mut message = parse_message(raw, folder_key, received);
            let state = flag_state(f, uid);
            message.unread = !state.seen;
            message.flagged = state.flagged;
            message.tags = state.keywords.clone();
            if state.answered || state.forwarded {
                answered.push(state);
            }
            Some(SyncedMessage {
                uid,
                message,
                raw: raw.to_vec(),
            })
        })
        .collect();
    Ok((messages, answered))
}

/// \Seen, \Flagged and the keywords of messages already stored.
async fn fetch_flags<T: ImapStream>(
    session: &mut Session<T>,
    uids: &[u32],
) -> Result<Vec<ServerFlags>, SyncError> {
    let fetches = timeout(COMMAND_TIMEOUT, async {
        let stream = session.uid_fetch(uid_set(uids), "(UID FLAGS)").await?;
        stream.try_collect::<Vec<Fetch>>().await
    })
    .await
    .map_err(|_| SyncError::Timeout)??;
    Ok(fetches
        .iter()
        .filter_map(|f| Some(flag_state(f, f.uid?)))
        .collect())
}

fn flag_state(f: &Fetch, uid: u32) -> ServerFlags {
    let mut state = ServerFlags {
        uid,
        ..ServerFlags::default()
    };
    for flag in f.flags() {
        match flag {
            Flag::Seen => state.seen = true,
            Flag::Flagged => state.flagged = true,
            Flag::Answered => state.answered = true,
            Flag::Custom(k) if crate::tags::is_keyword(&k) => {
                state.forwarded |= k.eq_ignore_ascii_case(FORWARDED);
                state.keywords.push(k.to_ascii_lowercase());
            }
            _ => {}
        }
    }
    state
}

/// The keyword for forwarded mail, as Thunderbird, Apple Mail and Dovecot write it (RFC 5788's
/// registry). A server keeps it where it keeps keywords: Gmail does (\* in PERMANENTFLAGS).
const FORWARDED: &str = "$Forwarded";

/// The folder selected keeps this keyword: its PERMANENTFLAGS lists \* or the keyword, or
/// says nothing (RFC 3501: then every flag is kept).
fn keeps_keyword(permanent: &[Flag<'_>], keyword: &str) -> bool {
    permanent.is_empty()
        || permanent.iter().any(|f| match f {
            Flag::MayCreate => true,
            Flag::Custom(k) => k.eq_ignore_ascii_case(keyword),
            _ => false,
        })
}

// ---- Uploading ----

/// Most UIDs in one command: keeps command lines short on fragmented folders.
const BATCH: usize = 500;

/// Changes that can go to the server as one command: the same flag change, a move to the
/// same folder, or expunges. Flags come first, in the order they were made; moves and
/// expunges after them, since they end the messages' UIDs in this folder.
fn batches(changes: Vec<PendingChange>) -> Vec<Vec<PendingChange>> {
    let mut groups: Vec<Vec<PendingChange>> = Vec::new();
    let order = |c: &PendingChange| matches!(c.kind, ChangeKind::Move(_) | ChangeKind::Expunge);
    let mut changes = changes;
    changes.sort_by_key(|c| (order(c), c.op));
    for change in changes {
        match groups.iter_mut().find(|g| g[0].kind == change.kind) {
            Some(group) => group.push(change),
            None => groups.push(vec![change]),
        }
    }
    groups
}

/// Do one kind of change for several messages of the folder that is selected, in one
/// command. Returns, per change, the message's new UID after a move when the server says it.
async fn apply_batch<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    changes: &[PendingChange],
    caps: ServerCaps,
) -> Result<Vec<Option<u32>>, ImapError> {
    let mut uids: Vec<u32> = changes.iter().filter_map(|c| c.uid).collect();
    uids.sort_unstable();
    uids.dedup();
    let set = uid_set(&uids);
    let none = vec![None; changes.len()];
    let Some(first) = changes.first() else {
        return Ok(none);
    };
    match &first.kind {
        ChangeKind::Seen(on) => store_flag(session, &set, "\\Seen", *on)
            .await
            .map(|()| none),
        ChangeKind::Flagged(on) => store_flag(session, &set, "\\Flagged", *on)
            .await
            .map(|()| none),
        ChangeKind::Answered(on) => store_flag(session, &set, "\\Answered", *on)
            .await
            .map(|()| none),
        ChangeKind::Forwarded(on) => store_flag(session, &set, FORWARDED, *on)
            .await
            .map(|()| none),
        ChangeKind::Keyword { keyword, on } => {
            store_flag(session, &set, keyword, *on).await.map(|()| none)
        }
        ChangeKind::Expunge => expunge(session, &set, caps).await.map(|()| none),
        ChangeKind::Append { .. } => Ok(none), // sent before the folders, in push_queued_after
        ChangeKind::Move(to) => {
            let dest = remote_name(account, to).ok_or_else(|| {
                ImapError::No(tr!(
                    "that folder no longer exists",
                    "tego folderu już nie ma"
                ))
            })?;
            let pairs = if caps.can_move {
                run_with_copyuid(session, &format!("UID MOVE {set} {}", quote(&dest))).await?
            } else {
                let pairs =
                    run_with_copyuid(session, &format!("UID COPY {set} {}", quote(&dest))).await?;
                expunge(session, &set, caps).await?;
                pairs
            };
            Ok(changes
                .iter()
                .map(|c| {
                    let uid = c.uid?;
                    pairs
                        .iter()
                        .find(|(from, _)| *from == uid)
                        .map(|(_, to)| *to)
                })
                .collect())
        }
    }
}

async fn store_flag<T: ImapStream>(
    session: &mut Session<T>,
    uids: &str,
    flag: &str,
    on: bool,
) -> Result<(), ImapError> {
    let sign = if on { '+' } else { '-' };
    let stream = session
        .uid_store(uids, format!("{sign}FLAGS.SILENT ({flag})"))
        .await?;
    stream.try_collect::<Vec<_>>().await?;
    Ok(())
}

/// Remove messages for good. Without UIDPLUS the only way is a plain EXPUNGE, which
/// also removes other messages already marked \Deleted in that folder, as any client would.
async fn expunge<T: ImapStream>(
    session: &mut Session<T>,
    uids: &str,
    caps: ServerCaps,
) -> Result<(), ImapError> {
    store_flag(session, uids, "\\Deleted", true).await?;
    if caps.uidplus {
        session
            .uid_expunge(uids)
            .await?
            .try_collect::<Vec<_>>()
            .await?;
    } else {
        session.expunge().await?.try_collect::<Vec<_>>().await?;
    }
    Ok(())
}

/// Run a COPY or MOVE and pick the old and new UIDs out of its COPYUID response code
/// (RFC 4315): the two sets list the messages in the same order. MOVE sends it in an
/// untagged OK, COPY in the tagged one. Empty when the server doesn't say.
async fn run_with_copyuid<T: ImapStream>(
    session: &mut Session<T>,
    command: &str,
) -> Result<Vec<(u32, u32)>, ImapError> {
    let tag = session.run_command(command).await?;
    let mut pairs = Vec::new();
    loop {
        let response = session
            .read_response()
            .await?
            .ok_or(ImapError::ConnectionLost)?;
        match response.parsed() {
            Response::Data {
                code: Some(ResponseCode::CopyUid(_, from, to)),
                ..
            } => pairs.extend(uid_pairs(from, to)),
            Response::Done {
                tag: done,
                status,
                code,
                information,
            } if *done == tag => {
                if let Some(ResponseCode::CopyUid(_, from, to)) = code {
                    pairs.extend(uid_pairs(from, to));
                }
                let text = information.as_deref().unwrap_or_default().to_owned();
                return match status {
                    Status::Ok => Ok(pairs),
                    Status::No => Err(ImapError::No(text)),
                    _ => Err(ImapError::Bad(text)),
                };
            }
            _ => {}
        }
    }
}

/// The two sets of a COPYUID, side by side. Sets that don't match up give nothing; the
/// messages are then found by Message-ID.
fn uid_pairs(from: &[UidSetMember], to: &[UidSetMember]) -> Vec<(u32, u32)> {
    let (from, to) = (expand_set(from), expand_set(to));
    if from.len() != to.len() {
        return Vec::new();
    }
    from.into_iter().zip(to).collect()
}

fn expand_set(set: &[UidSetMember]) -> Vec<u32> {
    set.iter()
        .flat_map(|m| match m {
            UidSetMember::Uid(u) => *u..=*u,
            UidSetMember::UidRange(r) => *r.start()..=*r.end(),
        })
        .take(100_000)
        .collect()
}

/// Count a refusal; on the last attempt drop the change and say so.
fn note_refusal(
    store: &Mutex<MailStore>,
    change: &PendingChange,
    reason: &str,
    refused: &mut Vec<String>,
) -> Result<(), SyncError> {
    let mut store = lock(store)?;
    if store.change_failed(change.op, reason)? >= MAX_ATTEMPTS {
        store.give_up_change(change)?;
        // A whole sentence for each change: languages put the subject in different places.
        let (subject, reason) = (&change.subject, reason.trim());
        refused.push(match &change.kind {
            ChangeKind::Seen(true) => tr!(
                "the server wouldn't mark \"{subject}\" as read ({reason})",
                "serwer nie pozwolił oznaczyć wiadomości „{subject}” jako przeczytanej ({reason})"
            ),
            ChangeKind::Seen(false) => tr!(
                "the server wouldn't mark \"{subject}\" as unread ({reason})",
                "serwer nie pozwolił oznaczyć wiadomości „{subject}” jako nieprzeczytanej ({reason})"
            ),
            ChangeKind::Flagged(true) => tr!(
                "the server wouldn't flag \"{subject}\" ({reason})",
                "serwer nie pozwolił oznaczyć flagą wiadomości „{subject}” ({reason})"
            ),
            ChangeKind::Flagged(false) => tr!(
                "the server wouldn't unflag \"{subject}\" ({reason})",
                "serwer nie pozwolił zdjąć flagi z wiadomości „{subject}” ({reason})"
            ),
            ChangeKind::Answered(_) => tr!(
                "the server wouldn't mark \"{subject}\" as answered ({reason})",
                "serwer nie pozwolił oznaczyć wiadomości „{subject}” jako odpowiedzianej ({reason})"
            ),
            ChangeKind::Forwarded(_) => tr!(
                "the server wouldn't mark \"{subject}\" as forwarded ({reason})",
                "serwer nie pozwolił oznaczyć wiadomości „{subject}” jako przekazanej dalej ({reason})"
            ),
            ChangeKind::Keyword { on: true, .. } => tr!(
                "the server wouldn't tag \"{subject}\" ({reason})",
                "serwer nie pozwolił oznaczyć etykietą wiadomości „{subject}” ({reason})"
            ),
            ChangeKind::Keyword { on: false, .. } => tr!(
                "the server wouldn't take a tag off \"{subject}\" ({reason})",
                "serwer nie pozwolił zdjąć etykiety z wiadomości „{subject}” ({reason})"
            ),
            ChangeKind::Move(_) => tr!(
                "the server wouldn't move \"{subject}\" ({reason})",
                "serwer nie pozwolił przenieść wiadomości „{subject}” ({reason})"
            ),
            ChangeKind::Expunge => tr!(
                "the server wouldn't delete \"{subject}\" ({reason})",
                "serwer nie pozwolił usunąć wiadomości „{subject}” ({reason})"
            ),
            ChangeKind::Append { flags: None, .. } => tr!(
                "the server wouldn't save a copy of \"{subject}\" in Sent ({reason})",
                "serwer nie pozwolił zapisać kopii wiadomości „{subject}” w folderze Wysłane ({reason})"
            ),
            ChangeKind::Append { .. } => tr!(
                "the server wouldn't take \"{subject}\" ({reason})",
                "serwer nie pozwolił przesłać wiadomości „{subject}” ({reason})"
            ),
        });
    }
    Ok(())
}

/// The server's name for one of the account's folders.
fn remote_name(account: &Account, folder_key: &str) -> Option<String> {
    account
        .folders
        .iter()
        .find(|f| account.folder_key(&f.id) == folder_key)
        .and_then(|f| f.remote.clone())
}

/// An IMAP quoted string: quotes and backslashes escaped. Line breaks and NULs can't be in
/// one at all, so they are left out (folder names from the server's LIST have none).
fn quote(s: &str) -> String {
    let s: String = s
        .chars()
        .filter(|c| !matches!(c, '\r' | '\n' | '\0'))
        .collect();
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The UIDs in the selected folder of the message with this Message-ID. An id that isn't
/// plain printable ASCII isn't searched for (no server would match it, and some answer BAD);
/// a server that refuses the search counts as not finding it, so one odd message can't stop
/// an account's sync.
async fn find_by_message_id<T: ImapStream>(
    session: &mut Session<T>,
    mid: &str,
) -> Result<HashSet<u32>, SyncError> {
    if mid.is_empty() || !mid.bytes().all(|b| (0x21..0x7f).contains(&b)) {
        return Ok(HashSet::new());
    }
    let query = format!("HEADER Message-ID {}", quote(&format!("<{mid}>")));
    match timeout(COMMAND_TIMEOUT, session.uid_search(query))
        .await
        .map_err(|_| SyncError::Timeout)?
    {
        Ok(found) => Ok(found),
        Err(ImapError::No(_) | ImapError::Bad(_)) => Ok(HashSet::new()),
        Err(e) => Err(e.into()),
    }
}

// ---- Pure helpers (unit-tested) ----

/// Which UIDs to download: everything newer than `last_uid`, or on the first sync
/// only the newest `first_limit`. Sorted oldest first, so an interrupted sync resumes
/// where it stopped.
pub fn uids_to_fetch(
    on_server: &HashSet<u32>,
    last_uid: Option<u32>,
    first_limit: usize,
) -> Vec<u32> {
    let mut uids: Vec<u32> = on_server
        .iter()
        .copied()
        .filter(|&u| last_uid.is_none_or(|last| u > last))
        .collect();
    uids.sort_unstable();
    if last_uid.is_none() && uids.len() > first_limit {
        uids.drain(..uids.len() - first_limit);
    }
    uids
}

/// Which older UIDs to download: up to `limit` of the newest that are on the server but not
/// here, below the newest one already downloaded (anything above it is new mail, for a
/// regular sync). Sorted oldest first. Nothing before a folder's first sync.
pub fn older_to_fetch(
    on_server: &HashSet<u32>,
    local: &HashSet<u32>,
    last_uid: Option<u32>,
    limit: usize,
) -> Vec<u32> {
    let Some(last) = last_uid else {
        return Vec::new();
    };
    let mut uids: Vec<u32> = on_server
        .iter()
        .copied()
        .filter(|u| *u <= last && !local.contains(u))
        .collect();
    uids.sort_unstable_by(|a, b| b.cmp(a));
    uids.truncate(limit);
    uids.reverse();
    uids
}

/// IMAP sequence-set syntax for sorted UIDs: [1, 2, 3, 5] → "1:3,5".
fn uid_set(uids: &[u32]) -> String {
    let mut parts = Vec::new();
    let mut i = 0;
    while i < uids.len() {
        let start = uids[i];
        let mut end = start;
        while i + 1 < uids.len() && uids[i + 1] == end + 1 {
            i += 1;
            end = uids[i];
        }
        parts.push(if start == end {
            start.to_string()
        } else {
            format!("{start}:{end}")
        });
        i += 1;
    }
    parts.join(",")
}

/// The kinds a server folder can be besides the Inbox and the user's own, in the order
/// [`plan_folders`] hands them out.
pub const SPECIAL_KINDS: [FolderKind; 5] = [
    FolderKind::Sent,
    FolderKind::Drafts,
    FolderKind::Trash,
    FolderKind::Spam,
    FolderKind::Archive,
];

/// What Telex knows about an account's folders that the server's list doesn't say, by
/// server name: the kinds the user chose, the kinds they had last time, and which hold mail.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderHints {
    pub chosen: HashMap<String, FolderKind>,
    pub was: HashMap<String, FolderKind>,
    pub held: HashSet<String>,
}

/// Decide which server folders Telex shows, and their ids, names and kinds.
/// Ids come from the server names, so they stay the same from one sync to the next; names
/// are the server's own ("Wysłane"), and the UI shows special folders by their kind.
pub fn plan_folders(remote: &[RemoteFolder], hints: &FolderHints) -> Vec<Folder> {
    let syncable: Vec<&RemoteFolder> = remote.iter().filter(|f| f.syncable).collect();

    // Kinds: INBOX by name. Each special kind then goes to one folder: the one the user
    // chose; else, of those the server flags for it, whose name says it or that had it last
    // time, one that holds mail before an empty one (a server's own empty "Sent" next to the
    // "Wysłane" another program filled), then the one that had it (so it doesn't hop about),
    // then the server's flag, then the name. The rest are the user's own.
    let mut kinds = vec![FolderKind::Custom; syncable.len()];
    if let Some(i) = syncable
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case("INBOX"))
    {
        kinds[i] = FolderKind::Inbox;
    }
    for kind in SPECIAL_KINDS {
        let best = syncable
            .iter()
            .enumerate()
            .filter(|&(i, f)| {
                kinds[i] == FolderKind::Custom
                    && hints.chosen.get(&f.name).is_none_or(|&c| c == kind)
            })
            .filter_map(|(i, f)| {
                let chosen = hints.chosen.contains_key(&f.name);
                let was = hints.was.get(&f.name) == Some(&kind);
                let flagged = f.special == Some(kind);
                let named = f.path().last().and_then(|leaf| guess_kind(leaf)) == Some(kind);
                let held = hints.held.contains(&f.name);
                (chosen || was || flagged || named)
                    .then_some((i, (chosen, held, was, flagged, named)))
            })
            // The best by those, in that order; of equals the first the server listed.
            .min_by_key(|&(i, rank)| (std::cmp::Reverse(rank), i));
        if let Some((i, _)) = best {
            kinds[i] = kind;
        }
    }

    // "inbox" is the INBOX's; "outbox" is the local Outbox's, never a server folder's.
    let mut used = HashSet::from(["inbox".to_owned(), OUTBOX_ID.to_owned()]);
    let mut folders: Vec<Folder> = syncable
        .iter()
        .zip(&kinds)
        .map(|(f, &kind)| {
            let path = f.path();
            let id = if kind == FolderKind::Inbox {
                "inbox".to_owned()
            } else {
                let base = match slug(&path.join(" ")) {
                    s if s.is_empty() => "folder".to_owned(),
                    s => s,
                };
                let mut id = base.clone();
                let mut n = 2;
                while !used.insert(id.clone()) {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                id
            };
            let name = match kind {
                FolderKind::Inbox => "Inbox".to_owned(),
                _ => display_path(&path),
            };
            Folder {
                id,
                name,
                kind,
                remote: Some(f.name.clone()),
            }
        })
        .collect();
    folders.sort_by_key(|f| (f.kind.rank(), f.name.to_lowercase()));
    folders
}

/// "INBOX / Klienci / Kowalski" reads better as "Klienci / Kowalski".
fn display_path(path: &[String]) -> String {
    let parts = match path {
        [first, rest @ ..] if !rest.is_empty() && first.eq_ignore_ascii_case("INBOX") => rest,
        all => all,
    };
    parts.join(" / ")
}

/// Common English and Polish names, for servers that don't announce special use.
fn guess_kind(leaf: &str) -> Option<FolderKind> {
    let kind = match leaf.trim().to_lowercase().as_str() {
        "sent" | "sent items" | "sent messages" | "sent mail" | "wysłane" | "wyslane"
        | "elementy wysłane" => FolderKind::Sent,
        "drafts" | "draft" | "robocze" | "kopie robocze" | "szkice" => FolderKind::Drafts,
        "trash" | "deleted" | "deleted items" | "deleted messages" | "bin" | "kosz"
        | "elementy usunięte" => FolderKind::Trash,
        "spam" | "junk" | "junk e-mail" | "junk email" | "bulk mail" | "wiadomości-śmieci" => {
            FolderKind::Spam
        }
        "archive" | "archives" | "archiwum" => FolderKind::Archive,
        _ => return None,
    };
    Some(kind)
}

/// Decode an IMAP folder name from modified UTF-7 (RFC 3501 §5.1.3):
/// "Wys&AUI-ane" → "Wysłane", "&-" → "&". Anything malformed is left as it is.
pub fn decode_folder_name(name: &str) -> String {
    let mut out = String::new();
    let mut rest = name;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('-') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let chunk = &after[..end];
        if chunk.is_empty() {
            out.push('&');
        } else if let Some(text) = decode_utf16_base64(chunk) {
            out.push_str(&text);
        } else {
            out.push_str(&rest[start..start + end + 2]);
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Modified base64 ("," instead of "/", no padding) of UTF-16BE text.
fn decode_utf16_base64(chunk: &str) -> Option<String> {
    let mut bits: u32 = 0;
    let mut nbits = 0;
    let mut bytes = Vec::new();
    for c in chunk.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b',' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(v);
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            bytes.push((bits >> nbits) as u8);
            bits &= (1 << nbits) - 1;
        }
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&p| u16::from_be_bytes(p))
        .collect();
    String::from_utf16(&units).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AccountSetup;
    use std::net::SocketAddr;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    #[test]
    fn folder_names_decode_from_modified_utf7() {
        assert_eq!(decode_folder_name("Wys&AUI-ane"), "Wysłane");
        assert_eq!(
            decode_folder_name("Elementy usuni&ARk-te"),
            "Elementy usunięte"
        );
        assert_eq!(decode_folder_name("INBOX.Faktury"), "INBOX.Faktury");
        assert_eq!(decode_folder_name("R&-D"), "R&D");
        assert_eq!(decode_folder_name("broken&AUI"), "broken&AUI");
    }

    #[test]
    fn uid_sets_are_compact() {
        assert_eq!(uid_set(&[1, 2, 3, 5, 7, 8]), "1:3,5,7:8");
        assert_eq!(uid_set(&[42]), "42");
    }

    #[test]
    fn quoting_escapes_quotes_and_backslashes() {
        assert_eq!(quote("Kosz"), "\"Kosz\"");
        assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn sync_errors_in_polish_keep_the_servers_words() {
        use crate::i18n::{self, Lang};
        let login = SyncError::Login("[AUTHENTICATIONFAILED] Invalid credentials".into());
        assert_eq!(
            i18n::with(Lang::Pl, || login.to_string()),
            "serwer odrzucił nazwę użytkownika lub hasło \
             ([AUTHENTICATIONFAILED] Invalid credentials)"
        );
        let lost = i18n::with(Lang::Pl, || {
            SyncError::from(ImapError::ConnectionLost).to_string()
        });
        assert_eq!(lost, "serwer zgłosił problem: połączenie zostało zamknięte");
    }

    #[test]
    fn first_sync_takes_the_newest_then_only_new_ones() {
        let on_server: HashSet<u32> = (1..=10).collect();
        assert_eq!(uids_to_fetch(&on_server, None, 3), [8, 9, 10]);
        assert_eq!(uids_to_fetch(&on_server, Some(7), 3), [8, 9, 10]);
        assert_eq!(uids_to_fetch(&on_server, Some(4), 3), [5, 6, 7, 8, 9, 10]);
        assert!(uids_to_fetch(&on_server, Some(10), 3).is_empty());
    }

    fn remote(name: &str, special: Option<FolderKind>, syncable: bool) -> RemoteFolder {
        RemoteFolder {
            name: name.into(),
            delimiter: Some(".".into()),
            special,
            syncable,
        }
    }

    #[test]
    fn folders_get_kinds_from_flags_then_names() {
        let plan = plan_folders(
            &[
                remote("INBOX", None, true),
                remote("INBOX.Sent", None, true),
                remote("INBOX.Sent Items", None, true),
                remote("INBOX.Robocze", None, true),
                remote("INBOX.Klienci", None, true),
                remote("Kopie", Some(FolderKind::Drafts), true),
                remote("[Gmail]", None, false),
            ],
            &FolderHints::default(),
        );
        let got: Vec<_> = plan
            .iter()
            .map(|f| (f.id.as_str(), f.name.as_str(), f.kind))
            .collect();
        assert_eq!(
            got,
            [
                ("inbox", "Inbox", FolderKind::Inbox),
                ("kopie", "Kopie", FolderKind::Drafts),
                ("inbox-sent", "Sent", FolderKind::Sent),
                ("inbox-klienci", "Klienci", FolderKind::Custom),
                ("inbox-robocze", "Robocze", FolderKind::Custom),
                ("inbox-sent-items", "Sent Items", FolderKind::Custom),
            ]
        );
        assert_eq!(plan[2].remote.as_deref(), Some("INBOX.Sent"));
    }

    /// The kinds a plan gave, by server name, as the next plan's hints see them.
    fn kinds_of(plan: &[Folder]) -> HashMap<String, FolderKind> {
        plan.iter()
            .map(|f| (f.remote.clone().unwrap(), f.kind))
            .collect()
    }

    fn kind_in(plan: &[Folder], id: &str) -> FolderKind {
        plan.iter().find(|f| f.id == id).unwrap().kind
    }

    #[test]
    fn sent_is_the_folder_that_holds_sent_mail_and_stays_there() {
        // A server's own empty "Sent" (flagged), next to the "Wysłane" another program filled.
        let list = [
            remote("INBOX", None, true),
            remote("Sent", Some(FolderKind::Sent), true),
            remote("Wys&AUI-ane", None, true),
        ];
        // Nothing downloaded yet: the server's flag decides.
        let first = plan_folders(&list, &FolderHints::default());
        assert_eq!(kind_in(&first, "sent"), FolderKind::Sent);
        assert_eq!(kind_in(&first, "wysłane"), FolderKind::Custom);

        // Only "Wysłane" holds mail: it is Sent, and the empty one the user's own.
        let mut hints = FolderHints {
            was: kinds_of(&first),
            held: HashSet::from(["Wys&AUI-ane".to_owned()]),
            ..FolderHints::default()
        };
        let second = plan_folders(&list, &hints);
        assert_eq!(kind_in(&second, "wysłane"), FolderKind::Sent);
        assert_eq!(kind_in(&second, "sent"), FolderKind::Custom);
        let names: Vec<_> = second.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            ["Inbox", "Wysłane", "Sent"],
            "server names, Sent first"
        );

        // Later a phone saves into the server's own as well: Sent doesn't hop back.
        hints.was = kinds_of(&second);
        hints.held.insert("Sent".into());
        assert_eq!(plan_folders(&list, &hints), second);

        // The user's choice wins, and is not taken for another kind.
        hints.chosen = HashMap::from([("Sent".to_owned(), FolderKind::Sent)]);
        let chosen = plan_folders(&list, &hints);
        assert_eq!(kind_in(&chosen, "sent"), FolderKind::Sent);
        assert_eq!(kind_in(&chosen, "wysłane"), FolderKind::Custom);
        hints.chosen = HashMap::from([("Sent".to_owned(), FolderKind::Archive)]);
        let archive = plan_folders(&list, &hints);
        assert_eq!(kind_in(&archive, "sent"), FolderKind::Archive);
        assert_eq!(kind_in(&archive, "wysłane"), FolderKind::Sent);
    }

    // ---- A small scripted IMAP server: enough of RFC 3501, MOVE and UIDPLUS for sync ----

    #[derive(Clone)]
    struct FakeMail {
        uid: u32,
        flags: Vec<String>,
        raw: Vec<u8>,
    }

    impl FakeMail {
        fn has(&self, flag: &str) -> bool {
            self.flags.iter().any(|f| f == flag)
        }
        fn subject(&self) -> String {
            let raw = String::from_utf8_lossy(&self.raw);
            raw.lines()
                .find_map(|l| l.strip_prefix("Subject: "))
                .unwrap_or_default()
                .to_owned()
        }
    }

    struct FakeBox {
        name: &'static str,
        attrs: &'static str,
        uid_validity: u32,
        next_uid: u32,
        mail: Vec<FakeMail>,
    }

    struct FakeServer {
        capabilities: &'static str,
        boxes: Vec<FakeBox>,
        /// Every command the client sent, without its tag.
        commands: Vec<String>,
        /// Wakes a client in IDLE: the server tells it how many messages the Inbox has.
        idle: Arc<tokio::sync::Notify>,
        /// What SELECT says in PERMANENTFLAGS; `None` says nothing (every flag is kept).
        permanent: Option<&'static str>,
    }

    impl FakeServer {
        fn find(&self, name: &str) -> Option<&FakeBox> {
            self.boxes.iter().find(|b| b.name == name)
        }
        fn subjects(&self, name: &str) -> Vec<String> {
            let mut s: Vec<_> = self
                .find(name)
                .unwrap()
                .mail
                .iter()
                .map(FakeMail::subject)
                .collect();
            s.sort();
            s
        }
        fn mail(&self, name: &str, subject: &str) -> FakeMail {
            self.find(name)
                .unwrap()
                .mail
                .iter()
                .find(|m| m.subject() == subject)
                .unwrap()
                .clone()
        }
    }

    type Shared = Arc<Mutex<FakeServer>>;

    fn mail(uid: u32, flags: &[&str], subject: &str, body_cp1250: &[u8]) -> FakeMail {
        let mut raw = format!(
            "From: Anna Kowalska <anna@kowalska-architekci.pl>\r\n\
             To: biuro@twojastudio.pl\r\n\
             Subject: {subject}\r\n\
             Message-ID: <m{uid}@kowalska-architekci.pl>\r\n\
             Date: Fri, 09 Oct 2026 14:30:00 +0200\r\n\
             Content-Type: text/plain; charset=windows-1250\r\n\
             Content-Transfer-Encoding: 8bit\r\n\r\n"
        )
        .into_bytes();
        raw.extend_from_slice(body_cp1250);
        raw.extend_from_slice(b"\r\n");
        FakeMail {
            uid,
            flags: flags.iter().map(|f| f.to_string()).collect(),
            raw,
        }
    }

    fn fake_box(name: &'static str, attrs: &'static str, mail: Vec<FakeMail>) -> FakeBox {
        FakeBox {
            name,
            attrs,
            uid_validity: 7,
            next_uid: mail.iter().map(|m| m.uid).max().unwrap_or(0) + 1,
            mail,
        }
    }

    fn fake_boxes() -> Vec<FakeBox> {
        vec![
            fake_box(
                "INBOX",
                "\\HasNoChildren",
                vec![
                    mail(1, &["\\Seen"], "Wycena", b"Dzie\xf1 dobry"),
                    mail(2, &["\\Flagged"], "Poprawki", b"Prosz\xea o zmiany"),
                ],
            ),
            fake_box(
                "Wys&AUI-ane",
                "\\HasNoChildren \\Sent",
                vec![mail(5, &["\\Seen"], "Re: Wycena", b"Dzi\xeakuj\xea")],
            ),
            fake_box("Projekty", "\\Noselect \\HasChildren", vec![]),
            fake_box("Projekty/Klinika", "\\HasNoChildren", vec![]),
            fake_box("Kosz", "\\HasNoChildren", vec![]),
        ]
    }

    async fn start_fake_server(capabilities: &'static str) -> (SocketAddr, Shared) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server: Shared = Arc::new(Mutex::new(FakeServer {
            capabilities,
            boxes: fake_boxes(),
            commands: Vec::new(),
            idle: Arc::new(tokio::sync::Notify::new()),
            permanent: None,
        }));
        let shared = server.clone();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(serve(socket, shared.clone()));
            }
        });
        (addr, server)
    }

    async fn serve(socket: TcpStream, server: Shared) -> std::io::Result<()> {
        let (read, mut write) = socket.into_split();
        let mut reader = BufReader::new(read);
        write
            .write_all(b"* OK [CAPABILITY IMAP4rev1] Fake IMAP ready\r\n")
            .await?;
        let mut selected = 0;
        loop {
            let mut buf = String::new();
            if reader.read_line(&mut buf).await? == 0 {
                break;
            }
            let line = buf.trim_end_matches(['\r', '\n']).to_owned();
            // A command ending in {n} sends n more bytes (a literal) after our "+".
            let literal = match line.rfind('{').filter(|_| line.ends_with('}')) {
                Some(open) => {
                    let n: usize = line[open + 1..line.len() - 1].parse().unwrap_or(0);
                    write.write_all(b"+ Ready for literal data\r\n").await?;
                    let mut data = vec![0; n];
                    reader.read_exact(&mut data).await?;
                    reader.read_line(&mut String::new()).await?; // the CRLF after it
                    data
                }
                None => Vec::new(),
            };
            let (tag, command) = line.split_once(' ').unwrap_or((&line, ""));
            if command.eq_ignore_ascii_case("IDLE") {
                // Wait for DONE; meanwhile say so whenever the test pokes the Inbox.
                let wake = {
                    let mut s = server.lock().unwrap();
                    s.commands.push("IDLE".into());
                    s.idle.clone()
                };
                write.write_all(b"+ idling\r\n").await?;
                loop {
                    let mut done = String::new();
                    tokio::select! {
                        read = reader.read_line(&mut done) => {
                            if read? == 0 {
                                return Ok(());
                            }
                            server.lock().unwrap().commands.push(done.trim().to_owned());
                            write.write_all(format!("{tag} OK IDLE terminated\r\n").as_bytes()).await?;
                            break;
                        }
                        () = wake.notified() => {
                            let n = server.lock().unwrap().boxes[0].mail.len();
                            write.write_all(format!("* {n} EXISTS\r\n").as_bytes()).await?;
                        }
                    }
                }
                continue;
            }
            let reply = {
                let mut s = server.lock().unwrap();
                s.commands.push(command.to_owned());
                respond(&mut s, tag, command, literal, &mut selected)
            };
            write.write_all(&reply).await?;
            if command.eq_ignore_ascii_case("LOGOUT") {
                break;
            }
        }
        Ok(())
    }

    fn unquote(s: &str) -> String {
        s.trim()
            .trim_matches('"')
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    }

    /// A reply in bytes: message bodies may be Windows-1250, which isn't valid UTF-8.
    #[derive(Default)]
    struct Reply(Vec<u8>);

    impl Reply {
        fn line(&mut self, text: String) {
            self.0.extend(text.into_bytes());
        }
        fn raw(&mut self, bytes: &[u8]) {
            self.0.extend_from_slice(bytes);
        }
    }

    fn respond(
        s: &mut FakeServer,
        tag: &str,
        command: &str,
        literal: Vec<u8>,
        selected: &mut usize,
    ) -> Vec<u8> {
        let upper = command.to_ascii_uppercase();
        let uidplus = s.capabilities.contains("UIDPLUS");
        let mut out = Reply::default();
        if upper.starts_with("LOGIN ") {
            if command.ends_with("\"secret\"") {
                out.line(format!("{tag} OK LOGIN completed\r\n"));
            } else {
                out.line(format!(
                    "{tag} NO [AUTHENTICATIONFAILED] Invalid credentials\r\n"
                ));
            }
        } else if upper == "CAPABILITY" {
            out.line(format!(
                "* CAPABILITY {}\r\n{tag} OK CAPABILITY completed\r\n",
                s.capabilities
            ));
        } else if upper.starts_with("GETQUOTAROOT ") && s.capabilities.contains("QUOTA") {
            out.line(format!(
                "* QUOTAROOT INBOX \"\"\r\n* QUOTA \"\" (STORAGE 512 10240 MESSAGE 7 1000)\r\n\
                 {tag} OK GETQUOTAROOT completed\r\n"
            ));
        } else if upper.starts_with("LIST ") {
            for b in &s.boxes {
                out.line(format!("* LIST ({}) \"/\" \"{}\"\r\n", b.attrs, b.name));
            }
            out.line(format!("{tag} OK LIST completed\r\n"));
        } else if upper.starts_with("EXAMINE ") || upper.starts_with("SELECT ") {
            let (verb, name) = command.split_once(' ').unwrap();
            let name = unquote(name);
            match s.boxes.iter().position(|b| b.name == name) {
                Some(i) => {
                    *selected = i;
                    let b = &s.boxes[i];
                    let mode = if verb.eq_ignore_ascii_case("SELECT") {
                        "READ-WRITE"
                    } else {
                        "READ-ONLY"
                    };
                    if let (Some(flags), "READ-WRITE") = (s.permanent, mode) {
                        out.line(format!("* OK [PERMANENTFLAGS ({flags})] Limited\r\n"));
                    }
                    out.line(format!(
                        "* {} EXISTS\r\n* 0 RECENT\r\n* OK [UIDVALIDITY {}] UIDs valid\r\n\
                         * OK [UIDNEXT {}] Predicted next UID\r\n\
                         * FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
                         {tag} OK [{mode}] {verb} completed\r\n",
                        b.mail.len(),
                        b.uid_validity,
                        b.next_uid
                    ));
                }
                None => out.line(format!("{tag} NO Mailbox doesn't exist\r\n")),
            }
        } else if let Some(query) = upper.strip_prefix("UID SEARCH ").map(|_| &command[11..]) {
            let mail = &s.boxes[*selected].mail;
            let hits: Vec<u32> = if query.eq_ignore_ascii_case("ALL") {
                mail.iter().map(|m| m.uid).collect()
            } else if let Some(set) = query.strip_prefix("UID ") {
                let wanted = expand_uid_set(set);
                mail.iter()
                    .map(|m| m.uid)
                    .filter(|u| wanted.contains(u))
                    .collect()
            } else if let Some(id) = query.strip_prefix("HEADER Message-ID ") {
                let header = format!("Message-ID: {}", unquote(id));
                mail.iter()
                    .filter(|m| String::from_utf8_lossy(&m.raw).contains(&header))
                    .map(|m| m.uid)
                    .collect()
            } else {
                Vec::new()
            };
            let hits: Vec<String> = hits.iter().map(u32::to_string).collect();
            let sep = if hits.is_empty() { "" } else { " " };
            out.line(format!(
                "* SEARCH{sep}{}\r\n{tag} OK SEARCH completed\r\n",
                hits.join(" ")
            ));
        } else if upper.starts_with("UID FETCH ") {
            let mut parts = command[10..].splitn(2, ' ');
            let wanted = expand_uid_set(parts.next().unwrap_or_default());
            let with_body = parts.next().unwrap_or_default().contains("BODY");
            for (seq, m) in s.boxes[*selected].mail.iter().enumerate() {
                if !wanted.contains(&m.uid) {
                    continue;
                }
                let flags = m.flags.join(" ");
                if with_body {
                    out.line(format!(
                        "* {} FETCH (UID {} FLAGS ({flags}) INTERNALDATE \"09-Oct-2026 14:30:00 +0200\" \
                         RFC822.SIZE {len} BODY[] {{{len}}}\r\n",
                        seq + 1,
                        m.uid,
                        len = m.raw.len()
                    ));
                    out.raw(&m.raw);
                    out.line(")\r\n".into());
                } else {
                    out.line(format!(
                        "* {} FETCH (UID {} FLAGS ({flags}))\r\n",
                        seq + 1,
                        m.uid
                    ));
                }
            }
            out.line(format!("{tag} OK FETCH completed\r\n"));
        } else if upper.starts_with("UID STORE ") {
            // UID STORE <uid set> +FLAGS.SILENT (\Seen)
            let mut parts = command[10..].split(' ');
            let uids = expand_uid_set(parts.next().unwrap());
            let op = parts.next().unwrap();
            let flag = parts
                .next()
                .unwrap()
                .trim_matches(|c| c == '(' || c == ')')
                .to_owned();
            for m in s.boxes[*selected]
                .mail
                .iter_mut()
                .filter(|m| uids.contains(&m.uid))
            {
                m.flags.retain(|f| *f != flag);
                if op.starts_with('+') {
                    m.flags.push(flag.clone());
                }
            }
            out.line(format!("{tag} OK STORE completed\r\n"));
        } else if upper.starts_with("UID MOVE ") || upper.starts_with("UID COPY ") {
            let moving = upper.starts_with("UID MOVE ");
            let (set, dest) = command[9..].split_once(' ').unwrap();
            let wanted = expand_uid_set(set);
            let dest = unquote(dest);
            let Some(to) = s.boxes.iter().position(|b| b.name == dest) else {
                out.line(format!("{tag} NO [TRYCREATE] No such mailbox\r\n"));
                return out.0;
            };
            if dest == "Zablokowane" {
                out.line(format!("{tag} NO [CANNOT] Permission denied\r\n"));
                return out.0;
            }
            let from = *selected;
            let mut picked: Vec<FakeMail> = s.boxes[from]
                .mail
                .iter()
                .filter(|m| wanted.contains(&m.uid))
                .cloned()
                .collect();
            if picked.is_empty() {
                out.line(format!("{tag} OK No messages\r\n"));
                return out.0;
            }
            picked.sort_by_key(|m| m.uid);
            let validity = s.boxes[to].uid_validity;
            let (mut old, mut new) = (Vec::new(), Vec::new());
            for mut copy in picked {
                old.push(copy.uid);
                copy.uid = s.boxes[to].next_uid;
                s.boxes[to].next_uid += 1;
                new.push(copy.uid);
                s.boxes[to].mail.push(copy);
            }
            let copyuid = format!("[COPYUID {validity} {} {}] ", uid_set(&old), uid_set(&new));
            if moving {
                if uidplus {
                    out.line(format!("* OK {copyuid}Moved\r\n"));
                }
                // Expunged from the top down, so the sequence numbers stay right.
                let mut seqs: Vec<usize> = s.boxes[from]
                    .mail
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| old.contains(&m.uid))
                    .map(|(i, _)| i + 1)
                    .collect();
                seqs.reverse();
                s.boxes[from].mail.retain(|m| !old.contains(&m.uid));
                for seq in seqs {
                    out.line(format!("* {seq} EXPUNGE\r\n"));
                }
                out.line(format!("{tag} OK MOVE completed\r\n"));
            } else {
                let code = if uidplus { copyuid } else { String::new() };
                out.line(format!("{tag} OK {code}COPY completed\r\n"));
            }
        } else if upper.starts_with("UID EXPUNGE ") || upper == "EXPUNGE" {
            let only = upper.strip_prefix("UID EXPUNGE ").map(expand_uid_set);
            let mail = &mut s.boxes[*selected].mail;
            let mut seq = 1;
            mail.retain(|m| {
                let gone = m.has("\\Deleted") && only.as_ref().is_none_or(|o| o.contains(&m.uid));
                if gone {
                    out.line(format!("* {seq} EXPUNGE\r\n"));
                } else {
                    seq += 1;
                }
                !gone
            });
            out.line(format!("{tag} OK EXPUNGE completed\r\n"));
        } else if let Some(rest) = upper.strip_prefix("APPEND ").map(|_| &command[7..]) {
            // APPEND "Sent" (\Seen) {123}
            let (name, after) = match rest.strip_prefix('"') {
                Some(quoted) => quoted.split_once('"').unwrap_or((quoted, "")),
                None => rest.split_once(' ').unwrap_or((rest, "")),
            };
            let flags: Vec<String> = after
                .trim()
                .strip_prefix('(')
                .and_then(|f| f.split_once(')'))
                .map(|(f, _)| f.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_default();
            match s.boxes.iter_mut().find(|b| b.name == name) {
                Some(b) => {
                    let uid = b.next_uid;
                    b.next_uid += 1;
                    b.mail.push(FakeMail {
                        uid,
                        flags,
                        raw: literal,
                    });
                    out.line(format!(
                        "{tag} OK [APPENDUID {} {uid}] APPEND completed\r\n",
                        b.uid_validity
                    ));
                }
                None => out.line(format!("{tag} NO [TRYCREATE] No such mailbox\r\n")),
            }
        } else if upper == "LOGOUT" {
            out.line(format!(
                "* BYE Fake IMAP logging out\r\n{tag} OK LOGOUT completed\r\n"
            ));
        } else {
            out.line(format!("{tag} BAD Fake IMAP doesn't know that\r\n"));
        }
        out.0
    }

    fn expand_uid_set(set: &str) -> HashSet<u32> {
        set.split(',')
            .flat_map(|part| match part.split_once(':') {
                Some((a, b)) => a.parse().unwrap()..=b.parse().unwrap(),
                None => part.parse().unwrap()..=part.parse().unwrap(),
            })
            .collect()
    }

    fn settings() -> ImapSettings {
        ImapSettings {
            host: "localhost".into(),
            port: 0,
            username: "biuro@twojastudio.pl".into(),
            security: SmtpSecurity::Tls,
            pop3: None,
        }
    }

    async fn login(addr: SocketAddr, password: &str) -> Result<Session<TcpStream>, SyncError> {
        let tcp = TcpStream::connect(addr).await.unwrap();
        open_session(Client::new(tcp), &settings(), password).await
    }

    fn store_with_account() -> (Mutex<MailStore>, Account) {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "localhost".into(),
                port: 993,
                username: "biuro@twojastudio.pl".into(),
                ..Default::default()
            })
            .unwrap();
        (Mutex::new(store), account)
    }

    fn no_progress(_: SyncProgress) {}

    /// Log in and download everything once. Returns the session, ready for more.
    async fn synced_once(
        capabilities: &'static str,
    ) -> (Shared, Mutex<MailStore>, Account, Session<TcpStream>) {
        let (addr, server) = start_fake_server(capabilities).await;
        let (store, account) = store_with_account();
        let mut session = login(addr, "secret").await.unwrap();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let account = store.lock().unwrap().account(&account.id).unwrap().clone();
        (server, store, account, session)
    }

    /// Id of the stored message with this subject in this folder.
    fn id_of(
        store: &Mutex<MailStore>,
        account: &Account,
        folder: &str,
        subject: &str,
    ) -> MessageId {
        store
            .lock()
            .unwrap()
            .list(&account.folder_key(folder))
            .unwrap()
            .into_iter()
            .find(|m| m.subject == subject)
            .unwrap_or_else(|| panic!("no \"{subject}\" in {folder}"))
            .id
    }

    #[tokio::test]
    async fn selected_folder_is_checked_first_without_reordering_the_tree() {
        let (addr, server) = start_fake_server("IMAP4rev1 MOVE UIDPLUS").await;
        let (store, account) = store_with_account();
        let mut session = login(addr, "secret").await.unwrap();
        sync_session(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let folders = store
            .lock()
            .unwrap()
            .account(&account.id)
            .unwrap()
            .folders
            .clone();
        server.lock().unwrap().commands.clear();
        sync_session(
            &mut session,
            &account,
            &store,
            &no_progress,
            Scope::AllFirst("projekty-klinika".into()),
        )
        .await
        .unwrap();
        let commands = server.lock().unwrap().commands.clone();
        let examined: Vec<_> = commands
            .iter()
            .filter(|c| c.starts_with("EXAMINE"))
            .collect();
        assert!(examined[0].contains("Projekty"), "{examined:?}");
        assert!(examined.iter().any(|c| c.contains("INBOX")));
        assert_eq!(
            store.lock().unwrap().account(&account.id).unwrap().folders,
            folders
        );
        // A folder removed on the server falls back to the ordinary order.
        server.lock().unwrap().commands.clear();
        sync_session(
            &mut session,
            &account,
            &store,
            &no_progress,
            Scope::AllFirst("gone".into()),
        )
        .await
        .unwrap();
        let commands = server.lock().unwrap().commands.clone();
        assert!(commands
            .iter()
            .find(|c| c.starts_with("EXAMINE"))
            .unwrap()
            .contains("INBOX"));
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn downloading_never_changes_the_server() {
        let (addr, server) = start_fake_server("IMAP4rev1 MOVE UIDPLUS").await;
        let (store, account) = store_with_account();
        let events = Mutex::new(Vec::new());
        let progress = |p: SyncProgress| events.lock().unwrap().push(p);
        let mut session = login(addr, "secret").await.unwrap();

        let new = sync_session(&mut session, &account, &store, &progress, Scope::All)
            .await
            .unwrap()
            .new;
        assert_eq!(new.len(), 3);
        assert!(store
            .lock()
            .unwrap()
            .folder_tree()
            .unwrap()
            .counts
            .values()
            .all(|c| c.new_unread == 0));
        {
            let s = store.lock().unwrap();
            let folders: Vec<_> = s
                .account(&account.id)
                .unwrap()
                .folders
                .iter()
                .map(|f| (f.id.clone(), f.name.clone(), f.kind))
                .collect();
            assert_eq!(
                folders,
                [
                    ("inbox".into(), "Inbox".into(), FolderKind::Inbox),
                    ("outbox".into(), "Outbox".into(), FolderKind::Outbox),
                    ("wysłane".into(), "Wysłane".into(), FolderKind::Sent),
                    (
                        "projekty-klinika".into(),
                        "Projekty / Klinika".into(),
                        FolderKind::Custom
                    ),
                    ("kosz".into(), "Kosz".into(), FolderKind::Trash),
                ]
            );
            let inbox = s.list(&account.folder_key("inbox")).unwrap();
            assert_eq!(inbox.len(), 2);
            let wycena = inbox.iter().find(|m| m.subject == "Wycena").unwrap();
            assert!(!wycena.unread, "\\Seen on the server means read");
            let poprawki = inbox.iter().find(|m| m.subject == "Poprawki").unwrap();
            assert!(poprawki.unread && poprawki.flagged);
            assert_eq!(s.get(poprawki.id).unwrap().body.trim(), "Proszę o zmiany");
            assert_eq!(s.list(&account.folder_key("wysłane")).unwrap().len(), 1);
        }
        assert_eq!(
            events.lock().unwrap().last(),
            Some(&SyncProgress {
                account: "Studio".into(),
                folder: "Wysłane".into(),
                key: account.folder_key("wysłane"),
                done: 1,
                total: 1
            })
        );

        // Nothing changed on the server: nothing is downloaded.
        let new = sync_session(&mut session, &account, &store, &progress, Scope::All)
            .await
            .unwrap()
            .new;
        assert!(new.is_empty());

        // New mail arrives and message 1 is deleted elsewhere (say, on the phone).
        {
            let mut s = server.lock().unwrap();
            let inbox = &mut s.boxes[0];
            inbox.mail.retain(|m| m.uid != 1);
            inbox
                .mail
                .push(mail(3, &[], "Nowa wiadomo\u{15b}\u{107}", b"Hej"));
            inbox.next_uid = 4;
        }
        let new = sync_session(&mut session, &account, &store, &progress, Scope::All)
            .await
            .unwrap()
            .new;
        assert_eq!(new.len(), 1);
        assert_eq!(
            store.lock().unwrap().folder_tree().unwrap().counts[&account.folder_key("inbox")]
                .new_unread,
            1
        );
        {
            let s = store.lock().unwrap();
            let mut subjects: Vec<_> = s
                .list(&account.folder_key("inbox"))
                .unwrap()
                .into_iter()
                .map(|m| m.subject)
                .collect();
            subjects.sort();
            assert_eq!(subjects, ["Nowa wiadomość", "Poprawki"]);
        }

        // The server renumbers the Inbox: its mail is downloaded again.
        server.lock().unwrap().boxes[0].uid_validity = 8;
        let new = sync_session(&mut session, &account, &store, &progress, Scope::All)
            .await
            .unwrap()
            .new;
        assert_eq!(new.len(), 2);
        logout(&mut session).await;

        // Downloading only reads: EXAMINE, never SELECT, and every body fetch is a PEEK.
        let commands = server.lock().unwrap().commands.clone();
        assert!(commands.iter().any(|c| c.starts_with("EXAMINE")));
        for c in &commands {
            let words: Vec<_> = c.split(' ').collect();
            let verb = if words[0] == "UID" {
                words[1]
            } else {
                words[0]
            };
            assert!(
                ["LOGIN", "LIST", "EXAMINE", "SEARCH", "FETCH", "LOGOUT"].contains(&verb),
                "unexpected command: {c}"
            );
            if verb == "FETCH" && c.contains("BODY") {
                assert!(c.contains("BODY.PEEK[]"), "{c}");
            }
        }
    }

    #[tokio::test]
    async fn the_storage_quota_comes_with_the_sync() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        let (_, store, ..) = synced_once("IMAP4rev1 QUOTA").await;
        let usage = store.lock().unwrap().usage(today).unwrap();
        assert_eq!(
            usage.accounts[0].quota,
            Some(Quota {
                used_kb: 512,
                limit_kb: 10_240
            })
        );
        let (_, store, ..) = synced_once("IMAP4rev1").await;
        let usage = store.lock().unwrap().usage(today).unwrap();
        assert_eq!(usage.accounts[0].quota, None, "no QUOTA, no question");
    }

    #[tokio::test]
    async fn changes_reach_the_server() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        let wycena = id_of(&store, &account, "inbox", "Wycena");
        {
            let mut s = store.lock().unwrap();
            s.set_unread(poprawki, false).unwrap();
            s.set_flagged(poprawki, false).unwrap();
            s.delete(wycena).unwrap(); // to Trash ("Kosz")
        }
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(
            report.refused.is_empty() && report.new.is_empty(),
            "{report:?}"
        );
        {
            let srv = server.lock().unwrap();
            let p = srv.mail("INBOX", "Poprawki");
            assert!(p.has("\\Seen") && !p.has("\\Flagged"));
            assert_eq!(srv.subjects("INBOX"), ["Poprawki"]);
            assert_eq!(srv.subjects("Kosz"), ["Wycena"]);
        }
        {
            let s = store.lock().unwrap();
            assert!(!s.has_pending_changes(&account.id).unwrap());
            let trash = s.list(&account.folder_key("kosz")).unwrap();
            assert_eq!(trash.len(), 1, "moved once, not downloaded again");
            assert_eq!(trash[0].id, wycena, "same message, same id");
            assert_eq!(
                s.local_uids(&account.folder_key("kosz")).unwrap(),
                HashSet::from([1]),
                "UID from COPYUID"
            );
        }

        // Del in Trash removes it on the server too.
        store.lock().unwrap().delete(wycena).unwrap();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(server.lock().unwrap().subjects("Kosz").is_empty());
        assert!(!store
            .lock()
            .unwrap()
            .has_pending_changes(&account.id)
            .unwrap());
        logout(&mut session).await;

        let commands = server.lock().unwrap().commands.clone();
        assert!(commands
            .iter()
            .any(|c| c.starts_with("UID MOVE 1 \"Kosz\"")));
        assert!(commands.iter().any(|c| c.starts_with("UID EXPUNGE")));
        assert!(
            !commands.iter().any(|c| c == "EXPUNGE"),
            "never a blanket EXPUNGE with UIDPLUS"
        );
    }

    #[tokio::test]
    async fn mail_moved_to_this_pc_leaves_the_server_and_comes_back_with_its_flags() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        let wycena = id_of(&store, &account, "inbox", "Wycena");
        {
            let mut s = store.lock().unwrap();
            s.move_many(&[poprawki, wycena], "local/archive").unwrap();
            // One of them had its UID forgotten, as when it moved on the server just before:
            // it is found by its Message-ID.
            s.db.execute(
                "UPDATE pending_ops SET uid = NULL WHERE kind = 'expunge' AND origin = ?1",
                [wycena as i64],
            )
            .unwrap();
        }
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(
            report.refused.is_empty() && report.new.is_empty(),
            "{report:?}"
        );
        assert!(server.lock().unwrap().subjects("INBOX").is_empty());
        {
            let s = store.lock().unwrap();
            assert!(!s.has_pending_changes(&account.id).unwrap());
            let kept = s.list("local/archive").unwrap();
            assert_eq!(kept.len(), 2, "both stay on this PC, not downloaded again");
            assert!(s
                .local_uids(&account.folder_key("inbox"))
                .unwrap()
                .is_empty());
        }

        // Back to the Inbox, flagged: uploaded as it is now, then adopted, not stored twice.
        {
            let mut s = store.lock().unwrap();
            s.set_flagged(poprawki, true).unwrap();
            s.move_to(poprawki, &account.folder_key("inbox")).unwrap();
        }
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(
            report.refused.is_empty() && report.new.is_empty(),
            "{report:?}"
        );
        {
            let srv = server.lock().unwrap();
            assert_eq!(srv.subjects("INBOX"), ["Poprawki"]);
            let p = srv.mail("INBOX", "Poprawki");
            assert!(p.has("\\Flagged") && !p.has("\\Seen"), "{:?}", p.flags);
        }
        {
            let s = store.lock().unwrap();
            let inbox = s.list(&account.folder_key("inbox")).unwrap();
            assert_eq!(inbox.len(), 1);
            assert_eq!(inbox[0].id, poprawki, "same message, same id");
            assert_eq!(s.local_uids(&account.folder_key("inbox")).unwrap().len(), 1);
        }
        logout(&mut session).await;
        let commands = server.lock().unwrap().commands.clone();
        assert!(commands
            .iter()
            .any(|c| c == "UID SEARCH HEADER Message-ID \"<m1@kowalska-architekci.pl>\""));
    }

    #[tokio::test]
    async fn without_move_telex_copies_expunges_and_finds_the_new_uid() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1").await;
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        store
            .lock()
            .unwrap()
            .move_to(poprawki, &account.folder_key("kosz"))
            .unwrap();
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(
            report.refused.is_empty() && report.new.is_empty(),
            "{report:?}"
        );
        {
            let srv = server.lock().unwrap();
            assert_eq!(srv.subjects("INBOX"), ["Wycena"]);
            assert_eq!(srv.subjects("Kosz"), ["Poprawki"]);
        }
        {
            let s = store.lock().unwrap();
            let trash = s.list(&account.folder_key("kosz")).unwrap();
            assert_eq!(trash.len(), 1, "no duplicate after the download");
            assert_eq!(trash[0].id, poprawki);
            assert_eq!(
                s.local_uids(&account.folder_key("kosz")).unwrap(),
                HashSet::from([1])
            );
        }
        logout(&mut session).await;
        let commands = server.lock().unwrap().commands.clone();
        for expected in [
            "UID COPY 2 \"Kosz\"",
            "UID STORE 2 +FLAGS.SILENT (\\Deleted)",
            "EXPUNGE",
        ] {
            assert!(commands.iter().any(|c| c == expected), "missing {expected}");
        }
        assert!(commands
            .iter()
            .any(|c| c == "UID SEARCH HEADER Message-ID \"<m2@kowalska-architekci.pl>\""));
    }

    #[tokio::test]
    async fn flags_changed_elsewhere_come_down() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        assert!(store.lock().unwrap().get(poprawki).unwrap().unread);
        // Read on the phone, and unflagged there.
        {
            let mut srv = server.lock().unwrap();
            let m = srv.boxes[0].mail.iter_mut().find(|m| m.uid == 2).unwrap();
            m.flags = vec!["\\Seen".into()];
        }
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let m = store.lock().unwrap().get(poprawki).unwrap();
        assert!(!m.unread && !m.flagged);
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn a_refused_move_is_retried_then_undone() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        server
            .lock()
            .unwrap()
            .boxes
            .push(fake_box("Zablokowane", "\\HasNoChildren", vec![]));
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let account = store.lock().unwrap().account(&account.id).unwrap().clone();
        let wycena = id_of(&store, &account, "inbox", "Wycena");
        store
            .lock()
            .unwrap()
            .move_to(wycena, &account.folder_key("zablokowane"))
            .unwrap();

        for attempt in 1..=MAX_ATTEMPTS {
            let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
                .await
                .unwrap();
            let s = store.lock().unwrap();
            if attempt < MAX_ATTEMPTS {
                assert!(report.refused.is_empty(), "attempt {attempt}");
                assert_eq!(
                    s.get(wycena).unwrap().folder,
                    account.folder_key("zablokowane")
                );
            } else {
                assert_eq!(report.refused.len(), 1);
                assert!(
                    report.refused[0].contains("wouldn't move \"Wycena\""),
                    "{:?}",
                    report.refused
                );
                assert!(report.refused[0].contains("Permission denied"));
                assert_eq!(
                    s.get(wycena).unwrap().folder,
                    account.folder_key("inbox"),
                    "put back"
                );
                assert!(!s.has_pending_changes(&account.id).unwrap());
            }
        }
        assert_eq!(
            server.lock().unwrap().subjects("INBOX"),
            ["Poprawki", "Wycena"]
        );
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn a_message_deleted_elsewhere_drops_its_queued_changes() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let wycena = id_of(&store, &account, "inbox", "Wycena");
        store
            .lock()
            .unwrap()
            .move_to(wycena, &account.folder_key("kosz"))
            .unwrap();
        server.lock().unwrap().boxes[0].mail.retain(|m| m.uid != 1);
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(report.refused.is_empty());
        {
            let s = store.lock().unwrap();
            assert!(s.get(wycena).is_err());
            assert!(!s.has_pending_changes(&account.id).unwrap());
        }
        assert!(server.lock().unwrap().subjects("Kosz").is_empty());
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn a_change_made_during_the_download_goes_out_in_the_same_sync() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        {
            let mut srv = server.lock().unwrap();
            srv.boxes[0].mail.push(mail(3, &[], "Nowa", b"Hej"));
            srv.boxes[0].next_uid = 4;
        }
        // The user reads "Poprawki" while the new message downloads.
        let progress = |_: SyncProgress| store.lock().unwrap().set_unread(poprawki, false).unwrap();
        let report = sync_all(&mut session, &account, &store, &progress, Scope::All)
            .await
            .unwrap();
        assert_eq!(report.new.len(), 1);
        assert!(server
            .lock()
            .unwrap()
            .mail("INBOX", "Poprawki")
            .has("\\Seen"));
        assert!(!store
            .lock()
            .unwrap()
            .has_pending_changes(&account.id)
            .unwrap());
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn sent_mail_is_saved_in_sent_once() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        // What smtp::send_outbox leaves behind: the message filed in Sent, a copy queued.
        let raw = b"Message-ID: <wyslana.1@twojastudio.pl>\r\nFrom: biuro@twojastudio.pl\r\n\
To: anna@kowalska-architekci.pl\r\nSubject: Oferta\r\n\r\nW za\xc5\x82\xc4\x85czeniu.\r\n";
        let id = {
            let mut s = store.lock().unwrap();
            // A server that files no copy itself (finding that out has its own test).
            s.set_sent_copy(&account.id, crate::model::SentCopy::Telex)
                .unwrap();
            let message = parse_message(raw, "", None);
            let id = s
                .queue_outgoing(
                    &account.id,
                    &message,
                    raw,
                    "biuro@twojastudio.pl",
                    &["anna@kowalska-architekci.pl".into()],
                    None,
                    None,
                )
                .unwrap();
            s.outgoing_sent(id, chrono::Local::now().naive_local())
                .unwrap();
            id
        };
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(
            report.refused.is_empty() && report.new.is_empty(),
            "{report:?}"
        );

        let copy = server.lock().unwrap().mail("Wys&AUI-ane", "Oferta");
        assert_eq!(copy.raw, raw, "the exact bytes that were sent");
        assert!(copy.has("\\Seen"));
        {
            let s = store.lock().unwrap();
            let sent = s.list(&account.folder_key("wysłane")).unwrap();
            assert_eq!(sent.len(), 2, "the old one and this one, no duplicate");
            assert!(sent.iter().any(|m| m.id == id));
            assert_eq!(
                s.local_uids(&account.folder_key("wysłane")).unwrap().len(),
                2,
                "the download gave it its UID"
            );
            assert!(!s.has_pending_changes(&account.id).unwrap());
        }
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn rules_sort_arriving_mail_but_not_the_first_download() {
        use crate::filters::{Action, Condition, Field, Op, Rule};
        let (addr, server) = start_fake_server("IMAP4rev1 MOVE UIDPLUS").await;
        let (store, account) = store_with_account();
        let flag_anna = Rule {
            id: 0,
            name: "Anna".into(),
            enabled: true,
            account: None,
            match_all: true,
            conditions: vec![Condition {
                header: None,
                field: Field::From,
                op: Op::Contains,
                value: "kowalska".into(),
            }],
            actions: vec![Action::Flag],
            stop: false,
            ..Rule::default()
        };
        store.lock().unwrap().save_rule(&flag_anna).unwrap();
        let mut session = login(addr, "secret").await.unwrap();

        // The first download is old mail: the rule leaves it alone.
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!(report.sorted, 0);
        let account = store.lock().unwrap().account(&account.id).unwrap().clone();
        let wycena = id_of(&store, &account, "inbox", "Wycena");
        assert!(!store.lock().unwrap().get(wycena).unwrap().flagged);

        // A rule that files invoices, then an invoice arrives.
        let invoices = Rule {
            name: "Faktury".into(),
            account: Some(account.id.clone()),
            conditions: vec![Condition {
                header: None,
                field: Field::Subject,
                op: Op::StartsWith,
                value: "faktura".into(),
            }],
            actions: vec![Action::Move {
                folder: account.folder_key("projekty-klinika"),
            }],
            ..flag_anna
        };
        store.lock().unwrap().save_rule(&invoices).unwrap();
        {
            let mut srv = server.lock().unwrap();
            srv.boxes[0]
                .mail
                .push(mail(3, &[], "Faktura 10/2026", b"Kwota: 1230 z\xb3"));
            srv.boxes[0].next_uid = 4;
        }
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!((report.new.len(), report.sorted), (1, 1), "{report:?}");
        let invoice = id_of(&store, &account, "projekty-klinika", "Faktura 10/2026");
        let m = store.lock().unwrap().get(invoice).unwrap();
        assert!(m.flagged, "the first rule ran too");
        assert_eq!(
            m.note.as_deref(),
            Some("Rule \"Anna\" flagged it. Rule \"Faktury\" moved it to Projekty / Klinika.")
        );
        // The same sync moved it on the server.
        {
            let srv = server.lock().unwrap();
            assert_eq!(srv.subjects("Projekty/Klinika"), ["Faktura 10/2026"]);
            assert!(srv
                .mail("Projekty/Klinika", "Faktura 10/2026")
                .has("\\Flagged"));
        }
        assert!(!store
            .lock()
            .unwrap()
            .has_pending_changes(&account.id)
            .unwrap());
        logout(&mut session).await;
    }

    /// A message in UTF-8 from `from`, with words picked from `words` by `seed`.
    fn worded(uid: u32, from: &str, words: &[&str], seed: usize) -> FakeMail {
        let pick = |n: usize| words[(seed * 7 + n * 13) % words.len()];
        let subject = (0..3).map(pick).collect::<Vec<_>>().join(" ");
        let body = (3..40).map(pick).collect::<Vec<_>>().join(" ");
        FakeMail {
            uid,
            flags: vec!["\\Seen".into()],
            raw: format!(
                "From: {from}
To: biuro@twojastudio.pl
Subject: {subject}
                 Message-ID: <w{uid}.{seed}@example.net>
                 Date: Fri, 09 Oct 2026 14:30:00 +0200
                 Content-Type: text/plain; charset=utf-8

{body}
"
            )
            .into_bytes(),
        }
    }

    #[tokio::test]
    async fn the_spam_filter_learns_from_spam_and_sorts_arriving_mail_first() {
        const JUNK: &[&str] = &[
            "lottery",
            "winner",
            "prize",
            "claim",
            "bitcoin",
            "casino",
            "bonus",
            "jackpot",
            "crypto",
            "pills",
            "unclaimed",
            "inheritance",
            "guaranteed",
            "beneficiary",
        ];
        const WORK: &[&str] = &[
            "faktura",
            "projekt",
            "spotkanie",
            "wycena",
            "umowa",
            "termin",
            "poprawki",
            "makieta",
            "klient",
            "harmonogram",
            "strona",
            "przegląd",
            "prezentacja",
        ];
        let (addr, server) = start_fake_server("IMAP4rev1 MOVE UIDPLUS").await;
        {
            let mut srv = server.lock().unwrap();
            let junk = (1..=60)
                .map(|n| worded(n, &format!("promo{n}@deals{n}.example"), JUNK, n as usize))
                .collect();
            srv.boxes
                .push(fake_box("Spam", "\\HasNoChildren \\Junk", junk));
            let inbox = &mut srv.boxes[0];
            for n in 10..70 {
                inbox
                    .mail
                    .push(worded(n, "anna@kowalska-architekci.pl", WORK, n as usize));
            }
            inbox.next_uid = 70;
        }
        let (store, account) = store_with_account();
        let mut session = login(addr, "secret").await.unwrap();
        // The first download learns from what the server already sorted.
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!(report.spam, 0);
        let status = store.lock().unwrap().spam_status().unwrap();
        assert_eq!(
            (status.spam, status.ham),
            (60, 62),
            "and Wycena, read, and Sent"
        );

        // Spam and a message about work arrive: the spam goes to Spam, the rest stays.
        {
            let mut srv = server.lock().unwrap();
            let inbox = &mut srv.boxes[0];
            inbox
                .mail
                .push(worded(70, "offers@prizes-now.example", JUNK, 99));
            inbox
                .mail
                .push(worded(71, "piotr@nowak-meble.pl", WORK, 98));
            inbox.next_uid = 72;
        }
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!((report.new.len(), report.spam), (2, 1), "{report:?}");
        assert_eq!(report.arrived.len(), 1, "no notification for spam");
        let account = store.lock().unwrap().account(&account.id).unwrap().clone();
        let caught = store
            .lock()
            .unwrap()
            .list(&account.folder_key("spam"))
            .unwrap()
            .into_iter()
            .find(|m| m.from_email == "offers@prizes-now.example")
            .expect("in Spam");
        let note = store.lock().unwrap().get(caught.id).unwrap().note.unwrap();
        assert!(note.contains("spam filter"), "{note}");
        // The same sync moved it on the server.
        assert!(server
            .lock()
            .unwrap()
            .find("Spam")
            .unwrap()
            .mail
            .iter()
            .any(|m| String::from_utf8_lossy(&m.raw).contains("offers@prizes-now.example")));
        logout(&mut session).await;
    }

    /// More mail in the fake Inbox, "Nr <uid>", unread.
    fn more_inbox_mail(server: &Shared, uids: std::ops::RangeInclusive<u32>) {
        let mut srv = server.lock().unwrap();
        for uid in uids {
            srv.boxes[0]
                .mail
                .push(mail(uid, &[], &format!("Nr {uid}"), b"Hej"));
        }
        srv.boxes[0].next_uid = srv.boxes[0].mail.iter().map(|m| m.uid).max().unwrap() + 1;
    }

    fn commands_since(server: &Shared, from: usize) -> Vec<String> {
        server.lock().unwrap().commands[from..].to_vec()
    }

    fn tag_on(store: &Mutex<MailStore>, ids: &[MessageId], keyword: &str, on: bool) {
        store
            .lock()
            .unwrap()
            .set_tag_many(ids, keyword, on)
            .unwrap();
    }

    fn tags_of(store: &Mutex<MailStore>, id: MessageId) -> Vec<String> {
        store.lock().unwrap().get(id).unwrap().tags
    }

    #[tokio::test]
    async fn tags_go_up_as_keywords_one_command_each_and_come_down() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let wycena = id_of(&store, &account, "inbox", "Wycena");
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        tag_on(&store, &[wycena, poprawki], "$label2", true);
        tag_on(&store, &[wycena], "$label4", true);
        let from = server.lock().unwrap().commands.len();
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(report.refused.is_empty(), "{report:?}");
        let commands = commands_since(&server, from);
        let stores: Vec<&String> = commands
            .iter()
            .filter(|c| c.starts_with("UID STORE"))
            .collect();
        assert_eq!(
            stores,
            [
                "UID STORE 1:2 +FLAGS.SILENT ($label2)",
                "UID STORE 1 +FLAGS.SILENT ($label4)"
            ],
            "{commands:?}"
        );
        {
            let srv = server.lock().unwrap();
            let w = srv.mail("INBOX", "Wycena");
            assert!(
                w.has("$label2") && w.has("$label4") && w.has("\\Seen"),
                "{:?}",
                w.flags
            );
        }
        assert!(!store
            .lock()
            .unwrap()
            .has_pending_changes(&account.id)
            .unwrap());
        // Another program takes one off, adds one and marks spam its own way.
        {
            let mut srv = server.lock().unwrap();
            let inbox = &mut srv.boxes[0].mail;
            inbox[1].flags.retain(|f| f != "$label2");
            inbox[0].flags.push("$Label5".into());
            inbox[0].flags.push("NonJunk".into());
        }
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!(tags_of(&store, wycena), ["$label2", "$label4", "$label5"]);
        assert!(tags_of(&store, poprawki).is_empty());
        // Taken off here: -FLAGS.
        tag_on(&store, &[wycena], "$label4", false);
        let from = server.lock().unwrap().commands.len();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(commands_since(&server, from)
            .iter()
            .any(|c| c == "UID STORE 1 -FLAGS.SILENT ($label4)"));
        assert!(!server
            .lock()
            .unwrap()
            .mail("INBOX", "Wycena")
            .has("$label4"));
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn a_server_that_cant_keep_tags_leaves_them_on_this_pc_and_says_so_once() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        server.lock().unwrap().permanent = Some("\\Answered \\Flagged \\Deleted \\Seen \\Draft");
        let wycena = id_of(&store, &account, "inbox", "Wycena");
        tag_on(&store, &[wycena], "$label1", true);
        store.lock().unwrap().set_flagged(wycena, true).unwrap();
        let from = server.lock().unwrap().commands.len();
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!(report.refused.len(), 1, "{report:?}");
        assert!(report.refused[0].contains("tags"), "{report:?}");
        let commands = commands_since(&server, from);
        assert!(
            !commands.iter().any(|c| c.contains("$label1")),
            "{commands:?}"
        );
        assert!(
            commands
                .iter()
                .any(|c| c.contains("+FLAGS.SILENT (\\Flagged)")),
            "other changes still go: {commands:?}"
        );
        assert_eq!(
            tags_of(&store, wycena),
            ["$label1"],
            "kept after the download"
        );
        assert!(store.lock().unwrap().tags_kept_here(&account.id).unwrap());
        // Said once: the next tag stays here without a word.
        tag_on(&store, &[wycena], "$label3", true);
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(report.refused.is_empty(), "{report:?}");
        assert_eq!(tags_of(&store, wycena), ["$label1", "$label3"]);
        assert!(!store
            .lock()
            .unwrap()
            .has_pending_changes(&account.id)
            .unwrap());
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn mail_from_this_pc_goes_up_with_its_tags() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        store
            .lock()
            .unwrap()
            .move_many(&[poprawki], "local/archive")
            .unwrap();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        tag_on(&store, &[poprawki], "$label3", true);
        assert_eq!(tags_of(&store, poprawki), ["$label3"], "tags on this PC");
        store
            .lock()
            .unwrap()
            .move_to(poprawki, &account.folder_key("kosz"))
            .unwrap();
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(report.refused.is_empty(), "{report:?}");
        let up = server.lock().unwrap().mail("Kosz", "Poprawki");
        assert!(up.has("$label3") && up.has("\\Flagged"), "{:?}", up.flags);
        assert_eq!(tags_of(&store, poprawki), ["$label3"], "and still here");
        logout(&mut session).await;
    }

    /// A reply (or forward) of `original` that the SMTP server took.
    fn answer(store: &Mutex<MailStore>, account: &Account, original: MessageId, forward: bool) {
        let mut s = store.lock().unwrap();
        let raw = b"From: biuro@twojastudio.pl\r\nTo: anna@kowalska-architekci.pl\r\nSubject: Re\r\n\r\nOK\r\n";
        let message = crate::mime::parse_message(raw, &account.folder_key("outbox"), None);
        let to = ["anna@kowalska-architekci.pl".to_owned()];
        let id = s
            .queue_outgoing(
                &account.id,
                &message,
                raw,
                "biuro@twojastudio.pl",
                &to,
                None,
                None,
            )
            .unwrap();
        let (reply, forwarded) = if forward {
            (None, Some(original))
        } else {
            (Some(original), None)
        };
        s.note_answering(id, reply, forwarded).unwrap();
        s.outgoing_sent(id, Local::now().naive_local()).unwrap();
    }

    fn answered(store: &Mutex<MailStore>, id: MessageId) -> (bool, bool) {
        let m = store.lock().unwrap().summary(id).unwrap();
        (m.answered, m.forwarded)
    }

    #[tokio::test]
    async fn answered_and_forwarded_go_up_come_down_and_travel_with_the_mail() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let wycena = id_of(&store, &account, "inbox", "Wycena");
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        answer(&store, &account, wycena, false);
        answer(&store, &account, poprawki, true);
        let from = server.lock().unwrap().commands.len();
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(report.refused.is_empty(), "{report:?}");
        let commands = commands_since(&server, from);
        assert!(
            commands
                .iter()
                .any(|c| c.ends_with("+FLAGS.SILENT (\\Answered)")),
            "{commands:?}"
        );
        assert!(
            commands
                .iter()
                .any(|c| c.ends_with("+FLAGS.SILENT ($Forwarded)")),
            "{commands:?}"
        );
        {
            let srv = server.lock().unwrap();
            assert!(srv.mail("INBOX", "Wycena").has("\\Answered"));
            assert!(srv.mail("INBOX", "Poprawki").has("$Forwarded"));
        }
        assert_eq!(answered(&store, wycena), (true, false));
        assert_eq!(answered(&store, poprawki), (false, true));

        // The phone: Wycena's answer taken off, Poprawki answered there.
        {
            let mut srv = server.lock().unwrap();
            let inbox = &mut srv.boxes[0].mail;
            for m in inbox.iter_mut() {
                if m.flags.iter().any(|f| f == "\\Answered") {
                    m.flags.retain(|f| f != "\\Answered");
                } else {
                    m.flags.push("\\Answered".into());
                }
            }
        }
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!(answered(&store, wycena), (false, false));
        assert_eq!(answered(&store, poprawki), (true, true));

        // To this PC and to another server folder: uploaded with both.
        store
            .lock()
            .unwrap()
            .move_many(&[poprawki], "local/archive")
            .unwrap();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!(answered(&store, poprawki), (true, true), "kept on this PC");
        store
            .lock()
            .unwrap()
            .move_to(poprawki, &account.folder_key("kosz"))
            .unwrap();
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(report.refused.is_empty(), "{report:?}");
        let up = server.lock().unwrap().mail("Kosz", "Poprawki");
        assert!(
            up.has("\\Answered") && up.has("$Forwarded"),
            "{:?}",
            up.flags
        );
        assert_eq!(answered(&store, poprawki), (true, true));

        // A server that keeps no keywords: $Forwarded stays here, \Answered still goes.
        server.lock().unwrap().permanent = Some("\\Answered \\Flagged \\Deleted \\Seen \\Draft");
        answer(&store, &account, wycena, true);
        answer(&store, &account, wycena, false);
        let from = server.lock().unwrap().commands.len();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let commands = commands_since(&server, from);
        assert!(
            !commands.iter().any(|c| c.contains("$Forwarded")),
            "{commands:?}"
        );
        assert!(server
            .lock()
            .unwrap()
            .mail("INBOX", "Wycena")
            .has("\\Answered"));
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert_eq!(
            answered(&store, wycena),
            (true, true),
            "the download keeps it"
        );
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn many_changes_go_out_as_one_command_each() {
        let (addr, server) = start_fake_server("IMAP4rev1 MOVE UIDPLUS").await;
        more_inbox_mail(&server, 3..=6);
        let (store, account) = store_with_account();
        let mut session = login(addr, "secret").await.unwrap();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let account = store.lock().unwrap().account(&account.id).unwrap().clone();
        let inbox = store
            .lock()
            .unwrap()
            .list(&account.folder_key("inbox"))
            .unwrap();
        assert_eq!(inbox.len(), 6);
        let all: Vec<MessageId> = inbox.iter().map(|m| m.id).collect();
        let numbered: Vec<MessageId> = inbox
            .iter()
            .filter(|m| m.subject.starts_with("Nr "))
            .map(|m| m.id)
            .collect();
        {
            let mut s = store.lock().unwrap();
            assert_eq!(s.set_unread_many(&all, false).unwrap(), 5);
            s.move_many(&numbered, &account.folder_key("kosz")).unwrap();
        }
        let from = server.lock().unwrap().commands.len();
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(report.refused.is_empty(), "{report:?}");
        let commands = commands_since(&server, from);
        let stores: Vec<&String> = commands
            .iter()
            .filter(|c| c.starts_with("UID STORE"))
            .collect();
        assert_eq!(
            stores,
            ["UID STORE 2:6 +FLAGS.SILENT (\\Seen)"],
            "{commands:?}"
        );
        let moves: Vec<&String> = commands
            .iter()
            .filter(|c| c.starts_with("UID MOVE"))
            .collect();
        assert_eq!(moves, ["UID MOVE 3:6 \"Kosz\""]);
        {
            let srv = server.lock().unwrap();
            assert_eq!(srv.subjects("Kosz"), ["Nr 3", "Nr 4", "Nr 5", "Nr 6"]);
            assert!(srv
                .find("Kosz")
                .unwrap()
                .mail
                .iter()
                .all(|m| m.has("\\Seen")));
        }
        {
            let s = store.lock().unwrap();
            assert!(!s.has_pending_changes(&account.id).unwrap());
            let trash = account.folder_key("kosz");
            assert_eq!(
                s.local_uids(&trash).unwrap(),
                HashSet::from([1, 2, 3, 4]),
                "from COPYUID"
            );
            let mut kept: Vec<MessageId> = s.list(&trash).unwrap().iter().map(|m| m.id).collect();
            kept.sort_unstable();
            let mut expected = numbered.clone();
            expected.sort_unstable();
            assert_eq!(kept, expected, "the same messages, not copies");
        }
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn a_refused_batch_is_tried_one_by_one() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        server
            .lock()
            .unwrap()
            .boxes
            .push(fake_box("Zablokowane", "\\HasNoChildren", vec![]));
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let account = store.lock().unwrap().account(&account.id).unwrap().clone();
        let ids: Vec<MessageId> = store
            .lock()
            .unwrap()
            .list(&account.folder_key("inbox"))
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        store
            .lock()
            .unwrap()
            .move_many(&ids, &account.folder_key("zablokowane"))
            .unwrap();
        let mut refused = Vec::new();
        for _ in 0..MAX_ATTEMPTS {
            refused = sync_all(&mut session, &account, &store, &no_progress, Scope::All)
                .await
                .unwrap()
                .refused;
        }
        assert_eq!(refused.len(), 2, "each message on its own: {refused:?}");
        {
            let s = store.lock().unwrap();
            for id in ids {
                assert_eq!(
                    s.get(id).unwrap().folder,
                    account.folder_key("inbox"),
                    "put back"
                );
            }
        }
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn older_mail_comes_down_on_request() {
        let (addr, server) = start_fake_server("IMAP4rev1").await;
        // 304 messages: the first download takes the newest 300.
        more_inbox_mail(&server, 3..=(FIRST_SYNC_LIMIT as u32 + 4));
        let (store, account) = store_with_account();
        let mut session = login(addr, "secret").await.unwrap();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let account = store.lock().unwrap().account(&account.id).unwrap().clone();
        let inbox = account.folder_key("inbox");
        let older = |store: &Mutex<MailStore>| {
            store.lock().unwrap().folder_tree().unwrap().counts[&inbox].older
        };
        assert_eq!(older(&store), 4);

        let events = Mutex::new(Vec::new());
        let progress = |p: SyncProgress| events.lock().unwrap().push(p);
        let new = download_older_in(&mut session, &account, &store, &inbox, 3, &progress)
            .await
            .unwrap();
        assert_eq!(new.len(), 3);
        assert_eq!(older(&store), 1);
        assert_eq!(
            events.lock().unwrap().last().map(|p| (p.done, p.total)),
            Some((3, 3))
        );
        let local = store.lock().unwrap().local_uids(&inbox).unwrap();
        assert!(
            local.contains(&2) && local.contains(&4) && !local.contains(&1),
            "newest first"
        );

        let new = download_older_in(&mut session, &account, &store, &inbox, 10, &no_progress)
            .await
            .unwrap();
        assert_eq!(new.len(), 1);
        assert_eq!(older(&store), 0);
        assert_eq!(
            store.lock().unwrap().list(&inbox).unwrap().len(),
            FIRST_SYNC_LIMIT + 4
        );
        logout(&mut session).await;
    }

    #[test]
    fn older_uids_are_the_newest_missing_ones() {
        let on_server: HashSet<u32> = (1..=10).collect();
        let local: HashSet<u32> = [6, 8, 9, 10].into();
        assert_eq!(older_to_fetch(&on_server, &local, Some(10), 3), [4, 5, 7]);
        assert_eq!(
            older_to_fetch(&on_server, &local, Some(10), 99),
            [1, 2, 3, 4, 5, 7]
        );
        assert_eq!(
            older_to_fetch(&on_server, &local, Some(5), 99),
            [1, 2, 3, 4, 5],
            "not new mail"
        );
        assert!(
            older_to_fetch(&on_server, &local, None, 99).is_empty(),
            "not before a first sync"
        );
    }

    #[tokio::test]
    async fn an_inbox_check_looks_at_the_inbox_only() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        more_inbox_mail(&server, 3..=3);
        {
            let mut srv = server.lock().unwrap();
            srv.boxes[1]
                .mail
                .push(mail(6, &["\\Seen"], "Re: Nr 3", b"OK"));
            srv.boxes[1].next_uid = 7;
        }
        let from = server.lock().unwrap().commands.len();
        let report = sync_all(&mut session, &account, &store, &no_progress, Scope::Inbox)
            .await
            .unwrap();
        assert_eq!(report.new.len(), 1);
        let commands = commands_since(&server, from);
        assert!(
            !commands.iter().any(|c| c.starts_with("LIST")),
            "{commands:?}"
        );
        assert!(!commands.iter().any(|c| c.contains("Wys")), "{commands:?}");
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn idle_says_when_the_inbox_changes() {
        let (addr, server) = start_fake_server("IMAP4rev1 IDLE").await;
        let session = login(addr, "secret").await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let watcher = tokio::spawn(async move {
            let changed = move || {
                let _ = tx.send(());
            };
            watch_session(session, &changed, Duration::from_millis(300)).await
        });
        let idles = |server: &Shared| {
            server
                .lock()
                .unwrap()
                .commands
                .iter()
                .filter(|c| *c == "IDLE")
                .count()
        };
        for _ in 0..100 {
            if idles(&server) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(server
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|c| c == "EXAMINE \"INBOX\""));
        more_inbox_mail(&server, 3..=3);
        let wake = server.lock().unwrap().idle.clone();
        wake.notify_one();
        timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("told within 5 s")
            .expect("still watching");
        // A quiet spell: DONE, then IDLE again.
        tokio::time::sleep(Duration::from_millis(800)).await;
        assert!(idles(&server) >= 2, "{:?}", server.lock().unwrap().commands);
        assert!(server.lock().unwrap().commands.iter().any(|c| c == "DONE"));
        assert!(!watcher.is_finished());
        watcher.abort();
    }

    #[tokio::test]
    async fn without_idle_watching_ends_quietly() {
        let (addr, _server) = start_fake_server("IMAP4rev1").await;
        let session = login(addr, "secret").await.unwrap();
        let outcome = watch_session(session, &|| {}, IDLE_RENEW).await.unwrap();
        assert_eq!(outcome, Watched::NoIdle);
    }

    #[tokio::test]
    async fn a_goodbye_instead_of_a_greeting_is_an_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket
                .write_all(b"* BYE Too many connections from your address\r\n")
                .await
                .unwrap();
        });
        let err = login(addr, "secret").await.unwrap_err();
        assert!(matches!(err, SyncError::Server(_)), "{err}");
        assert!(err.to_string().contains("Too many connections"), "{err}");
    }

    #[test]
    fn a_resting_server_says_when_telex_tries_again() {
        let until = Local::now()
            .date_naive()
            .and_hms_opt(17, 42, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap();
        crate::i18n::with(crate::i18n::Lang::En, || {
            assert_eq!(
                SyncError::Resting(until).to_string(),
                "the server stopped answering; Telex tries again after 17:42"
            );
        });
        crate::i18n::with(crate::i18n::Lang::Pl, || {
            assert!(SyncError::Resting(until).to_string().ends_with("po 17:42"));
        });
    }

    #[tokio::test]
    async fn a_wrong_password_is_a_login_error() {
        let (addr, _server) = start_fake_server("IMAP4rev1").await;
        let err = login(addr, "wrong").await.unwrap_err();
        assert!(matches!(err, SyncError::Login(_)), "{err}");
        assert!(err
            .to_string()
            .contains("rejected the user name or password"));
    }

    /// Needs the internet: `cargo test -p telex-core -- --ignored`. Checks that the TLS
    /// setup (rustls with Mozilla's root certificates) reaches a real server. No login.
    #[tokio::test]
    #[ignore = "needs the internet"]
    async fn tls_reaches_a_real_server() {
        let settings = ImapSettings {
            host: "imap.gmail.com".into(),
            port: 993,
            username: String::new(),
            security: SmtpSecurity::Tls,
            pop3: None,
        };
        // Connecting reads the greeting: it fails if the server says nothing.
        let mut trace = Trace::new(Purpose::Test, "imap", "", "imap.gmail.com", 993);
        connect(&settings, "imap.gmail.com", &mut trace)
            .await
            .unwrap();
    }

    use crate::model::OutgoingDraft;

    // ---- Drafts on the server, and servers that file sent mail themselves ----

    /// A draft started on the phone, in the server's Drafts ("Robocze"): a reply to m1.
    const PHONE_DRAFT: &[u8] = b"From: Studio <biuro@twojastudio.pl>\r\n\
To: anna@kowalska-architekci.pl\r\n\
Subject: Szkic z telefonu\r\n\
Message-ID: <telefon.1@twojastudio.pl>\r\n\
In-Reply-To: <m1@kowalska-architekci.pl>\r\n\
References: <m1@kowalska-architekci.pl>\r\n\
Date: Fri, 09 Oct 2026 14:30:00 +0200\r\n\
Content-Type: text/plain; charset=utf-8\r\n\r\n\
Started on the phone.\r\n";

    /// [`synced_once`] on a server that also has Drafts, holding the phone's draft.
    async fn synced_with_drafts() -> (Shared, Mutex<MailStore>, Account, Session<TcpStream>) {
        let (addr, server) = start_fake_server("IMAP4rev1 MOVE UIDPLUS").await;
        let phone = FakeMail {
            uid: 1,
            flags: vec!["\\Draft".into(), "\\Seen".into()],
            raw: PHONE_DRAFT.to_vec(),
        };
        server.lock().unwrap().boxes.push(fake_box(
            "Robocze",
            "\\HasNoChildren \\Drafts",
            vec![phone],
        ));
        let (store, account) = store_with_account();
        let mut session = login(addr, "secret").await.unwrap();
        sync_all(&mut session, &account, &store, &no_progress, Scope::All)
            .await
            .unwrap();
        let account = store.lock().unwrap().account(&account.id).unwrap().clone();
        (server, store, account, session)
    }

    /// Save a draft as the window does: built whole, keeping what it was edited from.
    fn save_draft(store: &Mutex<MailStore>, account: &Account, d: OutgoingDraft) -> MessageId {
        let mut s = store.lock().unwrap();
        let edited = d
            .replaces
            .or(d.autosaved)
            .and_then(|id| s.raw_of(id).unwrap());
        let originals = crate::smtp::Originals {
            edited,
            ..Default::default()
        };
        let raw = crate::smtp::compose_draft(account, &d, &originals, Local::now()).unwrap();
        s.save_outgoing(
            d,
            Some(&raw),
            FolderKind::Drafts,
            Local::now().naive_local(),
        )
        .unwrap()
    }

    fn written(account: &Account, subject: &str, body: &str) -> OutgoingDraft {
        OutgoingDraft {
            account: account.id.clone(),
            to: "anna@kowalska-architekci.pl".into(),
            subject: subject.into(),
            body: body.into(),
            ..OutgoingDraft::default()
        }
    }

    async fn sync(session: &mut Session<TcpStream>, account: &Account, store: &Mutex<MailStore>) {
        let report = sync_all(session, account, store, &no_progress, Scope::All)
            .await
            .unwrap();
        assert!(report.refused.is_empty(), "{report:?}");
    }

    fn drafts_on(server: &Shared, subject: &str) -> Vec<FakeMail> {
        let s = server.lock().unwrap();
        s.find("Robocze")
            .unwrap()
            .mail
            .iter()
            .filter(|m| m.subject() == subject)
            .cloned()
            .collect()
    }

    fn text(mail: &FakeMail) -> String {
        String::from_utf8_lossy(&mail.raw).into_owned()
    }

    #[tokio::test]
    async fn drafts_live_on_the_servers_drafts_too() {
        let (server, store, account, mut session) = synced_with_drafts().await;
        let drafts = account.folder_key("robocze");
        assert_eq!(
            account.folder_of_kind(FolderKind::Drafts).unwrap().id,
            "robocze"
        );
        let phone = id_of(&store, &account, "robocze", "Szkic z telefonu");

        // Saved here: it goes up as a draft, and the download doesn't bring it back twice.
        let first = save_draft(&store, &account, written(&account, "Oferta", "First."));
        sync(&mut session, &account, &store).await;
        let up = drafts_on(&server, "Oferta");
        assert_eq!(up.len(), 1);
        assert!(
            up[0].has("\\Draft") && up[0].has("\\Seen"),
            "{:?}",
            up[0].flags
        );
        let mid = store
            .lock()
            .unwrap()
            .get(first)
            .unwrap()
            .message_id
            .unwrap();
        assert!(text(&up[0]).contains(&format!("Message-ID: <{mid}>")));
        {
            let s = store.lock().unwrap();
            assert_eq!(s.list(&drafts).unwrap().len(), 2);
            assert_eq!(s.local_uids(&drafts).unwrap().len(), 2, "its UID is known");
        }

        // An autosave stays here; saving again replaces the server's copy, same Message-ID.
        let autosaved = save_draft(
            &store,
            &account,
            OutgoingDraft {
                replaces: Some(first),
                autosave: true,
                ..written(&account, "Oferta", "Second, typing.")
            },
        );
        sync(&mut session, &account, &store).await;
        assert!(text(&drafts_on(&server, "Oferta")[0]).contains("First."));
        let second = save_draft(
            &store,
            &account,
            OutgoingDraft {
                replaces: Some(first),
                autosaved: Some(autosaved),
                ..written(&account, "Oferta", "Second.")
            },
        );
        sync(&mut session, &account, &store).await;
        let up = drafts_on(&server, "Oferta");
        assert_eq!(up.len(), 1, "the old version left the server");
        assert!(text(&up[0]).contains("Second.") && text(&up[0]).contains(&mid));
        assert_eq!(store.lock().unwrap().list(&drafts).unwrap().len(), 2);

        // The phone's draft, finished here: replaced on the server, still in its thread.
        let finished = save_draft(
            &store,
            &account,
            OutgoingDraft {
                replaces: Some(phone),
                forward_of: Some(phone),
                ..written(&account, "Szkic z telefonu", "Finished in Telex.")
            },
        );
        sync(&mut session, &account, &store).await;
        let up = drafts_on(&server, "Szkic z telefonu");
        assert_eq!(up.len(), 1);
        assert_ne!(up[0].uid, 1, "the phone's copy is gone");
        let raw = text(&up[0]);
        assert!(raw.contains("Finished in Telex."), "{raw}");
        assert!(
            raw.contains("Message-ID: <telefon.1@twojastudio.pl>"),
            "{raw}"
        );
        assert!(
            raw.contains("In-Reply-To: <m1@kowalska-architekci.pl>"),
            "{raw}"
        );

        // Sent from there: the draft leaves the server's Drafts.
        {
            let mut s = store.lock().unwrap();
            let d = OutgoingDraft {
                replaces: Some(finished),
                ..written(&account, "Szkic z telefonu", "Finished in Telex.")
            };
            let originals = crate::smtp::Originals {
                edited: s.raw_of(finished).unwrap(),
                ..Default::default()
            };
            let composed = crate::smtp::compose(&account, &d, &originals, Local::now()).unwrap();
            assert!(
                String::from_utf8_lossy(&composed.raw)
                    .contains("In-Reply-To: <m1@kowalska-architekci.pl>"),
                "the sent message stays in the thread"
            );
            s.queue_outgoing(
                &account.id,
                &composed.message,
                &composed.raw,
                &composed.env_from,
                &composed.env_to,
                Some(finished),
                None,
            )
            .unwrap();
        }
        sync(&mut session, &account, &store).await;
        assert!(drafts_on(&server, "Szkic z telefonu").is_empty());

        // Discarded: gone from the server too.
        assert!(store.lock().unwrap().discard_draft(second).unwrap());
        sync(&mut session, &account, &store).await;
        assert!(server.lock().unwrap().subjects("Robocze").is_empty());
        assert!(store.lock().unwrap().list(&drafts).unwrap().is_empty());
        logout(&mut session).await;
        let commands = server.lock().unwrap().commands.clone();
        assert!(commands
            .iter()
            .any(|c| c.starts_with("APPEND \"Robocze\" (\\Seen \\Draft)")));
    }

    #[tokio::test]
    async fn a_draft_saved_again_during_its_upload_leaves_no_old_copy() {
        let (server, store, account, mut session) = synced_with_drafts().await;
        let drafts = account.folder_key("robocze");
        let first = save_draft(&store, &account, written(&account, "Oferta", "First."));
        let upload = store.lock().unwrap().pending_changes(&account.id).unwrap()[0].clone();
        // The upload reached the server, and meanwhile the user saved again.
        let uid = {
            let raw = store.lock().unwrap().raw_of(first).unwrap().unwrap();
            let mut s = server.lock().unwrap();
            let b = s.boxes.iter_mut().find(|b| b.name == "Robocze").unwrap();
            let uid = b.next_uid;
            b.next_uid += 1;
            b.mail.push(FakeMail {
                uid,
                flags: vec!["\\Seen".into(), "\\Draft".into()],
                raw,
            });
            uid
        };
        save_draft(
            &store,
            &account,
            OutgoingDraft {
                replaces: Some(first),
                ..written(&account, "Oferta", "Second.")
            },
        );
        store
            .lock()
            .unwrap()
            .change_appended(&upload, first, &drafts, Some(uid))
            .unwrap();
        sync(&mut session, &account, &store).await;
        let up = drafts_on(&server, "Oferta");
        assert_eq!(up.len(), 1, "{:?}", up.iter().map(text).collect::<Vec<_>>());
        assert!(text(&up[0]).contains("Second."));
        assert_eq!(store.lock().unwrap().list(&drafts).unwrap().len(), 2);
        logout(&mut session).await;
    }

    /// Send a message as `smtp::send_outbox` leaves it: filed in Sent, maybe a copy queued.
    fn sent_here(store: &Mutex<MailStore>, account: &Account, n: u32) -> (MessageId, Vec<u8>) {
        let raw = format!(
            "Message-ID: <wyslana.{n}@twojastudio.pl>\r\nFrom: biuro@twojastudio.pl\r\n\
             To: anna@kowalska-architekci.pl\r\nSubject: Oferta {n}\r\n\r\nTekst.\r\n"
        )
        .into_bytes();
        let mut s = store.lock().unwrap();
        let message = parse_message(&raw, "", None);
        let id = s
            .queue_outgoing(
                &account.id,
                &message,
                &raw,
                "biuro@twojastudio.pl",
                &["anna@kowalska-architekci.pl".into()],
                None,
                None,
            )
            .unwrap();
        s.outgoing_sent(id, Local::now().naive_local()).unwrap();
        (id, raw)
    }

    /// The server files the message in Sent itself, as Gmail does.
    fn filed_by_server(server: &Shared, raw: &[u8]) {
        let mut s = server.lock().unwrap();
        let b = s
            .boxes
            .iter_mut()
            .find(|b| b.name == "Wys&AUI-ane")
            .unwrap();
        let uid = b.next_uid;
        b.next_uid += 1;
        b.mail.push(FakeMail {
            uid,
            flags: vec!["\\Seen".into()],
            raw: raw.to_vec(),
        });
    }

    fn appends(server: &Shared) -> usize {
        let s = server.lock().unwrap();
        s.commands
            .iter()
            .filter(|c| c.starts_with("APPEND"))
            .count()
    }

    #[tokio::test]
    async fn a_server_that_files_sent_mail_itself_gets_no_copy() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let sent = account.folder_key("wysłane");
        // Not known yet: Telex looks in Sent before uploading, finds the server's own copy.
        let (id, raw) = sent_here(&store, &account, 1);
        filed_by_server(&server, &raw);
        sync(&mut session, &account, &store).await;
        assert_eq!(appends(&server), 0);
        assert_eq!(server.lock().unwrap().subjects("Wys&AUI-ane").len(), 2);
        {
            let s = store.lock().unwrap();
            assert_eq!(
                s.account(&account.id).unwrap().server_files_sent,
                Some(true)
            );
            let list = s.list(&sent).unwrap();
            assert_eq!(list.len(), 2, "the old one and this one, not twice");
            assert!(list.iter().any(|m| m.id == id));
            assert_eq!(s.local_uids(&sent).unwrap().len(), 2);
            assert!(!s.has_pending_changes(&account.id).unwrap());
        }

        // From now on nothing is queued; the download adopts the server's copy.
        let (id, raw) = sent_here(&store, &account, 2);
        assert!(!store
            .lock()
            .unwrap()
            .has_pending_changes(&account.id)
            .unwrap());
        filed_by_server(&server, &raw);
        sync(&mut session, &account, &store).await;
        assert_eq!(appends(&server), 0);
        {
            let s = store.lock().unwrap();
            let list = s.list(&sent).unwrap();
            assert_eq!(list.len(), 3);
            assert!(list.iter().any(|m| m.id == id));
            assert_eq!(s.local_uids(&sent).unwrap().len(), 3);
        }
        logout(&mut session).await;
    }

    #[tokio::test]
    async fn a_server_that_files_nothing_gets_a_moment_then_a_copy() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1 MOVE UIDPLUS").await;
        let (id, raw) = sent_here(&store, &account, 1);
        // Just sent: not in Sent yet, but the server may still be filing it.
        sync(&mut session, &account, &store).await;
        assert_eq!(appends(&server), 0);
        assert!(store
            .lock()
            .unwrap()
            .has_pending_changes(&account.id)
            .unwrap());
        // A minute later it still isn't there: Telex uploads it and remembers.
        {
            let s = store.lock().unwrap();
            let earlier = Local::now().naive_local() - chrono::TimeDelta::minutes(1);
            s.db.execute(
                "UPDATE messages SET date = ?1 WHERE id = ?2",
                rusqlite::params![earlier, id as i64],
            )
            .unwrap();
        }
        sync(&mut session, &account, &store).await;
        assert_eq!(appends(&server), 1);
        assert_eq!(
            server.lock().unwrap().mail("Wys&AUI-ane", "Oferta 1").raw,
            raw
        );
        {
            let s = store.lock().unwrap();
            assert_eq!(
                s.account(&account.id).unwrap().server_files_sent,
                Some(false)
            );
            assert!(!s.has_pending_changes(&account.id).unwrap());
        }
        // The next one goes up at once.
        sent_here(&store, &account, 2);
        sync(&mut session, &account, &store).await;
        assert_eq!(appends(&server), 2);
        logout(&mut session).await;
    }
}
