//! Reading mail from an IMAP server.
//!
//! Telex only reads for now. Folders are opened with EXAMINE, which servers treat as
//! read-only, and messages are fetched with BODY.PEEK so they aren't even marked as
//! read. A test below runs a full sync against a fake server and checks that no
//! command that changes anything is ever sent.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_imap::error::Error as ImapError;
use async_imap::types::{Fetch, Flag, Name, NameAttribute};
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

use crate::mime::parse_message;
use crate::model::{Account, Folder, FolderKind, ImapSettings, MessageId};
use crate::store::{slug, MailStore, StoreError, SyncedMessage};

/// How many of the newest messages to download per folder the first time.
pub const FIRST_SYNC_LIMIT: usize = 300;
/// Messages per FETCH command: small enough to report progress often.
const FETCH_BATCH: usize = 20;
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
        match self {
            Self::Connect { host, reason } => write!(f, "can't reach {host}: {reason}"),
            Self::Tls { host, reason } => write!(f, "no secure connection to {host}: {reason}"),
            Self::Login(reason) => {
                write!(
                    f,
                    "the server rejected the user name or password ({reason})"
                )
            }
            Self::Timeout => write!(f, "the server stopped answering"),
            Self::Server(reason) => write!(f, "the server reported a problem: {reason}"),
            Self::Store(e) => e.fmt(f),
        }
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
            ImapError::ConnectionLost => Self::Server("the connection was closed".into()),
            ImapError::Io(e) => Self::Server(format!("connection error: {e}")),
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

/// Download what's new in every folder of `account` into `store`, and drop local copies
/// of mail that was deleted on the server. Returns the ids of new messages.
///
/// The store is locked only between network calls, so the UI stays responsive.
pub async fn sync_account(
    account: &Account,
    password: &str,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<Vec<MessageId>, SyncError> {
    let Some(settings) = &account.imap else {
        return Ok(Vec::new());
    };
    let client = connect_tls(settings).await?;
    let mut session = open_session(client, settings, password).await?;
    let result = sync_session(&mut session, account, store, progress).await;
    logout(&mut session).await;
    result
}

/// The sync itself, over a session that is already logged in.
pub async fn sync_session<T: ImapStream>(
    session: &mut Session<T>,
    account: &Account,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<Vec<MessageId>, SyncError> {
    let remote = list_folders(session).await?;
    let folders = plan_folders(&remote);
    lock(store)?.set_folders(&account.id, &folders)?;

    let mut new_ids = Vec::new();
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
        let on_server = if mailbox.exists == 0 {
            HashSet::new()
        } else {
            timeout(COMMAND_TIMEOUT, session.uid_search("ALL"))
                .await
                .map_err(|_| SyncError::Timeout)??
        };
        lock(store)?.forget_vanished(&key, &on_server)?;

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
            new_ids.extend(lock(store)?.add_synced(&key, &batch)?);
            done += chunk.len();
            report(done);
        }
    }
    Ok(new_ids)
}

// ---- Connection ----

fn tls_connector() -> TlsConnector {
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
        .map_err(|_| unreachable("no answer within 20 seconds".into()))?
        .map_err(|e| unreachable(e.to_string()))?;
    let name = ServerName::try_from(host)
        .map_err(|_| unreachable("that isn't a valid server name".into()))?
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
        "its certificate is for a different name. Use the server name from your provider's setup guide".into()
    } else if text.contains("UnknownIssuer") {
        "its certificate isn't from a trusted authority".into()
    } else if text.contains("Expired") {
        "its certificate has expired".into()
    } else {
        text
    }
}

/// Read the server's greeting and log in.
async fn open_session<T: ImapStream>(
    mut client: Client<T>,
    settings: &ImapSettings,
    password: &str,
) -> Result<Session<T>, SyncError> {
    match timeout(CONNECT_TIMEOUT, client.read_response()).await {
        Err(_) => return Err(SyncError::Timeout),
        Ok(Err(e)) => return Err(SyncError::Server(format!("connection error: {e}"))),
        Ok(Ok(None)) => return Err(SyncError::Server("it hung up before saying hello".into())),
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

/// The store is shared with the UI thread; lock it only for quick database work.
fn lock(store: &Mutex<MailStore>) -> Result<MutexGuard<'_, MailStore>, SyncError> {
    store.lock().map_err(|_| {
        SyncError::Store(StoreError::Database(
            "another task crashed while using it; restart Telex".into(),
        ))
    })
}

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
            let flags: Vec<Flag<'_>> = f.flags().collect();
            message.unread = !flags.contains(&Flag::Seen);
            message.flagged = flags.contains(&Flag::Flagged);
            Some(SyncedMessage {
                uid,
                message,
                raw: raw.to_vec(),
            })
        })
        .collect())
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

    let mut used = HashSet::from(["inbox".to_owned()]);
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
    folders.sort_by_key(|f| (kind_rank(f.kind), f.name.to_lowercase()));
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
        FolderKind::Drafts => Some("Drafts"),
        FolderKind::Sent => Some("Sent"),
        FolderKind::Archive => Some("Archive"),
        FolderKind::Spam => Some("Spam"),
        FolderKind::Trash => Some("Trash"),
        FolderKind::Custom => None,
    }
}

/// Folder order in the tree, as in The Bat!: Inbox, Drafts, Sent, own folders, then the rest.
fn kind_rank(kind: FolderKind) -> u8 {
    match kind {
        FolderKind::Inbox => 0,
        FolderKind::Drafts => 1,
        FolderKind::Sent => 2,
        FolderKind::Custom => 3,
        FolderKind::Archive => 4,
        FolderKind::Spam => 5,
        FolderKind::Trash => 6,
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
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
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

    // ---- A tiny scripted IMAP server: just enough of RFC 3501 for a read-only sync ----

    #[derive(Clone)]
    struct FakeMail {
        uid: u32,
        flags: &'static str,
        raw: Vec<u8>,
    }

    struct FakeBox {
        name: &'static str,
        attrs: &'static str,
        uid_validity: u32,
        mail: Vec<FakeMail>,
    }

    #[derive(Default)]
    struct FakeServer {
        boxes: Vec<FakeBox>,
        /// Every command the client sent, without its tag.
        commands: Vec<String>,
    }

    type Shared = Arc<Mutex<FakeServer>>;

    fn mail(uid: u32, flags: &'static str, subject: &str, body_cp1250: &[u8]) -> FakeMail {
        let mut raw = format!(
            "From: Anna Kowalska <anna@kowalska-architekci.pl>\r\n\
             To: biuro@twojastudio.pl\r\n\
             Subject: {subject}\r\n\
             Date: Fri, 09 Oct 2026 14:30:00 +0200\r\n\
             Content-Type: text/plain; charset=windows-1250\r\n\
             Content-Transfer-Encoding: 8bit\r\n\r\n"
        )
        .into_bytes();
        raw.extend_from_slice(body_cp1250);
        raw.extend_from_slice(b"\r\n");
        FakeMail { uid, flags, raw }
    }

    fn fake_boxes() -> Vec<FakeBox> {
        vec![
            FakeBox {
                name: "INBOX",
                attrs: "\\HasNoChildren",
                uid_validity: 7,
                mail: vec![
                    mail(1, "\\Seen", "Wycena", b"Dzie\xf1 dobry"),
                    mail(2, "\\Flagged", "Poprawki", b"Prosz\xea o zmiany"),
                ],
            },
            FakeBox {
                name: "Wys&AUI-ane",
                attrs: "\\HasNoChildren \\Sent",
                uid_validity: 7,
                mail: vec![mail(5, "\\Seen", "Re: Wycena", b"Dzi\xeakuj\xea")],
            },
            FakeBox {
                name: "Projekty",
                attrs: "\\Noselect \\HasChildren",
                uid_validity: 7,
                mail: vec![],
            },
            FakeBox {
                name: "Projekty/Klinika",
                attrs: "\\HasNoChildren",
                uid_validity: 7,
                mail: vec![],
            },
            FakeBox {
                name: "Kosz",
                attrs: "\\HasNoChildren",
                uid_validity: 7,
                mail: vec![],
            },
        ]
    }

    async fn start_fake_server() -> (SocketAddr, Shared) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server: Shared = Arc::new(Mutex::new(FakeServer {
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
        let mut lines = BufReader::new(read).lines();
        write
            .write_all(b"* OK [CAPABILITY IMAP4rev1] Fake IMAP ready\r\n")
            .await?;
        let mut selected = 0;
        while let Some(line) = lines.next_line().await? {
            let (tag, command) = line.split_once(' ').unwrap_or((&line, ""));
            let reply = {
                let mut s = server.lock().unwrap();
                s.commands.push(command.to_owned());
                respond(&s, tag, command, &mut selected)
            };
            write.write_all(&reply).await?;
            if command.eq_ignore_ascii_case("LOGOUT") {
                break;
            }
        }
        Ok(())
    }

    fn respond(s: &FakeServer, tag: &str, command: &str, selected: &mut usize) -> Vec<u8> {
        let upper = command.to_ascii_uppercase();
        let mut out = String::new();
        let mut body = Vec::new();
        if upper.starts_with("LOGIN ") {
            if command.ends_with("\"secret\"") {
                out += &format!("{tag} OK LOGIN completed\r\n");
            } else {
                out += &format!("{tag} NO [AUTHENTICATIONFAILED] Invalid credentials\r\n");
            }
        } else if upper.starts_with("LIST ") {
            for b in &s.boxes {
                out += &format!("* LIST ({}) \"/\" \"{}\"\r\n", b.attrs, b.name);
            }
            out += &format!("{tag} OK LIST completed\r\n");
        } else if let Some(name) = upper.strip_prefix("EXAMINE ").map(|_| &command[8..]) {
            let name = name.trim().trim_matches('"');
            match s.boxes.iter().position(|b| b.name == name) {
                Some(i) => {
                    *selected = i;
                    let b = &s.boxes[i];
                    let next = b.mail.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
                    out += &format!(
                        "* {} EXISTS\r\n* 0 RECENT\r\n* OK [UIDVALIDITY {}] UIDs valid\r\n\
                         * OK [UIDNEXT {next}] Predicted next UID\r\n\
                         * FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n\
                         {tag} OK [READ-ONLY] EXAMINE completed\r\n",
                        b.mail.len(),
                        b.uid_validity
                    );
                }
                None => out += &format!("{tag} NO Mailbox doesn't exist\r\n"),
            }
        } else if upper.starts_with("UID SEARCH") {
            let uids: Vec<String> = s.boxes[*selected]
                .mail
                .iter()
                .map(|m| m.uid.to_string())
                .collect();
            out += &format!(
                "* SEARCH {}\r\n{tag} OK SEARCH completed\r\n",
                uids.join(" ")
            );
        } else if upper.starts_with("UID FETCH ") {
            let set = command[10..].split(' ').next().unwrap_or_default();
            let wanted = expand_uid_set(set);
            for (seq, m) in s.boxes[*selected].mail.iter().enumerate() {
                if !wanted.contains(&m.uid) {
                    continue;
                }
                body.extend(
                    format!(
                        "* {} FETCH (UID {} FLAGS ({}) INTERNALDATE \"09-Oct-2026 14:30:00 +0200\" \
                         RFC822.SIZE {len} BODY[] {{{len}}}\r\n",
                        seq + 1,
                        m.uid,
                        m.flags,
                        len = m.raw.len()
                    )
                    .bytes(),
                );
                body.extend(&m.raw);
                body.extend(b")\r\n");
            }
            body.extend(format!("{tag} OK FETCH completed\r\n").bytes());
        } else if upper == "LOGOUT" {
            out += &format!("* BYE Fake IMAP logging out\r\n{tag} OK LOGOUT completed\r\n");
        } else {
            out += &format!("{tag} BAD Fake IMAP doesn't know that\r\n");
        }
        let mut reply = out.into_bytes();
        reply.extend(body);
        reply
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
            })
            .unwrap();
        (Mutex::new(store), account)
    }

    #[tokio::test]
    async fn sync_reads_mail_and_never_changes_the_server() {
        let (addr, server) = start_fake_server().await;
        let (store, account) = store_with_account();
        let events = Mutex::new(Vec::new());
        let progress = |p: SyncProgress| events.lock().unwrap().push(p);
        let mut session = login(addr, "secret").await.unwrap();

        let new = sync_session(&mut session, &account, &store, &progress)
            .await
            .unwrap();
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
            .unwrap();
        assert!(new.is_empty());

        // New mail arrives and message 1 is deleted elsewhere (say, on the phone).
        {
            let mut s = server.lock().unwrap();
            let inbox = &mut s.boxes[0].mail;
            inbox.retain(|m| m.uid != 1);
            inbox.push(mail(3, "", "Nowa wiadomo\u{15b}\u{107}", b"Hej"));
        }
        let new = sync_session(&mut session, &account, &store, &progress)
            .await
            .unwrap();
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
            .unwrap();
        assert_eq!(new.len(), 2);
        logout(&mut session).await;

        // Read-only: no command that changes the server, and every fetch is a PEEK.
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
            if verb == "FETCH" {
                assert!(c.contains("BODY.PEEK[]"), "{c}");
            }
        }
    }

    #[tokio::test]
    async fn a_wrong_password_is_a_login_error() {
        let (addr, _server) = start_fake_server().await;
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
        };
        let mut client = connect_tls(&settings).await.unwrap();
        let greeting = client.read_response().await.unwrap();
        assert!(greeting.is_some());
    }
}
