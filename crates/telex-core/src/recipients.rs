//! Who the user writes to, for suggestions while typing To, Cc and Bcc. The address book's
//! contacts come first (marked, with the names the user saved), then the people of the mail
//! base itself: those the user has written to, then those who write most, each with the name
//! they used last.

use std::collections::{HashMap, HashSet};

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
    /// The contact this address belongs to, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<i64>,
}

/// Whom the user has written to, and what: for the spam filter, which leaves their mail alone.
#[derive(Debug, Default)]
pub(crate) struct Correspondence {
    /// Every address in To, Cc and Bcc of mail the user wrote, lowercase.
    pub addresses: HashSet<String>,
    /// The Message-IDs of that mail: an answer to one of them is no spam.
    pub message_ids: HashSet<String>,
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
    /// Everyone the user has written to (Sent, Drafts and the Outbox), and those messages'
    /// Message-IDs.
    pub(crate) fn correspondence(&self) -> Result<Correspondence, StoreError> {
        let outgoing: Vec<String> = self
            .accounts()
            .iter()
            .flat_map(|a| {
                a.folders
                    .iter()
                    .filter(|f| f.kind.is_outgoing())
                    .map(|f| a.folder_key(&f.id))
            })
            .collect();
        let mut known = Correspondence::default();
        if outgoing.is_empty() {
            return Ok(known);
        }
        let marks = vec!["?"; outgoing.len()].join(", ");
        let mut stmt = self.db.prepare(&format!(
            "SELECT to_addr, cc, bcc, message_id FROM messages WHERE folder IN ({marks})"
        ))?;
        let rows = stmt.query_map(rusqlite::params_from_iter(&outgoing), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        for row in rows {
            let (to, cc, bcc, message_id) = row?;
            for entry in split_addresses(&format!("{to}, {cc}, {bcc}")) {
                let (_, email) = mailbox_parts(&entry);
                if email.contains('@') {
                    known.addresses.insert(email.trim().to_lowercase());
                }
            }
            known.message_ids.extend(message_id);
        }
        Ok(known)
    }

    /// The people the user mails with, most likely first: the contacts (those mailed most
    /// first, each address of one, the primary first), then everyone they wrote to (Sent,
    /// Drafts and the Outbox), then everyone who wrote to them (not Spam or Trash). Their own
    /// addresses, and robots' addresses outside the address book, are left out.
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
        let contacts = self.contacts()?;
        let in_book: HashSet<String> = contacts
            .iter()
            .flat_map(|c| c.emails.iter().map(|e| e.to_lowercase()))
            .collect();
        let mut people: HashMap<String, Tally> = HashMap::new();
        // `count` messages with this person, the last at `at`; `sent`: written by the user.
        let mut note = |name: &str, email: &str, at: NaiveDateTime, count: usize, sent: bool| {
            let email = email.trim();
            let key = email.to_lowercase();
            if !email.contains('@')
                || own.contains(&key)
                || (is_robot(&key) && !in_book.contains(&key))
            {
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

        // The contacts, with what the mail base knows of each address.
        let mut book: Vec<(usize, String, Vec<Recipient>)> = Vec::new();
        for c in &contacts {
            let mut entries = Vec::new();
            let mut best = 0;
            for email in &c.emails {
                let key = email.to_lowercase();
                if own.contains(&key) {
                    continue;
                }
                let t = people.remove(&key).unwrap_or_default();
                best = best.max(t.score());
                entries.push(Recipient {
                    name: if c.name.is_empty() {
                        t.name
                    } else {
                        c.name.clone()
                    },
                    email: email.clone(),
                    sent: t.sent,
                    received: t.received,
                    contact: Some(c.id),
                });
            }
            if !entries.is_empty() {
                book.push((best, crate::filters::fold(&c.name), entries));
            }
        }
        book.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        let mut list: Vec<Tally> = people.into_values().collect();
        list.sort_by(|a, b| {
            b.score()
                .cmp(&a.score())
                .then(b.last.cmp(&a.last))
                .then(a.email.cmp(&b.email))
        });
        list.truncate(MAX_RECIPIENTS);
        Ok(book
            .into_iter()
            .flat_map(|(_, _, entries)| entries)
            .chain(list.into_iter().map(|t| Recipient {
                name: t.name,
                email: t.email,
                sent: t.sent,
                received: t.received,
                contact: None,
            }))
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
    fn contacts_come_first_with_their_saved_names() {
        use crate::contacts::Contact;
        let mut store = sample_store();
        let before = store.recipients().unwrap();
        // Someone far down the list, and someone the mail base never saw.
        let last = before.last().unwrap().clone();
        let known = store
            .save_contact(&Contact {
                name: "Żaneta Ćwik".into(),
                emails: vec!["nowa@przyklad.pl".into(), last.email.to_uppercase()],
                ..Contact::default()
            })
            .unwrap();
        let after = store.recipients().unwrap();
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(
            after[0].email, "nowa@przyklad.pl",
            "the primary address first"
        );
        assert_eq!(after[0].name, "Żaneta Ćwik");
        assert_eq!(after[0].contact, Some(known.id));
        assert_eq!(after[1].email, last.email.to_uppercase());
        assert_eq!(
            after[1].name, "Żaneta Ćwik",
            "the saved name, not the mail's"
        );
        assert_eq!(
            (after[1].sent, after[1].received),
            (last.sent, last.received),
            "the counts stay"
        );
        assert!(after[2..].iter().all(|r| r.contact.is_none()));
        assert!(!after[2..]
            .iter()
            .any(|r| r.email.eq_ignore_ascii_case(&last.email)));
        // A robot the user saved is offered too.
        store
            .save_contact(&Contact {
                name: "Biuro".into(),
                emails: vec!["no-reply@biuro.pl".into()],
                ..Contact::default()
            })
            .unwrap();
        assert!(store
            .recipients()
            .unwrap()
            .iter()
            .any(|r| r.email == "no-reply@biuro.pl"));
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
