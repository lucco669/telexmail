//! POP3 accounts (RFC 1939, with CAPA from RFC 2449, STLS from RFC 2595 and AUTH PLAIN from
//! RFC 5034): a server that keeps only an Inbox, from which Telex downloads.
//!
//! Every folder of a POP3 account is here, like the folders on this PC: reading, flagging and
//! moving change nothing on the server. What comes down is remembered by its UIDL (the id a
//! server gives a message for good) in `pop3_uidls`, so nothing comes twice, also mail deleted
//! here that is still on the server. What stays on the server is the account's [`Pop3Keep`]:
//! by default everything, until it is deleted here for good; or removed once it is here, or
//! after some days. Removing is DELE, which the server carries out at QUIT.
//!
//! The first check takes the newest [`imap::FIRST_SYNC_LIMIT`] messages, as for IMAP; the
//! status bar offers the older ones (`download_older`). Later checks take everything new, and
//! it goes through the same spam filter, rules and away reply as new IMAP Inbox mail.
//!
//! The connection is TLS from the first byte (port 995) or STLS before the login (port 110),
//! never plain; a server that won't encrypt is never sent the password. Lines, listings and
//! messages have limits, and every read a timeout, so a hostile server can't hang a check or
//! fill the memory. The tests run all of it against a fake server.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use chrono::{Local, NaiveDate, NaiveDateTime, TimeDelta};
use rusqlite::{params, OptionalExtension};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;

use crate::imap::{self, Downloaded, SyncError, SyncProgress, SyncReport, FIRST_SYNC_LIMIT};
use crate::mime::parse_message;
use crate::model::{Account, FolderKind, ImapSettings, Message, MessageId, Pop3Keep, SmtpSecurity};
use crate::servers::{self, Purpose, Step, Trace, Turn, SERVERS};
use crate::store::{insert_message, MailStore, StoreError};
use crate::tr;

/// SSL/TLS from the first byte.
pub const PORT_TLS: u16 = 995;
/// A plain greeting, then STLS before anything else.
pub const PORT_STLS: u16 = 110;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// The longest wait for any one line: a slow line still sends something this often.
const READ_TIMEOUT: Duration = Duration::from_secs(120);
/// A status or listing line: RFC 2449 says 512 octets; some servers say more in their greeting.
const STATUS_LINE: usize = 8 * 1024;
/// A line of a message: mail should keep them under 1,000, HTML from some senders doesn't.
const MESSAGE_LINE: usize = 4 * 1024 * 1024;
/// The largest message Telex downloads: more than any provider sends. A larger one stays on
/// the server, and the check says so once.
pub const MAX_MESSAGE: u64 = 100 * 1024 * 1024;
/// The most messages a listing may have, and the longest UIDL Telex keeps (RFC 1939: 70).
const MAX_LISTING: usize = 1_000_000;
const MAX_UIDL: usize = 255;
/// Messages stored per transaction, and between two progress events.
const FETCH_BATCH: usize = 20;
/// A UIDL the server hasn't listed for this long is forgotten.
const FORGET_AFTER_DAYS: i64 = 90;

/// What the server says it can do (CAPA). `None` everywhere when it doesn't answer CAPA: an
/// old server, which may still do it all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caps {
    pub answered: bool,
    pub uidl: bool,
    pub user: bool,
    pub stls: bool,
    pub sasl: Vec<String>,
}

impl Caps {
    fn plain(&self) -> bool {
        self.sasl.iter().any(|m| m.eq_ignore_ascii_case("PLAIN"))
    }
}

/// One POP3 connection over any stream: TLS in the app, plain TCP in the tests.
pub struct Session<S> {
    io: BufReader<S>,
    pub caps: Caps,
}

fn closed() -> SyncError {
    SyncError::Server(tr!(
        "the connection was closed",
        "połączenie zostało zamknięte"
    ))
}

fn too_long() -> SyncError {
    SyncError::Server(tr!(
        "it sent a line longer than any mail server would",
        "wysłał wiersz dłuższy niż wysłałby jakikolwiek serwer poczty"
    ))
}

/// A server that keeps no lasting ids can't be told apart from new mail at every check.
fn no_uidl() -> SyncError {
    SyncError::Server(tr!(
        "this POP3 server gives messages no lasting ids (UIDL), which Telex needs to know what it already downloaded. Use IMAP if your provider offers it",
        "ten serwer POP3 nie nadaje wiadomościom stałych identyfikatorów (UIDL), a Telex ich potrzebuje, żeby wiedzieć, co już pobrał. Użyj IMAP, jeśli Twój dostawca go oferuje"
    ))
}

/// A "-ERR" answer, in the server's words. RFC 2449's codes say a refused login is something
/// else: the mailbox is in use by another program, or the server has a passing problem.
fn refusal(text: &str, login: bool) -> SyncError {
    let upper = text.to_ascii_uppercase();
    let passing = upper.starts_with("[IN-USE]") || upper.starts_with("[SYS/TEMP]");
    if login && !passing {
        SyncError::Login(text.to_owned())
    } else if upper.starts_with("[IN-USE]") {
        SyncError::Server(tr!(
            "another program is reading this mailbox right now ({text}); Telex tries again at the next check",
            "inny program właśnie czyta tę skrzynkę ({text}); Telex spróbuje ponownie przy następnym sprawdzaniu"
        ))
    } else {
        SyncError::Server(text.to_owned())
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Session<S> {
    pub fn new(stream: S) -> Self {
        Self {
            io: BufReader::new(stream),
            caps: Caps::default(),
        }
    }

    /// One line, without its line end, of at most `limit` bytes.
    async fn line(&mut self, limit: usize) -> Result<Vec<u8>, SyncError> {
        let mut raw = Vec::new();
        let mut taken = (&mut self.io).take(limit as u64 + 2);
        let n = timeout(READ_TIMEOUT, taken.read_until(b'\n', &mut raw))
            .await
            .map_err(|_| SyncError::Timeout)?
            .map_err(|e| SyncError::Server(e.to_string()))?;
        if n == 0 {
            return Err(closed());
        }
        if !raw.ends_with(b"\n") {
            return Err(if n > limit { too_long() } else { closed() });
        }
        raw.pop();
        if raw.ends_with(b"\r") {
            raw.pop();
        }
        Ok(raw)
    }

    /// "+OK …" (its text) or "-ERR …" (its text as the error).
    async fn status(&mut self) -> Result<Result<String, String>, SyncError> {
        let line = self.line(STATUS_LINE).await?;
        let text = String::from_utf8_lossy(&line);
        if let Some(rest) = text.strip_prefix("+OK") {
            Ok(Ok(rest.trim().to_owned()))
        } else if let Some(rest) = text.strip_prefix("-ERR") {
            Ok(Err(rest.trim().to_owned()))
        } else {
            Err(SyncError::Server(tr!(
                "it answered something that isn't POP3 ({text})",
                "odpowiedział czymś, co nie jest POP3 ({text})"
            )))
        }
    }

    async fn send(&mut self, line: &str) -> Result<(), SyncError> {
        let stream = self.io.get_mut();
        timeout(READ_TIMEOUT, async {
            stream.write_all(line.as_bytes()).await?;
            stream.write_all(b"\r\n").await?;
            stream.flush().await
        })
        .await
        .map_err(|_| SyncError::Timeout)?
        .map_err(|_| closed())
    }

    /// A command and its one-line answer: the text after "+OK", or the "-ERR" as an error.
    async fn ask(&mut self, line: &str) -> Result<String, SyncError> {
        self.send(line).await?;
        self.status().await?.map_err(|text| refusal(&text, false))
    }

    /// The lines of a multi-line answer up to the line with a single dot, the leading dot
    /// taken off the others (dot-unstuffing), each at most `line_limit` bytes and all of them
    /// at most `total` bytes and [`MAX_LISTING`] lines.
    async fn lines(&mut self, line_limit: usize, total: u64) -> Result<Vec<Vec<u8>>, SyncError> {
        let mut out = Vec::new();
        let mut size = 0u64;
        loop {
            let mut line = self.line(line_limit).await?;
            if line == b"." {
                return Ok(out);
            }
            if line.first() == Some(&b'.') {
                line.remove(0);
            }
            size += line.len() as u64 + 2;
            if size > total || out.len() >= MAX_LISTING {
                return Err(too_long());
            }
            out.push(line);
        }
    }

    /// Read the greeting. A server that greets with -ERR doesn't want this visitor now.
    pub async fn greet(&mut self) -> Result<(), SyncError> {
        let first = timeout(CONNECT_TIMEOUT, self.status())
            .await
            .map_err(|_| SyncError::Timeout)??;
        first.map(|_| ()).map_err(|reason| {
            SyncError::Server(tr!(
                "it said goodbye before logging in ({reason})",
                "pożegnał się przed logowaniem ({reason})"
            ))
        })
    }

    /// Ask what the server can do. An old server without CAPA says -ERR: then nothing is known.
    pub async fn capa(&mut self) -> Result<(), SyncError> {
        self.send("CAPA").await?;
        let mut caps = Caps::default();
        if self.status().await?.is_ok() {
            caps.answered = true;
            for line in self.lines(STATUS_LINE, 64 * 1024).await? {
                let line = String::from_utf8_lossy(&line);
                let mut words = line.split_whitespace();
                match words.next().map(str::to_ascii_uppercase).as_deref() {
                    Some("UIDL") => caps.uidl = true,
                    Some("USER") => caps.user = true,
                    Some("STLS") => caps.stls = true,
                    Some("SASL") => caps.sasl = words.map(str::to_owned).collect(),
                    _ => {}
                }
            }
        }
        self.caps = caps;
        Ok(())
    }

    /// Ask for STLS and hand back the stream for the TLS handshake. Nothing may follow the
    /// server's +OK in the plain part: what did would be read as if it came encrypted.
    pub async fn stls(mut self, host: &str) -> Result<S, SyncError> {
        let refused = |reason: String| SyncError::Tls {
            host: host.to_owned(),
            reason: tr!(
                "it doesn't offer STLS ({reason}). Try SSL/TLS on port 995",
                "nie obsługuje STLS ({reason}). Spróbuj SSL/TLS na porcie 995"
            ),
        };
        self.send("STLS").await?;
        self.status().await?.map_err(refused)?;
        if !self.io.buffer().is_empty() {
            return Err(SyncError::Tls {
                host: host.to_owned(),
                reason: tr!(
                    "it sent more than it should before encrypting",
                    "wysłał więcej, niż powinien, przed szyfrowaniem"
                ),
            });
        }
        Ok(self.io.into_inner())
    }

    /// Log in: AUTH PLAIN where the server offers it, else USER and PASS.
    pub async fn login(&mut self, user: &str, password: &str) -> Result<(), SyncError> {
        if self.caps.plain() {
            self.send("AUTH PLAIN").await?;
            let line = self.line(STATUS_LINE).await?;
            if line.starts_with(b"+ ") || line == b"+" {
                let token = base64::engine::general_purpose::STANDARD
                    .encode(format!("\0{user}\0{password}"));
                self.send(&token).await?;
                return self
                    .status()
                    .await?
                    .map(|_| ())
                    .map_err(|text| refusal(&text, true));
            }
            // Refused as a way of logging in (not the password): USER and PASS then.
            if !line.starts_with(b"-ERR") {
                return Err(SyncError::Server(
                    String::from_utf8_lossy(&line).into_owned(),
                ));
            }
        }
        self.send(&format!("USER {user}")).await?;
        self.status().await?.map_err(|text| refusal(&text, true))?;
        self.send(&format!("PASS {password}")).await?;
        self.status()
            .await?
            .map(|_| ())
            .map_err(|text| refusal(&text, true))
    }

    /// Every message's number and UIDL. A server without UIDL can't be used.
    pub async fn uidls(&mut self) -> Result<Vec<(u32, String)>, SyncError> {
        self.send("UIDL").await?;
        if self.status().await?.is_err() {
            return Err(no_uidl());
        }
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for line in self.lines(STATUS_LINE, 128 * 1024 * 1024).await? {
            let line = String::from_utf8_lossy(&line);
            let mut words = line.split_whitespace();
            let (Some(n), Some(uidl)) = (words.next(), words.next()) else {
                continue;
            };
            let Ok(n) = n.parse::<u32>() else { continue };
            let printable = uidl.bytes().all(|b| (0x21..=0x7e).contains(&b));
            // Two messages with one UIDL are a server's mistake: the first one counts.
            if n > 0 && printable && uidl.len() <= MAX_UIDL && seen.insert(uidl.to_owned()) {
                out.push((n, uidl.to_owned()));
            }
        }
        Ok(out)
    }

    /// Every message's size in bytes, by number.
    pub async fn sizes(&mut self) -> Result<HashMap<u32, u64>, SyncError> {
        self.ask("LIST").await?;
        let mut out = HashMap::new();
        for line in self.lines(STATUS_LINE, 128 * 1024 * 1024).await? {
            let line = String::from_utf8_lossy(&line);
            let mut words = line.split_whitespace();
            if let (Some(Ok(n)), Some(Ok(size))) = (
                words.next().map(str::parse::<u32>),
                words.next().map(str::parse::<u64>),
            ) {
                out.insert(n, size);
            }
        }
        Ok(out)
    }

    /// One message, exactly as stored on the server, lines ending in CRLF.
    pub async fn retr(&mut self, n: u32) -> Result<Vec<u8>, SyncError> {
        self.ask(&format!("RETR {n}")).await?;
        let lines = self.lines(MESSAGE_LINE, MAX_MESSAGE).await?;
        let mut raw = Vec::with_capacity(lines.iter().map(|l| l.len() + 2).sum());
        for line in lines {
            raw.extend_from_slice(&line);
            raw.extend_from_slice(b"\r\n");
        }
        Ok(raw)
    }

    /// Mark a message to be removed; the server removes it at QUIT.
    pub async fn dele(&mut self, n: u32) -> Result<(), SyncError> {
        self.ask(&format!("DELE {n}")).await.map(|_| ())
    }

    /// End the session: only now does the server remove what DELE marked.
    pub async fn quit(&mut self) -> Result<(), SyncError> {
        self.ask("QUIT").await.map(|_| ())
    }
}

// ---- Connecting ----

/// Connect, encrypted from the first byte or after STLS, read the greeting, ask CAPA and log
/// in, through one of the server's places (`servers`) and into the connection log, as IMAP
/// does. Only the greeting, CAPA and STLS go unencrypted.
async fn connect_and_login(
    settings: &ImapSettings,
    password: &str,
    purpose: Purpose,
) -> Result<(Session<TlsStream<TcpStream>>, Turn), SyncError> {
    let host = &settings.host;
    let mut trace = Trace::new(purpose, "pop3", &settings.username, host, settings.port);
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
    let mut session = match connect(settings, turn.key(), &mut trace).await {
        Ok(session) => session,
        Err(e) => return Err(failed(trace, e)),
    };
    trace.begin(Step::Login);
    let logged_in = async {
        session.capa().await?;
        session.login(&settings.username, password).await
    }
    .await;
    if let Err(e) = logged_in {
        return Err(failed(trace, e));
    }
    trace.done();
    turn.opened(trace);
    Ok((session, turn))
}

fn failed(trace: Trace, e: SyncError) -> SyncError {
    trace.fail(&e.to_string());
    e
}

fn outcome<T>(turn: &mut Turn, result: &Result<T, SyncError>) {
    if let Err(e) = result {
        turn.failed(e);
    }
}

/// A connection with the greeting read, encrypted. A server that takes the connection but
/// doesn't greet (or greets with -ERR) rests before anything connects to it again.
async fn connect(
    settings: &ImapSettings,
    key: &str,
    trace: &mut Trace,
) -> Result<Session<TlsStream<TcpStream>>, SyncError> {
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
        let mut plain = Session::new(tcp);
        plain.greet().await.map_err(silent)?;
        let tcp = plain.stls(host).await?;
        trace.done();
        tcp
    } else {
        tcp
    };
    trace.begin(Step::Tls);
    let tls = timeout(CONNECT_TIMEOUT, imap::tls_connector().connect(name, tcp))
        .await
        .map_err(|_| silent(SyncError::Timeout))?
        .map_err(|e| SyncError::Tls {
            host: host.to_owned(),
            reason: e.to_string(),
        })?;
    trace.done();
    let mut session = Session::new(tls);
    // After STLS the server says nothing more; its greeting came before.
    if !starttls {
        trace.begin(Step::Greeting);
        session.greet().await.map_err(silent)?;
        trace.done();
    }
    SERVERS.answered(key);
    Ok(session)
}

// ---- What to download and remove ----

/// A message another program downloaded from a POP3 server: its UIDL, when it came, and
/// whether it was deleted there (to be removed from the server).
pub type Seen = (String, Option<NaiveDateTime>, bool);

/// What Telex knows about one UIDL of an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    pub state: State,
    /// The message here, while it is here.
    pub message: Option<MessageId>,
    pub downloaded: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Downloaded (`message` is NULL once deleted here for good).
    Here,
    /// Left on the server by the first check; downloaded on request.
    Older,
    /// Downloaded by another program (Thunderbird) before Telex.
    Known,
    /// Too big to download.
    Big,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            Self::Here => "here",
            Self::Older => "older",
            Self::Known => "known",
            Self::Big => "big",
        }
    }

    fn from_name(name: &str) -> Self {
        match name {
            "older" => Self::Older,
            "known" => Self::Known,
            "big" => Self::Big,
            _ => Self::Here,
        }
    }
}

/// What a check downloads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Message numbers and UIDLs to download, oldest first: an interrupted check goes on where
    /// it stopped, and the last one is the newest.
    pub fetch: Vec<(u32, String)>,
    /// What the first check leaves on the server, older than what it takes.
    pub older: Vec<String>,
    /// Too big to download, with their sizes.
    pub big: Vec<(String, u64)>,
}

/// Plan a check: what the server lists that Telex doesn't know yet. On the first check only
/// the newest `first_limit`; the rest wait as older mail.
pub fn plan(
    listing: &[(u32, String)],
    sizes: &HashMap<u32, u64>,
    known: &HashMap<String, Known>,
    first: bool,
    first_limit: usize,
) -> Plan {
    let mut plan = Plan::default();
    let mut new: Vec<(u32, String)> = Vec::new();
    for (n, uidl) in listing {
        if known.contains_key(uidl) {
            continue;
        }
        match sizes.get(n) {
            Some(&size) if size > MAX_MESSAGE => plan.big.push((uidl.clone(), size)),
            _ => new.push((*n, uidl.clone())),
        }
    }
    new.sort_by_key(|(n, _)| *n);
    if first && new.len() > first_limit {
        let older = new.len() - first_limit;
        plan.older = new.drain(..older).map(|(_, u)| u).collect();
    }
    plan.fetch = new;
    plan
}

/// Which listed messages to remove from the server now, by the account's [`Pop3Keep`]. Never
/// one Telex hasn't downloaded (older or too big mail).
pub fn to_remove(
    listing: &[(u32, String)],
    known: &HashMap<String, Known>,
    keep: &Pop3Keep,
    now: NaiveDateTime,
) -> Vec<u32> {
    listing
        .iter()
        .filter(|(_, uidl)| {
            let Some(k) = known.get(uidl) else {
                return false;
            };
            if !matches!(k.state, State::Here | State::Known) {
                return false;
            }
            if !keep.leave {
                return true;
            }
            let aged = keep.days.is_some_and(|days| {
                k.downloaded
                    .is_some_and(|at| at + TimeDelta::days(i64::from(days)) <= now)
            });
            let deleted_here = keep.until_deleted && k.state == State::Here && k.message.is_none();
            aged || deleted_here
        })
        .map(|(n, _)| *n)
        .collect()
}

// ---- The mail base's side ----

impl MailStore {
    /// Every UIDL Telex knows of an account.
    pub fn pop3_known(&self, account: &str) -> Result<HashMap<String, Known>, StoreError> {
        let mut stmt = self.db.prepare_cached(
            "SELECT uidl, state, message, downloaded FROM pop3_uidls WHERE account = ?1",
        )?;
        let rows = stmt.query_map([account], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Known {
                    state: State::from_name(&r.get::<_, String>(1)?),
                    message: r.get::<_, Option<i64>>(2)?.map(|id| id as MessageId),
                    downloaded: r.get(3)?,
                },
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The server listed these today: they aren't forgotten.
    fn pop3_listed(
        &mut self,
        account: &str,
        listing: &[(u32, String)],
        today: NaiveDate,
    ) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        {
            let mut update = tx.prepare_cached(
                "UPDATE pop3_uidls SET listed = ?3 WHERE account = ?1 AND uidl = ?2 AND listed <> ?3",
            )?;
            for (_, uidl) in listing {
                update.execute(params![account, uidl, today])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Remember UIDLs in a state without a message here (older or too big mail).
    fn pop3_note(
        &mut self,
        account: &str,
        uidls: &[&str],
        state: State,
        today: NaiveDate,
    ) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT OR IGNORE INTO pop3_uidls (account, uidl, state, listed) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for uidl in uidls {
                insert.execute(params![account, uidl, state.as_str(), today])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Store downloaded messages in the account's Inbox, each with its UIDL, in one
    /// transaction. A UIDL already here is skipped. Returns the new ids.
    pub fn pop3_add(
        &mut self,
        account: &str,
        batch: &[(String, Message, Vec<u8>)],
        now: NaiveDateTime,
    ) -> Result<Vec<MessageId>, StoreError> {
        let inbox = self
            .account(account)?
            .folder_of_kind(FolderKind::Inbox)
            .map(|f| format!("{account}/{}", f.id))
            .ok_or_else(|| StoreError::MissingFolder {
                account: account.to_owned(),
                kind: FolderKind::Inbox,
            })?;
        let tx = self.db.transaction()?;
        let mut ids = Vec::new();
        for (uidl, message, raw) in batch {
            let here: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM pop3_uidls WHERE account = ?1 AND uidl = ?2 AND state IN ('here', 'known'))",
                params![account, uidl],
                |r| r.get(0),
            )?;
            if here {
                continue;
            }
            let id = insert_message(&tx, &inbox, None, message, None, Some(raw))?;
            tx.execute(
                "INSERT INTO pop3_uidls (account, uidl, state, message, downloaded, listed)
                 VALUES (?1, ?2, 'here', ?3, ?4, ?5)
                 ON CONFLICT (account, uidl) DO UPDATE SET state = 'here',
                     message = excluded.message, downloaded = excluded.downloaded,
                     listed = excluded.listed",
                params![account, uidl, id as i64, now, now.date()],
            )?;
            ids.push(id);
        }
        tx.commit()?;
        Ok(ids)
    }

    /// The account's first check is over: from now on, new mail is mail that arrived.
    fn pop3_first_done(&mut self, account: &str) -> Result<(), StoreError> {
        self.db.execute(
            "UPDATE folders SET last_uid = 0 WHERE account = ?1 AND kind = 'inbox' AND last_uid IS NULL",
            [account],
        )?;
        Ok(())
    }

    /// Before the first check of a POP3 account.
    fn pop3_is_first(&self, account: &str) -> Result<bool, StoreError> {
        Ok(self
            .db
            .query_row(
                "SELECT last_uid IS NULL FROM folders WHERE account = ?1 AND kind = 'inbox'",
                [account],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(true))
    }

    /// How many older messages wait on the server (the status bar offers them), and forget
    /// UIDLs the server hasn't listed for a long time.
    fn pop3_tidy(
        &mut self,
        account: &str,
        listing: &[(u32, String)],
        today: NaiveDate,
    ) -> Result<(), StoreError> {
        let older: HashSet<String> = self
            .pop3_known(account)?
            .into_iter()
            .filter(|(_, k)| k.state == State::Older)
            .map(|(u, _)| u)
            .collect();
        let missing = listing.iter().filter(|(_, u)| older.contains(u)).count();
        let tx = self.db.transaction()?;
        tx.execute(
            "UPDATE folders SET missing = ?2 WHERE account = ?1 AND kind = 'inbox'",
            params![account, missing as i64],
        )?;
        tx.execute(
            "DELETE FROM pop3_uidls WHERE account = ?1 AND listed < ?2",
            params![account, today - TimeDelta::days(FORGET_AFTER_DAYS)],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// UIDLs another program downloaded (Thunderbird's popstate.dat), so Telex doesn't fetch
    /// that mail again: `(uidl, when, deleted)`. One it deleted (Thunderbird's "d") counts as
    /// deleted here; one whose message came along (an X-UIDL header) is linked to it, so
    /// deleting it here removes it from the server too. UIDLs Telex knows already stay.
    pub fn pop3_mark_known(
        &mut self,
        account: &str,
        seen: &[Seen],
        today: NaiveDate,
    ) -> Result<usize, StoreError> {
        let linked = self.pop3_x_uidls(account)?;
        let tx = self.db.transaction()?;
        let mut added = 0;
        {
            let mut insert = tx.prepare_cached(
                "INSERT OR IGNORE INTO pop3_uidls (account, uidl, state, message, downloaded, listed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for (uidl, at, deleted) in seen {
                let message = linked.get(uidl).copied();
                let state = if *deleted || message.is_some() {
                    State::Here
                } else {
                    State::Known
                };
                let message = message.filter(|_| !deleted).map(|id| id as i64);
                added +=
                    insert.execute(params![account, uidl, state.as_str(), message, at, today])?;
            }
        }
        tx.commit()?;
        Ok(added)
    }

    /// The X-UIDL headers of the account's mail (Thunderbird writes one into each message it
    /// leaves on the server), from the start of each message.
    fn pop3_x_uidls(&self, account: &str) -> Result<HashMap<String, MessageId>, StoreError> {
        let mut stmt = self.db.prepare(
            "SELECT m.id, substr(c.raw, 1, 65536) FROM messages m JOIN message_content c ON c.id = m.id
             WHERE m.folder >= ?1 || '/' AND m.folder < ?1 || '0' AND c.raw IS NOT NULL",
        )?;
        let rows = stmt.query_map([account], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        let mut found = HashMap::new();
        for row in rows {
            let (id, head) = row?;
            if let Some(uidl) = x_uidl(&head) {
                found.entry(uidl).or_insert(id as MessageId);
            }
        }
        Ok(found)
    }
}

/// The X-UIDL header in a message's headers.
fn x_uidl(head: &[u8]) -> Option<String> {
    for line in head.split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            return None; // the end of the headers
        }
        if line.len() > 7 && line[..7].eq_ignore_ascii_case(b"X-UIDL:") {
            let value = String::from_utf8_lossy(&line[7..]).trim().to_owned();
            return (!value.is_empty()).then_some(value);
        }
    }
    None
}

// ---- Checking ----

/// Log in once to check the settings: a server without UIDL is refused here, before the
/// account is saved.
pub async fn check_account(settings: &ImapSettings, password: &str) -> Result<(), SyncError> {
    let (mut session, mut turn) = connect_and_login(settings, password, Purpose::Test).await?;
    let result = session.uidls().await.map(|_| ());
    let _ = session.quit().await;
    outcome(&mut turn, &result);
    result
}

/// A check of a POP3 account: download what is new into its Inbox, remove from the server
/// what its [`Pop3Keep`] says, then the spam filter, rules and away reply on what arrived.
pub async fn sync_account(
    account: &Account,
    password: &str,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<SyncReport, SyncError> {
    let Some(settings) = &account.imap else {
        return Ok(SyncReport::default());
    };
    let (mut session, mut turn) = connect_and_login(settings, password, Purpose::Check).await?;
    let result = sync_session(&mut session, account, store, progress).await;
    outcome(&mut turn, &result);
    result
}

/// [`sync_account`] over a session that is logged in. It ends the session (QUIT), which is
/// when the server removes what was marked.
pub async fn sync_session<S: AsyncRead + AsyncWrite + Unpin + Send>(
    session: &mut Session<S>,
    account: &Account,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<SyncReport, SyncError> {
    let keep = account
        .imap
        .as_ref()
        .and_then(|i| i.pop3)
        .unwrap_or_default();
    let mut report = SyncReport::default();
    let downloaded = download(session, account, store, progress, &keep, &mut report).await;
    // What came down before a failure is here: the rules still see it.
    let arrived = match &downloaded {
        Ok(d) => d.arrived.clone(),
        Err((d, _)) => d.arrived.clone(),
    };
    let new = match &downloaded {
        Ok(d) => d.new.clone(),
        Err((d, _)) => d.new.clone(),
    };
    report.new = new;
    lock(store)?.set_quota(&account.id, None)?;
    imap::sort_arrived(store, &account.id, arrived, &mut report)?;
    match downloaded {
        Ok(_) => Ok(report),
        Err((_, e)) => Err(e),
    }
}

/// Download and remove, then QUIT. On a failure, what was stored so far comes with the error.
async fn download<S: AsyncRead + AsyncWrite + Unpin + Send>(
    session: &mut Session<S>,
    account: &Account,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
    keep: &Pop3Keep,
    report: &mut SyncReport,
) -> Result<Downloaded, (Downloaded, SyncError)> {
    let mut downloaded = Downloaded::default();
    macro_rules! go {
        ($e:expr) => {
            match $e {
                Ok(v) => v,
                Err(e) => return Err((downloaded, SyncError::from(e))),
            }
        };
    }
    let listing = go!(session.uidls().await);
    let sizes = go!(session.sizes().await);
    let now = Local::now().naive_local();
    let today = now.date();
    let (first, plan) = {
        let mut s = go!(lock(store));
        let first = go!(s.pop3_is_first(&account.id));
        let known = go!(s.pop3_known(&account.id));
        let plan = plan(&listing, &sizes, &known, first, FIRST_SYNC_LIMIT);
        go!(s.pop3_listed(&account.id, &listing, today));
        let older: Vec<&str> = plan.older.iter().map(String::as_str).collect();
        go!(s.pop3_note(&account.id, &older, State::Older, today));
        let big: Vec<&str> = plan.big.iter().map(|(u, _)| u.as_str()).collect();
        go!(s.pop3_note(&account.id, &big, State::Big, today));
        (first, plan)
    };
    for (_, size) in &plan.big {
        let mb = size.div_ceil(1024 * 1024);
        report.refused.push(tr!(
            "a message of {mb} MB is too big to download; it stays on the server",
            "wiadomość o rozmiarze {mb} MB jest za duża do pobrania; zostaje na serwerze"
        ));
    }
    let inbox_key = account
        .folder_of_kind(FolderKind::Inbox)
        .map(|f| account.folder_key(&f.id))
        .unwrap_or_default();
    go!(fetch(
        session,
        account,
        store,
        progress,
        &plan.fetch,
        &inbox_key,
        first,
        &mut downloaded
    )
    .await);
    if first {
        go!(go!(lock(store)).pop3_first_done(&account.id));
    }

    // Removing: what the account keeps no longer, now that the new mail is stored.
    let remove = {
        let s = go!(lock(store));
        let known = go!(s.pop3_known(&account.id));
        to_remove(&listing, &known, keep, now)
    };
    for n in remove {
        // One the server won't remove (gone meanwhile) doesn't stop the others.
        if let Err(SyncError::Server(_)) = session.dele(n).await {
            continue;
        }
    }
    if session.quit().await.is_err() {
        report.refused.push(tr!(
            "the server didn't confirm the end of the check; mail to be removed from it stays there until the next one",
            "serwer nie potwierdził końca sprawdzania; poczta do usunięcia z niego zostaje tam do następnego"
        ));
    }
    go!(go!(lock(store)).pop3_tidy(&account.id, &listing, today));
    Ok(downloaded)
}

/// Download `wanted` into the Inbox, a batch per transaction, with progress events. On the
/// first check nothing counts as arrived: that is old mail.
#[allow(clippy::too_many_arguments)]
async fn fetch<S: AsyncRead + AsyncWrite + Unpin + Send>(
    session: &mut Session<S>,
    account: &Account,
    store: &Mutex<MailStore>,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
    wanted: &[(u32, String)],
    inbox_key: &str,
    old: bool,
    downloaded: &mut Downloaded,
) -> Result<(), SyncError> {
    let total = wanted.len();
    let folder = account
        .folder_of_kind(FolderKind::Inbox)
        .map(|f| f.name.clone())
        .unwrap_or_default();
    let report = |done| {
        progress(SyncProgress {
            account: account.name.clone(),
            folder: folder.clone(),
            key: inbox_key.to_owned(),
            done,
            total,
        });
    };
    if total > 0 {
        report(0);
    }
    let mut done = 0;
    for chunk in wanted.chunks(FETCH_BATCH) {
        let mut batch = Vec::with_capacity(chunk.len());
        for (n, uidl) in chunk {
            let raw = session.retr(*n).await?;
            let message = parse_message(&raw, inbox_key, Some(Local::now().naive_local()));
            batch.push((uidl.clone(), message, raw));
        }
        let ids = lock(store)?.pop3_add(&account.id, &batch, Local::now().naive_local())?;
        if !old {
            downloaded.arrived.extend(&ids);
        }
        downloaded.new.extend(ids);
        done += chunk.len();
        report(done);
    }
    Ok(())
}

/// Download up to `limit` of the newest messages the first check left on the server. They are
/// old mail: no rules run on them.
pub async fn download_older(
    account: &Account,
    password: &str,
    store: &Mutex<MailStore>,
    limit: usize,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<Vec<MessageId>, SyncError> {
    let Some(settings) = &account.imap else {
        return Ok(Vec::new());
    };
    let (mut session, mut turn) = connect_and_login(settings, password, Purpose::Older).await?;
    let result = download_older_in(&mut session, account, store, limit, progress).await;
    outcome(&mut turn, &result);
    result
}

/// [`download_older`] over a session that is logged in; it ends the session.
pub async fn download_older_in<S: AsyncRead + AsyncWrite + Unpin + Send>(
    session: &mut Session<S>,
    account: &Account,
    store: &Mutex<MailStore>,
    limit: usize,
    progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> Result<Vec<MessageId>, SyncError> {
    let listing = session.uidls().await?;
    let known = lock(store)?.pop3_known(&account.id)?;
    let mut wanted: Vec<(u32, String)> = listing
        .iter()
        .filter(|(_, u)| known.get(u).is_some_and(|k| k.state == State::Older))
        .cloned()
        .collect();
    wanted.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    wanted.truncate(limit);
    wanted.reverse();
    let inbox_key = account
        .folder_of_kind(FolderKind::Inbox)
        .map(|f| account.folder_key(&f.id))
        .unwrap_or_default();
    let mut downloaded = Downloaded::default();
    let fetched = fetch(
        session,
        account,
        store,
        progress,
        &wanted,
        &inbox_key,
        true,
        &mut downloaded,
    )
    .await;
    let _ = session.quit().await;
    fetched?;
    lock(store)?.pop3_tidy(&account.id, &listing, Local::now().date_naive())?;
    Ok(downloaded.new)
}

fn lock(store: &Mutex<MailStore>) -> Result<std::sync::MutexGuard<'_, MailStore>, SyncError> {
    store
        .lock()
        .map_err(|_| SyncError::Store(StoreError::poisoned()))
}

#[cfg(test)]
mod tests;
