//! The address book: contacts and groups, kept in the mail base (schema 15).
//!
//! A contact is a name with any number of addresses (the first is the primary one), a phone,
//! an organization and a note. An address belongs to one contact at most, so the sender of a
//! message is one contact or none. A group is a named list of contacts and of addresses that
//! aren't contacts; typed in To, Cc or Bcc it is put in as its members' addresses. Nothing is
//! added by itself: people the user writes to are only offered ([`MailStore::suggested_contacts`]).
//! Contacts are mail data, not settings, so the settings file leaves them out; backups have them.
//! vCard files come in and go out through [`crate::vcard`].

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::filters::fold;
use crate::recipients::Recipient;
use crate::vcard::{self, Card};
use crate::{tr, MailStore, StoreError};

/// Someone in the address book.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    /// 0 for a new one.
    #[serde(default)]
    pub id: i64,
    /// The vCard UID: an imported card's, or one made when the contact was first saved.
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub name: String,
    /// The primary address first.
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub note: String,
    /// The groups it is in, by id.
    #[serde(default)]
    pub groups: Vec<i64>,
}

/// A named list of people.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub members: Vec<Member>,
}

/// One person of a group: a contact (read with its name and primary address), or an address
/// that isn't one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    #[serde(default)]
    pub contact: Option<i64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

/// An address of a message that belongs to a contact, with the name the user saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContactName {
    pub email: String,
    pub id: i64,
    pub name: String,
}

/// What a vCard import (or moving in an address book) did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    /// New contacts.
    pub added: usize,
    /// Contacts that were there (same UID or address) and got something new.
    pub updated: usize,
    /// Contacts that were there already as they are.
    pub same: usize,
    /// Cards without a name or an address.
    pub skipped: usize,
    /// New groups.
    pub groups: usize,
}

impl ImportReport {
    pub fn add(&mut self, other: &ImportReport) {
        self.added += other.added;
        self.updated += other.updated;
        self.same += other.same;
        self.skipped += other.skipped;
        self.groups += other.groups;
    }
}

const MAX_NAME: usize = 200;
const MAX_GROUP_NAME: usize = 80;
const MAX_NOTE: usize = 10_000;
/// Addresses a contact can have, and members a group can.
const MAX_EMAILS: usize = 20;
const MAX_MEMBERS: usize = 2_000;

/// Spaces collapsed, no line breaks.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// An address as it is kept, or `None` when it isn't one: `mailto:` and `<>` taken off.
pub(crate) fn clean_email(raw: &str) -> Option<String> {
    let e = raw.trim();
    let e = e.strip_prefix("mailto:").unwrap_or(e);
    let e = e.trim_start_matches('<').trim_end_matches('>').trim();
    let (local, domain) = e.rsplit_once('@')?;
    let fine = !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && e.len() <= 254
        && !e
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || ",;<>\"()[]".contains(c));
    fine.then(|| e.to_owned())
}

/// A UID for a contact written here: a random UUID (version 4).
fn new_uid() -> String {
    use ring::rand::SecureRandom;
    let mut b = [0u8; 16];
    if ring::rand::SystemRandom::new().fill(&mut b).is_err() {
        // Practically never; time and a counter still make it unique on this PC.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        b = (nanos ^ (u128::from(n) << 64)).to_le_bytes();
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// The key the address book is sorted by: the name, else the address, folded.
fn sort_key(c: &Contact) -> String {
    fold(if c.name.is_empty() {
        c.emails.first().map_or("", String::as_str)
    } else {
        &c.name
    })
}

fn now() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

impl Contact {
    /// Trimmed and checked, or a sentence saying what's wrong.
    fn cleaned(&self) -> Result<Contact, StoreError> {
        let mut c = self.clone();
        c.name = one_line(&c.name);
        c.phone = one_line(&c.phone);
        c.organization = one_line(&c.organization);
        c.note = c.note.replace("\r\n", "\n").trim().to_owned();
        let mut emails: Vec<String> = Vec::new();
        for raw in &self.emails {
            if raw.trim().is_empty() {
                continue;
            }
            let Some(email) = clean_email(raw) else {
                return Err(StoreError::Invalid(tr!(
                    "“{email}” isn't an email address.",
                    "„{email}” nie jest adresem e-mail.",
                    email = raw.trim()
                )));
            };
            if !emails.iter().any(|e| e.eq_ignore_ascii_case(&email)) {
                emails.push(email);
            }
        }
        c.emails = emails;
        if c.name.is_empty() && c.emails.is_empty() {
            return Err(StoreError::Invalid(tr!(
                "Give the contact a name or an address.",
                "Podaj imię i nazwisko albo adres kontaktu."
            )));
        }
        if c.emails.len() > MAX_EMAILS {
            return Err(StoreError::Invalid(tr!(
                "A contact can have at most {MAX_EMAILS} addresses.",
                "Kontakt może mieć najwyżej {MAX_EMAILS} adresów."
            )));
        }
        if [&c.name, &c.phone, &c.organization]
            .iter()
            .any(|s| s.chars().count() > MAX_NAME)
            || c.note.chars().count() > MAX_NOTE
        {
            return Err(StoreError::Invalid(tr!(
                "That is too long for the address book.",
                "To za długie jak na książkę adresową."
            )));
        }
        c.groups.sort_unstable();
        c.groups.dedup();
        Ok(c)
    }
}

/// The contact that has `email`, other than `except`.
fn owner(db: &Connection, email: &str, except: i64) -> Result<Option<(i64, String)>, StoreError> {
    Ok(db
        .query_row(
            "SELECT c.id, c.name FROM contact_emails e JOIN contacts c ON c.id = e.contact
             WHERE lower(e.email) = lower(?1) AND e.contact != ?2",
            params![email, except],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn read_contacts(db: &Connection, only: Option<i64>) -> Result<Vec<Contact>, StoreError> {
    let filter = if only.is_some() { "WHERE id = ?1" } else { "" };
    let mut contacts: Vec<Contact> = {
        let mut stmt = db.prepare(&format!(
            "SELECT id, uid, name, phone, organization, note FROM contacts {filter}"
        ))?;
        let map = |r: &rusqlite::Row| {
            Ok(Contact {
                id: r.get(0)?,
                uid: r.get(1)?,
                name: r.get(2)?,
                phone: r.get(3)?,
                organization: r.get(4)?,
                note: r.get(5)?,
                ..Contact::default()
            })
        };
        let rows = match only {
            Some(id) => stmt.query_map([id], map)?.collect::<Result<Vec<_>, _>>()?,
            None => stmt.query_map([], map)?.collect::<Result<Vec<_>, _>>()?,
        };
        rows
    };
    let at: HashMap<i64, usize> = contacts
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id, i))
        .collect();
    {
        let mut stmt =
            db.prepare("SELECT contact, email FROM contact_emails ORDER BY contact, position")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, email) = row?;
            if let Some(&i) = at.get(&id) {
                contacts[i].emails.push(email);
            }
        }
    }
    {
        let mut stmt = db.prepare(
            "SELECT DISTINCT contact, grp FROM contact_group_members WHERE contact IS NOT NULL
             ORDER BY contact, grp",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (id, group) = row?;
            if let Some(&i) = at.get(&id) {
                contacts[i].groups.push(group);
            }
        }
    }
    contacts.sort_by_cached_key(|c| (sort_key(c), c.id));
    Ok(contacts)
}

fn read_groups(db: &Connection) -> Result<Vec<Group>, StoreError> {
    let mut groups: Vec<Group> = {
        let mut stmt = db.prepare("SELECT id, name FROM contact_groups")?;
        let rows = stmt.query_map([], |r| {
            Ok(Group {
                id: r.get(0)?,
                name: r.get(1)?,
                members: Vec::new(),
            })
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let at: HashMap<i64, usize> = groups.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
    let mut stmt = db.prepare(
        "SELECT m.grp, m.contact, coalesce(c.name, m.name),
                coalesce((SELECT email FROM contact_emails e WHERE e.contact = m.contact
                          ORDER BY position LIMIT 1), m.email)
         FROM contact_group_members m LEFT JOIN contacts c ON c.id = m.contact
         ORDER BY m.grp, m.position",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            Member {
                contact: r.get(1)?,
                name: r.get(2)?,
                email: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            },
        ))
    })?;
    for row in rows {
        let (group, member) = row?;
        if let Some(&i) = at.get(&group) {
            groups[i].members.push(member);
        }
    }
    groups.sort_by_cached_key(|g| (fold(&g.name), g.id));
    Ok(groups)
}

/// Put `contact` at the end of `group` unless it is there.
fn join_group(db: &Connection, group: i64, member: &Member) -> Result<bool, StoreError> {
    let there: Option<i64> = match member.contact {
        Some(id) => db
            .query_row(
                "SELECT 1 FROM contact_group_members WHERE grp = ?1 AND contact = ?2",
                params![group, id],
                |r| r.get(0),
            )
            .optional()?,
        None => db
            .query_row(
                "SELECT 1 FROM contact_group_members WHERE grp = ?1 AND contact IS NULL
                 AND lower(email) = lower(?2)",
                params![group, member.email],
                |r| r.get(0),
            )
            .optional()?,
    };
    if there.is_some() {
        return Ok(false);
    }
    db.execute(
        "INSERT INTO contact_group_members (grp, position, contact, name, email)
         VALUES (?1, (SELECT coalesce(max(position), -1) + 1 FROM contact_group_members
                      WHERE grp = ?1), ?2, ?3, ?4)",
        params![
            group,
            member.contact,
            if member.contact.is_some() {
                ""
            } else {
                &member.name
            },
            if member.contact.is_some() {
                ""
            } else {
                &member.email
            },
        ],
    )?;
    Ok(true)
}

/// Write a cleaned contact: its row (new when `id` is 0, or `insert_as` for a restore), its
/// addresses (those another contact has are refused, or left out when `lenient`) and its groups.
fn write_contact(
    db: &Connection,
    c: &Contact,
    insert_as: Option<i64>,
    lenient: bool,
) -> Result<i64, StoreError> {
    let mut emails = Vec::new();
    for email in &c.emails {
        match owner(db, email, c.id)? {
            Some((_, name)) if !lenient => {
                return Err(StoreError::Invalid(tr!(
                    "{email} is in the address book already, as {name}.",
                    "{email} jest już w książce adresowej, jako {name}.",
                    name = if name.is_empty() { email.clone() } else { name }
                )));
            }
            Some(_) => {}
            None => emails.push(email.clone()),
        }
    }
    let id = if c.id == 0 || insert_as.is_some() {
        let uid_taken = !c.uid.is_empty()
            && db
                .query_row("SELECT 1 FROM contacts WHERE uid = ?1", [&c.uid], |r| {
                    r.get::<_, i64>(0)
                })
                .optional()?
                .is_some();
        let uid = if c.uid.trim().is_empty() || uid_taken {
            new_uid()
        } else {
            c.uid.trim().to_owned()
        };
        db.execute(
            "INSERT INTO contacts (id, uid, name, phone, organization, note, changed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                insert_as,
                uid,
                c.name,
                c.phone,
                c.organization,
                c.note,
                now()
            ],
        )?;
        db.last_insert_rowid()
    } else {
        let changed = db.execute(
            "UPDATE contacts SET name = ?2, phone = ?3, organization = ?4, note = ?5,
             changed = ?6 WHERE id = ?1",
            params![c.id, c.name, c.phone, c.organization, c.note, now()],
        )?;
        if changed == 0 {
            return Err(StoreError::Invalid(tr!(
                "That contact is no longer in the address book.",
                "Tego kontaktu nie ma już w książce adresowej."
            )));
        }
        c.id
    };
    db.execute("DELETE FROM contact_emails WHERE contact = ?1", [id])?;
    for (position, email) in emails.iter().enumerate() {
        db.execute(
            "INSERT INTO contact_emails (contact, position, email) VALUES (?1, ?2, ?3)",
            params![id, position as i64, email],
        )?;
    }
    // Groups: out of those not ticked, into those ticked that exist.
    let ticked = c
        .groups
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    db.execute(
        &format!("DELETE FROM contact_group_members WHERE contact = ?1 AND grp NOT IN ({ticked})"),
        [id],
    )?;
    for &group in &c.groups {
        let exists = db
            .query_row("SELECT 1 FROM contact_groups WHERE id = ?1", [group], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?
            .is_some();
        if exists {
            join_group(
                db,
                group,
                &Member {
                    contact: Some(id),
                    ..Member::default()
                },
            )?;
        }
    }
    Ok(id)
}

fn clean_group_name(name: &str) -> Result<String, StoreError> {
    let name = one_line(name);
    if name.is_empty()
        || name.chars().count() > MAX_GROUP_NAME
        || name.chars().any(|c| ",;<>\"@".contains(c))
    {
        return Err(StoreError::Invalid(tr!(
            "Give the group a name of 1–80 characters, without , ; < > \" or @.",
            "Nadaj grupie nazwę od 1 do 80 znaków, bez , ; < > \" ani @."
        )));
    }
    Ok(name)
}

/// The group called `name` (case ignored), made when there is none. True when it was made.
fn group_named(db: &Connection, name: &str) -> Result<(i64, bool), StoreError> {
    let name = clean_group_name(&name.replace([',', ';', '<', '>', '"', '@'], " "))?;
    if let Some(id) = db
        .query_row(
            "SELECT id FROM contact_groups WHERE name = ?1",
            [&name],
            |r| r.get(0),
        )
        .optional()?
    {
        return Ok((id, false));
    }
    db.execute("INSERT INTO contact_groups (name) VALUES (?1)", [&name])?;
    Ok((db.last_insert_rowid(), true))
}

impl MailStore {
    /// Every contact, by name (diacritics and case ignored).
    pub fn contacts(&self) -> Result<Vec<Contact>, StoreError> {
        read_contacts(&self.db, None)
    }

    pub fn contact(&self, id: i64) -> Result<Contact, StoreError> {
        read_contacts(&self.db, Some(id))?.pop().ok_or_else(|| {
            StoreError::Invalid(tr!(
                "That contact is no longer in the address book.",
                "Tego kontaktu nie ma już w książce adresowej."
            ))
        })
    }

    /// Save a contact (id 0: a new one). An address another contact has is refused.
    pub fn save_contact(&mut self, contact: &Contact) -> Result<Contact, StoreError> {
        let c = contact.cleaned()?;
        let tx = self.db.transaction()?;
        let id = write_contact(&tx, &c, None, false)?;
        tx.commit()?;
        self.contact(id)
    }

    /// Remove contacts. Returns them as they were (with their groups), for undo.
    pub fn delete_contacts(&mut self, ids: &[i64]) -> Result<Vec<Contact>, StoreError> {
        let gone: Vec<Contact> = self
            .contacts()?
            .into_iter()
            .filter(|c| ids.contains(&c.id))
            .collect();
        let tx = self.db.transaction()?;
        for c in &gone {
            tx.execute("DELETE FROM contacts WHERE id = ?1", [c.id])?;
        }
        tx.commit()?;
        Ok(gone)
    }

    /// Put back contacts [`Self::delete_contacts`] removed: their ids where free, their
    /// addresses that no other contact took meanwhile, at the end of their groups.
    pub fn restore_contacts(&mut self, contacts: &[Contact]) -> Result<(), StoreError> {
        let tx = self.db.transaction()?;
        for c in contacts {
            let Ok(clean) = c.cleaned() else { continue };
            let free = tx
                .query_row("SELECT 1 FROM contacts WHERE id = ?1", [c.id], |r| {
                    r.get::<_, i64>(0)
                })
                .optional()?
                .is_none();
            let clean = Contact {
                id: if free { c.id } else { 0 },
                ..clean
            };
            write_contact(&tx, &clean, free.then_some(c.id), true)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Every group, A to Z, with its members (contacts with their name and primary address).
    pub fn contact_groups(&self) -> Result<Vec<Group>, StoreError> {
        read_groups(&self.db)
    }

    /// Save a group (id 0: a new one) with its members in order. An address that is a
    /// contact's becomes that contact; the same person twice stays once.
    pub fn save_group(&mut self, group: &Group) -> Result<Group, StoreError> {
        self.write_group(group, false)
    }

    /// Put back a group [`Self::delete_group`] removed.
    pub fn restore_group(&mut self, group: &Group) -> Result<Group, StoreError> {
        self.write_group(group, true)
    }

    fn write_group(&mut self, group: &Group, restore: bool) -> Result<Group, StoreError> {
        let name = clean_group_name(&group.name)?;
        let tx = self.db.transaction()?;
        let mut members: Vec<Member> = Vec::new();
        for m in &group.members {
            let member = match m.contact {
                Some(id) => {
                    let exists = tx
                        .query_row("SELECT 1 FROM contacts WHERE id = ?1", [id], |r| {
                            r.get::<_, i64>(0)
                        })
                        .optional()?
                        .is_some();
                    if !exists {
                        continue;
                    }
                    Member {
                        contact: Some(id),
                        ..Member::default()
                    }
                }
                None => {
                    if m.email.trim().is_empty() {
                        continue;
                    }
                    let Some(email) = clean_email(&m.email) else {
                        return Err(StoreError::Invalid(tr!(
                            "“{email}” isn't an email address.",
                            "„{email}” nie jest adresem e-mail.",
                            email = m.email.trim()
                        )));
                    };
                    match owner(&tx, &email, 0)? {
                        Some((id, _)) => Member {
                            contact: Some(id),
                            ..Member::default()
                        },
                        None => Member {
                            contact: None,
                            name: one_line(&m.name),
                            email,
                        },
                    }
                }
            };
            let twice = members.iter().any(|o| match member.contact {
                Some(id) => o.contact == Some(id),
                None => o.contact.is_none() && o.email.eq_ignore_ascii_case(&member.email),
            });
            if !twice {
                members.push(member);
            }
        }
        if members.len() > MAX_MEMBERS {
            return Err(StoreError::Invalid(tr!(
                "A group can have at most {MAX_MEMBERS} people.",
                "Grupa może mieć najwyżej {MAX_MEMBERS} osób."
            )));
        }
        let duplicate = tx
            .query_row(
                "SELECT 1 FROM contact_groups WHERE name = ?1 AND id != ?2",
                params![name, if restore { 0 } else { group.id }],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        if duplicate {
            return Err(StoreError::Invalid(tr!(
                "A group already has that name.",
                "Grupa o takiej nazwie już jest."
            )));
        }
        let id = if group.id == 0 || restore {
            let free = group.id != 0
                && tx
                    .query_row(
                        "SELECT 1 FROM contact_groups WHERE id = ?1",
                        [group.id],
                        |r| r.get::<_, i64>(0),
                    )
                    .optional()?
                    .is_none();
            tx.execute(
                "INSERT INTO contact_groups (id, name) VALUES (?1, ?2)",
                params![free.then_some(group.id), name],
            )?;
            tx.last_insert_rowid()
        } else {
            if tx.execute(
                "UPDATE contact_groups SET name = ?2 WHERE id = ?1",
                params![group.id, name],
            )? == 0
            {
                return Err(StoreError::Invalid(tr!(
                    "That group is no longer in the address book.",
                    "Tej grupy nie ma już w książce adresowej."
                )));
            }
            group.id
        };
        tx.execute("DELETE FROM contact_group_members WHERE grp = ?1", [id])?;
        for m in &members {
            join_group(&tx, id, m)?;
        }
        tx.commit()?;
        self.contact_groups()?
            .into_iter()
            .find(|g| g.id == id)
            .ok_or_else(|| StoreError::Database("the group just saved is gone".into()))
    }

    /// Remove a group (never its contacts). Returns it as it was, for undo.
    pub fn delete_group(&mut self, id: i64) -> Result<Option<Group>, StoreError> {
        let gone = self.contact_groups()?.into_iter().find(|g| g.id == id);
        self.db
            .execute("DELETE FROM contact_groups WHERE id = ?1", [id])?;
        Ok(gone)
    }

    /// The contacts among these addresses, with the names the user saved.
    pub fn contact_names<'a>(
        &self,
        emails: impl IntoIterator<Item = &'a str>,
    ) -> Result<Vec<ContactName>, StoreError> {
        let mut stmt = self.db.prepare_cached(
            "SELECT c.id, c.name FROM contact_emails e JOIN contacts c ON c.id = e.contact
             WHERE lower(e.email) = lower(?1)",
        )?;
        let mut found: Vec<ContactName> = Vec::new();
        for email in emails {
            let email = email.trim();
            if email.is_empty() || found.iter().any(|f| f.email.eq_ignore_ascii_case(email)) {
                continue;
            }
            if let Some((id, name)) = stmt
                .query_row([email], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })
                .optional()?
            {
                found.push(ContactName {
                    email: email.to_owned(),
                    id,
                    name,
                });
            }
        }
        Ok(found)
    }

    /// People the user has written to who aren't contacts, most written to first: offered
    /// in the address book, never added by themselves.
    pub fn suggested_contacts(&self, limit: usize) -> Result<Vec<Recipient>, StoreError> {
        Ok(self
            .recipients()?
            .into_iter()
            .filter(|r| r.contact.is_none() && r.sent > 0)
            .take(limit)
            .collect())
    }

    /// Bring in cards (a .vcf file, another program's address book). A card with the UID or
    /// an address of a contact fills in what that contact lacks; groups come from group cards
    /// and CATEGORIES. Importing the same file twice changes nothing the second time.
    pub fn import_cards(&mut self, cards: &[Card]) -> Result<ImportReport, StoreError> {
        let mut report = ImportReport::default();
        let tx = self.db.transaction()?;
        let mut existing = read_contacts(&tx, None)?;
        // The contact each card became, by the card's index.
        let mut became: HashMap<usize, i64> = HashMap::new();
        for (i, card) in cards.iter().enumerate() {
            if card.group || card.member_only {
                continue;
            }
            let incoming = Contact {
                uid: card.uid.clone(),
                name: card.name.clone(),
                emails: card.emails.iter().filter_map(|e| clean_email(e)).collect(),
                phone: card.phone.clone(),
                organization: card.organization.clone(),
                note: card.note.clone(),
                ..Contact::default()
            };
            let Ok(incoming) = incoming.cleaned() else {
                report.skipped += 1;
                continue;
            };
            let known = existing.iter().position(|c| {
                vcard::same_uid(&c.uid, &incoming.uid)
                    || c.emails.iter().any(|e| {
                        incoming
                            .emails
                            .iter()
                            .any(|n: &String| n.eq_ignore_ascii_case(e))
                    })
            });
            let id = match known {
                Some(at) => {
                    let mut merged = existing[at].clone();
                    for (mine, theirs) in [
                        (&mut merged.name, &incoming.name),
                        (&mut merged.phone, &incoming.phone),
                        (&mut merged.organization, &incoming.organization),
                        (&mut merged.note, &incoming.note),
                    ] {
                        if mine.is_empty() {
                            mine.clone_from(theirs);
                        }
                    }
                    for e in &incoming.emails {
                        if !merged.emails.iter().any(|m| m.eq_ignore_ascii_case(e))
                            && merged.emails.len() < MAX_EMAILS
                        {
                            merged.emails.push(e.clone());
                        }
                    }
                    if merged == existing[at] {
                        report.same += 1;
                    } else {
                        write_contact(&tx, &merged, None, true)?;
                        existing[at] = merged;
                        report.updated += 1;
                    }
                    existing[at].id
                }
                None => {
                    let id = write_contact(&tx, &incoming, None, true)?;
                    existing.push(Contact { id, ..incoming });
                    report.added += 1;
                    id
                }
            };
            became.insert(i, id);
        }
        // The group called `name`, and whether it is new; none when the name can't be one.
        let group_for = |tx: &Connection, name: &str| -> Result<Option<(i64, bool)>, StoreError> {
            match group_named(tx, name) {
                Ok(found) => Ok(Some(found)),
                Err(StoreError::Invalid(_)) => Ok(None),
                Err(e) => Err(e),
            }
        };
        let contact_member = |id: i64| Member {
            contact: Some(id),
            ..Member::default()
        };
        // An address as a member: the contact that has it, else the address itself.
        let address_member = |tx: &Connection, name: &str, email: &str| match clean_email(email) {
            Some(email) => owner(tx, &email, 0).map(|o| {
                Some(match o {
                    Some((id, _)) => contact_member(id),
                    None => Member {
                        contact: None,
                        name: one_line(name),
                        email,
                    },
                })
            }),
            None => Ok(None),
        };
        for (i, card) in cards.iter().enumerate() {
            // Google's own labels.
            let skip = |c: &str| {
                let c = c.to_lowercase();
                c == "mycontacts" || c == "starred" || c == "* mycontacts" || c == "* starred"
            };
            let member = match became.get(&i) {
                Some(&id) => Some(contact_member(id)),
                None if card.member_only => match card.emails.first() {
                    Some(email) => address_member(&tx, &card.name, email)?,
                    None => None,
                },
                None => None,
            };
            if let Some(member) = member {
                for category in card.categories.iter().filter(|c| !skip(c.as_str())) {
                    if let Some((group, made)) = group_for(&tx, category)? {
                        report.groups += usize::from(made);
                        join_group(&tx, group, &member)?;
                    }
                }
            }
            if !card.group {
                continue;
            }
            let Some((group, made)) = group_for(&tx, &card.name)? else {
                report.skipped += 1;
                continue;
            };
            report.groups += usize::from(made);
            for value in &card.members {
                let member = if let Some(email) = value.strip_prefix("mailto:") {
                    address_member(&tx, "", email)?
                } else if let Some(at) = cards
                    .iter()
                    .position(|c| !c.group && vcard::same_uid(&c.uid, value))
                {
                    became.get(&at).map(|&id| contact_member(id))
                } else {
                    existing
                        .iter()
                        .find(|c| vcard::same_uid(&c.uid, value))
                        .map(|c| contact_member(c.id))
                };
                if let Some(member) = member {
                    join_group(&tx, group, &member)?;
                }
            }
        }
        tx.commit()?;
        Ok(report)
    }

    /// A .vcf file's cards brought in: [`Self::import_cards`] of [`vcard::parse`].
    pub fn import_vcards(&mut self, bytes: &[u8]) -> Result<ImportReport, StoreError> {
        let cards = vcard::parse(bytes);
        if cards.is_empty() {
            return Err(StoreError::Invalid(tr!(
                "That file has no contacts (vCard).",
                "W tym pliku nie ma kontaktów (vCard)."
            )));
        }
        self.import_cards(&cards)
    }

    /// The contacts with these ids (all of them when empty) as one vCard 3.0 file, with the
    /// groups they are in. All of them also brings groups' addresses that aren't contacts.
    pub fn export_vcards(&self, ids: &[i64]) -> Result<String, StoreError> {
        let contacts: Vec<Contact> = self
            .contacts()?
            .into_iter()
            .filter(|c| ids.is_empty() || ids.contains(&c.id))
            .collect();
        let mut groups = self.contact_groups()?;
        if !ids.is_empty() {
            for g in &mut groups {
                g.members.retain(|m| m.contact.is_some());
            }
        }
        Ok(vcard::write(&contacts, &groups))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::sample_store;

    fn anna() -> Contact {
        Contact {
            name: "  Anna   Kowalska ".into(),
            emails: vec!["anna@studio.pl".into(), " <Anna.K@gmail.com> ".into()],
            phone: "+48 600 100 200".into(),
            organization: "Studio Łódź".into(),
            note: "Projekty okładek\r\n".into(),
            ..Contact::default()
        }
    }

    #[test]
    fn contacts_are_saved_checked_and_kept_apart() {
        let mut store = MailStore::in_memory().unwrap();
        let a = store.save_contact(&anna()).unwrap();
        assert!(a.id > 0);
        assert_eq!(a.name, "Anna Kowalska");
        assert_eq!(a.emails, ["anna@studio.pl", "Anna.K@gmail.com"]);
        assert_eq!(a.note, "Projekty okładek");
        assert_eq!(a.uid.len(), 36, "a UUID: {}", a.uid);
        // Another contact can't have one of her addresses.
        let err = store
            .save_contact(&Contact {
                name: "Ktoś".into(),
                emails: vec!["ANNA@studio.pl".into()],
                ..Contact::default()
            })
            .unwrap_err();
        assert!(err.to_string().contains("Anna Kowalska"), "{err}");
        assert!(store
            .save_contact(&Contact {
                emails: vec!["nie adres".into()],
                ..Contact::default()
            })
            .is_err());
        assert!(store.save_contact(&Contact::default()).is_err());
        // Saving her again with the primary address swapped keeps her id and order.
        let swapped = store
            .save_contact(&Contact {
                emails: vec![a.emails[1].clone(), a.emails[0].clone()],
                ..a.clone()
            })
            .unwrap();
        assert_eq!(swapped.id, a.id);
        assert_eq!(swapped.emails[0], "Anna.K@gmail.com");
        let names = store
            .contact_names(["anna.k@GMAIL.com", "nikt@x.pl"])
            .unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].name, "Anna Kowalska");
        // Sorted with diacritics ignored: Łukasz between Kowalska and Nowak.
        for name in ["Piotr Nowak", "Łukasz Żak"] {
            store
                .save_contact(&Contact {
                    name: name.into(),
                    ..Contact::default()
                })
                .unwrap();
        }
        let order: Vec<String> = store
            .contacts()
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(order, ["Anna Kowalska", "Łukasz Żak", "Piotr Nowak"]);
    }

    #[test]
    fn groups_hold_contacts_and_addresses_and_deletes_undo() {
        let mut store = MailStore::in_memory().unwrap();
        let a = store.save_contact(&anna()).unwrap();
        let board = store
            .save_group(&Group {
                id: 0,
                name: "Zarząd".into(),
                members: vec![
                    Member {
                        contact: Some(a.id),
                        ..Member::default()
                    },
                    // Her address: she is there once.
                    Member {
                        email: "anna@studio.pl".into(),
                        ..Member::default()
                    },
                    Member {
                        name: "Ewa Świątek".into(),
                        email: "ewa@x.pl".into(),
                        ..Member::default()
                    },
                ],
            })
            .unwrap();
        assert_eq!(board.members.len(), 2);
        assert_eq!(board.members[0].name, "Anna Kowalska");
        assert_eq!(
            board.members[0].email, "anna@studio.pl",
            "her primary address"
        );
        assert_eq!(board.members[1].email, "ewa@x.pl");
        assert!(store
            .save_group(&Group {
                id: 0,
                name: "zarząd".into(),
                members: vec![],
            })
            .is_err());
        assert!(store
            .save_group(&Group {
                id: 0,
                name: "a, b".into(),
                members: vec![],
            })
            .is_err());
        assert_eq!(store.contact(a.id).unwrap().groups, [board.id]);

        // Untick the group on the contact, then tick it again: she goes to the end.
        store
            .save_contact(&Contact {
                groups: vec![],
                ..store.contact(a.id).unwrap()
            })
            .unwrap();
        assert_eq!(store.contact_groups().unwrap()[0].members.len(), 1);
        store
            .save_contact(&Contact {
                groups: vec![board.id],
                ..store.contact(a.id).unwrap()
            })
            .unwrap();
        let members = &store.contact_groups().unwrap()[0].members;
        assert_eq!(members[1].contact, Some(a.id));

        // Delete her and undo: same id, same group.
        let gone = store.delete_contacts(&[a.id]).unwrap();
        assert_eq!(store.contact_groups().unwrap()[0].members.len(), 1);
        assert!(store.contacts().unwrap().is_empty());
        store.restore_contacts(&gone).unwrap();
        let back = store.contact(a.id).unwrap();
        assert_eq!(back.emails, a.emails);
        assert_eq!(back.uid, a.uid);
        assert_eq!(back.groups, [board.id]);

        // Delete the group and undo.
        let group = store.delete_group(board.id).unwrap().unwrap();
        assert!(store.contact_groups().unwrap().is_empty());
        assert!(store.contact(a.id).unwrap().groups.is_empty());
        let again = store.restore_group(&group).unwrap();
        assert_eq!(again.id, board.id);
        assert_eq!(again.members.len(), 2);
    }

    #[test]
    fn vcards_come_in_once_and_go_out_whole() {
        let mut store = MailStore::in_memory().unwrap();
        let mine = store
            .save_contact(&Contact {
                name: "".into(),
                emails: vec!["piotr@nowak-meble.pl".into()],
                ..Contact::default()
            })
            .unwrap();
        let file = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:card-1\r\nFN:Zażółć Gęślą\r\nEMAIL:jazn@x.pl\r\nCATEGORIES:Klub,myContacts\r\nEND:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Piotr Nowak\r\nEMAIL:PIOTR@nowak-meble.pl\r\nTEL:22 111\r\nEND:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:3.0\r\nTEL:600 000 000\r\nEND:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:4.0\r\nKIND:group\r\nFN:Meble\r\nMEMBER:urn:uuid:card-1\r\nMEMBER:mailto:piotr@nowak-meble.pl\r\nMEMBER:mailto:nowy@x.pl\r\nEND:VCARD\r\n";
        let report = store.import_vcards(file.as_bytes()).unwrap();
        assert_eq!(
            report,
            ImportReport {
                added: 1,
                updated: 1,
                same: 0,
                skipped: 1,
                groups: 2,
            }
        );
        let piotr = store.contact(mine.id).unwrap();
        assert_eq!(piotr.name, "Piotr Nowak", "filled in");
        assert_eq!(piotr.emails, ["piotr@nowak-meble.pl"]);
        assert_eq!(piotr.phone, "22 111");
        let groups = store.contact_groups().unwrap();
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Klub", "Meble"], "not Google's myContacts");
        let meble = &groups[1];
        assert_eq!(meble.members.len(), 3);
        assert_eq!(meble.members[0].name, "Zażółć Gęślą");
        assert_eq!(meble.members[1].contact, Some(mine.id));
        assert_eq!(meble.members[2].contact, None);
        // Again: nothing new.
        let again = store.import_vcards(file.as_bytes()).unwrap();
        assert_eq!((again.added, again.updated, again.groups), (0, 0, 0));
        assert_eq!(again.same, 2);
        assert_eq!(store.contact_groups().unwrap()[1].members.len(), 3);

        // Out and into an empty address book: the same people and groups.
        let out = store.export_vcards(&[]).unwrap();
        let mut other = MailStore::in_memory().unwrap();
        other.import_vcards(out.as_bytes()).unwrap();
        let strip = |cs: Vec<Contact>| -> Vec<(String, String, Vec<String>, String)> {
            cs.into_iter()
                .map(|c| (c.uid, c.name, c.emails, c.phone))
                .collect()
        };
        assert_eq!(
            strip(other.contacts().unwrap()),
            strip(store.contacts().unwrap())
        );
        let shape = |gs: Vec<Group>| -> Vec<(String, Vec<(bool, String)>)> {
            gs.into_iter()
                .map(|g| {
                    let mut m: Vec<(bool, String)> = g
                        .members
                        .into_iter()
                        .map(|m| (m.contact.is_some(), m.email.to_lowercase()))
                        .collect();
                    m.sort();
                    (g.name, m)
                })
                .collect()
        };
        assert_eq!(
            shape(other.contact_groups().unwrap()),
            shape(store.contact_groups().unwrap())
        );
        // Only one: just that card, without the group's other addresses.
        let one = store.export_vcards(&[mine.id]).unwrap();
        assert_eq!(one.matches("BEGIN:VCARD").count(), 1);
        assert!(one.contains("CATEGORIES:Meble"));
        assert!(store.import_vcards(b"nothing here").is_err());
    }

    #[test]
    fn people_written_to_are_offered_until_they_are_contacts() {
        let mut store = sample_store();
        let offered = store.suggested_contacts(50).unwrap();
        assert!(!offered.is_empty());
        assert!(offered.iter().all(|r| r.sent > 0 && r.contact.is_none()));
        let first = offered[0].clone();
        store
            .save_contact(&Contact {
                name: first.name.clone(),
                emails: vec![first.email.clone()],
                ..Contact::default()
            })
            .unwrap();
        assert!(!store
            .suggested_contacts(50)
            .unwrap()
            .iter()
            .any(|r| r.email.eq_ignore_ascii_case(&first.email)));
    }
}
