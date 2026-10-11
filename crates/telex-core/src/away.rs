//! An automatic reply while the user is away, one per account.
//!
//! Where the server allows it, the reply lives there as a Sieve `vacation` (see `sieve`): the
//! server answers even while Telex is closed, and keeps to the dates itself. Otherwise Telex
//! answers: new Inbox mail, when it checks, with the safeguards rules' auto-replies have (not to
//! lists, robots, Spam or the account's own addresses, and the same sender once in `days`).

use chrono::{DateTime, Local, NaiveDate, TimeDelta};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::filters::{self, account_of, Replier};
use crate::model::{Account, FolderKind, ImapSettings, MessageId};
use crate::sieve::{self, SieveError, Vacation};
use crate::store::{MailStore, StoreError};

/// Where the reply is answered from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// Telex answers new mail when it checks.
    #[default]
    Telex,
    /// A Sieve script on the server answers, even while Telex is closed.
    Server,
}

impl Place {
    fn as_str(self) -> &'static str {
        match self {
            Self::Telex => "telex",
            Self::Server => "server",
        }
    }
}

/// An account's away reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Away {
    pub account: String,
    pub enabled: bool,
    /// First and last day; none: from now on, until it is turned off.
    #[serde(default)]
    pub starts: Option<NaiveDate>,
    #[serde(default)]
    pub ends: Option<NaiveDate>,
    /// Empty: "Re: " and the subject of the message answered.
    #[serde(default)]
    pub subject: String,
    pub body: String,
    /// The same sender gets it at most once in this many days.
    #[serde(default = "default_days")]
    pub days: u16,
    #[serde(default)]
    pub place: Place,
    /// The server's script holding Telex's part, and whether Telex made it.
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub created: bool,
}

fn default_days() -> u16 {
    4
}

impl Away {
    /// Off, with nothing written yet.
    pub fn new(account: &str) -> Self {
        Self {
            account: account.to_owned(),
            enabled: false,
            starts: None,
            ends: None,
            subject: String::new(),
            body: String::new(),
            days: default_days(),
            place: Place::Telex,
            script: None,
            created: false,
        }
    }

    /// On, and today is within its days.
    pub fn answers_on(&self, day: NaiveDate) -> bool {
        self.enabled && self.starts.is_none_or(|s| s <= day) && self.ends.is_none_or(|e| day <= e)
    }

    /// Catch mistakes before saving, in words for the form.
    pub fn check(&self) -> Result<(), String> {
        if self.enabled && self.body.trim().is_empty() {
            return Err(crate::tr!(
                "Write the text of the automatic reply.",
                "Napisz treść automatycznej odpowiedzi."
            ));
        }
        if let (Some(s), Some(e)) = (self.starts, self.ends) {
            if e < s {
                return Err(crate::tr!(
                    "The last day comes before the first.",
                    "Ostatni dzień wypada przed pierwszym."
                ));
            }
        }
        if !(1..=365).contains(&self.days) {
            return Err(crate::tr!(
                "Choose how often one person may get it: 1 to 365 days.",
                "Wybierz, jak często jedna osoba może ją dostać: od 1 do 365 dni."
            ));
        }
        Ok(())
    }
}

impl MailStore {
    /// An account's away reply, or one that is off.
    pub fn away(&self, account: &str) -> Result<Away, StoreError> {
        self.account(account)?;
        Ok(self
            .db
            .query_row(
                "SELECT enabled, starts, ends, subject, body, days, place, script, created
                 FROM away WHERE account = ?1",
                [account],
                |r| {
                    Ok(Away {
                        account: account.to_owned(),
                        enabled: r.get(0)?,
                        starts: r.get(1)?,
                        ends: r.get(2)?,
                        subject: r.get(3)?,
                        body: r.get(4)?,
                        days: r.get(5)?,
                        place: if r.get::<_, String>(6)? == "server" {
                            Place::Server
                        } else {
                            Place::Telex
                        },
                        script: r.get(7)?,
                        created: r.get(8)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_else(|| Away::new(account)))
    }

    pub fn save_away(&mut self, away: &Away) -> Result<(), StoreError> {
        self.account(&away.account)?;
        self.db.execute(
            "INSERT INTO away (account, enabled, starts, ends, subject, body, days, place, script, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (account) DO UPDATE SET enabled = excluded.enabled,
               starts = excluded.starts, ends = excluded.ends, subject = excluded.subject,
               body = excluded.body, days = excluded.days, place = excluded.place,
               script = excluded.script, created = excluded.created",
            params![
                away.account,
                away.enabled,
                away.starts,
                away.ends,
                away.subject,
                away.body,
                away.days,
                away.place.as_str(),
                away.script,
                away.created
            ],
        )?;
        Ok(())
    }

    /// The away replies that are on, whether or not today is within their days.
    pub fn away_on(&self) -> Result<Vec<AwayOn>, StoreError> {
        let mut stmt = self.db.prepare_cached(
            "SELECT account, place, starts, ends FROM away WHERE enabled = 1 ORDER BY account",
        )?;
        let list = stmt
            .query_map([], |r| {
                Ok(AwayOn {
                    account: r.get(0)?,
                    place: if r.get::<_, String>(1)? == "server" {
                        Place::Server
                    } else {
                        Place::Telex
                    },
                    starts: r.get(2)?,
                    ends: r.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(list)
    }

    pub fn last_away_reply(
        &self,
        account: &str,
        sender: &str,
    ) -> Result<Option<chrono::NaiveDateTime>, StoreError> {
        Ok(self
            .db
            .query_row(
                "SELECT sent_at FROM away_replies WHERE account = ?1 AND sender = ?2",
                params![account, sender],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn record_away_reply(
        &mut self,
        account: &str,
        sender: &str,
        at: chrono::NaiveDateTime,
    ) -> Result<(), StoreError> {
        self.db.execute(
            "INSERT INTO away_replies (account, sender, sent_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (account, sender) DO UPDATE SET sent_at = excluded.sent_at",
            params![account, sender, at],
        )?;
        Ok(())
    }
}

/// An away reply that is on, for the status bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AwayOn {
    pub account: String,
    pub place: Place,
    pub starts: Option<NaiveDate>,
    pub ends: Option<NaiveDate>,
}

/// What answering did: how many replies wait in the Outboxes, and what went wrong.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answered {
    pub replied: usize,
    pub problems: Vec<String>,
}

/// Answer new mail for the accounts whose away reply Telex sends. Mail the rules moved out of
/// the Inbox is answered too, except in Spam and Trash.
pub fn answer(store: &mut MailStore, ids: &[MessageId], now: DateTime<Local>) -> Answered {
    let mut done = Answered::default();
    let today = now.date_naive();
    for &id in ids {
        let Ok(m) = store.get(id) else { continue };
        let Ok(away) = store.away(account_of(&m.folder)) else {
            continue;
        };
        if away.place != Place::Telex || !away.answers_on(today) {
            continue;
        }
        if matches!(
            store.kind_of(&m.folder),
            Some(FolderKind::Trash | FolderKind::Spam)
        ) {
            continue;
        }
        let every = TimeDelta::days(i64::from(away.days));
        let replier = Replier::Away(&away.account);
        match filters::auto_reply(store, replier, &m, &away.subject, &away.body, every, now) {
            Ok(true) => done.replied += 1,
            Ok(false) => {}
            Err(e) => done.problems.push(e),
        }
    }
    done
}

/// The account's addresses: its own and its identities'.
fn addresses(account: &Account) -> Vec<String> {
    let mut list = vec![account.email.clone()];
    for i in &account.identities {
        if !list.iter().any(|a| a.eq_ignore_ascii_case(&i.email)) {
            list.push(i.email.clone());
        }
    }
    list
}

/// Put the reply on the server, or take it off when it is off. Returns the reply as it is now,
/// with where it answers from; when the server can't hold it, Telex answers and the reason
/// comes along (for the form to say why).
pub async fn apply(
    account: &Account,
    imap: &ImapSettings,
    password: &str,
    mut away: Away,
) -> (Away, Option<SieveError>) {
    let on_server = away.place == Place::Server && away.script.is_some();
    if !away.enabled {
        if on_server {
            if let Err(e) = remove(imap, password, &away).await {
                return (away, Some(e)); // still there: off locally, and the form says so
            }
        }
        away.place = Place::Telex;
        away.script = None;
        away.created = false;
        return (away, None);
    }
    let addresses = addresses(account);
    let vacation = Vacation {
        subject: &away.subject,
        body: &away.body,
        days: away.days,
        addresses: &addresses,
        starts: away.starts,
        ends: away.ends,
    };
    let result = async {
        let mut s = sieve::connect(&imap.host, &imap.username, password).await?;
        if !s.caps.has("vacation") {
            return Err(SieveError::NoVacation);
        }
        let dated = away.starts.is_some() || away.ends.is_some();
        if dated && !(s.caps.has("date") && s.caps.has("relational")) {
            return Err(SieveError::NoDates);
        }
        let installed = s.install(&vacation.block()).await;
        s.logout().await;
        installed
    }
    .await;
    match result {
        Ok((script, created)) => {
            away.place = Place::Server;
            away.script = Some(script);
            away.created = created;
            (away, None)
        }
        Err(e) => {
            if on_server {
                let _ = remove(imap, password, &away).await;
            }
            away.place = Place::Telex;
            away.script = None;
            away.created = false;
            (away, Some(e))
        }
    }
}

async fn remove(imap: &ImapSettings, password: &str, away: &Away) -> Result<(), SieveError> {
    let Some(script) = &away.script else {
        return Ok(());
    };
    let mut s = sieve::connect(&imap.host, &imap.username, password).await?;
    let done = s.uninstall(script, away.created).await;
    s.logout().await;
    done
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSetup, Folder, SmtpSecurity};
    use crate::store::SyncedMessage;
    use chrono::TimeZone;

    fn when(day: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, day, 9, 0, 0).unwrap()
    }

    fn store() -> (MailStore, Account) {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@twojastudio.pl".into(),
                host: "mail.twojastudio.pl".into(),
                port: 993,
                smtp_security: SmtpSecurity::Tls,
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
                    folder("spam", FolderKind::Spam),
                ],
            )
            .unwrap();
        let account = store.account(&account.id).unwrap().clone();
        (store, account)
    }

    fn deliver(store: &mut MailStore, account: &Account, uid: u32, from: &str) -> MessageId {
        let raw = format!(
            "Message-ID: <m{uid}@x.pl>\r\nFrom: {from}\r\nTo: biuro@twojastudio.pl\r\nSubject: Pytanie\r\n\r\nHej\r\n"
        );
        let mut message = crate::mime::parse_message(raw.as_bytes(), "", None);
        message.folder = account.folder_key("inbox");
        store
            .add_synced(
                &account.folder_key("inbox"),
                &[SyncedMessage {
                    uid,
                    message,
                    raw: raw.into_bytes(),
                }],
            )
            .unwrap()[0]
    }

    fn outbox(store: &MailStore, account: &Account) -> Vec<crate::model::MessageSummary> {
        store.list(&account.folder_key("outbox")).unwrap()
    }

    #[test]
    fn telex_answers_while_away_and_only_then() {
        let (mut store, account) = store();
        assert!(
            !store.away(&account.id).unwrap().enabled,
            "off to begin with"
        );
        let away = Away {
            enabled: true,
            starts: NaiveDate::from_ymd_opt(2026, 10, 12),
            ends: NaiveDate::from_ymd_opt(2026, 10, 20),
            subject: "Urlop".into(),
            body: "Wracam 21 października.".into(),
            ..Away::new(&account.id)
        };
        away.check().unwrap();
        store.save_away(&away).unwrap();
        assert_eq!(store.away(&account.id).unwrap(), away);
        assert_eq!(store.away_on().unwrap()[0].ends, away.ends);

        // Before the first day: nothing.
        let early = deliver(&mut store, &account, 1, "Anna <anna@x.pl>");
        assert_eq!(answer(&mut store, &[early], when(11)).replied, 0);
        // While away: one reply, with its subject.
        let id = deliver(&mut store, &account, 2, "Anna <anna@x.pl>");
        assert_eq!(answer(&mut store, &[id], when(13)).replied, 1);
        let replies = outbox(&store, &account);
        assert_eq!(replies[0].subject, "Urlop");
        assert!(replies[0].to.contains("anna@x.pl"));
        // The same person again two days later: not again.
        let again = deliver(&mut store, &account, 3, "anna@x.pl");
        assert_eq!(answer(&mut store, &[again], when(15)).replied, 0);
        // A robot, or mail in Spam: never.
        let robot = deliver(&mut store, &account, 4, "noreply@bank.pl");
        assert_eq!(answer(&mut store, &[robot], when(15)).replied, 0);
        let spam = deliver(&mut store, &account, 5, "piotr@y.pl");
        store.move_to(spam, &account.folder_key("spam")).unwrap();
        assert_eq!(answer(&mut store, &[spam], when(15)).replied, 0);
        // After the last day: nothing.
        let late = deliver(&mut store, &account, 6, "piotr@y.pl");
        assert_eq!(answer(&mut store, &[late], when(21)).replied, 0);
        // On the server: the server answers, not Telex.
        store
            .save_away(&Away {
                place: Place::Server,
                script: Some("roundcube".into()),
                ..away.clone()
            })
            .unwrap();
        let other = deliver(&mut store, &account, 7, "piotr@y.pl");
        assert_eq!(answer(&mut store, &[other], when(14)).replied, 0);
        assert_eq!(outbox(&store, &account).len(), 1);
    }

    #[test]
    fn mistakes_are_caught_before_saving() {
        let mut away = Away {
            enabled: true,
            ..Away::new("x")
        };
        assert!(away.check().is_err(), "no text");
        away.body = "Wracam".into();
        away.starts = NaiveDate::from_ymd_opt(2026, 10, 20);
        away.ends = NaiveDate::from_ymd_opt(2026, 10, 12);
        assert!(away.check().is_err(), "the end before the start");
        away.ends = None;
        away.days = 0;
        assert!(away.check().is_err());
        away.days = 7;
        assert!(away.check().is_ok());
    }
}
