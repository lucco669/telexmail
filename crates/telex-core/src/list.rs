//! The message list, a page at a time: counts for the folder tree, and pages of one folder
//! (or of all mail) in any order the list offers, with the quick filter applied in SQL.
//!
//! A folder's pages are found by keyset: "the 100 messages after this one in this order".
//! That costs the same at the top of a folder as deep inside it, and a page stays put when
//! new mail arrives above it. Only a jump with the scroll bar (`PageAt::Row`) counts rows
//! from the top, through the folder's index.
//!
//! A filtered list is a search: which messages match is worked out once, in order, and its
//! pages are read from that result until the mail base changes.

use std::collections::HashMap;

use rusqlite::functions::{Context, FunctionFlags};
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use crate::model::{Account, MessageId, MessageSummary};
use crate::store::{fts_query, summary_from_row, MailStore, StoreError, SUMMARY_COLUMNS};

/// The list's columns that it can be sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    #[default]
    Date,
    Size,
    /// Flagged first, then unread.
    Flag,
    /// The sender's name; the first recipient's in Sent, Drafts and the Outbox.
    From,
    /// The subject without "Re:" and "Fwd:".
    Subject,
}

/// Which messages a list shows, and in what order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ListView {
    /// A folder key ("firma/inbox"), or `None` for all mail: every folder of every
    /// account except Spam and Trash.
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub sort: SortKey,
    /// Oldest, smallest or A first. Lists start newest first.
    #[serde(default)]
    pub ascending: bool,
    /// The quick filter. It matches any part of the sender, recipients and subject,
    /// ignoring case; with `bodies` also words in the text, through the search index.
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub bodies: bool,
}

/// Where in the list a page starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageAt {
    /// The top of the list.
    First,
    /// The bottom.
    Last,
    /// The messages after this one: scrolling down.
    After(MessageId),
    /// The messages before this one: scrolling up.
    Before(MessageId),
    /// This message in the middle of the page, to keep the selection in view after a
    /// change. The first page when the message isn't in the list (any more).
    Around(MessageId),
    /// From this row on: a jump with the scroll bar.
    Row(usize),
}

/// One page of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Page {
    /// Messages in the whole list, for the scroll bar.
    pub total: usize,
    /// Position of the first row in the list.
    pub start: usize,
    pub rows: Vec<MessageSummary>,
}

/// How many messages a folder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct FolderCount {
    pub total: usize,
    pub unread: usize,
}

/// Everything the folder tree shows.
#[derive(Debug, Clone, Serialize)]
pub struct FolderTree {
    pub accounts: Vec<Account>,
    /// By folder key. Empty folders are left out.
    pub counts: HashMap<String, FolderCount>,
}

/// The largest page the UI can ask for.
pub const MAX_PAGE: usize = 500;

/// A filtered list, worked out once: the matching messages in order. `MailStore` keeps the
/// last one, so scrolling through it doesn't search again.
#[derive(Debug)]
pub(crate) struct Found {
    view: ListView,
    /// `Connection::total_changes` when it was worked out: any change since makes it stale.
    changes: u64,
    ids: Vec<MessageId>,
    at: HashMap<MessageId, usize>,
}

/// Folder keys of the given kinds, for SQL: `folder IN (…)`.
fn folders_of_kind(kinds: &str) -> String {
    format!("(SELECT account || '/' || id FROM folders WHERE kind IN ({kinds}))")
}

/// A list view turned into SQL: its WHERE clause with arguments, and its order.
struct Query {
    filter: String,
    args: Vec<Value>,
    /// Each term of the order: an SQL expression, and whether it runs high to low. The
    /// last term is always the id, so no two rows tie.
    order: Vec<(String, bool)>,
}

impl Query {
    fn new(store: &MailStore, view: &ListView) -> Result<Self, StoreError> {
        let mut filter = String::new();
        let mut args = Vec::new();
        match &view.folder {
            Some(key) => {
                if store.kind_of(key).is_none() {
                    return Err(StoreError::UnknownFolder(key.clone()));
                }
                filter.push_str("folder = ?");
                args.push(Value::Text(key.clone()));
            }
            None => {
                filter.push_str("folder NOT IN ");
                filter.push_str(&folders_of_kind("'spam', 'trash'"));
            }
        }
        let needle = view.filter.trim().to_lowercase();
        if !needle.is_empty() {
            filter.push_str(" AND (telex_has(?, from_name, from_email, subject, to_addr)");
            args.push(Value::Text(needle));
            let words = if view.bodies {
                fts_query(&view.filter)
            } else {
                String::new()
            };
            if !words.is_empty() {
                filter.push_str(
                    " OR id IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?)",
                );
                args.push(Value::Text(words));
            }
            filter.push(')');
        }

        let desc = !view.ascending;
        let who = match view.folder.as_deref().and_then(|k| store.kind_of(k)) {
            Some(kind) if kind.is_outgoing() => "telex_recipient(to_addr)".to_owned(),
            Some(_) => "telex_lower(from_name)".to_owned(),
            None => format!(
                "CASE WHEN folder IN {} THEN telex_recipient(to_addr) ELSE telex_lower(from_name) END",
                folders_of_kind("'sent', 'drafts', 'outbox'")
            ),
        };
        let key = match view.sort {
            SortKey::Date => "date".to_owned(),
            SortKey::Size => "size_kb".to_owned(),
            SortKey::Flag => "(flagged * 2 + unread)".to_owned(),
            SortKey::From => who,
            SortKey::Subject => "telex_subject(subject)".to_owned(),
        };
        // Ties: newest first, as the list always did. For dates, the id breaks them.
        let order = if view.sort == SortKey::Date {
            vec![(key, desc), ("id".to_owned(), desc)]
        } else {
            vec![
                (key, desc),
                ("date".to_owned(), true),
                ("id".to_owned(), true),
            ]
        };
        Ok(Self {
            filter,
            args,
            order,
        })
    }

    /// The order the other way round.
    fn reversed(&self) -> Vec<(String, bool)> {
        self.order.iter().map(|(e, d)| (e.clone(), !d)).collect()
    }

    /// The values of the order's terms for one message, or `None` if it doesn't exist.
    fn keys(&self, db: &Connection, id: MessageId) -> Result<Option<Vec<Value>>, StoreError> {
        let cols: Vec<&str> = self.order.iter().map(|(e, _)| e.as_str()).collect();
        let sql = format!("SELECT {} FROM messages WHERE id = ?1", cols.join(", "));
        let mut stmt = db.prepare_cached(&sql)?;
        let mut rows = stmt.query([id as i64])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let keys = (0..cols.len())
            .map(|i| row.get::<_, Value>(i))
            .collect::<Result<_, _>>()?;
        Ok(Some(keys))
    }

    /// Up to `limit` rows in `order`, after the row whose keys are `after` if given.
    fn rows(
        &self,
        db: &Connection,
        order: &[(String, bool)],
        after: Option<Vec<Value>>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MessageSummary>, StoreError> {
        let mut sql = format!(
            "SELECT {SUMMARY_COLUMNS} FROM messages WHERE {}",
            self.filter
        );
        let mut args = self.args.clone();
        if let Some(keys) = after {
            let (cond, values) = beyond(order, &keys);
            sql.push_str(" AND ");
            sql.push_str(&cond);
            args.extend(values);
        }
        let terms: Vec<String> = order
            .iter()
            .map(|(e, desc)| format!("{e} {}", if *desc { "DESC" } else { "ASC" }))
            .collect();
        sql.push_str(" ORDER BY ");
        sql.push_str(&terms.join(", "));
        sql.push_str(" LIMIT ? OFFSET ?");
        args.push(Value::Integer(limit as i64));
        args.push(Value::Integer(offset as i64));
        let mut stmt = db.prepare_cached(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(args), summary_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// How many rows the list has; with `before`, how many come before that row.
    fn count(&self, db: &Connection, before: Option<Vec<Value>>) -> Result<usize, StoreError> {
        let mut sql = format!("SELECT COUNT(*) FROM messages WHERE {}", self.filter);
        let mut args = self.args.clone();
        if let Some(keys) = before {
            let (cond, values) = beyond(&self.reversed(), &keys);
            sql.push_str(" AND ");
            sql.push_str(&cond);
            args.extend(values);
        }
        let n: i64 = db
            .prepare_cached(&sql)?
            .query_row(params_from_iter(args), |r| r.get(0))?;
        Ok(usize::try_from(n).unwrap_or_default())
    }

    /// Every message of the list, in order.
    fn ids(&self, db: &Connection) -> Result<Vec<MessageId>, StoreError> {
        let terms: Vec<String> = self
            .order
            .iter()
            .map(|(e, desc)| format!("{e} {}", if *desc { "DESC" } else { "ASC" }))
            .collect();
        let sql = format!(
            "SELECT id FROM messages WHERE {} ORDER BY {}",
            self.filter,
            terms.join(", ")
        );
        let mut stmt = db.prepare_cached(&sql)?;
        let ids = stmt
            .query_map(params_from_iter(self.args.iter()), |r| r.get::<_, i64>(0))?
            .map(|id| id.map(|id| id as MessageId))
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    fn contains(&self, db: &Connection, id: MessageId) -> Result<bool, StoreError> {
        let sql = format!(
            "SELECT EXISTS (SELECT 1 FROM messages WHERE id = ? AND {})",
            self.filter
        );
        let mut args = vec![Value::Integer(id as i64)];
        args.extend(self.args.iter().cloned());
        Ok(db
            .prepare_cached(&sql)?
            .query_row(params_from_iter(args), |r| r.get(0))?)
    }
}

/// SQL for "comes after the row with these keys" in this order, and the values to bind.
/// Terms that all run the same way become one row-value comparison, which SQLite answers
/// from an index; mixed ones are spelled out: a > ? OR (a = ? AND (b < ? OR (b = ? AND …))).
fn beyond(order: &[(String, bool)], keys: &[Value]) -> (String, Vec<Value>) {
    let op = |desc: bool| if desc { "<" } else { ">" };
    if order.iter().all(|(_, d)| *d == order[0].1) {
        let cols: Vec<&str> = order.iter().map(|(e, _)| e.as_str()).collect();
        let marks = vec!["?"; order.len()].join(", ");
        let sql = format!("({}) {} ({marks})", cols.join(", "), op(order[0].1));
        return (sql, keys.to_vec());
    }
    let last = order.len() - 1;
    let mut sql = format!("{} {} ?", order[last].0, op(order[last].1));
    let mut values = vec![keys[last].clone()];
    for i in (0..last).rev() {
        let (e, desc) = &order[i];
        sql = format!("({e} {} ? OR ({e} = ? AND {sql}))", op(*desc));
        values.splice(0..0, [keys[i].clone(), keys[i].clone()]);
    }
    (sql, values)
}

impl MailStore {
    /// Accounts with their folders, and how many messages, and unread ones, each folder holds.
    pub fn folder_tree(&self) -> Result<FolderTree, StoreError> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT folder, COUNT(*), SUM(unread) FROM messages GROUP BY folder")?;
        let counts = stmt
            .query_map([], |r| {
                let (total, unread): (i64, i64) = (r.get(1)?, r.get(2)?);
                Ok((
                    r.get::<_, String>(0)?,
                    FolderCount {
                        total: usize::try_from(total).unwrap_or_default(),
                        unread: usize::try_from(unread).unwrap_or_default(),
                    },
                ))
            })?
            .collect::<Result<_, _>>()?;
        Ok(FolderTree {
            accounts: self.accounts().to_vec(),
            counts,
        })
    }

    /// One page of a list: up to `limit` summaries in the view's order, where it says.
    /// An `After` or `Before` message that no longer exists (or, in a filtered list, no
    /// longer matches) is an `UnknownMessage` error; the UI then asks for the row instead.
    pub fn page(&self, view: &ListView, at: PageAt, limit: usize) -> Result<Page, StoreError> {
        let limit = limit.clamp(1, MAX_PAGE);
        let q = Query::new(self, view)?;
        if view.filter.trim().is_empty() {
            self.keyset_page(&q, at, limit)
        } else {
            self.found_page(&q, view, at, limit)
        }
    }

    fn keyset_page(&self, q: &Query, at: PageAt, limit: usize) -> Result<Page, StoreError> {
        let db = &self.db;
        let total = q.count(db, None)?;
        let keys = |id| q.keys(db, id)?.ok_or(StoreError::UnknownMessage(id));
        let (rows, start) = match at {
            PageAt::First => (q.rows(db, &q.order, None, limit, 0)?, Some(0)),
            PageAt::Last => {
                let mut rows = q.rows(db, &q.reversed(), None, limit, 0)?;
                rows.reverse();
                let start = total.saturating_sub(rows.len());
                (rows, Some(start))
            }
            PageAt::After(id) => (q.rows(db, &q.order, Some(keys(id)?), limit, 0)?, None),
            PageAt::Before(id) => {
                let mut rows = q.rows(db, &q.reversed(), Some(keys(id)?), limit, 0)?;
                rows.reverse();
                (rows, None)
            }
            PageAt::Around(id) if q.contains(db, id)? => {
                let mut rows = q.rows(db, &q.reversed(), Some(keys(id)?), limit / 2, 0)?;
                rows.reverse();
                let after = limit.saturating_sub(rows.len() + 1);
                rows.push(self.summary(id)?);
                if after > 0 {
                    rows.extend(q.rows(db, &q.order, Some(keys(id)?), after, 0)?);
                }
                (rows, None)
            }
            PageAt::Around(_) => (q.rows(db, &q.order, None, limit, 0)?, Some(0)),
            PageAt::Row(n) => {
                let offset = n.min(total.saturating_sub(limit));
                (q.rows(db, &q.order, None, limit, offset)?, Some(offset))
            }
        };
        let start = match (start, rows.first()) {
            (Some(start), _) => start,
            (None, Some(first)) => q.count(db, Some(keys(first.id)?))?,
            (None, None) => total,
        };
        Ok(Page { total, start, rows })
    }

    /// A page of a filtered list, from the matches worked out by the last call for the same
    /// list, or worked out now.
    fn found_page(
        &self,
        q: &Query,
        view: &ListView,
        at: PageAt,
        limit: usize,
    ) -> Result<Page, StoreError> {
        let changes = self.db.total_changes();
        let mut found = self.found.borrow_mut();
        let found = match found.take() {
            Some(f) if f.view == *view && f.changes == changes => found.insert(f),
            _ => {
                let ids = q.ids(&self.db)?;
                let at = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
                found.insert(Found {
                    view: view.clone(),
                    changes,
                    ids,
                    at,
                })
            }
        };
        let total = found.ids.len();
        let place = |id| {
            found
                .at
                .get(&id)
                .copied()
                .ok_or(StoreError::UnknownMessage(id))
        };
        let (start, end) = match at {
            PageAt::First => (0, limit),
            PageAt::Last => (total.saturating_sub(limit), total),
            PageAt::After(id) => {
                let i = place(id)? + 1;
                (i, i + limit)
            }
            PageAt::Before(id) => {
                let i = place(id)?;
                (i.saturating_sub(limit), i)
            }
            PageAt::Around(id) => {
                let i = place(id).map_or(0, |i| i.saturating_sub(limit / 2));
                (i, i + limit)
            }
            PageAt::Row(n) => {
                let i = n.min(total.saturating_sub(limit));
                (i, i + limit)
            }
        };
        let end = end.min(total);
        let start = start.min(end);
        Ok(Page {
            total,
            start,
            rows: self.summaries(&found.ids[start..end])?,
        })
    }

    /// The list rows of these messages, in this order.
    fn summaries(&self, ids: &[MessageId]) -> Result<Vec<MessageSummary>, StoreError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let marks = vec!["?"; ids.len()].join(", ");
        let sql = format!("SELECT {SUMMARY_COLUMNS} FROM messages WHERE id IN ({marks})");
        let mut stmt = self.db.prepare_cached(&sql)?;
        let mut rows: HashMap<MessageId, MessageSummary> = stmt
            .query_map(
                params_from_iter(ids.iter().map(|&id| id as i64)),
                summary_from_row,
            )?
            .map(|r| r.map(|m| (m.id, m)))
            .collect::<Result<_, _>>()?;
        Ok(ids.iter().filter_map(|id| rows.remove(id)).collect())
    }
}

/// SQL functions the list uses to sort and filter the way the window always has:
/// Unicode-aware lowercase, the recipient's display name, the subject without prefixes.
pub(crate) fn register_functions(db: &Connection) -> rusqlite::Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    fn text<'a>(ctx: &'a Context<'_>, i: usize) -> &'a str {
        ctx.get_raw(i).as_str().unwrap_or("")
    }
    db.create_scalar_function("telex_lower", 1, flags, |ctx| {
        Ok(text(ctx, 0).to_lowercase())
    })?;
    db.create_scalar_function("telex_recipient", 1, flags, |ctx| {
        Ok(display_name(text(ctx, 0)).to_lowercase())
    })?;
    db.create_scalar_function("telex_subject", 1, flags, |ctx| {
        Ok(subject_key(text(ctx, 0)))
    })?;
    // telex_has(needle, field, …): the fields, joined by spaces and lowercased, contain
    // the (already lowercased) needle. Without spaces in it, a match can't span two
    // fields, so each is searched on its own, without copying when it is plain ASCII.
    db.create_scalar_function("telex_has", -1, flags, |ctx| {
        let needle = text(ctx, 0);
        let mut fields = (1..ctx.len()).map(|i| text(ctx, i));
        if needle.contains(char::is_whitespace) {
            let hay = fields.collect::<Vec<_>>().join(" ");
            return Ok(hay.to_lowercase().contains(needle));
        }
        Ok(fields.any(|field| contains_folded(field, needle)))
    })?;
    Ok(())
}

/// Whether `hay` contains `needle` (already lowercase), ignoring case.
fn contains_folded(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if hay.is_ascii() {
        // ASCII lowercases to ASCII, so only an ASCII needle can match.
        let needle = needle.as_bytes();
        needle.is_ascii()
            && hay
                .as_bytes()
                .windows(needle.len())
                .any(|w| w.eq_ignore_ascii_case(needle))
    } else {
        hay.to_lowercase().contains(needle)
    }
}

/// The name in "Anna Kowalska <anna@x.pl>", else the whole text, as the list shows it.
pub(crate) fn display_name(addr: &str) -> &str {
    let s = addr.trim();
    if s.is_empty() {
        return "(no recipient)";
    }
    let rest = s.strip_prefix('"').unwrap_or(s);
    let end = rest.find(['"', '<']).unwrap_or(rest.len());
    let name = rest[..end].trim_end();
    let tail = &rest[end..];
    let tail = tail.strip_prefix('"').unwrap_or(tail).trim_start();
    if !name.is_empty() && tail.starts_with('<') {
        name
    } else {
        s
    }
}

/// What the subject sorts by: lowercase, without leading "Re:", "Fw:" and "Fwd:".
pub(crate) fn subject_key(subject: &str) -> String {
    let lower = subject.to_lowercase();
    let mut s = lower.as_str();
    while let Some(rest) = ["re:", "fwd:", "fw:"]
        .iter()
        .find_map(|p| s.strip_prefix(p))
    {
        s = rest.trim_start();
    }
    s.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSetup, Attachment, Folder, FolderKind, Message};
    use crate::store::SyncedMessage;
    use chrono::{NaiveDate, TimeDelta};
    use std::cmp::Ordering;

    const SENDERS: &[&str] = &[
        "Łukasz Żak",
        "anna Kowalska",
        "Anna Kowalska",
        "Zofia Nowak",
        "",
        "Ćwik Piotr",
        "Émile Zola",
        "zbigniew",
    ];
    const SUBJECTS: &[&str] = &[
        "Re: Wycena",
        "wycena",
        "Fwd: Re: Projekt",
        "Faktura",
        "RE:  Ładny dom",
        "fw: Zdjęcia",
        "Odp: Termin",
        "Żółty kanał",
    ];
    const RECIPIENTS: &[&str] = &[
        "\"Kowalska, Anna\" <anna@x.pl>",
        "Piotr <p@x.pl>",
        "zofia@x.pl",
        "Łucja Ł <l@x.pl>",
        "",
        "<bez@nazwy.pl>",
    ];

    /// A server account whose Inbox, Sent, Spam and Trash hold `n` messages each, with
    /// ties in every column the list sorts by.
    fn store(n: usize) -> MailStore {
        let mut store = MailStore::in_memory().unwrap();
        let account = store
            .upsert_account(&AccountSetup {
                name: "Studio".into(),
                email: "biuro@studio.pl".into(),
                host: "mail.studio.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        let folders = [
            ("inbox", FolderKind::Inbox),
            ("sent", FolderKind::Sent),
            ("spam", FolderKind::Spam),
            ("trash", FolderKind::Trash),
        ];
        let list: Vec<Folder> = folders
            .iter()
            .map(|(id, kind)| Folder {
                id: (*id).into(),
                name: (*id).into(),
                kind: *kind,
                remote: Some(id.to_uppercase()),
            })
            .collect();
        store.set_folders(&account.id, &list).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 10, 1)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        for (f, (folder, _)) in folders.iter().enumerate() {
            let batch: Vec<SyncedMessage> = (0..n)
                .map(|i| {
                    let k = i + f;
                    SyncedMessage {
                        uid: i as u32 + 1,
                        message: Message {
                            id: 0,
                            folder: String::new(),
                            from_name: SENDERS[k % SENDERS.len()].into(),
                            from_email: format!("osoba{}@x.pl", k % 5),
                            to: RECIPIENTS[k % RECIPIENTS.len()].into(),
                            cc: String::new(),
                            subject: SUBJECTS[k % SUBJECTS.len()].into(),
                            // Every three messages share a date.
                            date: day + TimeDelta::hours((i / 3) as i64),
                            size_kb: (k % 4) as u32 + 1,
                            unread: k % 3 == 0,
                            flagged: k % 5 == 0,
                            remote_images: false,
                            note: None,
                            attachments: if k % 4 == 0 {
                                vec![Attachment {
                                    name: "a.pdf".into(),
                                    size_kb: 1,
                                }]
                            } else {
                                vec![]
                            },
                            body: if k % 6 == 0 {
                                "Przesyłam zdjęcie żółwia.".into()
                            } else {
                                "Dzień dobry.".into()
                            },
                            message_id: Some(format!("{folder}.{i}@x.pl")),
                            bcc: String::new(),
                            refs: Vec::new(),
                        },
                        raw: b"x".to_vec(),
                    }
                })
                .collect();
            store
                .add_synced(&account.folder_key(folder), &batch)
                .unwrap();
        }
        store
    }

    /// The order the list promises, written out the slow way.
    fn reference_order(rows: &mut [MessageSummary], view: &ListView, outgoing: bool) {
        let who = |m: &MessageSummary| {
            if outgoing {
                display_name(&m.to).to_lowercase()
            } else {
                m.from_name.to_lowercase()
            }
        };
        rows.sort_by(|a, b| {
            let key = match view.sort {
                SortKey::Date => a.date.cmp(&b.date).then(a.id.cmp(&b.id)),
                SortKey::Size => a.size_kb.cmp(&b.size_kb),
                SortKey::Flag => {
                    let f = |m: &MessageSummary| u8::from(m.flagged) * 2 + u8::from(m.unread);
                    f(a).cmp(&f(b))
                }
                SortKey::From => who(a).cmp(&who(b)),
                SortKey::Subject => subject_key(&a.subject).cmp(&subject_key(&b.subject)),
            };
            let key = if view.ascending { key } else { key.reverse() };
            match (key, view.sort) {
                (Ordering::Equal, SortKey::Date) => Ordering::Equal,
                (Ordering::Equal, _) => b.date.cmp(&a.date).then(b.id.cmp(&a.id)),
                (o, _) => o,
            }
        });
    }

    fn matches(m: &MessageSummary, filter: &str) -> bool {
        let needle = filter.trim().to_lowercase();
        [&m.from_name, &m.from_email, &m.subject, &m.to]
            .map(|s| s.as_str())
            .join(" ")
            .to_lowercase()
            .contains(&needle)
    }

    fn ids(rows: &[MessageSummary]) -> Vec<MessageId> {
        rows.iter().map(|m| m.id).collect()
    }

    #[test]
    fn pages_walk_the_list_in_every_order() {
        let store = store(40);
        let sorts = [
            SortKey::Date,
            SortKey::Size,
            SortKey::Flag,
            SortKey::From,
            SortKey::Subject,
        ];
        for folder in ["studio/inbox", "studio/sent"] {
            for sort in sorts {
                for ascending in [false, true] {
                    for filter in ["", "kow", " ŁUCJA ", "WYC"] {
                        let view = ListView {
                            folder: Some(folder.into()),
                            sort,
                            ascending,
                            filter: filter.into(),
                            bodies: false,
                        };
                        check_paging(&store, &view, folder.ends_with("sent"));
                    }
                }
            }
        }
    }

    fn check_paging(store: &MailStore, view: &ListView, outgoing: bool) {
        let folder = view.folder.as_deref().unwrap();
        let mut all: Vec<_> = store
            .list(folder)
            .unwrap()
            .into_iter()
            .filter(|m| matches(m, &view.filter))
            .collect();
        reference_order(&mut all, view, outgoing);
        let all = ids(&all);
        let what = format!("{view:?}");

        // Down from the top, one page after the other.
        let mut seen = Vec::new();
        let mut page = store.page(view, PageAt::First, 7).unwrap();
        loop {
            assert_eq!(page.total, all.len(), "{what}");
            assert_eq!(page.start, seen.len(), "{what}");
            seen.extend(ids(&page.rows));
            let Some(last) = page.rows.last() else { break };
            page = store.page(view, PageAt::After(last.id), 7).unwrap();
        }
        assert_eq!(seen, all, "{what}: down");

        // Up from the bottom.
        let mut seen = Vec::new();
        let mut page = store.page(view, PageAt::Last, 6).unwrap();
        while let Some(first) = page.rows.first() {
            assert_eq!(
                page.start + page.rows.len() + seen.len(),
                all.len(),
                "{what}"
            );
            let mut rows = ids(&page.rows);
            rows.append(&mut seen);
            seen = rows;
            page = store.page(view, PageAt::Before(first.id), 6).unwrap();
        }
        assert_eq!(seen, all, "{what}: up");

        // Jumps, and a page around one message.
        for n in [0, 5, all.len().saturating_sub(3), all.len() + 10] {
            let page = store.page(view, PageAt::Row(n), 5).unwrap();
            let start = n.min(all.len().saturating_sub(5));
            assert_eq!(page.start, start, "{what}: row {n}");
            let end = (start + 5).min(all.len());
            assert_eq!(ids(&page.rows), all[start..end], "{what}: row {n}");
        }
        for &id in all.iter().step_by(4) {
            let page = store.page(view, PageAt::Around(id), 5).unwrap();
            let at = all.iter().position(|&x| x == id).unwrap();
            let end = page.start + page.rows.len();
            assert!(page.start <= at && at < end, "{what}: around {id}");
            assert_eq!(ids(&page.rows), all[page.start..end], "{what}: around {id}");
        }
    }

    #[test]
    fn the_filter_finds_words_in_the_text_only_when_asked() {
        let store = store(12);
        let mut view = ListView {
            folder: Some("studio/inbox".into()),
            filter: "zolwia".into(),
            ..Default::default()
        };
        assert_eq!(store.page(&view, PageAt::First, 50).unwrap().total, 0);
        view.bodies = true;
        let page = store.page(&view, PageAt::First, 50).unwrap();
        assert_eq!(page.total, 2, "messages 0 and 6 of the Inbox say żółwia");
        for m in &page.rows {
            assert!(store.get(m.id).unwrap().body.contains("żółwia"));
        }
        // Headers still match as typed, with the text search on.
        view.filter = "owalsk".into();
        let by_header = store.list("studio/inbox").unwrap();
        let by_header = by_header.iter().filter(|m| matches(m, "owalsk")).count();
        assert!(by_header > 0);
        assert_eq!(
            store.page(&view, PageAt::First, 50).unwrap().total,
            by_header
        );
    }

    #[test]
    fn a_filtered_list_notices_changes() {
        let mut store = store(12);
        let view = ListView {
            folder: Some("studio/inbox".into()),
            filter: "wycena".into(),
            ..Default::default()
        };
        let first = store.page(&view, PageAt::First, 2).unwrap();
        let gone = first.rows[0].id;
        store.forget_message(gone).unwrap();
        let again = store.page(&view, PageAt::First, 2).unwrap();
        assert_eq!(again.total, first.total - 1);
        assert_eq!(again.rows[0].id, first.rows[1].id);
        assert_eq!(
            store.page(&view, PageAt::After(gone), 2),
            Err(StoreError::UnknownMessage(gone))
        );
    }

    #[test]
    fn the_header_match_is_the_old_one_made_faster() {
        let db = Connection::open_in_memory().unwrap();
        register_functions(&db).unwrap();
        let has = |needle: &str, fields: [&str; 3]| -> bool {
            db.query_row(
                "SELECT telex_has(?1, ?2, ?3, ?4)",
                [needle, fields[0], fields[1], fields[2]],
                |r| r.get(0),
            )
            .unwrap()
        };
        let fields = ["Łukasz Żak", "LUKASZ@ZAK.PL", "Re: Wycena"];
        assert!(has("łukasz", fields));
        assert!(has("kasz@z", fields));
        assert!(
            has("żak lukasz@", fields),
            "a match across two fields, as before"
        );
        assert!(has("wycena", fields));
        assert!(!has("lukasz zak", fields));
        assert!(!has("ż", ["zak", "", ""]));
        assert!(
            has("kelvin", ["\u{212A}ELVIN", "", ""]),
            "K (Kelvin sign) lowercases to k"
        );
    }

    #[test]
    fn all_mail_leaves_out_spam_and_trash() {
        let store = store(10);
        let view = ListView {
            folder: None,
            filter: "wycena".into(),
            ..Default::default()
        };
        let page = store.page(&view, PageAt::First, 100).unwrap();
        assert!(page.total > 0);
        assert_eq!(page.rows.len(), page.total);
        for m in &page.rows {
            assert!(
                m.folder == "studio/inbox" || m.folder == "studio/sent",
                "{}",
                m.folder
            );
        }
        let everything = ListView::default();
        assert_eq!(store.page(&everything, PageAt::Last, 3).unwrap().total, 20);
    }

    #[test]
    fn missing_folders_and_messages_are_errors() {
        let store = store(3);
        let view = ListView {
            folder: Some("studio/inbox".into()),
            ..Default::default()
        };
        assert_eq!(
            store.page(&view, PageAt::After(9_999), 10),
            Err(StoreError::UnknownMessage(9_999))
        );
        // Around a message that isn't in the list: the top of the list.
        let sent = store.list("studio/sent").unwrap()[0].id;
        let page = store.page(&view, PageAt::Around(sent), 10).unwrap();
        assert_eq!((page.start, page.rows.len()), (0, 3));
        let nowhere = ListView {
            folder: Some("studio/nope".into()),
            ..Default::default()
        };
        assert!(matches!(
            store.page(&nowhere, PageAt::First, 10),
            Err(StoreError::UnknownFolder(_))
        ));
    }

    #[test]
    fn the_tree_counts_all_and_unread() {
        let mut store = store(9);
        let inbox = store.list("studio/inbox").unwrap();
        let counts = store.folder_tree().unwrap().counts;
        let c = counts["studio/inbox"];
        assert_eq!(c.total, 9);
        assert_eq!(c.unread, inbox.iter().filter(|m| m.unread).count());
        assert!(
            !counts.contains_key("studio/outbox"),
            "empty folders are left out"
        );
        let unread = inbox.iter().find(|m| m.unread).unwrap().id;
        store.set_unread(unread, false).unwrap();
        let after = store.folder_tree().unwrap().counts["studio/inbox"];
        assert_eq!(after.unread, c.unread - 1);
    }

    #[test]
    fn the_window_s_requests_are_understood() {
        // As ui/app.js sends them through the bridge.
        let view: ListView = serde_json::from_str(
            r#"{"folder":null,"sort":"subject","ascending":true,"filter":"kow","bodies":true}"#,
        )
        .unwrap();
        assert_eq!(view.folder, None);
        assert_eq!(view.sort, SortKey::Subject);
        assert!(view.ascending && view.bodies);
        let at = |json: &str| serde_json::from_str::<PageAt>(json).unwrap();
        assert_eq!(at(r#""first""#), PageAt::First);
        assert_eq!(at(r#""last""#), PageAt::Last);
        assert_eq!(at(r#"{"after":7}"#), PageAt::After(7));
        assert_eq!(at(r#"{"before":7}"#), PageAt::Before(7));
        assert_eq!(at(r#"{"around":7}"#), PageAt::Around(7));
        assert_eq!(at(r#"{"row":20000}"#), PageAt::Row(20_000));
        let page = serde_json::to_value(store(1).page(&view, PageAt::First, 5).unwrap()).unwrap();
        assert!(page["total"].is_number() && page["start"].is_number() && page["rows"].is_array());
        let tree = serde_json::to_value(store(1).folder_tree().unwrap()).unwrap();
        assert_eq!(tree["counts"]["studio/inbox"]["total"], 1);
    }

    #[test]
    fn names_and_subjects_sort_as_the_list_shows_them() {
        assert_eq!(display_name("Anna Kowalska <anna@x.pl>"), "Anna Kowalska");
        assert_eq!(
            display_name("\"Kowalska, Anna\" <anna@x.pl>"),
            "Kowalska, Anna"
        );
        assert_eq!(display_name("  Piotr   <p@x.pl>, Ewa <e@x.pl>"), "Piotr");
        assert_eq!(display_name("zofia@x.pl, ewa@x.pl"), "zofia@x.pl, ewa@x.pl");
        assert_eq!(display_name("<bez@nazwy.pl>"), "<bez@nazwy.pl>");
        assert_eq!(display_name("  "), "(no recipient)");
        assert_eq!(subject_key("Re: RE:Fwd:  Wycena"), "wycena");
        assert_eq!(subject_key("Fw: ŁADNY dom"), "ładny dom");
        assert_eq!(
            subject_key("Odp: Termin"),
            "odp: termin",
            "only Re and Fwd, as before"
        );
        assert_eq!(subject_key("Rekrutacja"), "rekrutacja");
    }
}
