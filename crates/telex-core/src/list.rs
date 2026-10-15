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

use std::collections::{HashMap, HashSet};

use rusqlite::functions::{Context, FunctionFlags};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::model::{Account, FolderKind, MessageId, MessageSummary, ThreadRow};
use crate::store::{fts_query, summary_from_row, MailStore, StoreError, LOCAL_ID, SUMMARY_COLUMNS};

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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListView {
    /// A folder key ("firma/inbox"), `"*/drafts"` for that kind of folder in every account
    /// (see [`every_kind`]), or `None` for all mail: every folder of every account except
    /// Spam and Trash.
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub sort: SortKey,
    /// Oldest, smallest or A first. Lists start newest first.
    #[serde(default)]
    pub ascending: bool,
    /// The quick filter. It matches any part of the sender, recipients and subject,
    /// ignoring case; with `bodies` also words in the text, through the search index. It
    /// also takes operators (see [`Terms`]): `from:anna is:unread after:2026-09`.
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub bodies: bool,
    /// Show the conversation this message belongs to, in every folder of its account
    /// except Spam and Trash; `folder` doesn't count then.
    #[serde(default)]
    pub thread: Option<MessageId>,
    /// Only unread messages: of the folder, or with `folder: None` of all mail (the Unread
    /// list). Those in `keep` stay too.
    #[serde(default)]
    pub unread: bool,
    /// Messages an unread list keeps after they were read: the window passes the ones read
    /// while the list is open, so a message doesn't vanish from under the reader.
    #[serde(default)]
    pub keep: Vec<MessageId>,
    /// Collapse related messages into one row, with the given groups expanded.
    #[serde(default)]
    pub grouped: bool,
    #[serde(default)]
    pub expanded: Vec<MessageId>,
}

/// The quick filter, read: plain words, and operators. Unknown operators and values that
/// don't make sense are plain words; an operator without a value (still being typed) is
/// left out.
///
/// `from:` `to:` (To, Cc and Bcc) `subject:` match any part, case ignored; `is:unread`
/// `is:read` `is:flagged` `is:unflagged`; `has:file`; `after:` and `before:` a year, month or
/// day (2026, 2026-09, 2026-09-15 or 15.09.2026: after is from its start, before is before
/// its start); `larger:` and `smaller:` a size (500k, 5m, 1g; KB without a unit). Values with
/// spaces go in quotes: `from:"Anna Kowalska"`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Terms {
    /// The plain words, joined by spaces.
    pub text: String,
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub subject: Vec<String>,
    pub unread: Option<bool>,
    pub flagged: Option<bool>,
    pub has_file: bool,
    /// "2026-09-01 00:00:00": on or after, and before.
    pub after: Option<String>,
    pub before: Option<String>,
    /// KB: at least, and less than.
    pub larger_kb: Option<u64>,
    pub smaller_kb: Option<u64>,
}

impl Terms {
    pub fn parse(filter: &str) -> Self {
        let mut terms = Terms::default();
        let mut words = Vec::new();
        for token in tokens(filter) {
            let Some((key, value)) = token.split_once(':') else {
                words.push(token);
                continue;
            };
            let value = value.trim_matches('"').trim();
            let known = matches!(
                key.to_ascii_lowercase().as_str(),
                "from"
                    | "to"
                    | "subject"
                    | "is"
                    | "has"
                    | "after"
                    | "before"
                    | "larger"
                    | "smaller"
            );
            if known && value.is_empty() {
                continue; // still being typed
            }
            let used = match key.to_ascii_lowercase().as_str() {
                "from" => {
                    terms.from.push(value.to_lowercase());
                    true
                }
                "to" => {
                    terms.to.push(value.to_lowercase());
                    true
                }
                "subject" => {
                    terms.subject.push(value.to_lowercase());
                    true
                }
                "is" => {
                    let (unread, flagged) = match value.to_ascii_lowercase().as_str() {
                        "unread" | "new" => (Some(true), None),
                        "read" | "seen" => (Some(false), None),
                        "flagged" | "starred" => (None, Some(true)),
                        "unflagged" => (None, Some(false)),
                        _ => (None, None),
                    };
                    terms.unread = unread.or(terms.unread);
                    terms.flagged = flagged.or(terms.flagged);
                    unread.is_some() || flagged.is_some()
                }
                "has" => {
                    let file = matches!(
                        value.to_ascii_lowercase().as_str(),
                        "file" | "files" | "attachment" | "attachments"
                    );
                    terms.has_file |= file;
                    file
                }
                "after" => parse_day(value).map(|d| terms.after = Some(d)).is_some(),
                "before" => parse_day(value).map(|d| terms.before = Some(d)).is_some(),
                "larger" => parse_kb(value).map(|k| terms.larger_kb = Some(k)).is_some(),
                "smaller" => parse_kb(value)
                    .map(|k| terms.smaller_kb = Some(k))
                    .is_some(),
                _ => false,
            };
            if !used {
                words.push(token);
            }
        }
        terms.text = words.join(" ");
        terms
    }
}

/// The filter split at spaces, except inside quotes; the quotes of a plain phrase go.
fn tokens(filter: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in filter.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                if current.contains(':') {
                    current.push(c); // key:"value", unquoted by the parser
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The first moment of a year, month or day: "2026", "2026-09", "2026/09/15", "15.09.2026",
/// "09.2026" → "2026-09-15 00:00:00".
fn parse_day(value: &str) -> Option<String> {
    let parts: Vec<&str> = value.split(['-', '/', '.']).collect();
    let num = |s: &str| s.parse::<u32>().ok();
    let (y, m, d) = match parts.as_slice() {
        [y] if y.len() == 4 => (num(y)?, 1, 1),
        [y, m] if y.len() == 4 => (num(y)?, num(m)?, 1),
        [m, y] if y.len() == 4 => (num(y)?, num(m)?, 1),
        [y, m, d] if y.len() == 4 => (num(y)?, num(m)?, num(d)?),
        [d, m, y] if y.len() == 4 => (num(y)?, num(m)?, num(d)?),
        _ => return None,
    };
    let day = chrono::NaiveDate::from_ymd_opt(i32::try_from(y).ok()?, m, d)?;
    Some(format!("{day} 00:00:00"))
}

/// A size in KB: "500k", "5m", "5 MB", "1.5g", "300" (KB).
fn parse_kb(value: &str) -> Option<u64> {
    let v = value.trim().to_ascii_lowercase();
    let v = v.trim_end_matches('b').trim();
    let (number, unit) = match v.char_indices().find(|(_, c)| c.is_ascii_alphabetic()) {
        Some((i, _)) => (v[..i].trim(), &v[i..]),
        None => (v, "k"),
    };
    let n: f64 = number.replace(',', ".").parse().ok()?;
    let factor = match unit {
        "k" => 1.0,
        "m" => 1024.0,
        "g" => 1024.0 * 1024.0,
        _ => return None,
    };
    (n >= 0.0).then(|| (n * factor).round() as u64)
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
    /// Visible representative when Around targeted a collapsed message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus: Option<MessageId>,
}

/// How many messages a folder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct FolderCount {
    pub total: usize,
    pub unread: usize,
    /// Older messages on the server that weren't downloaded (the first download takes the
    /// newest 300), as of the last sync.
    #[serde(skip_serializing_if = "is_zero")]
    pub older: usize,
    /// In an Outbox: messages that didn't go (a server refused them, the last try failed,
    /// or the user took them back), rather than waiting their turn.
    #[serde(skip_serializing_if = "is_zero")]
    pub stuck: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Everything the folder tree shows.
#[derive(Debug, Clone, Serialize)]
pub struct FolderTree {
    pub accounts: Vec<Account>,
    /// By folder key. Empty folders are left out.
    pub counts: HashMap<String, FolderCount>,
    /// Why an account's last check failed, by account id; gone once a check works.
    pub problems: HashMap<String, String>,
    /// The away replies that are on.
    pub away: Vec<crate::away::AwayOn>,
    /// Each account's storage on its server, when the server says, and its last good check:
    /// for the status bar and Settings › Accounts, without working out the whole Usage tab.
    pub quota: HashMap<String, crate::usage::Quota>,
    pub synced: HashMap<String, chrono::NaiveDateTime>,
}

/// An unread message and where it is in its list: what Space goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UnreadAt {
    pub id: MessageId,
    /// Its row in the list, counted from 0.
    pub row: usize,
    /// Expand this group before selecting a hidden unread message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<MessageId>,
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

/// The reply graph of an account (or of every account): which conversation each message is
/// in. Worked out again only when mail comes, goes, moves or gets new headers (`shape`), not
/// when it is read or flagged.
#[derive(Debug)]
pub(crate) struct Graph {
    /// The account ("" for all), and the Spam or Trash folder the list shows, if it does.
    scope: (String, String),
    shape: i64,
    /// Each message's conversation: an index into `keys`.
    conversation: HashMap<MessageId, usize>,
    /// The oldest stored id of each conversation, even when it is in another folder.
    keys: Vec<MessageId>,
}

/// A list's thread rows: the list's messages in groups, in its order. Only ids are cached;
/// summaries and read states stay in SQLite and are read a page at a time.
#[derive(Debug)]
pub(crate) struct Grouped {
    view: ListView,
    shape: i64,
    /// `total_changes` too, for lists whose members or order depend on read and flag states.
    changes: Option<u64>,
    groups: Vec<Vec<MessageId>>,
    /// The conversation's id of each group (see `Graph::keys`): what `expanded` names.
    keys: Vec<MessageId>,
    /// Where each message is: its group, and its place in the group.
    member: HashMap<MessageId, (usize, usize)>,
    /// Each group's first message among all messages of the list, collapsed or not.
    flat_start: Vec<usize>,
    /// The expanded groups the rows below were laid out for.
    expanded: Vec<MessageId>,
    /// Each group's first row, and the number of rows at the end: one row per collapsed
    /// group, one per message of an expanded one.
    row_start: Vec<usize>,
}

impl Grouped {
    fn total(&self) -> usize {
        self.row_start.last().copied().unwrap_or_default()
    }

    fn is_open(&self, group: usize) -> bool {
        self.row_start[group + 1] - self.row_start[group] > 1
    }

    /// Lay the rows out for these expanded groups, unless they already are.
    fn expand(&mut self, expanded: &[MessageId]) {
        let mut wanted = expanded.to_vec();
        wanted.sort_unstable();
        wanted.dedup();
        if wanted == self.expanded && self.row_start.len() == self.groups.len() + 1 {
            return;
        }
        let open: HashSet<_> = wanted.iter().copied().collect();
        self.row_start = Vec::with_capacity(self.groups.len() + 1);
        let mut row = 0;
        for (group, key) in self.groups.iter().zip(&self.keys) {
            self.row_start.push(row);
            row += if open.contains(key) { group.len() } else { 1 };
        }
        self.row_start.push(row);
        self.expanded = wanted;
    }

    /// The row a message is shown on, if it is shown (not hidden in a collapsed group).
    fn row_of(&self, id: MessageId) -> Option<usize> {
        let &(group, i) = self.member.get(&id)?;
        (i == 0 || self.is_open(group)).then(|| self.row_start[group] + i)
    }

    /// The row of a message, or of its group's first message when it is hidden.
    fn row_or_head(&self, id: MessageId) -> Option<usize> {
        let &(group, _) = self.member.get(&id)?;
        self.row_of(id).or(Some(self.row_start[group]))
    }

    /// The rows from `start` to `end`: each message, its group and its place in the group.
    fn rows(&self, start: usize, end: usize) -> Vec<(MessageId, usize, usize)> {
        let end = end.min(self.total());
        let mut out = Vec::with_capacity(end.saturating_sub(start));
        if start >= end {
            return out;
        }
        // The group the first row is in: the last one starting at or before it.
        let mut group = self.row_start.partition_point(|&r| r <= start) - 1;
        let mut i = start - self.row_start[group];
        while out.len() < end - start {
            out.push((self.groups[group][i], group, i));
            i += 1;
            if self.row_start[group] + i >= self.row_start[group + 1] {
                group += 1;
                i = 0;
            }
        }
        out
    }
}

/// Counts changes that can move a message to another conversation, group or place in a
/// list: mail that comes, goes or moves, new headers, a folder that becomes Spam or Trash.
/// Reading and flagging mail don't count, so thread rows aren't worked out again on every
/// arrow key. Temporary, like `total_changes`: it belongs to this connection, which makes
/// every change to the mail base, and is never saved.
pub(crate) fn watch_shape(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TEMP TABLE list_shape (n INTEGER NOT NULL);
         INSERT INTO list_shape VALUES (0);
         CREATE TEMP TRIGGER list_shape_added AFTER INSERT ON messages
         BEGIN UPDATE list_shape SET n = n + 1; END;
         CREATE TEMP TRIGGER list_shape_removed AFTER DELETE ON messages
         BEGIN UPDATE list_shape SET n = n + 1; END;
         CREATE TEMP TRIGGER list_shape_changed AFTER UPDATE OF folder, message_id, refs,
             from_name, from_email, to_addr, cc, bcc, subject, date, size_kb, attachments
             ON messages
         BEGIN UPDATE list_shape SET n = n + 1; END;
         CREATE TEMP TRIGGER list_shape_folder_added AFTER INSERT ON folders
         BEGIN UPDATE list_shape SET n = n + 1; END;
         CREATE TEMP TRIGGER list_shape_folder_removed AFTER DELETE ON folders
         BEGIN UPDATE list_shape SET n = n + 1; END;
         CREATE TEMP TRIGGER list_shape_folder_kind AFTER UPDATE OF kind ON folders
             WHEN old.kind IS NOT new.kind
         BEGIN UPDATE list_shape SET n = n + 1; END;",
    )
}

fn shape(db: &Connection) -> Result<i64, StoreError> {
    Ok(db
        .prepare_cached("SELECT n FROM temp.list_shape")?
        .query_row([], |r| r.get(0))?)
}

/// The kind of folder a view of every account shows ("*/drafts": every Drafts, "*/outbox"),
/// if `key` is one.
pub fn every_kind(key: &str) -> Option<FolderKind> {
    key.strip_prefix("*/")
        .map(FolderKind::from_name)
        .filter(|&k| k != FolderKind::Custom)
}

/// Folder keys of the given kinds, for SQL: `folder IN (…)`.
fn folders_of_kind(kinds: &str) -> String {
    format!("(SELECT account || '/' || id FROM folders WHERE kind IN ({kinds}))")
}

/// A list view turned into SQL: its WHERE clause with arguments, and its order.
#[derive(Clone)]
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
        match (&view.folder, view.thread) {
            (_, Some(id)) => {
                let ids = store.conversation(id)?;
                filter.push_str("id IN (SELECT value FROM json_each(?))");
                args.push(Value::Text(
                    serde_json::to_string(&ids).map_err(|e| StoreError::Database(e.to_string()))?,
                ));
            }
            (Some(key), None) if every_kind(key).is_some() => {
                let kind = every_kind(key).map_or("", FolderKind::as_str);
                filter.push_str("folder IN ");
                filter.push_str(&folders_of_kind(&format!("'{kind}'")));
            }
            (Some(key), None) => {
                if store.kind_of(key).is_none() {
                    return Err(StoreError::UnknownFolder(key.clone()));
                }
                filter.push_str("folder = ?");
                args.push(Value::Text(key.clone()));
            }
            (None, None) => {
                filter.push_str("folder NOT IN ");
                filter.push_str(&folders_of_kind("'spam', 'trash'"));
            }
        }
        let terms = Terms::parse(&view.filter);
        let needle = terms.text.trim().to_lowercase();
        if !needle.is_empty() {
            filter.push_str(" AND (telex_has(?, from_name, from_email, subject, to_addr)");
            args.push(Value::Text(needle));
            let words = if view.bodies {
                fts_query(&terms.text)
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
        let mut matching = |fields: &str, values: &[String]| {
            for v in values {
                filter.push_str(&format!(" AND telex_has(?, {fields})"));
                args.push(Value::Text(v.clone()));
            }
        };
        matching("from_name, from_email", &terms.from);
        matching("to_addr, cc, bcc", &terms.to);
        matching("subject", &terms.subject);
        let mut condition = |sql: &str, value: Value| {
            filter.push_str(sql);
            args.push(value);
        };
        if let Some(unread) = terms.unread {
            condition(" AND unread = ?", Value::Integer(i64::from(unread)));
        }
        if let Some(flagged) = terms.flagged {
            condition(" AND flagged = ?", Value::Integer(i64::from(flagged)));
        }
        if let Some(day) = terms.after {
            condition(" AND date >= ?", Value::Text(day));
        }
        if let Some(day) = terms.before {
            condition(" AND date < ?", Value::Text(day));
        }
        let kb = |n: u64| Value::Integer(i64::try_from(n).unwrap_or(i64::MAX));
        if let Some(n) = terms.larger_kb {
            condition(" AND size_kb >= ?", kb(n));
        }
        if let Some(n) = terms.smaller_kb {
            condition(" AND size_kb < ?", kb(n));
        }
        if terms.has_file {
            filter.push_str(" AND attachments <> '[]'");
        }
        if view.unread {
            if view.keep.is_empty() {
                filter.push_str(" AND unread = 1");
            } else {
                filter.push_str(" AND (unread = 1 OR id IN (SELECT value FROM json_each(?)))");
                args.push(Value::Text(
                    serde_json::to_string(&view.keep)
                        .map_err(|e| StoreError::Database(e.to_string()))?,
                ));
            }
        }

        let desc = !view.ascending;
        let folder = view.folder.as_deref().filter(|_| view.thread.is_none());
        let who = match folder.and_then(|k| every_kind(k).or_else(|| store.kind_of(k))) {
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
        let mut counts: HashMap<String, FolderCount> = stmt
            .query_map([], |r| {
                let (total, unread): (i64, i64) = (r.get(1)?, r.get(2)?);
                Ok((
                    r.get::<_, String>(0)?,
                    FolderCount {
                        total: usize::try_from(total).unwrap_or_default(),
                        unread: usize::try_from(unread).unwrap_or_default(),
                        ..FolderCount::default()
                    },
                ))
            })?
            .collect::<Result<_, _>>()?;
        let mut stmt = self.db.prepare_cached(
            "SELECT account || '/' || id, missing FROM folders WHERE missing > 0",
        )?;
        let older = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for row in older {
            let (key, n) = row?;
            counts.entry(key).or_default().older = usize::try_from(n).unwrap_or_default();
        }
        let mut stmt = self.db.prepare_cached(
            "SELECT m.folder, COUNT(*) FROM outbox o JOIN messages m ON m.id = o.message
             WHERE o.refused OR o.last_error IS NOT NULL GROUP BY m.folder",
        )?;
        let stuck = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for row in stuck {
            let (key, n) = row?;
            counts.entry(key).or_default().stuck = usize::try_from(n).unwrap_or_default();
        }
        let mut stmt = self
            .db
            .prepare_cached("SELECT id, sync_error FROM accounts WHERE sync_error IS NOT NULL")?;
        let problems = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let away = self.away_on()?;
        let mut quota = HashMap::new();
        let mut synced = HashMap::new();
        let mut stmt = self.db.prepare_cached(
            "SELECT id, quota_used_kb, quota_limit_kb, synced_at FROM accounts WHERE imap_host IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<chrono::NaiveDateTime>>(3)?,
            ))
        })?;
        for row in rows {
            let (id, used, limit, at) = row?;
            if let (Some(used), Some(limit)) = (used, limit) {
                if limit > 0 {
                    let kb = |n: i64| u64::try_from(n).unwrap_or_default();
                    let q = crate::usage::Quota {
                        used_kb: kb(used),
                        limit_kb: kb(limit),
                    };
                    quota.insert(id.clone(), q);
                }
            }
            if let Some(at) = at {
                synced.insert(id, at);
            }
        }
        Ok(FolderTree {
            away,
            quota,
            synced,
            accounts: self.accounts().to_vec(),
            counts,
            problems,
        })
    }

    /// One page of a list: up to `limit` summaries in the view's order, where it says.
    /// An `After` or `Before` message that no longer exists (or, in a filtered list, no
    /// longer matches) is an `UnknownMessage` error; the UI then asks for the row instead.
    /// An unread list is worked out once, like a filtered one: unread mail is a few rows
    /// scattered through a folder's index.
    pub fn page(&self, view: &ListView, at: PageAt, limit: usize) -> Result<Page, StoreError> {
        let limit = limit.clamp(1, MAX_PAGE);
        let q = Query::new(self, view)?;
        if view.grouped {
            return self.grouped_page(&q, view, at, limit);
        }
        if view.filter.trim().is_empty() && view.thread.is_none() && !view.unread {
            self.keyset_page(&q, at, limit)
        } else {
            self.found_page(&q, view, at, limit)
        }
    }

    /// The messages of a list from `from` to `to` (either way round, both included), in the
    /// list's order: selecting a range with Shift. The whole list when either is missing or
    /// no longer in it: Ctrl+A.
    pub fn ids_between(
        &self,
        view: &ListView,
        from: Option<MessageId>,
        to: Option<MessageId>,
    ) -> Result<Vec<MessageId>, StoreError> {
        if let (true, Some(from), Some(to)) = (view.grouped, from, to) {
            let q = Query::new(self, view)?;
            self.cache_groups(&q, view)?;
            let cache = self.grouped.borrow();
            let cache = cache.as_ref().expect("cached groups");
            if let (Some(a), Some(b)) = (cache.row_of(from), cache.row_of(to)) {
                let rows = cache.rows(a.min(b), a.max(b) + 1);
                return Ok(rows.into_iter().map(|r| r.0).collect());
            }
        }
        // A filtered list the window is paging through is worked out already.
        let changes = self.db.total_changes();
        let cached = self
            .found
            .borrow()
            .as_ref()
            .filter(|f| f.view == *view && f.changes == changes)
            .map(|f| f.ids.clone());
        let ids = match cached {
            Some(ids) => ids,
            None => Query::new(self, view)?.ids(&self.db)?,
        };
        let place = |id: Option<MessageId>| id.and_then(|id| ids.iter().position(|&x| x == id));
        Ok(match (place(from), place(to)) {
            (Some(a), Some(b)) => ids[a.min(b)..=a.max(b)].to_vec(),
            _ => ids,
        })
    }

    /// The first unread message of a list after `after` in the list's order (before it, with
    /// `back`), and its row; from the list's start (end) when `after` is `None` or no longer
    /// exists. `None` when there is no unread message that way. Space in the window goes there.
    pub fn next_unread(
        &self,
        view: &ListView,
        after: Option<MessageId>,
        back: bool,
    ) -> Result<Option<UnreadAt>, StoreError> {
        let q = Query::new(self, view)?;
        if view.grouped {
            self.cache_groups(&q, view)?;
            let cache = self.grouped.borrow();
            let cache = cache.as_ref().expect("cached groups");
            // Every message counts, collapsed or not, in the grouped list's order: the
            // nearest unread one beyond `after`.
            let flat = |id: &MessageId| {
                cache
                    .member
                    .get(id)
                    .map(|&(group, i)| cache.flat_start[group] + i)
            };
            let from = after.as_ref().and_then(flat);
            let mut unread = q.clone();
            unread.filter.push_str(" AND unread = 1");
            let candidates = unread
                .ids(&self.db)?
                .into_iter()
                .filter_map(|id| flat(&id).map(|at| (at, id)));
            let found = if back {
                let before = from.unwrap_or(usize::MAX);
                candidates.filter(|&(at, _)| at < before).max()
            } else {
                candidates
                    .filter(|&(at, _)| from.is_none_or(|from| at > from))
                    .min()
            };
            let Some((_, id)) = found else {
                return Ok(None);
            };
            let (group, _) = cache.member[&id];
            let thread = (cache.groups[group].len() > 1).then_some(cache.keys[group]);
            let row = cache.row_or_head(id).expect("group member");
            return Ok(Some(UnreadAt { id, row, thread }));
        }
        let mut unread = q.clone();
        unread.filter.push_str(" AND unread = 1");
        let order = if back { q.reversed() } else { q.order.clone() };
        let from = match after {
            Some(id) => q.keys(&self.db, id)?,
            None => None,
        };
        let Some(found) = unread.rows(&self.db, &order, from, 1, 0)?.pop() else {
            return Ok(None);
        };
        let keys = q.keys(&self.db, found.id)?;
        let row = q.count(&self.db, keys)?;
        Ok(Some(UnreadAt {
            id: found.id,
            row,
            thread: None,
        }))
    }

    /// Every message of the conversation `id` is in: the messages it answers and those
    /// that answer it, directly or not, found by Message-ID, In-Reply-To and References, in
    /// its account and on this PC, not in Spam or Trash (except `id` itself). In no order; the list
    /// sorts them.
    pub fn conversation(&self, id: MessageId) -> Result<Vec<MessageId>, StoreError> {
        let changes = self.db.total_changes();
        if let Some((cached, at, ids)) = &*self.thread.borrow() {
            if *cached == id && *at == changes {
                return Ok(ids.clone());
            }
        }
        let (folder, message_id, refs): (String, Option<String>, String) = self
            .db
            .query_row(
                "SELECT folder, message_id, refs FROM messages WHERE id = ?1",
                [id as i64],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
            .ok_or(StoreError::UnknownMessage(id))?;
        let mut start: Vec<String> = refs.split_whitespace().map(str::to_owned).collect();
        start.extend(message_id);
        if start.is_empty() {
            return Ok(vec![id]);
        }
        // One pass over the account's links (only messages that have some), and those of the
        // folders on this PC, where its older mail may be kept; then the thread is followed in
        // memory, however deep it goes. A message on this PC came from any account.
        let account = folder.split('/').next().unwrap_or_default();
        let accounts: Vec<&str> = if account == LOCAL_ID {
            self.accounts().iter().map(|a| a.id.as_str()).collect()
        } else {
            vec![account, LOCAL_ID]
        };
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT id, COALESCE(message_id, ''), refs FROM messages
             WHERE folder >= ?1 || '/' AND folder < ?1 || '0' AND folder NOT IN {}
               AND (message_id IS NOT NULL OR refs <> '')",
            folders_of_kind("'spam', 'trash'")
        ))?;
        let mut rows: Vec<(i64, String, String)> = Vec::new();
        for account in accounts {
            for row in
                stmt.query_map(params![account], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            {
                rows.push(row?);
            }
        }
        // Every Message-ID, and the messages that are it or point at it.
        let mut linked: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, (_, mid, refs)) in rows.iter().enumerate() {
            for key in std::iter::once(mid.as_str()).chain(refs.split_whitespace()) {
                if !key.is_empty() {
                    linked.entry(key).or_default().push(i);
                }
            }
        }
        let mut found = HashSet::from([id]);
        let mut seen: HashSet<&str> = HashSet::new();
        let mut todo: Vec<&str> = start.iter().map(String::as_str).collect();
        while let Some(key) = todo.pop() {
            if !seen.insert(key) {
                continue;
            }
            for &i in linked.get(key).into_iter().flatten() {
                let (row_id, mid, refs) = &rows[i];
                if found.insert(*row_id as MessageId) {
                    todo.push(mid.as_str());
                    todo.extend(refs.split_whitespace());
                }
            }
        }
        let ids: Vec<MessageId> = found.into_iter().collect();
        *self.thread.borrow_mut() = Some((id, changes, ids.clone()));
        Ok(ids)
    }

    /// Connect real reply headers, including shared ancestors that aren't downloaded.
    /// Equal subjects alone never join messages. Each account has its own graph.
    fn cache_graph(&self, view: &ListView, shape: i64) -> Result<(), StoreError> {
        // A reply moved to Sent must still connect the messages left in the Inbox.
        // Read headers across the account, then show only members of the requested view.
        let folder = view.folder.as_deref().unwrap_or_default();
        let account = if folder.starts_with("*/") || view.thread.is_some() {
            ""
        } else {
            folder.split('/').next().unwrap_or_default()
        };
        // Spam and Trash stay out, unless the list is one of them.
        let extra = match self.kind_of(folder) {
            Some(FolderKind::Spam | FolderKind::Trash) => folder,
            _ => "",
        };
        let scope = (account.to_owned(), extra.to_owned());
        if self
            .graph
            .borrow()
            .as_ref()
            .is_some_and(|g| g.scope == scope && g.shape == shape)
        {
            return Ok(());
        }
        fn root(parents: &mut [usize], mut i: usize) -> usize {
            while parents[i] != i {
                parents[i] = parents[parents[i]];
                i = parents[i];
            }
            i
        }
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT id, folder, COALESCE(message_id, ''), refs FROM messages
             WHERE (folder NOT IN {} OR folder = ?1)
               AND (?2 = '' OR (folder >= ?2 || '/' AND folder < ?2 || '0'))",
            folders_of_kind("'spam', 'trash'")
        ))?;
        let rows = stmt
            .query_map(params![extra, account], |r| {
                Ok((
                    r.get::<_, i64>(0)? as MessageId,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut parents: Vec<usize> = (0..rows.len()).collect();
        let mut links: HashMap<(&str, &str), usize> = HashMap::new();
        for (i, (_, folder, mid, refs)) in rows.iter().enumerate() {
            let account = folder.split('/').next().unwrap_or_default();
            for key in std::iter::once(mid.as_str())
                .chain(refs.split_whitespace())
                .filter(|k| !k.is_empty())
            {
                if let Some(&j) = links.get(&(account, key)) {
                    let a = root(&mut parents, i);
                    let b = root(&mut parents, j);
                    // Always link to the smaller root, keeping the trees shallow.
                    parents[a.max(b)] = a.min(b);
                } else {
                    links.insert((account, key), i);
                }
            }
        }
        drop(links);
        let mut conversation = HashMap::with_capacity(rows.len());
        let mut keys: Vec<MessageId> = Vec::new();
        let mut index = HashMap::new();
        for (i, row) in rows.iter().enumerate() {
            let r = root(&mut parents, i);
            let c = *index.entry(r).or_insert_with(|| {
                keys.push(row.0);
                keys.len() - 1
            });
            keys[c] = keys[c].min(row.0);
            conversation.insert(row.0, c);
        }
        *self.graph.borrow_mut() = Some(Graph {
            scope,
            shape,
            conversation,
            keys,
        });
        Ok(())
    }

    /// The list's thread rows, worked out again only when mail came, went or moved, or (for
    /// lists that depend on them) when read and flag states changed; then laid out for the
    /// view's expanded groups.
    fn cache_groups(&self, q: &Query, view: &ListView) -> Result<(), StoreError> {
        let mut base = view.clone();
        base.expanded.clear();
        let shape = shape(&self.db)?;
        // Unread lists, the Flag order and the quick filter (is:unread, is:flagged) change
        // with read and flag states.
        let flags_count =
            view.unread || view.sort == SortKey::Flag || !view.filter.trim().is_empty();
        let changes = flags_count.then(|| self.db.total_changes());
        let fresh = self
            .grouped
            .borrow()
            .as_ref()
            .is_some_and(|c| c.view == base && c.shape == shape && c.changes == changes);
        if !fresh {
            self.cache_graph(view, shape)?;
            let ids = q.ids(&self.db)?;
            let graph = self.graph.borrow();
            let graph = graph.as_ref().expect("cached graph");
            let mut groups: Vec<Vec<MessageId>> = Vec::new();
            let mut keys = Vec::new();
            let mut groups_at = HashMap::new();
            for (i, &id) in ids.iter().enumerate() {
                // A conversation can include its starting message even in Spam or Trash.
                let c = graph.conversation.get(&id).copied();
                let at = *groups_at
                    .entry(c.unwrap_or(graph.keys.len() + i))
                    .or_insert_with(|| {
                        groups.push(Vec::new());
                        keys.push(c.map_or(id, |c| graph.keys[c]));
                        groups.len() - 1
                    });
                groups[at].push(id);
            }
            let mut member = HashMap::with_capacity(ids.len());
            let mut flat_start = Vec::with_capacity(groups.len());
            let mut flat = 0;
            for (g, group) in groups.iter().enumerate() {
                flat_start.push(flat);
                flat += group.len();
                for (i, &id) in group.iter().enumerate() {
                    member.insert(id, (g, i));
                }
            }
            *self.grouped.borrow_mut() = Some(Grouped {
                view: base,
                shape,
                changes,
                groups,
                keys,
                member,
                flat_start,
                expanded: Vec::new(),
                row_start: Vec::new(),
            });
        }
        self.grouped
            .borrow_mut()
            .as_mut()
            .expect("cached groups")
            .expand(&view.expanded);
        Ok(())
    }

    fn grouped_page(
        &self,
        q: &Query,
        view: &ListView,
        at: PageAt,
        limit: usize,
    ) -> Result<Page, StoreError> {
        self.cache_groups(q, view)?;
        let cache = self.grouped.borrow();
        let cache = cache.as_ref().expect("cached groups");
        let total = cache.total();
        let place = |id| cache.row_of(id).ok_or(StoreError::UnknownMessage(id));
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
                let i = cache
                    .row_or_head(id)
                    .map_or(0, |i| i.saturating_sub(limit / 2));
                (i, i + limit)
            }
            PageAt::Row(n) => {
                let i = n.min(total.saturating_sub(limit));
                (i, i + limit)
            }
        };
        let end = end.min(total);
        let start = start.min(end);
        let entries = cache.rows(start, end);
        let ids: Vec<_> = entries.iter().map(|r| r.0).collect();
        let mut rows = self.summaries(&ids)?;
        // Read states aren't cached: count the unread mail of the groups on this page.
        let mut unread: HashMap<usize, usize> = HashMap::new();
        let mut shown: Vec<usize> = entries.iter().map(|r| r.1).collect();
        shown.dedup();
        let members: Vec<MessageId> = shown
            .iter()
            .filter(|&&g| cache.groups[g].len() > 1)
            .flat_map(|&g| cache.groups[g].iter().copied())
            .collect();
        if !members.is_empty() {
            let json =
                serde_json::to_string(&members).map_err(|e| StoreError::Database(e.to_string()))?;
            let mut stmt = self.db.prepare_cached(
                "SELECT id FROM messages
                 WHERE id IN (SELECT value FROM json_each(?1)) AND unread = 1",
            )?;
            for id in stmt.query_map([json], |r| r.get::<_, i64>(0))? {
                if let Some(&(g, _)) = cache.member.get(&(id? as MessageId)) {
                    *unread.entry(g).or_default() += 1;
                }
            }
        }
        for (row, &(_, g, i)) in rows.iter_mut().zip(&entries) {
            let group = &cache.groups[g];
            row.thread = (group.len() > 1).then(|| ThreadRow {
                id: cache.keys[g],
                head: group[0],
                count: group.len(),
                unread: unread.get(&g).copied().unwrap_or_default(),
                child: i > 0,
            });
        }
        let focus = match at {
            PageAt::Around(id) if cache.row_of(id).is_none() => {
                cache.member.get(&id).map(|&(g, _)| cache.groups[g][0])
            }
            _ => None,
        };
        Ok(Page {
            total,
            start,
            rows,
            focus,
        })
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
        Ok(Page {
            total,
            start,
            rows,
            focus: None,
        })
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
            focus: None,
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
    use crate::model::{AccountSetup, Attachment, Folder, FolderKind, Message, OutgoingDraft};
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
                                    path: None,
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
                            // Each message answers the one before it in the folder.
                            refs: if i > 0 {
                                vec![format!("{folder}.{}@x.pl", i - 1)]
                            } else {
                                vec![]
                            },
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
                        for unread in [false, true] {
                            let view = ListView {
                                folder: Some(folder.into()),
                                sort,
                                ascending,
                                filter: filter.into(),
                                unread,
                                ..Default::default()
                            };
                            check_paging(&store, &view, folder.ends_with("sent"));
                        }
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
            .filter(|m| !view.unread || m.unread || view.keep.contains(&m.id))
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
    fn filter_operators_are_read() {
        let t = Terms::parse(
            r#"from:"Anna K" faktura is:unread has:file after:2026-09 before:15.10.2026 larger:5m smaller:300 to:Piotr subject:Re: Re: notakey:x is:weird from:"#,
        );
        assert_eq!(t.from, ["anna k"]);
        assert_eq!(t.to, ["piotr"]);
        assert_eq!(t.subject, ["re:"]);
        assert_eq!((t.unread, t.flagged, t.has_file), (Some(true), None, true));
        assert_eq!(t.after.as_deref(), Some("2026-09-01 00:00:00"));
        assert_eq!(t.before.as_deref(), Some("2026-10-15 00:00:00"));
        assert_eq!((t.larger_kb, t.smaller_kb), (Some(5120), Some(300)));
        assert_eq!(
            t.text, "faktura Re: notakey:x is:weird",
            "the rest is plain words"
        );
        assert_eq!(Terms::parse("  ").text, "");
        assert_eq!(Terms::parse("\"anna kow\"").text, "anna kow");
        let odd = Terms::parse("after:2026-13 larger:lots 10:30");
        assert_eq!(odd.after, None);
        assert_eq!(odd.text, "after:2026-13 larger:lots 10:30");
        assert_eq!(parse_kb("1,5 MB"), Some(1536));
        assert_eq!(parse_day("2026"), Some("2026-01-01 00:00:00".into()));
        assert_eq!(parse_day("09.2026"), Some("2026-09-01 00:00:00".into()));
    }

    #[test]
    fn filter_operators_narrow_the_list() {
        let store = store(40);
        let all = store.list("studio/inbox").unwrap();
        let count = |filter: &str| {
            let view = ListView {
                folder: Some("studio/inbox".into()),
                filter: filter.into(),
                ..Default::default()
            };
            store.page(&view, PageAt::First, 500).unwrap().total
        };
        let expect =
            |keep: &dyn Fn(&MessageSummary) -> bool| all.iter().filter(|m| keep(m)).count();
        assert_eq!(count("is:unread"), expect(&|m| m.unread));
        assert_eq!(count("is:read"), expect(&|m| !m.unread));
        assert_eq!(count("is:flagged"), expect(&|m| m.flagged));
        assert_eq!(count("has:file"), expect(&|m| m.has_attachments));
        assert_eq!(count("larger:3"), expect(&|m| m.size_kb >= 3));
        assert_eq!(
            count("smaller:2 is:unread"),
            expect(&|m| m.size_kb < 2 && m.unread)
        );
        assert_eq!(
            count("from:kowalska"),
            expect(&|m| m.from_name.to_lowercase().contains("kowalska"))
        );
        assert_eq!(
            count("subject:wycena is:flagged"),
            expect(&|m| m.flagged && m.subject.to_lowercase().contains("wycena"))
        );
        assert_eq!(count("after:2026-10-01"), all.len());
        assert_eq!(count("after:2026-10-02"), 0);
        assert_eq!(count("before:2026-10-02"), all.len());
        // Operators and words together: the words still match as before.
        assert_eq!(
            count("is:unread kow"),
            expect(&|m| m.unread && matches(m, "kow"))
        );
        assert!(
            count("is:unread") > 0 && count("has:file") > 0,
            "the fixture has both"
        );
    }

    #[test]
    fn a_range_is_every_message_between_two() {
        let store = store(12);
        let view = ListView {
            folder: Some("studio/inbox".into()),
            sort: SortKey::Subject,
            ascending: true,
            ..Default::default()
        };
        let all = ids(&store.page(&view, PageAt::First, 100).unwrap().rows);
        let range = store
            .ids_between(&view, Some(all[7]), Some(all[3]))
            .unwrap();
        assert_eq!(range, all[3..=7], "either way round, in the list's order");
        assert_eq!(store.ids_between(&view, None, None).unwrap(), all);
        assert_eq!(
            store
                .ids_between(&view, Some(99_999), Some(all[0]))
                .unwrap(),
            all
        );
    }

    #[test]
    fn space_finds_the_next_unread_in_the_list_s_order() {
        let store = store(12);
        for sort in [SortKey::Date, SortKey::Subject, SortKey::Flag] {
            for filter in ["", "wycena"] {
                let view = ListView {
                    folder: Some("studio/inbox".into()),
                    sort,
                    filter: filter.into(),
                    ..Default::default()
                };
                let rows = store.page(&view, PageAt::First, 100).unwrap().rows;
                let unread: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].unread).collect();
                let at = |i: usize| UnreadAt {
                    id: rows[i].id,
                    row: i,
                    thread: None,
                };
                let next =
                    |after: Option<MessageId>, back| store.next_unread(&view, after, back).unwrap();
                // From the top, then from each message on, both ways.
                assert_eq!(
                    next(None, false),
                    unread.first().map(|&i| at(i)),
                    "{sort:?} {filter:?}"
                );
                assert_eq!(next(None, true), unread.last().map(|&i| at(i)));
                for (i, row) in rows.iter().enumerate() {
                    let ahead = unread.iter().find(|&&u| u > i).map(|&u| at(u));
                    let behind = unread.iter().rev().find(|&&u| u < i).map(|&u| at(u));
                    assert_eq!(
                        next(Some(row.id), false),
                        ahead,
                        "{sort:?} {filter:?} after row {i}"
                    );
                    assert_eq!(
                        next(Some(row.id), true),
                        behind,
                        "{sort:?} {filter:?} before row {i}"
                    );
                }
            }
        }
        // A message that is gone counts as none: from the top.
        let view = ListView {
            folder: Some("studio/inbox".into()),
            ..Default::default()
        };
        assert_eq!(
            store.next_unread(&view, Some(99_999), false).unwrap(),
            store.next_unread(&view, None, false).unwrap()
        );
    }

    #[test]
    fn unread_lists_keep_what_was_read_while_they_were_open() {
        let mut store = store(12);
        let unread_in = |store: &MailStore, folder: &str| -> Vec<MessageId> {
            let mut rows: Vec<_> = store
                .list(folder)
                .unwrap()
                .into_iter()
                .filter(|m| m.unread)
                .collect();
            reference_order(&mut rows, &ListView::default(), false);
            ids(&rows)
        };
        // The Unread list: every folder but Spam and Trash, newest first.
        let everywhere = ListView {
            unread: true,
            ..Default::default()
        };
        let mut expected: Vec<_> = ["studio/inbox", "studio/sent"]
            .iter()
            .flat_map(|f| store.list(f).unwrap())
            .filter(|m| m.unread)
            .collect();
        reference_order(&mut expected, &everywhere, false);
        let page = store.page(&everywhere, PageAt::First, 100).unwrap();
        assert_eq!(ids(&page.rows), ids(&expected));
        assert!(unread_in(&store, "studio/spam")
            .iter()
            .all(|id| !ids(&page.rows).contains(id)));

        // Read one: it leaves the list, unless the list keeps it, in its place.
        let inbox = unread_in(&store, "studio/inbox");
        let view = ListView {
            folder: Some("studio/inbox".into()),
            unread: true,
            ..Default::default()
        };
        assert_eq!(
            ids(&store.page(&view, PageAt::First, 100).unwrap().rows),
            inbox
        );
        store.set_unread(inbox[1], false).unwrap();
        let rows = ids(&store.page(&view, PageAt::First, 100).unwrap().rows);
        assert_eq!(rows, [&inbox[..1], &inbox[2..]].concat());
        let kept = ListView {
            keep: vec![inbox[1]],
            ..view.clone()
        };
        assert_eq!(
            ids(&store.page(&kept, PageAt::First, 100).unwrap().rows),
            inbox
        );
        check_paging(&store, &kept, false);
        // Space skips it: it is read now.
        let next = store.next_unread(&kept, Some(inbox[0]), false).unwrap();
        assert_eq!(next.map(|u| (u.id, u.row)), Some((inbox[2], 2)));
        // Kept mail of another folder stays out.
        let elsewhere = ListView {
            keep: unread_in(&store, "studio/sent"),
            ..view
        };
        assert_eq!(
            ids(&store.page(&elsewhere, PageAt::First, 100).unwrap().rows),
            rows
        );
    }

    #[test]
    fn a_conversation_is_found_across_its_links() {
        let mut store = store(10);
        let inbox: Vec<MessageId> = store
            .list("studio/inbox")
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        // In the fixture each Inbox message answers the one before it: one long thread.
        let mut thread = store.conversation(inbox[4]).unwrap();
        thread.sort_unstable();
        let mut expected = inbox.clone();
        expected.sort_unstable();
        assert_eq!(thread, expected);
        let view = ListView {
            folder: Some("studio/sent".into()),
            thread: Some(inbox[0]),
            ..Default::default()
        };
        let page = store.page(&view, PageAt::First, 50).unwrap();
        assert_eq!(page.total, 10, "the folder doesn't count in a conversation");
        assert!(page.rows.iter().all(|m| m.folder == "studio/inbox"));
        // A message moved to Trash leaves the conversation, which is worked out again.
        let gone = inbox[9];
        store.move_to(gone, "studio/trash").unwrap();
        let after = store.conversation(inbox[0]).unwrap();
        assert_eq!(after.len(), 9);
        assert!(!after.contains(&gone));
        // Older mail kept on this PC is still part of it, seen from either side.
        let kept = inbox[5];
        store.move_to(kept, "local/archive").unwrap();
        assert!(store.conversation(inbox[0]).unwrap().contains(&kept));
        assert_eq!(store.conversation(kept).unwrap().len(), 9);
        // A message without any links is a conversation of one.
        let lone = store.list("studio/trash").unwrap()[0].id;
        assert!(store.conversation(lone).unwrap().contains(&lone));
    }

    #[test]
    fn grouped_folders_page_and_reveal_hidden_unread_messages() {
        let mut store = store(12);
        let plain = ListView {
            folder: Some("studio/inbox".into()),
            ..Default::default()
        };
        let original = store.page(&plain, PageAt::First, 100).unwrap();
        assert!(original.rows.iter().all(|m| m.thread.is_none()));
        let view = ListView {
            grouped: true,
            ..plain.clone()
        };
        let collapsed = store.page(&view, PageAt::First, 3).unwrap();
        assert_eq!(collapsed.total, 1);
        let head = collapsed.rows[0].id;
        let th = collapsed.rows[0].thread.unwrap();
        assert_eq!(
            (th.head, th.count, th.unread, th.child),
            (head, 12, 4, false)
        );
        assert!(
            !collapsed.rows[0].unread,
            "the row's own read state stays separate"
        );
        let hidden = original.rows.iter().find(|m| m.unread).unwrap().id;
        assert_eq!(
            store.next_unread(&view, Some(head), false).unwrap(),
            Some(UnreadAt {
                id: hidden,
                row: 0,
                thread: Some(th.id)
            })
        );
        let around = store.page(&view, PageAt::Around(hidden), 3).unwrap();
        assert_eq!(around.focus, Some(head));
        assert_eq!(
            store.ids_between(&view, Some(head), Some(head)).unwrap(),
            vec![head]
        );
        assert_eq!(store.ids_between(&view, None, None).unwrap().len(), 12);

        let expanded = ListView {
            expanded: vec![th.id],
            ..view.clone()
        };
        let mut got = Vec::new();
        let mut at = PageAt::First;
        loop {
            let page = store.page(&expanded, at, 3).unwrap();
            assert_eq!(page.total, 12);
            if page.rows.is_empty() {
                break;
            }
            at = PageAt::After(page.rows.last().unwrap().id);
            got.extend(page.rows);
        }
        assert_eq!(ids(&got), ids(&original.rows));
        assert!(got.iter().skip(1).all(|m| m.thread.unwrap().child));
        let before = store.page(&expanded, PageAt::Before(got[6].id), 3).unwrap();
        assert_eq!(ids(&before.rows), ids(&got[3..6]));
        let last = store.page(&expanded, PageAt::Last, 3).unwrap();
        assert_eq!((last.start, ids(&last.rows)), (9, ids(&got[9..])));
        assert_eq!(store.page(&expanded, PageAt::Row(5), 3).unwrap().start, 5);
        assert_eq!(
            store
                .next_unread(&expanded, Some(head), false)
                .unwrap()
                .unwrap()
                .row,
            2
        );
        assert_eq!(
            store
                .ids_between(&expanded, Some(got[1].id), Some(got[4].id))
                .unwrap(),
            ids(&got[1..5])
        );

        store.set_unread(hidden, false).unwrap();
        assert_eq!(
            store.page(&view, PageAt::First, 3).unwrap().rows[0]
                .thread
                .unwrap()
                .unread,
            3
        );
        store.move_to(hidden, "studio/sent").unwrap();
        assert_eq!(
            store.page(&view, PageAt::First, 3).unwrap().rows[0]
                .thread
                .unwrap()
                .count,
            11
        );
        store.move_to(th.id, "studio/sent").unwrap();
        let kept_open = store.page(&expanded, PageAt::First, 100).unwrap();
        assert_eq!(
            kept_open.total, 10,
            "moving the first message keeps the group expanded"
        );
        assert!(kept_open.rows.iter().all(|m| m.thread.unwrap().id == th.id));
        let ascending = ListView {
            ascending: true,
            ..view
        };
        let reversed = store.page(&ascending, PageAt::First, 3).unwrap();
        assert_eq!(
            reversed.rows[0].thread.unwrap().id,
            th.id,
            "expansion id survives sorting"
        );
        assert_ne!(reversed.rows[0].id, head);
    }

    #[test]
    fn thread_rows_are_worked_out_again_only_when_mail_comes_goes_or_moves() {
        let mut store = store(12);
        let view = ListView {
            folder: Some("studio/inbox".into()),
            grouped: true,
            ..Default::default()
        };
        let row = |store: &MailStore| store.page(&view, PageAt::First, 10).unwrap().rows[0].thread;
        assert_eq!(row(&store).unwrap().unread, 4);
        let built = shape(&store.db).unwrap();
        let cached = |store: &MailStore| {
            let graph = store.graph.borrow().as_ref().unwrap().shape;
            let groups = store.grouped.borrow().as_ref().unwrap().shape;
            (graph, groups)
        };
        assert_eq!(cached(&store), (built, built));

        // Reading and flagging leave the graph and the groups alone; the counts still follow.
        let all = store.ids_between(&view, None, None).unwrap();
        store.set_unread_many(&all, false).unwrap();
        store.set_flagged(all[3], true).unwrap();
        assert_eq!(shape(&store.db).unwrap(), built);
        assert_eq!(row(&store).unwrap().unread, 0);
        assert_eq!(cached(&store), (built, built));
        store.set_unread(all[5], true).unwrap();
        assert_eq!(row(&store).unwrap().unread, 1);
        assert_eq!(
            store.next_unread(&view, None, false).unwrap().unwrap().id,
            all[5]
        );

        // A move changes the shape: both are worked out again.
        store.move_to(all[5], "studio/sent").unwrap();
        assert_eq!(row(&store).unwrap().count, 11);
        assert_eq!(row(&store).unwrap().unread, 0);
        assert!(cached(&store).0 > built);

        // Lists ordered by flag follow flags at once, without working out the graph again.
        store
            .db
            .execute("UPDATE messages SET message_id = NULL, refs = ''", [])
            .unwrap();
        store.set_flagged(all[3], false).unwrap();
        let flags = ListView {
            sort: SortKey::Flag,
            ..view.clone()
        };
        let plain = ListView {
            grouped: false,
            ..flags.clone()
        };
        let first = |store: &MailStore, view: &ListView| {
            ids(&store.page(view, PageAt::First, 20).unwrap().rows)
        };
        let before = first(&store, &flags);
        assert_eq!(before.len(), 11, "no links, no threads");
        let graph = cached(&store).0;
        store.set_flagged(before[5], true).unwrap();
        let after = first(&store, &flags);
        assert_ne!(after, before);
        assert_eq!(after, first(&store, &plain));
        assert_eq!(cached(&store).0, graph);
    }

    #[test]
    fn grouped_folders_use_reply_links_and_survive_missing_parents_and_cycles() {
        let store = store(9);
        let view = ListView {
            folder: Some("studio/inbox".into()),
            grouped: true,
            ..Default::default()
        };
        let inbox = ids(&store
            .page(
                &ListView {
                    grouped: false,
                    ..view.clone()
                },
                PageAt::First,
                100,
            )
            .unwrap()
            .rows);
        store
            .db
            .execute(
                "UPDATE messages SET subject = 'Same', message_id = NULL, refs = ''",
                [],
            )
            .unwrap();
        assert_eq!(
            store.page(&view, PageAt::First, 100).unwrap().total,
            9,
            "a subject is not a thread"
        );
        for (id, mid, refs) in [
            (inbox[0], "root@x", ""),
            (inbox[1], "reply@x", "root@x"),
            (inbox[2], "a@x", "missing@x"),
            (inbox[3], "b@x", "missing@x"),
            (inbox[4], "cycle-a@x", "cycle-b@x"),
            (inbox[5], "cycle-b@x", "cycle-a@x"),
            (inbox[6], "root@x", ""),
        ] {
            store
                .db
                .execute(
                    "UPDATE messages SET message_id = ?1, refs = ?2 WHERE id = ?3",
                    params![mid, refs, id as i64],
                )
                .unwrap();
        }
        let page = store.page(&view, PageAt::First, 100).unwrap();
        assert_eq!(page.total, 5);
        assert_eq!(
            page.rows
                .iter()
                .map(|m| m.thread.map_or(1, |t| t.count))
                .collect::<Vec<_>>(),
            vec![3, 2, 2, 1, 1]
        );
        let all = ListView {
            folder: None,
            ..view.clone()
        };
        let before = store.page(&all, PageAt::First, 100).unwrap().total;
        let elsewhere = store.list("studio/sent").unwrap()[0].id;
        store.db.execute("UPDATE messages SET folder = 'other/inbox', message_id = 'root@x', refs = '' WHERE id = ?1", [elsewhere as i64]).unwrap();
        assert_eq!(
            store.page(&all, PageAt::First, 100).unwrap().total,
            before,
            "accounts have separate reply graphs"
        );
        assert_eq!(
            store
                .page(
                    &ListView {
                        folder: Some("studio/spam".into()),
                        ..view
                    },
                    PageAt::First,
                    100
                )
                .unwrap()
                .total,
            9
        );
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
    fn the_tree_says_which_accounts_failed_their_last_check() {
        let mut store = store(1);
        let at = chrono::Local::now().naive_local();
        assert!(store.folder_tree().unwrap().problems.is_empty());
        store
            .record_sync("studio", at, Some("can't reach mail.x.pl"))
            .unwrap();
        let problems = store.folder_tree().unwrap().problems;
        assert_eq!(problems["studio"], "can't reach mail.x.pl");
        store.record_sync("studio", at, None).unwrap();
        assert!(store.folder_tree().unwrap().problems.is_empty());
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

    #[test]
    fn every_accounts_drafts_and_outbox_in_one_list() {
        let mut store = store(1);
        let other = store
            .upsert_account(&AccountSetup {
                name: "Dom".into(),
                email: "jan@dom.pl".into(),
                host: "mail.dom.pl".into(),
                port: 993,
                ..Default::default()
            })
            .unwrap();
        let drafts = |id: &str| Folder {
            id: id.into(),
            name: id.into(),
            kind: FolderKind::Drafts,
            remote: Some(id.into()),
        };
        let mut studio = store.account("studio").unwrap().folders.clone();
        studio.retain(|f| f.remote.is_some());
        studio.push(drafts("drafts"));
        store.set_folders("studio", &studio).unwrap();
        store.set_folders(&other.id, &[drafts("Robocze")]).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 10, 2)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap();
        let mut outbox = Vec::new();
        for account in ["studio", other.id.as_str()] {
            let draft = OutgoingDraft {
                account: account.into(),
                to: format!("Zofia <z@{account}.pl>"),
                subject: format!("Szkic {account}"),
                ..OutgoingDraft::default()
            };
            store
                .save_outgoing(draft, None, FolderKind::Drafts, day)
                .unwrap();
            let mut message = store.list("studio/inbox").unwrap()[0].clone();
            message.subject = format!("Wysyłka {account}");
            let full = store.get(message.id).unwrap();
            let id = store
                .queue_outgoing(
                    account,
                    &full,
                    b"raw",
                    "a@b.pl",
                    &["c@d.pl".into()],
                    None,
                    None,
                )
                .unwrap();
            outbox.push(id);
        }
        store
            .outgoing_failed(outbox[1], "550 no such user", true)
            .unwrap();

        let rows = |folder: &str| -> Vec<String> {
            let view = ListView {
                folder: Some(folder.into()),
                sort: SortKey::From,
                ascending: true,
                ..ListView::default()
            };
            let page = store.page(&view, PageAt::First, 10).unwrap();
            page.rows.iter().map(|r| r.folder.clone()).collect()
        };
        let mut drafts = rows("*/drafts");
        drafts.sort();
        assert_eq!(
            drafts,
            [other.folder_key("Robocze"), "studio/drafts".into()]
        );
        let mut outboxes = rows("*/outbox");
        outboxes.sort();
        assert_eq!(
            outboxes,
            [other.folder_key("outbox"), "studio/outbox".into()]
        );
        let custom = ListView {
            folder: Some("*/klienci".into()),
            ..ListView::default()
        };
        assert!(
            store.page(&custom, PageAt::First, 10).is_err(),
            "kinds only"
        );

        let tree = store.folder_tree().unwrap();
        let outbox_of = |a: &str| tree.counts[&format!("{a}/outbox")];
        assert_eq!(
            (outbox_of("studio").total, outbox_of("studio").stuck),
            (1, 0)
        );
        assert_eq!(
            (outbox_of(&other.id).total, outbox_of(&other.id).stuck),
            (1, 1)
        );
    }
}
