//! How Telex treats one mail server: a few connections at a time, and a rest after it stops
//! answering.
//!
//! A server sees every account Telex has on it as one visitor (one IP address), and some
//! providers stop greeting an address that opens too many connections: the connection and
//! its TLS go through, then nothing. Thirty accounts on Purelymail checked all at once, each
//! with its own IDLE connection, and each tried again after every failure, kept that going
//! for good. So connections to one server wait for one of a few places, and a server that
//! stopped answering gets a rest that grows with each silence, during which nothing connects.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Checks, pushes and logins open at once to one server.
pub const SESSIONS_PER_SERVER: usize = 4;
/// Inboxes watched with IDLE on one server; the other accounts there get new mail from the
/// regular checks.
pub const WATCHES_PER_SERVER: usize = 8;
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

/// One of a server's places, held until it is dropped (with the connection it was for).
#[derive(Debug)]
pub struct Turn {
    key: String,
    _place: OwnedSemaphorePermit,
}

impl Turn {
    /// The server this turn is on, for [`Servers::answered`] and [`Servers::silent`].
    pub fn key(&self) -> &str {
        &self.key
    }
}

struct Server {
    sessions: Arc<Semaphore>,
    watches: Arc<Semaphore>,
    /// Silences in a row, and the end of the rest the last one started.
    silent: u32,
    until: Option<Instant>,
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

    fn with<R>(&self, key: &str, f: impl FnOnce(&mut Server) -> R) -> R {
        let mut known = self.known.lock().unwrap_or_else(|e| e.into_inner());
        let server = known.entry(key.to_owned()).or_insert_with(|| Server {
            sessions: Arc::new(Semaphore::new(self.sessions)),
            watches: Arc::new(Semaphore::new(self.watches)),
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
    /// resting, before the wait or after it.
    pub async fn turn(&self, key: &str, kind: Use) -> Result<Turn, Duration> {
        if let Some(left) = self.resting(key) {
            return Err(left);
        }
        let places = self.with(key, |s| match kind {
            Use::Session => s.sessions.clone(),
            Use::Watch => s.watches.clone(),
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
            _place: place,
        })
    }

    /// The server greeted: its rests start short again.
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
        let a = servers.turn("10.0.0.1", Use::Session).await.unwrap();
        let _b = servers.turn("10.0.0.1", Use::Session).await.unwrap();
        // The third check waits for a place…
        let third = tokio::time::timeout(wait, servers.turn("10.0.0.1", Use::Session)).await;
        assert!(third.is_err());
        // …but watching has places of its own, and another server is another server.
        let _w = servers.turn("10.0.0.1", Use::Watch).await.unwrap();
        assert!(
            tokio::time::timeout(wait, servers.turn("10.0.0.1", Use::Watch))
                .await
                .is_err()
        );
        let _other = servers.turn("10.0.0.2", Use::Session).await.unwrap();
        // A connection that ends gives its place to the next.
        drop(a);
        let third = tokio::time::timeout(wait, servers.turn("10.0.0.1", Use::Session)).await;
        assert_eq!(third.unwrap().unwrap().key(), "10.0.0.1");
    }

    #[tokio::test]
    async fn a_silent_server_rests_until_it_answers() {
        let servers = Servers::new(4, 4);
        assert!(servers.resting("10.0.0.1").is_none());
        servers.silent("10.0.0.1");
        let left = servers.resting("10.0.0.1").unwrap();
        assert!(left > Duration::from_secs(55) && left <= Duration::from_secs(60));
        let refused = servers.turn("10.0.0.1", Use::Watch).await.unwrap_err();
        assert!(refused <= Duration::from_secs(60));
        assert!(servers.turn("10.0.0.2", Use::Session).await.is_ok());
        // A second silence in a row rests longer.
        servers.silent("10.0.0.1");
        assert!(servers.resting("10.0.0.1").unwrap() > Duration::from_secs(115));
        // An answer (a greeting after the rest) ends it and starts the count again.
        servers.answered("10.0.0.1");
        assert!(servers.resting("10.0.0.1").is_none());
        assert!(servers.turn("10.0.0.1", Use::Session).await.is_ok());
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
}
