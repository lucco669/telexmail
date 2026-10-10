//! Two-way sync with an IMAP server.
//!
//! Downloading opens folders with EXAMINE (read-only) and fetches with BODY.PEEK, so
//! looking never changes anything. Changes the user makes are queued by `MailStore` and
//! sent by [`push_changes`]: flags with UID STORE, moves with UID MOVE (or COPY, then
//! delete and EXPUNGE on servers without MOVE), deletes from Trash with EXPUNGE.
//! The tests run all of it against a small fake server.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_imap::error::Error as ImapError;
use async_imap::imap_proto::{Response, ResponseCode, Status, UidSetMember};
use async_imap::types::{Capability, Fetch, Flag, Name, NameAttribute, QuotaResourceName};
use async_imap::{Client, Session};
use chrono::Local;
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
use crate::model::{Account, Folder, FolderKind, ImapSettings, MessageId};
use crate::store::{
    slug, ChangeKind, MailStore, PendingChange, StoreError, SyncedMessage, OUTBOX_ID,
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
    pub folder: String,
    pub done: usize,
    pub total: usize,
}

/// What one sync of an account did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Ids of messages that arrived.
    pub new: Vec<MessageId>,
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

/// Log in once to check the settings, and return the server's folders.
pub async fn check_account(
    settings: &ImapSettings,
    password: &str,
) -> Result<Vec<RemoteFolder>, SyncError> {
    let client = connect_tls(settings).await?;
    let mut session = open_session(client, settings, password).await?;
    let folders = list_folders(&mut session).await;
    logout(&mut session).await;
    folders
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
) -> Result<SyncReport, SyncError> {
    let Some(settings) = &account.imap else {
        return Ok(SyncReport::default());
    };
    let client = connect_tls(settings).await?;
    let mut session = open_session(client, settings, password).await?;
    let result = sync_all(&mut session, account, store, progress).await;
    logout(&mut session).await;
    result
}

/// Only send queued changes. Doesn't connect when there is nothing to send.
/// Returns the changes the server refused for good.
pub async fn push_account(
    account: &Account,
    password: &str,
    store: &Mutex<MailStore>,
) -> Result<Vec<String>, SyncError> {
    let Some(settings) = &account.imap else {
        return Ok(Vec::new());
    };
    if !lock(store)?.has_pending_changes(&account.id)? {
        return Ok(Vec::new());
    }
    let client = connect_tls(settings).await?;
    let mut session = open_session(client, settings, password).await?;
    let result = async {
        let caps = server_caps(&mut session).await?;
        push_changes(&mut session, account, store, caps).await
    }
    .await;
    logout(&mut session).await;
    result
}

/// [`sync_account`] over a session that is already logged in.
pub async fn sync_all<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<SyncReport, SyncError> {
    let caps = server_caps(session).await?;
    let queued_before = lock(store)?.last_change_op(&account.id)?;
    let refused = push_queued_after(session, account, store, caps, 0).await?;
    let downloaded = sync_session(session, account, store, progress).await?;
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
    if !downloaded.arrived.is_empty() {
        let run = {
            let mut s = lock(store)?;
            filters::run(&mut s, &downloaded.arrived, Trigger::Arrival, Local::now())?
        };
        report.sorted = run.matched;
        report.answered = run.replied;
        report.refused.extend(run.problems);
    }
    // Send what the user did during the download, but don't retry what was just refused.
    // The folder list may have changed, so read the account again.
    let account = lock(store)?.account(&account.id)?.clone();
    report
        .refused
        .extend(push_queued_after(session, &account, store, caps, queued_before).await?);
    Ok(report)
}

/// Download: refresh the folder list, then for every folder drop what was deleted on
/// the server, take flag changes made elsewhere, and fetch new messages.
pub async fn sync_session<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<Downloaded, SyncError> {
    let remote = list_folders(session).await?;
    let folders = plan_folders(&remote);
    lock(store)?.set_folders(&account.id, &folders)?;

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
                done,
                total,
            });
        };
        if total > 0 {
            report(0);
        }
        let mut done = 0;
        for chunk in wanted.chunks(FETCH_BATCH) {
            let batch = fetch_batch(session, chunk, &key).await?;
            let ids = lock(store)?.add_synced(&key, &batch)?;
            if arriving {
                downloaded.arrived.extend(&ids);
            }
            downloaded.new.extend(ids);
            done += chunk.len();
            report(done);
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
        .partition(|c| matches!(c.kind, ChangeKind::Append(_)));
    let mut refused = Vec::new();

    // Copies of sent mail for the Sent folder. APPEND needs no selected folder.
    for change in &appends {
        let (ChangeKind::Append(to), Some(id)) = (&change.kind, change.message) else {
            continue;
        };
        let raw = lock(store)?.raw_of(id)?;
        let (Some(raw), Some(remote)) = (raw, remote_name(account, to)) else {
            lock(store)?.change_done(change.op)?; // nothing to save, or nowhere to save it
            continue;
        };
        let result = timeout(
            FETCH_TIMEOUT,
            session.append(&remote, Some("(\\Seen)"), None, &raw),
        )
        .await
        .map_err(|_| SyncError::Timeout)?;
        match result {
            Ok(()) => lock(store)?.change_appended(change.op, id, to)?,
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
    for (key, mut group) in by_folder {
        // Flags first: they travel with a moved message, but its old UID is useless after.
        group.sort_by_key(|c| {
            (
                matches!(c.kind, ChangeKind::Move(_) | ChangeKind::Expunge),
                c.op,
            )
        });
        let Some(remote) = remote_name(account, &key) else {
            continue; // the folder is gone; the next download drops its mail and changes
        };
        match timeout(COMMAND_TIMEOUT, session.select(&remote))
            .await
            .map_err(|_| SyncError::Timeout)?
        {
            Ok(_) => {}
            Err(ImapError::No(reason)) => {
                for change in &group {
                    note_refusal(store, change, &reason, &mut refused)?;
                }
                continue;
            }
            Err(e) => return Err(e.into()),
        }
        let mut uids: Vec<u32> = group.iter().filter_map(|c| c.uid).collect();
        uids.sort_unstable();
        uids.dedup();
        let present = timeout(
            COMMAND_TIMEOUT,
            session.uid_search(format!("UID {}", uid_set(&uids))),
        )
        .await
        .map_err(|_| SyncError::Timeout)??;

        for change in &group {
            let Some(uid) = change.uid.filter(|u| present.contains(u)) else {
                // Already gone from the server (deleted elsewhere): nothing left to change.
                match change.message {
                    Some(id) => lock(store)?.forget_message(id)?,
                    None => lock(store)?.change_done(change.op)?,
                }
                continue;
            };
            let result = timeout(
                COMMAND_TIMEOUT,
                apply_change(session, account, change, uid, caps),
            )
            .await
            .map_err(|_| SyncError::Timeout)?;
            match (result, &change.kind, change.message) {
                (Ok(new_uid), ChangeKind::Move(to), Some(id)) => {
                    lock(store)?.change_moved(change.op, id, to, new_uid)?;
                    if new_uid.is_none() {
                        moved_without_uid.push((id, to.clone(), change.message_id.clone()));
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
        let query = format!("HEADER Message-ID {}", quote(&format!("<{mid}>")));
        let found = timeout(COMMAND_TIMEOUT, session.uid_search(query))
            .await
            .map_err(|_| SyncError::Timeout)??;
        let known = lock(store)?.local_uids(&key)?;
        if let Some(uid) = found.into_iter().filter(|u| !known.contains(u)).max() {
            lock(store)?.set_server_uid(id, &key, uid)?;
        }
    }
    Ok(refused)
}

// ---- Connection ----

pub(crate) fn tls_connector() -> TlsConnector {
    let roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("the ring provider supports TLS 1.2 and 1.3")
        .with_root_certificates(roots)
        .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

async fn connect_tls(settings: &ImapSettings) -> Result<Client<TlsStream<TcpStream>>, SyncError> {
    let host = settings.host.as_str();
    let unreachable = |reason: String| SyncError::Connect {
        host: host.to_owned(),
        reason,
    };
    let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect((host, settings.port)))
        .await
        .map_err(|_| {
            unreachable(tr!(
                "no answer within 20 seconds",
                "brak odpowiedzi w ciągu 20 sekund"
            ))
        })?
        .map_err(|e| unreachable(e.to_string()))?;
    let name = ServerName::try_from(host)
        .map_err(|_| {
            unreachable(tr!(
                "that isn't a valid server name",
                "to nie jest poprawna nazwa serwera"
            ))
        })?
        .to_owned();
    let tls = timeout(CONNECT_TIMEOUT, tls_connector().connect(name, tcp))
        .await
        .map_err(|_| SyncError::Timeout)?
        .map_err(|e| SyncError::Tls {
            host: host.to_owned(),
            reason: tls_reason(&e),
        })?;
    Ok(Client::new(tls))
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
async fn open_session<T: ImapStream>(
    mut client: Client<T>,
    settings: &ImapSettings,
    password: &str,
) -> Result<Session<T>, SyncError> {
    match timeout(CONNECT_TIMEOUT, client.read_response()).await {
        Err(_) => return Err(SyncError::Timeout),
        Ok(Err(e)) => return Err(SyncError::Server(connection_error(&e))),
        Ok(Ok(None)) => {
            return Err(SyncError::Server(tr!(
                "it hung up before saying hello",
                "rozłączył się, zanim się przywitał"
            )))
        }
        Ok(Ok(Some(_greeting))) => {}
    }
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

async fn fetch_batch<T: ImapStream>(
    session: &mut Session<T>,
    uids: &[u32],
    folder_key: &str,
) -> Result<Vec<SyncedMessage>, SyncError> {
    let fetches = timeout(FETCH_TIMEOUT, async {
        let stream = session.uid_fetch(uid_set(uids), FETCH_ITEMS).await?;
        stream.try_collect::<Vec<Fetch>>().await
    })
    .await
    .map_err(|_| SyncError::Timeout)??;
    let wanted: HashSet<u32> = uids.iter().copied().collect();
    Ok(fetches
        .iter()
        .filter_map(|f| {
            // Servers may add FETCH lines for other messages; keep only what was asked for.
            let uid = f.uid.filter(|u| wanted.contains(u))?;
            let raw = f.body()?;
            let received = f
                .internal_date()
                .map(|d| d.with_timezone(&Local).naive_local());
            let mut message = parse_message(raw, folder_key, received);
            let (seen, flagged) = seen_and_flagged(f);
            message.unread = !seen;
            message.flagged = flagged;
            Some(SyncedMessage {
                uid,
                message,
                raw: raw.to_vec(),
            })
        })
        .collect())
}

/// `(uid, seen, flagged)` for messages already stored.
async fn fetch_flags<T: ImapStream>(
    session: &mut Session<T>,
    uids: &[u32],
) -> Result<Vec<(u32, bool, bool)>, SyncError> {
    let fetches = timeout(COMMAND_TIMEOUT, async {
        let stream = session.uid_fetch(uid_set(uids), "(UID FLAGS)").await?;
        stream.try_collect::<Vec<Fetch>>().await
    })
    .await
    .map_err(|_| SyncError::Timeout)??;
    Ok(fetches
        .iter()
        .filter_map(|f| {
            let (seen, flagged) = seen_and_flagged(f);
            Some((f.uid?, seen, flagged))
        })
        .collect())
}

fn seen_and_flagged(f: &Fetch) -> (bool, bool) {
    let flags: Vec<Flag<'_>> = f.flags().collect();
    (flags.contains(&Flag::Seen), flags.contains(&Flag::Flagged))
}

// ---- Uploading ----

/// Do one change on the server, in the folder that is selected. For a move, returns the
/// message's new UID when the server says it.
async fn apply_change<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    change: &PendingChange,
    uid: u32,
    caps: ServerCaps,
) -> Result<Option<u32>, ImapError> {
    match &change.kind {
        ChangeKind::Seen(on) => store_flag(session, uid, "\\Seen", *on).await.map(|()| None),
        ChangeKind::Flagged(on) => store_flag(session, uid, "\\Flagged", *on)
            .await
            .map(|()| None),
        ChangeKind::Expunge => expunge(session, uid, caps).await.map(|()| None),
        ChangeKind::Append(_) => Ok(None), // sent before the folders, in push_queued_after
        ChangeKind::Move(to) => {
            let dest = remote_name(account, to).ok_or_else(|| {
                ImapError::No(tr!(
                    "that folder no longer exists",
                    "tego folderu już nie ma"
                ))
            })?;
            if caps.can_move {
                run_with_copyuid(session, &format!("UID MOVE {uid} {}", quote(&dest))).await
            } else {
                let new_uid =
                    run_with_copyuid(session, &format!("UID COPY {uid} {}", quote(&dest))).await?;
                expunge(session, uid, caps).await?;
                Ok(new_uid)
            }
        }
    }
}

async fn store_flag<T: ImapStream>(
    session: &mut Session<T>,
    uid: u32,
    flag: &str,
    on: bool,
) -> Result<(), ImapError> {
    let sign = if on { '+' } else { '-' };
    let stream = session
        .uid_store(uid.to_string(), format!("{sign}FLAGS.SILENT ({flag})"))
        .await?;
    stream.try_collect::<Vec<_>>().await?;
    Ok(())
}

/// Remove one message for good. Without UIDPLUS the only way is a plain EXPUNGE, which
/// also removes other messages already marked \Deleted in that folder, as any client would.
async fn expunge<T: ImapStream>(
    session: &mut Session<T>,
    uid: u32,
    caps: ServerCaps,
) -> Result<(), ImapError> {
    store_flag(session, uid, "\\Deleted", true).await?;
    if caps.uidplus {
        session
            .uid_expunge(uid.to_string())
            .await?
            .try_collect::<Vec<_>>()
            .await?;
    } else {
        session.expunge().await?.try_collect::<Vec<_>>().await?;
    }
    Ok(())
}

/// Run a COPY or MOVE and pick the destination UID out of its COPYUID response code
/// (RFC 4315). MOVE sends it in an untagged OK, COPY in the tagged one.
async fn run_with_copyuid<T: ImapStream>(
    session: &mut Session<T>,
    command: &str,
) -> Result<Option<u32>, ImapError> {
    let tag = session.run_command(command).await?;
    let mut new_uid = None;
    loop {
        let response = session
            .read_response()
            .await?
            .ok_or(ImapError::ConnectionLost)?;
        match response.parsed() {
            Response::Data {
                code: Some(ResponseCode::CopyUid(_, _, dest)),
                ..
            } => new_uid = first_uid(dest),
            Response::Done {
                tag: done,
                status,
                code,
                information,
            } if *done == tag => {
                if let Some(ResponseCode::CopyUid(_, _, dest)) = code {
                    new_uid = first_uid(dest);
                }
                let text = information.as_deref().unwrap_or_default().to_owned();
                return match status {
                    Status::Ok => Ok(new_uid),
                    Status::No => Err(ImapError::No(text)),
                    _ => Err(ImapError::Bad(text)),
                };
            }
            _ => {}
        }
    }
}

fn first_uid(set: &[UidSetMember]) -> Option<u32> {
    set.first().map(|m| match m {
        UidSetMember::Uid(u) => *u,
        UidSetMember::UidRange(r) => *r.start(),
    })
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
        // English names the change before the subject; Polish wraps the subject, as in
        // "oznaczyć wiadomości „X” jako przeczytanej".
        let (what, pl_verb, pl_after) = match &change.kind {
            ChangeKind::Seen(true) => ("mark as read", "oznaczyć", " jako przeczytanej"),
            ChangeKind::Seen(false) => ("mark as unread", "oznaczyć", " jako nieprzeczytanej"),
            ChangeKind::Flagged(true) => ("flag", "oznaczyć flagą", ""),
            ChangeKind::Flagged(false) => ("unflag", "zdjąć flagi z", ""),
            ChangeKind::Move(_) => ("move", "przenieść", ""),
            ChangeKind::Expunge => ("delete", "usunąć", ""),
            ChangeKind::Append(_) => (
                "save a copy in Sent of",
                "zapisać kopii",
                " w folderze Wysłane",
            ),
        };
        let (subject, reason) = (&change.subject, reason.trim());
        refused.push(tr!(
            "the server wouldn't {what} \"{subject}\" ({reason})",
            "serwer nie pozwolił {pl_verb} wiadomości „{subject}”{pl_after} ({reason})"
        ));
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

/// An IMAP quoted string. Folder names come from the server's own LIST, so they hold no
/// line breaks; quotes and backslashes are escaped.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
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

/// Decide which server folders Telex shows, and their ids, names and kinds.
/// Ids come from the server names, so they stay the same from one sync to the next.
pub fn plan_folders(remote: &[RemoteFolder]) -> Vec<Folder> {
    let syncable: Vec<&RemoteFolder> = remote.iter().filter(|f| f.syncable).collect();

    // Kinds: INBOX by name, then the server's special-use flags, then well-known names.
    // Each special kind goes to one folder only; the rest stay custom.
    let mut kinds = vec![FolderKind::Custom; syncable.len()];
    let mut taken = HashSet::new();
    for (i, f) in syncable.iter().enumerate() {
        if f.name.eq_ignore_ascii_case("INBOX") && taken.insert(FolderKind::Inbox) {
            kinds[i] = FolderKind::Inbox;
        }
    }
    for (i, f) in syncable.iter().enumerate() {
        if let Some(k) = f.special {
            if kinds[i] == FolderKind::Custom && taken.insert(k) {
                kinds[i] = k;
            }
        }
    }
    for (i, f) in syncable.iter().enumerate() {
        if kinds[i] != FolderKind::Custom {
            continue;
        }
        if let Some(k) = f.path().last().and_then(|leaf| guess_kind(leaf)) {
            if taken.insert(k) {
                kinds[i] = k;
            }
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
            let name = match default_name(kind) {
                Some(n) => n.to_owned(),
                None => display_path(&path),
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

/// Special folders get the same English names on every account.
fn default_name(kind: FolderKind) -> Option<&'static str> {
    match kind {
        FolderKind::Inbox => Some("Inbox"),
        FolderKind::Outbox => Some("Outbox"),
        FolderKind::Drafts => Some("Drafts"),
        FolderKind::Sent => Some("Sent"),
        FolderKind::Archive => Some("Archive"),
        FolderKind::Spam => Some("Spam"),
        FolderKind::Trash => Some("Trash"),
        FolderKind::Custom => None,
    }
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
        .chunks_exact(2)
        .map(|p| u16::from_be_bytes([p[0], p[1]]))
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
        let plan = plan_folders(&[
            remote("INBOX", None, true),
            remote("INBOX.Sent", None, true),
            remote("INBOX.Sent Items", None, true),
            remote("INBOX.Robocze", None, true),
            remote("INBOX.Klienci", None, true),
            remote("Kopie", Some(FolderKind::Drafts), true),
            remote("[Gmail]", None, false),
        ]);
        let got: Vec<_> = plan
            .iter()
            .map(|f| (f.id.as_str(), f.name.as_str(), f.kind))
            .collect();
        assert_eq!(
            got,
            [
                ("inbox", "Inbox", FolderKind::Inbox),
                ("kopie", "Drafts", FolderKind::Drafts),
                ("inbox-sent", "Sent", FolderKind::Sent),
                ("inbox-klienci", "Klienci", FolderKind::Custom),
                ("inbox-robocze", "Robocze", FolderKind::Custom),
                ("inbox-sent-items", "Sent Items", FolderKind::Custom),
            ]
        );
        assert_eq!(plan[2].remote.as_deref(), Some("INBOX.Sent"));
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
        }));
        let shared = server.clone();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let _ = serve(socket, shared.clone()).await;
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
            // UID STORE <uid> +FLAGS.SILENT (\Seen)
            let mut parts = command[10..].split(' ');
            let uid: u32 = parts.next().unwrap().parse().unwrap();
            let op = parts.next().unwrap();
            let flag = parts
                .next()
                .unwrap()
                .trim_matches(|c| c == '(' || c == ')')
                .to_owned();
            if let Some(m) = s.boxes[*selected].mail.iter_mut().find(|m| m.uid == uid) {
                m.flags.retain(|f| *f != flag);
                if op.starts_with('+') {
                    m.flags.push(flag);
                }
            }
            out.line(format!("{tag} OK STORE completed\r\n"));
        } else if upper.starts_with("UID MOVE ") || upper.starts_with("UID COPY ") {
            let moving = upper.starts_with("UID MOVE ");
            let (uid, dest) = command[9..].split_once(' ').unwrap();
            let uid: u32 = uid.parse().unwrap();
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
            let Some(seq) = s.boxes[from].mail.iter().position(|m| m.uid == uid) else {
                out.line(format!("{tag} OK No messages\r\n"));
                return out.0;
            };
            let mut copy = s.boxes[from].mail[seq].clone();
            if moving {
                s.boxes[from].mail.remove(seq);
            }
            let (validity, new_uid) = (s.boxes[to].uid_validity, s.boxes[to].next_uid);
            copy.uid = new_uid;
            s.boxes[to].next_uid += 1;
            s.boxes[to].mail.push(copy);
            let copyuid = format!("[COPYUID {validity} {uid} {new_uid}] ");
            if moving {
                if uidplus {
                    out.line(format!("* OK {copyuid}Moved\r\n"));
                }
                out.line(format!(
                    "* {} EXPUNGE\r\n{tag} OK MOVE completed\r\n",
                    seq + 1
                ));
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
            security: Default::default(),
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
        sync_all(&mut session, &account, &store, &no_progress)
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
    async fn downloading_never_changes_the_server() {
        let (addr, server) = start_fake_server("IMAP4rev1 MOVE UIDPLUS").await;
        let (store, account) = store_with_account();
        let events = Mutex::new(Vec::new());
        let progress = |p: SyncProgress| events.lock().unwrap().push(p);
        let mut session = login(addr, "secret").await.unwrap();

        let new = sync_session(&mut session, &account, &store, &progress)
            .await
            .unwrap()
            .new;
        assert_eq!(new.len(), 3);
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
                    ("wysłane".into(), "Sent".into(), FolderKind::Sent),
                    (
                        "projekty-klinika".into(),
                        "Projekty / Klinika".into(),
                        FolderKind::Custom
                    ),
                    ("kosz".into(), "Trash".into(), FolderKind::Trash),
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
                folder: "Sent".into(),
                done: 1,
                total: 1
            })
        );

        // Nothing changed on the server: nothing is downloaded.
        let new = sync_session(&mut session, &account, &store, &progress)
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
        let new = sync_session(&mut session, &account, &store, &progress)
            .await
            .unwrap()
            .new;
        assert_eq!(new.len(), 1);
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
        let new = sync_session(&mut session, &account, &store, &progress)
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
        let report = sync_all(&mut session, &account, &store, &no_progress)
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
        sync_all(&mut session, &account, &store, &no_progress)
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
    async fn without_move_telex_copies_expunges_and_finds_the_new_uid() {
        let (server, store, account, mut session) = synced_once("IMAP4rev1").await;
        let poprawki = id_of(&store, &account, "inbox", "Poprawki");
        store
            .lock()
            .unwrap()
            .move_to(poprawki, &account.folder_key("kosz"))
            .unwrap();
        let report = sync_all(&mut session, &account, &store, &no_progress)
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
        sync_all(&mut session, &account, &store, &no_progress)
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
        sync_all(&mut session, &account, &store, &no_progress)
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
            let report = sync_all(&mut session, &account, &store, &no_progress)
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
        let report = sync_all(&mut session, &account, &store, &no_progress)
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
        let report = sync_all(&mut session, &account, &store, &progress)
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
            let message = parse_message(raw, "", None);
            let id = s
                .queue_outgoing(
                    &account.id,
                    &message,
                    raw,
                    "biuro@twojastudio.pl",
                    &["anna@kowalska-architekci.pl".into()],
                    None,
                )
                .unwrap();
            s.outgoing_sent(id, chrono::Local::now().naive_local())
                .unwrap();
            id
        };
        let report = sync_all(&mut session, &account, &store, &no_progress)
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
                field: Field::From,
                op: Op::Contains,
                value: "kowalska".into(),
            }],
            actions: vec![Action::Flag],
            stop: false,
        };
        store.lock().unwrap().save_rule(&flag_anna).unwrap();
        let mut session = login(addr, "secret").await.unwrap();

        // The first download is old mail: the rule leaves it alone.
        let report = sync_all(&mut session, &account, &store, &no_progress)
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
        let report = sync_all(&mut session, &account, &store, &no_progress)
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
            security: Default::default(),
        };
        let mut client = connect_tls(&settings).await.unwrap();
        let greeting = client.read_response().await.unwrap();
        assert!(greeting.is_some());
    }
}
