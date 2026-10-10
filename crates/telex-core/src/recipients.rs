//! Who the user writes to, for suggestions while typing To, Cc and Bcc. There is no address
//! book to keep: the people come from the mail base itself. Those the user has written to
//! come first, then those who write most, each with the name they used last.

use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde::Serialize;

use crate::model::FolderKind;
use crate::smtp::{mailbox_parts, split_addresses};
use crate::store::{MailStore, StoreError};

/// How many people the suggestions keep; the rest wrote once, long ago.
pub const MAX_RECIPIENTS: usize = 2000;

/// Someone to suggest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recipient {
    /// The name as last seen; empty when there was none.
    pub name: String,
    pub email: String,
    /// Messages the user sent to them, and received from them.
    pub sent: usize,
    pub received: usize,
}

#[derive(Default)]
struct Tally {
    name: String,
    email: String,
    sent: usize,
    received: usize,
    last: Option<NaiveDateTime>,
}

impl Tally {
    /// Writing to someone counts more than hearing from them.
    fn score(&self) -> usize {
        self.sent * 5 + self.received
    }

    fn seen(&mut self, name: &str, at: NaiveDateTime) {
        if self.last.is_none_or(|last| at >= last) {
            self.last = Some(at);
            if !name.is_empty() {
                self.name = name.to_owned();
            }
        } else if self.name.is_empty() {
            self.name = name.to_owned();
        }
    }
}

/// Addresses nobody can write to: robots that send notifications.
fn is_robot(email: &str) -> bool {
    let local = email.split('@').next().unwrap_or_default();
    let squashed: String = local.chars().filter(|c| c.is_alphanumeric()).collect();
    [
        "noreply",
        "donotreply",
        "mailerdaemon",
        "postmaster",
        "bounce",
        "notreply",
    ]
    .iter()
    .any(|r| squashed.starts_with(r) || squashed.ends_with(r))
}

impl MailStore {
    /// The people the user mails with, most likely first: everyone they wrote to (Sent,
    /// Drafts and the Outbox), then everyone who wrote to them (not Spam or Trash). Their
    /// own addresses and robots' addresses are left out.
    pub fn recipients(&self) -> Result<Vec<Recipient>, StoreError> {
        let mut own: Vec<String> = Vec::new();
        let (mut outgoing, mut skipped) = (Vec::new(), Vec::new());
        for a in self.accounts() {
            own.push(a.email.to_lowercase());
            own.extend(a.identities.iter().map(|i| i.email.to_lowercase()));
            for f in &a.folders {
                let key = a.folder_key(&f.id);
                if f.kind.is_outgoing() {
                    outgoing.push(key);
                } else if matches!(f.kind, FolderKind::Spam | FolderKind::Trash) {
                    skipped.push(key);
                }
            }
        }
        let mut people: HashMap<String, Tally> = HashMap::new();
        // `count` messages with this person, the last at `at`; `sent`: written by the user.
        let mut note = |name: &str, email: &str, at: NaiveDateTime, count: usize, sent: bool| {
            let email = email.trim();
            let key = email.to_lowercase();
            if !email.contains('@') || own.contains(&key) || is_robot(&key) {
                return;
            }
            let t = people.entry(key).or_insert_with(|| Tally {
                email: email.to_owned(),
                ..Tally::default()
            });
            if sent {
                t.sent += count;
            } else {
                t.received += count;
            }
            t.seen(name, at);
        };

        // What the user wrote: every address in To, Cc and Bcc.
        if !outgoing.is_empty() {
            let marks = vec!["?"; outgoing.len()].join(", ");
            let mut stmt = self.db.prepare(&format!(
                "SELECT to_addr, cc, bcc, date FROM messages WHERE folder IN ({marks})"
            ))?;
            let rows = stmt.query_map(rusqlite::params_from_iter(&outgoing), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, NaiveDateTime>(3)?,
                ))
            })?;
            for row in rows {
                let (to, cc, bcc, at) = row?;
                for entry in split_addresses(&format!("{to}, {cc}, {bcc}")) {
                    let (name, email) = mailbox_parts(&entry);
                    note(&name, &email, at, 1, true);
                }
            }
        }

        // Who wrote to the user, counted once per address in SQL.
        let leave_out: Vec<&String> = outgoing.iter().chain(&skipped).collect();
        let marks = vec!["?"; leave_out.len()].join(", ");
        let filter = if leave_out.is_empty() {
            String::new()
        } else {
            format!("WHERE folder NOT IN ({marks})")
        };
        // SQLite takes the name from the row with the latest date.
        let mut stmt = self.db.prepare(&format!(
            "SELECT from_name, from_email, COUNT(*), MAX(date) FROM messages {filter}
             GROUP BY lower(from_email)"
        ))?;
        let rows = stmt.query_map(rusqlite::params_from_iter(leave_out), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, NaiveDateTime>(3)?,
            ))
        })?;
        for row in rows {
            let (name, email, count, at) = row?;
            // The list shows the address when a message has no name; that isn't a name.
            let name = if name.eq_ignore_ascii_case(&email) {
                ""
            } else {
                name.as_str()
            };
            note(name, &email, at, usize::try_from(count).unwrap_or(1), false);
        }

        let mut list: Vec<Tally> = people.into_values().collect();
        list.sort_by(|a, b| {
            b.score()
                .cmp(&a.score())
                .then(b.last.cmp(&a.last))
                .then(a.email.cmp(&b.email))
        });
        list.truncate(MAX_RECIPIENTS);
        Ok(list
            .into_iter()
            .map(|t| Recipient {
                name: t.name,
                email: t.email,
                sent: t.sent,
                received: t.received,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::sample_store;

    #[test]
    fn people_written_to_come_first_and_robots_and_myself_never() {
        let store = sample_store();
        let people = store.recipients().unwrap();
        assert!(!people.is_empty());
        let mine: Vec<String> = store
            .accounts()
            .iter()
            .map(|a| a.email.to_lowercase())
            .collect();
        for p in &people {
            assert!(!mine.contains(&p.email.to_lowercase()), "{p:?}");
            assert!(!is_robot(&p.email.to_lowercase()), "{p:?}");
            assert!(p.email.contains('@'));
        }
        // Scores never go up down the list.
        let score = |p: &Recipient| p.sent * 5 + p.received;
        assert!(people.windows(2).all(|w| score(&w[0]) >= score(&w[1])));
        // Every address once, whatever its case.
        let mut emails: Vec<String> = people.iter().map(|p| p.email.to_lowercase()).collect();
        emails.sort();
        emails.dedup();
        assert_eq!(emails.len(), people.len());
    }

    #[test]
    fn robots_are_recognized() {
        for robot in [
            "no-reply@x.pl",
            "noreply@x.pl",
            "do_not_reply@x.pl",
            "mailer-daemon@x.pl",
            "news-noreply@x.pl",
        ] {
            assert!(is_robot(robot), "{robot}");
        }
        for person in ["anna@x.pl", "replyall@x.pl", "noreen@x.pl"] {
            assert!(!is_robot(person), "{person}");
        }
    }
}
