//! How Telex treats one mail server: a few connections at a time, a rest after it stops
//! answering, and a log of the last connections for Settings › Connections.
//!
//! A server sees every account Telex has on it as one visitor (one IP address), and some
//! providers stop greeting an address that opens too many connections: the connection and
//! its TLS go through, then nothing. Thirty accounts on Purelymail checked all at once, each
//! with its own IDLE connection, and each tried again after every failure, kept that going
//! for good. So connections to one server wait for one of a few places, and a server that
//! stopped answering gets a rest that grows with each silence, during which nothing connects.
//!
//! The log is in memory only, since Telex started: the last [`LOG_SIZE`] connections, each
//! with its steps timed, where it failed and why. Nothing of it is written to disk or sent.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{Local, TimeDelta};
use serde::Serialize;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Checks, pushes and logins open at once to one server.
pub const SESSIONS_PER_SERVER: usize = 4;
/// Inboxes watched with IDLE on one server; the other accounts there get new mail from the
/// regular checks.
pub const WATCHES_PER_SERVER: usize = 8;
/// Connections the log keeps: a few rounds of checks of forty accounts.
pub const LOG_SIZE: usize = 1000;
const FIRST_REST: Duration = Duration::from_secs(60);
const LONGEST_REST: Duration = Duration::from_secs(15 * 60);
/// Looking up a server's address shouldn't hold a check up longer than connecting would.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

/// What a connection is for: the two kinds have places of their own, so watching Inboxes
/// never keeps a check from starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    Session,
    Watch,
}

/// What a connection was for, as the Connections tab names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// Downloading new mail (and sending queued changes).
    Check,
    /// Only sending queued changes.
    Push,
    /// Downloading older mail on request.
    Older,
    /// Logging in once to check an account's settings.
    Test,
    /// Waiting in IDLE for news in the Inbox.
    Watch,
    /// Handing a message to the SMTP server.
    Send,
}

impl Purpose {
    pub fn places(self) -> Use {
        match self {
            Self::Watch => Use::Watch,
            _ => Use::Session,
        }
    }
}

/// A step of a connection; the UI words them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// Finding the server's address (DNS).
    Lookup,
    /// Waiting for one of the server's places.
    Wait,
    /// TCP.
    Connect,
    /// The plain greeting and STARTTLS, before TLS on port 143.
    Starttls,
    /// The TLS handshake.
    Tls,
    /// The server's greeting.
    Greeting,
    Login,
    /// What the connection was for, once logged in.
    Work,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Timed {
    pub step: Step,
    pub ms: u64,
}

/// One connection in the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attempt {
    pub id: u64,
    /// Local `YYYY-MM-DDTHH:MM:SS`, like every date that crosses the bridge.
    pub started: String,
    pub purpose: Purpose,
    /// "imap" or "smtp".
    pub protocol: &'static str,
    /// The user name it logs in with (the window finds the account by it).
    pub user: String,
    pub host: String,
    pub port: u16,
    /// The IP address the server is known by, or its name when it couldn't be looked up;
    /// empty when nobody looked (sending).
    pub address: String,
    /// The steps it went through, each with how long it took.
    pub steps: Vec<Timed>,
    /// The step it failed at and why; none while it is open or when it went well.
    pub failed: Option<Step>,
    pub error: Option<String>,
    /// When it ended; none while it is open (an Inbox being watched, a check under way).
    pub ended: Option<String>,
}

fn now_text() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The log of the last connections.
pub struct Log {
    entries: Mutex<VecDeque<Attempt>>,
    next: AtomicU64,
}

/// The app's log.
pub static LOG: LazyLock<Log> = LazyLock::new(|| Log {
    entries: Mutex::new(VecDeque::new()),
    next: AtomicU64::new(1),
});

impl Log {
    fn entries(&self) -> MutexGuard<'_, VecDeque<Attempt>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn push(&self, attempt: Attempt) {
        let mut entries = self.entries();
        if entries.len() >= LOG_SIZE {
            entries.pop_front();
        }
        entries.push_back(attempt);
    }

    fn update(&self, id: u64, f: impl FnOnce(&mut Attempt)) {
        if let Some(a) = self.entries().iter_mut().rev().find(|a| a.id == id) {
            f(a);
        }
    }

    /// Every connection kept, newest first.
    pub fn newest_first(&self) -> Vec<Attempt> {
        self.entries().iter().rev().cloned().collect()
    }

    /// A connection that is over in one go, without steps of its own (sending through lettre,
    /// which doesn't say how far it got).
    pub fn once(
        &self,
        purpose: Purpose,
        user: &str,
        (host, port): (&str, u16),
        started: Instant,
        error: Option<String>,
    ) {
        let mut trace = Trace::new(purpose, "smtp", user, host, port);
        trace.started = (Local::now() - TimeDelta::from_std(started.elapsed()).unwrap_or_default())
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        trace.mark = started;
        trace.at = Step::Work;
        match error {
            Some(e) => trace.fail(&e),
            None => {
                trace.done();
                let mut attempt = trace.attempt();
                attempt.ended = Some(now_text());
                self.push(attempt);
            }
        }
    }
}

/// A connection being made: the steps so far, timed. It goes into the log when it fails
/// ([`Trace::fail`]) or once logged in ([`Turn::opened`]).
#[derive(Debug)]
pub struct Trace {
    started: String,
    purpose: Purpose,
    protocol: &'static str,
    user: String,
    host: String,
    port: u16,
    address: String,
    steps: Vec<Timed>,
    at: Step,
    mark: Instant,
}

impl Trace {
    pub fn new(
        purpose: Purpose,
        protocol: &'static str,
        user: &str,
        host: &str,
        port: u16,
    ) -> Self {
        Self {
            started: now_text(),
            purpose,
            protocol,
            user: user.to_owned(),
            host: host.to_owned(),
            port,
            address: String::new(),
            steps: Vec::new(),
            at: Step::Lookup,
            mark: Instant::now(),
        }
    }

    pub fn address(&mut self, address: &str) {
        address.clone_into(&mut self.address);
    }

    /// The next step starts now.
    pub fn begin(&mut self, step: Step) {
        self.at = step;
        self.mark = Instant::now();
    }

    /// The step under way went through.
    pub fn done(&mut self) {
        self.steps.push(Timed {
            step: self.at,
            ms: ms(self.mark),
        });
    }

    fn attempt(self) -> Attempt {
        Attempt {
            id: LOG.next.fetch_add(1, Ordering::Relaxed),
            started: self.started,
            purpose: self.purpose,
            protocol: self.protocol,
            user: self.user,
            host: self.host,
            port: self.port,
            address: self.address,
            steps: self.steps,
            failed: None,
            error: None,
            ended: None,
        }
    }

    /// The step under way failed: into the log with why.
    pub fn fail(mut self, error: &str) {
        self.done();
        let failed = self.at;
        let mut attempt = self.attempt();
        attempt.failed = Some(failed);
        attempt.error = Some(error.to_owned());
        attempt.ended = Some(now_text());
        LOG.push(attempt);
    }
}

/// One of a server's places, held until it is dropped (with the connection it was for).
/// Once the connection is logged in it carries its line in the log, which is closed when
/// the turn is dropped: with the error given to [`Turn::failed`], if any.
#[derive(Debug)]
pub struct Turn {
    key: String,
    logged: Option<(u64, Instant)>,
    error: Option<String>,
    _place: OwnedSemaphorePermit,
}

impl Turn {
    /// The server this turn is on, for [`Servers::answered`] and [`Servers::silent`].
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Logged in: the connection goes into the log, open, and its work starts.
    pub fn opened(&mut self, trace: Trace) {
        let attempt = trace.attempt();
        self.logged = Some((attempt.id, Instant::now()));
        LOG.push(attempt);
    }

    /// What the connection was for went wrong.
    pub fn failed(&mut self, error: &impl std::fmt::Display) {
        self.error = Some(error.to_string());
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        let Some((id, since)) = self.logged else {
            return;
        };
        let error = self.error.take();
        LOG.update(id, |a| {
            a.steps.push(Timed {
                step: Step::Work,
                ms: ms(since),
            });
            a.failed = error.is_some().then_some(Step::Work);
            a.error = error;
            a.ended = Some(now_text());
        });
    }
}

struct Server {
    sessions: Arc<Semaphore>,
    watches: Arc<Semaphore>,
    /// The names it was reached by.
    names: BTreeSet<String>,
    /// Silences in a row, and the end of the rest the last one started.
    silent: u32,
    until: Option<Instant>,
}

/// A server as Settings › Connections shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServerState {
    pub address: String,
    pub names: Vec<String>,
    /// Places in use and how many there are, for checks and for watching.
    pub sessions: [usize; 2],
    pub watches: [usize; 2],
    /// Silences in a row, and until when it rests (local time), if it does.
    pub silences: u32,
    pub resting_until: Option<String>,
}

/// Every server Telex has connected to since it started.
pub struct Servers {
    sessions: usize,
    watches: usize,
    known: Mutex<HashMap<String, Server>>,
}

/// The app's servers.
pub static SERVERS: LazyLock<Servers> =
    LazyLock::new(|| Servers::new(SESSIONS_PER_SERVER, WATCHES_PER_SERVER));

impl Servers {
    pub fn new(sessions: usize, watches: usize) -> Self {
        Self {
            sessions,
            watches,
            known: Mutex::new(HashMap::new()),
        }
    }

    fn known(&self) -> MutexGuard<'_, HashMap<String, Server>> {
        self.known.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn with<R>(&self, key: &str, f: impl FnOnce(&mut Server) -> R) -> R {
        let mut known = self.known();
        let server = known.entry(key.to_owned()).or_insert_with(|| Server {
            sessions: Arc::new(Semaphore::new(self.sessions)),
            watches: Arc::new(Semaphore::new(self.watches)),
            names: BTreeSet::new(),
            silent: 0,
            until: None,
        });
        f(server)
    }

    /// How long the server's rest still lasts, if it is resting.
    pub fn resting(&self, key: &str) -> Option<Duration> {
        let until = self.with(key, |s| s.until)?;
        until.checked_duration_since(Instant::now())
    }

    /// Wait for one of the server's places. `Err` with what is left of its rest when it is
    /// resting, before the wait or after it. `host` is the name it is reached by this time.
    pub async fn turn(&self, key: &str, host: &str, kind: Use) -> Result<Turn, Duration> {
        if let Some(left) = self.resting(key) {
            return Err(left);
        }
        let places = self.with(key, |s| {
            s.names.insert(host.to_ascii_lowercase());
            match kind {
                Use::Session => s.sessions.clone(),
                Use::Watch => s.watches.clone(),
            }
        });
        // The semaphores are never closed, so the wait always ends with a place.
        let place = places
            .acquire_owned()
            .await
            .expect("a server's places are never closed");
        if let Some(left) = self.resting(key) {
            return Err(left);
        }
        Ok(Turn {
            key: key.to_owned(),
            logged: None,
            error: None,
            _place: place,
        })
    }

    /// The server greeted: its rests start short again. Also the user's "Try now".
    pub fn answered(&self, key: &str) {
        self.with(key, |s| {
            s.silent = 0;
            s.until = None;
        });
    }

    /// The server took the connection but didn't greet (or said goodbye at once): nothing
    /// connects to it until a rest is over, longer each time it happens in a row.
    pub fn silent(&self, key: &str) {
        self.with(key, |s| {
            s.silent += 1;
            s.until = Some(Instant::now() + rest_after(s.silent));
        });
    }

    /// Every server reached since Telex started, by address.
    pub fn states(&self) -> Vec<ServerState> {
        let now = Instant::now();
        let mut states: Vec<ServerState> =
            self.known()
                .iter()
                .map(|(address, s)| ServerState {
                    address: address.clone(),
                    names: s.names.iter().cloned().collect(),
                    sessions: [
                        self.sessions - s.sessions.available_permits(),
                        self.sessions,
                    ],
                    watches: [self.watches - s.watches.available_permits(), self.watches],
                    silences: s.silent,
                    resting_until: s.until.and_then(|u| u.checked_duration_since(now)).map(
                        |left| {
                            (Local::now() + TimeDelta::from_std(left).unwrap_or_default())
                                .format("%Y-%m-%dT%H:%M:%S")
                                .to_string()
                        },
                    ),
                })
                .collect();
        states.sort_by(|a, b| {
            a.names
                .cmp(&b.names)
                .then_with(|| a.address.cmp(&b.address))
        });
        states
    }
}

/// The rest after so many silences in a row: a minute, doubling, at most a quarter of an hour.
pub fn rest_after(silences: u32) -> Duration {
    let doublings = silences.saturating_sub(1).min(10);
    (FIRST_REST * (1 << doublings)).min(LONGEST_REST)
}

/// What a server is known by: its address, as its limits count, so two names for one machine
/// (imap.purelymail.com is mailserver.purelymail.com) share their places; its name when it
/// can't be looked up (connecting will say why).
pub async fn key(host: &str, port: u16) -> String {
    let found = tokio::time::timeout(LOOKUP_TIMEOUT, tokio::net::lookup_host((host, port))).await;
    match found {
        Ok(Ok(addresses)) => addresses
            .map(|a| a.ip())
            .min()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| host.to_ascii_lowercase()),
        _ => host.to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rests_grow_to_a_quarter_of_an_hour() {
        assert_eq!(rest_after(1), Duration::from_secs(60));
        assert_eq!(rest_after(2), Duration::from_secs(120));
        assert_eq!(rest_after(4), Duration::from_secs(480));
        assert_eq!(rest_after(5), Duration::from_secs(900));
        assert_eq!(rest_after(u32::MAX), Duration::from_secs(900));
    }

    #[tokio::test]
    async fn a_server_has_a_few_places_of_each_kind() {
        let servers = Servers::new(2, 1);
        let wait = Duration::from_millis(50);
        let turn = |kind| servers.turn("10.0.0.1", "imap.x.pl", kind);
        let a = turn(Use::Session).await.unwrap();
        let _b = turn(Use::Session).await.unwrap();
        // The third check waits for a place…
        assert!(tokio::time::timeout(wait, turn(Use::Session))
            .await
            .is_err());
        // …but watching has places of its own, and another server is another server.
        let _w = turn(Use::Watch).await.unwrap();
        assert!(tokio::time::timeout(wait, turn(Use::Watch)).await.is_err());
        let _other = servers
            .turn("10.0.0.2", "Mail.Y.pl", Use::Session)
            .await
            .unwrap();
        let states = servers.states();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].address, "10.0.0.1");
        assert_eq!((states[0].sessions, states[0].watches), ([2, 2], [1, 1]));
        assert_eq!(states[1].names, ["mail.y.pl"]);
        // A connection that ends gives its place to the next.
        drop(a);
        let third = tokio::time::timeout(wait, turn(Use::Session)).await;
        assert_eq!(third.unwrap().unwrap().key(), "10.0.0.1");
    }

    #[tokio::test]
    async fn a_silent_server_rests_until_it_answers() {
        let servers = Servers::new(4, 4);
        assert!(servers.resting("10.0.0.1").is_none());
        servers.silent("10.0.0.1");
        let left = servers.resting("10.0.0.1").unwrap();
        assert!(left > Duration::from_secs(55) && left <= Duration::from_secs(60));
        let refused = servers.turn("10.0.0.1", "a", Use::Watch).await.unwrap_err();
        assert!(refused <= Duration::from_secs(60));
        assert!(servers.turn("10.0.0.2", "b", Use::Session).await.is_ok());
        let resting = servers
            .states()
            .into_iter()
            .find(|s| s.address == "10.0.0.1");
        let resting = resting.unwrap();
        assert_eq!(resting.silences, 1);
        assert!(resting.resting_until.is_some());
        // A second silence in a row rests longer.
        servers.silent("10.0.0.1");
        assert!(servers.resting("10.0.0.1").unwrap() > Duration::from_secs(115));
        // An answer (a greeting after the rest) ends it and starts the count again.
        servers.answered("10.0.0.1");
        assert!(servers.resting("10.0.0.1").is_none());
        assert!(servers.turn("10.0.0.1", "a", Use::Session).await.is_ok());
        servers.silent("10.0.0.1");
        assert!(servers.resting("10.0.0.1").unwrap() <= Duration::from_secs(60));
    }

    #[tokio::test]
    async fn two_names_for_one_address_are_one_server() {
        assert_eq!(key("127.0.0.1", 993).await, "127.0.0.1");
        assert_eq!(key("LOCALHOST", 993).await, key("localhost", 993).await);
        assert_eq!(
            key("no-such-host.invalid", 993).await,
            "no-such-host.invalid"
        );
    }

    fn logged(user: &str) -> Vec<Attempt> {
        LOG.newest_first()
            .into_iter()
            .filter(|a| a.user == user)
            .collect()
    }

    #[tokio::test]
    async fn the_log_says_where_a_connection_failed() {
        let mut trace = Trace::new(Purpose::Check, "imap", "log-fail@x.pl", "imap.x.pl", 993);
        trace.address("10.9.9.9");
        trace.done();
        trace.begin(Step::Connect);
        trace.done();
        trace.begin(Step::Greeting);
        trace.fail("the server stopped answering");
        let [a] = logged("log-fail@x.pl").try_into().unwrap();
        let steps: Vec<Step> = a.steps.iter().map(|s| s.step).collect();
        assert_eq!(steps, [Step::Lookup, Step::Connect, Step::Greeting]);
        assert_eq!(a.failed, Some(Step::Greeting));
        assert_eq!(a.error.as_deref(), Some("the server stopped answering"));
        assert_eq!((a.address.as_str(), a.port), ("10.9.9.9", 993));
        assert!(a.ended.is_some());
    }

    #[tokio::test]
    async fn a_connection_is_open_in_the_log_until_its_turn_ends() {
        let servers = Servers::new(1, 1);
        let mut turn = servers
            .turn("10.9.9.8", "imap.x.pl", Use::Watch)
            .await
            .unwrap();
        turn.opened(Trace::new(
            Purpose::Watch,
            "imap",
            "log-open@x.pl",
            "imap.x.pl",
            993,
        ));
        let [open] = logged("log-open@x.pl").try_into().unwrap();
        assert_eq!((open.ended, open.failed), (None, None));
        turn.failed(&"the connection was closed");
        drop(turn);
        let [closed] = logged("log-open@x.pl").try_into().unwrap();
        assert_eq!(closed.failed, Some(Step::Work));
        assert_eq!(closed.error.as_deref(), Some("the connection was closed"));
        assert!(closed.ended.is_some());
        // A turn that ends well closes its line without an error.
        let mut turn = servers
            .turn("10.9.9.8", "imap.x.pl", Use::Session)
            .await
            .unwrap();
        turn.opened(Trace::new(
            Purpose::Check,
            "imap",
            "log-good@x.pl",
            "imap.x.pl",
            993,
        ));
        drop(turn);
        let [good] = logged("log-good@x.pl").try_into().unwrap();
        assert_eq!((good.failed, good.error), (None, None));
        assert_eq!(good.steps.last().map(|s| s.step), Some(Step::Work));
    }

    #[test]
    fn a_send_is_one_line() {
        let started = Instant::now();
        LOG.once(
            Purpose::Send,
            "log-send@x.pl",
            ("smtp.x.pl", 465),
            started,
            None,
        );
        LOG.once(
            Purpose::Send,
            "log-send@x.pl",
            ("smtp.x.pl", 465),
            started,
            Some("refused".into()),
        );
        let [refused, sent] = logged("log-send@x.pl").try_into().unwrap();
        assert_eq!((sent.protocol, sent.failed), ("smtp", None));
        assert_eq!(refused.failed, Some(Step::Work));
        assert!(refused.id > sent.id);
    }
}
