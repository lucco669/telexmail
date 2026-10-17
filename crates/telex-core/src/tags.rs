//! Tags: colored labels on messages, several to a message, like The Bat!'s colored mail and
//! Thunderbird's tags.
//!
//! On a server a tag is an IMAP keyword (RFC 3501 §2.3.2), so other programs see the same
//! tags. Thunderbird's five come built in (`$label1` Important … `$label5` Later, named in the
//! interface's language until the user renames them); a tag the user makes gets its keyword
//! from its name the way Thunderbird makes one, so a tag made in both programs is one tag.
//!
//! A message keeps the keywords of its tags in `messages.keywords` (migrations/015_tags.sql).
//! Adding or taking off a tag applies here at once and, for server mail, is queued like a flag
//! (`pending_ops` kind 'keyword', one per message and keyword); the push sends one UID STORE
//! per keyword and sign. A server that can't keep keywords (its PERMANENTFLAGS lists neither
//! `\*` nor the keyword) leaves the account's tags on this PC (`accounts.local_tags`), which
//! the push says once. Mail on this PC keeps its tags in the column alone; moves keep them,
//! and mail uploaded to a server takes them along.
//!
//! Deleting a tag takes it off every message at once, here and on the server: a tag nobody
//! sees in Telex shouldn't live on in other programs, and nothing is left to clean up later.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::filters::fold;
use crate::model::{account_of, MessageId};
use crate::store::{MailStore, StoreError};
use crate::tr;

/// The colors a tag can have: each is a pair of tokens per theme in ui/themes.css
/// (`--tag-<color>` for the chip, `--tag-<color>-fg` for its text).
pub const COLORS: &[&str] = &[
    "red", "orange", "yellow", "green", "teal", "blue", "purple", "gray",
];

/// The most tags there can be.
pub const TAGS_MAX: usize = 50;

/// The longest name a tag can have.
const NAME_MAX: usize = 40;

/// Keywords other programs use for something else (spam marks, answered, read receipts):
/// never made into a tag's keyword.
const RESERVED: &[&str] = &[
    "$forwarded",
    "$mdnsent",
    "$junk",
    "$notjunk",
    "junk",
    "nonjunk",
    "notjunk",
    "$phishing",
    "$submitpending",
    "$submitted",
    "$important",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    /// The IMAP keyword; empty for a new tag, which gets one from its name.
    #[serde(default)]
    pub keyword: String,
    pub name: String,
    /// One of [`COLORS`].
    pub color: String,
    /// How many messages here carry it (in what [`MailStore::tags`] gives; ignored when saved).
    #[serde(default)]
    pub count: usize,
}

/// The name of one of Thunderbird's five while the user hasn't renamed it, in the
/// interface's language.
pub(crate) fn default_name(keyword: &str) -> Option<String> {
    Some(match keyword {
        "$label1" => tr!("Important", "Ważne"),
        "$label2" => tr!("Work", "Praca"),
        "$label3" => tr!("Personal", "Osobiste"),
        "$label4" => tr!("To do", "Do zrobienia"),
        "$label5" => tr!("Later", "Później"),
        _ => return None,
    })
}

fn tag_gone() -> StoreError {
    StoreError::Invalid(tr!(
        "That tag no longer exists.",
        "Tej etykiety już nie ma."
    ))
}

/// A keyword as IMAP allows one: an atom (printable ASCII without `(){ %*"\]`), not a
/// system flag (`\Seen`).
pub fn is_keyword(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && !s.starts_with('\\')
        && s.bytes().all(|b| {
            (0x21..0x7f).contains(&b)
                && !matches!(b, b'(' | b')' | b'{' | b'%' | b'*' | b'"' | b'\\' | b']')
        })
}

/// The keyword for a new tag called `name`, as Thunderbird makes one: spaces and `()/{%*<>"\`
/// become `_`, the rest is modified UTF-7 (as folder names are) in lowercase; "To do" →
/// "to_do", "Ważne" → "wa&axw-ne". One already taken, or one other programs use for
/// something else, gets an `a` added until it is free.
pub fn keyword_for(name: &str, taken: &HashSet<String>) -> String {
    let replaced: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_whitespace() || "()/{%*<>\"\\]".contains(c) {
                '_'
            } else {
                c
            }
        })
        .filter(|c| !c.is_control())
        .collect();
    let mut key = encode_mutf7(&replaced).to_ascii_lowercase();
    key.retain(|c| c.is_ascii_graphic());
    if key.is_empty() || key.starts_with('\\') {
        key.insert(0, 't');
    }
    key.truncate(90);
    while taken.contains(&key) || RESERVED.contains(&key.as_str()) {
        key.push('a');
    }
    key
}

/// Modified UTF-7 (RFC 3501 §5.1.3): ASCII stays, "&" is "&-", anything else is UTF-16 in
/// base64 with "," for "/" between "&" and "-".
fn encode_mutf7(s: &str) -> String {
    const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,";
    let mut out = String::new();
    let mut run: Vec<u16> = Vec::new();
    let flush = |run: &mut Vec<u16>, out: &mut String| {
        if run.is_empty() {
            return;
        }
        out.push('&');
        let bytes: Vec<u8> = run.iter().flat_map(|u| u.to_be_bytes()).collect();
        let (mut bits, mut nbits) = (0u32, 0);
        for b in bytes {
            bits = (bits << 8) | u32::from(b);
            nbits += 8;
            while nbits >= 6 {
                nbits -= 6;
                out.push(B64[((bits >> nbits) & 63) as usize] as char);
            }
        }
        if nbits > 0 {
            out.push(B64[((bits << (6 - nbits)) & 63) as usize] as char);
        }
        out.push('-');
        run.clear();
    };
    for c in s.chars() {
        if (' '..='~').contains(&c) {
            flush(&mut run, &mut out);
            out.push(c);
            if c == '&' {
                out.push('-');
            }
        } else {
            let mut units = [0u16; 2];
            run.extend_from_slice(c.encode_utf16(&mut units));
        }
    }
    flush(&mut run, &mut out);
    out
}

/// The tag color nearest to a color another program gave its tag ("#FF9900", "#f90"): by hue,
/// gray when it has hardly any (or is near black or white); gray for one that isn't a color.
pub fn color_from_hex(hex: &str) -> &'static str {
    let digits = hex.trim().trim_start_matches('#');
    let digits: String = match digits.len() {
        3 => digits.chars().flat_map(|c| [c, c]).collect(),
        6 => digits.to_owned(),
        _ => return "gray",
    };
    let Ok(rgb) = u32::from_str_radix(&digits, 16) else {
        return "gray";
    };
    let [r, g, b] = [rgb >> 16, rgb >> 8, rgb].map(|c| f64::from(c & 0xff) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let chroma = max - min;
    if chroma < 0.15 || max < 0.15 {
        return "gray";
    }
    let hue = if max == r {
        60.0 * ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / chroma + 2.0)
    } else {
        60.0 * ((r - g) / chroma + 4.0)
    };
    match hue {
        h if h < 18.0 => "red",
        h if h < 45.0 => "orange",
        h if h < 70.0 => "yellow",
        h if h < 160.0 => "green",
        h if h < 200.0 => "teal",
        h if h < 260.0 => "blue",
        h if h < 320.0 => "purple",
        _ => "red",
    }
}

/// The keywords of `messages.keywords`, as a list.
pub fn split(keywords: &str) -> Vec<String> {
    keywords.split_whitespace().map(str::to_owned).collect()
}

/// Keywords as `messages.keywords` holds them: lowercase, A to Z, each once.
pub fn join(keywords: &[String]) -> String {
    let mut all: Vec<String> = keywords
        .iter()
        .map(|k| k.trim().to_ascii_lowercase())
        .filter(|k| is_keyword(k))
        .collect();
    all.sort();
    all.dedup();
    all.join(" ")
}

/// Of a message's keywords, the ones that are tags here.
pub(crate) fn only_tags(keywords: &[String], tags: &HashSet<String>) -> Vec<String> {
    keywords
        .iter()
        .map(|k| k.to_ascii_lowercase())
        .filter(|k| tags.contains(k))
        .collect()
}

/// A stored message's tags after the server's keywords came: the server's tags, except a
/// keyword whose change is still queued (`queued`, keywords apart by spaces), which keeps the
/// value it has here.
pub(crate) fn merge(here: &str, server: &[String], queued: &str, tags: &HashSet<String>) -> String {
    let here: Vec<&str> = here.split_whitespace().collect();
    let queued: Vec<&str> = queued.split_whitespace().collect();
    let mut out: Vec<String> = only_tags(server, tags)
        .into_iter()
        .filter(|k| !queued.contains(&k.as_str()))
        .collect();
    out.extend(
        here.iter()
            .filter(|k| queued.contains(k))
            .map(|k| (*k).to_owned()),
    );
    join(&out)
}

/// Add a tag to messages (`on`) or take it off, here and, for server mail, in the queue for
/// the server, inside a transaction the caller holds. Only the latest change of one keyword
/// on one message is kept. Messages that already are so, or are gone, are left alone.
/// Returns how many changed.
pub(crate) fn set_tag_in(
    tx: &Connection,
    ids: &[MessageId],
    keyword: &str,
    on: bool,
) -> Result<usize, StoreError> {
    let mut read = tx.prepare_cached("SELECT keywords, srv_folder FROM messages WHERE id = ?1")?;
    let mut update = tx.prepare_cached("UPDATE messages SET keywords = ?2 WHERE id = ?1")?;
    let mut forget = tx.prepare_cached(
        "DELETE FROM pending_ops WHERE message = ?1 AND kind = 'keyword' AND target = ?2",
    )?;
    let mut queue = tx.prepare_cached(
        "INSERT INTO pending_ops (account, message, kind, value, target)
         VALUES (?1, ?2, 'keyword', ?3, ?4)",
    )?;
    let mut done = 0;
    for &id in ids {
        let Some((keywords, srv)): Option<(String, Option<String>)> = read
            .query_row([id as i64], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
        else {
            continue;
        };
        let mut list = split(&keywords);
        if list.iter().any(|k| k == keyword) == on {
            continue;
        }
        if on {
            list.push(keyword.to_owned());
        } else {
            list.retain(|k| k != keyword);
        }
        update.execute(params![id as i64, join(&list)])?;
        done += 1;
        if let Some(srv) = srv {
            forget.execute(params![id as i64, keyword])?;
            queue.execute(params![account_of(&srv), id as i64, on, keyword])?;
        }
    }
    Ok(done)
}

impl MailStore {
    /// Every tag, in the window's order, with how many messages carry it.
    pub fn tags(&self) -> Result<Vec<Tag>, StoreError> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT keyword, name, color FROM tags ORDER BY position, keyword")?;
        let mut tags: Vec<Tag> = stmt
            .query_map([], |r| {
                let keyword: String = r.get(0)?;
                let name: Option<String> = r.get(1)?;
                Ok(Tag {
                    name: name
                        .or_else(|| default_name(&keyword))
                        .unwrap_or_else(|| keyword.clone()),
                    keyword,
                    color: r.get(2)?,
                    count: 0,
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut counts = self.db.prepare_cached(
            "SELECT keywords, COUNT(*) FROM messages WHERE keywords <> '' GROUP BY keywords",
        )?;
        let rows = counts.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (keywords, n) = row?;
            for k in keywords.split_whitespace() {
                if let Some(tag) = tags.iter_mut().find(|t| t.keyword == k) {
                    tag.count += usize::try_from(n).unwrap_or_default();
                }
            }
        }
        Ok(tags)
    }

    /// The keywords that are tags here.
    pub(crate) fn tag_keywords(&self) -> Result<HashSet<String>, StoreError> {
        let mut stmt = self.db.prepare_cached("SELECT keyword FROM tags")?;
        let keywords = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(keywords)
    }

    /// A tag's name, or `None` when there is no such tag.
    pub fn tag_name(&self, keyword: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .tags()?
            .into_iter()
            .find(|t| t.keyword == keyword)
            .map(|t| t.name))
    }

    /// Save a tag: a new one (empty keyword) gets a keyword from its name and goes last; an
    /// existing one changes its name and color (its keyword stays, so mail keeps it).
    pub fn save_tag(&mut self, tag: &Tag) -> Result<Tag, StoreError> {
        self.store_tag(tag, tag.keyword.trim().is_empty())
    }

    /// Save a tag from a settings file: with the file's keyword, so the mail on a server
    /// that carries it shows it here too.
    pub fn import_tag(&mut self, tag: &Tag) -> Result<Tag, StoreError> {
        let keyword = tag.keyword.trim().to_ascii_lowercase();
        if !is_keyword(&keyword) {
            return Err(tag_gone());
        }
        let exists = self.tag_keywords()?.contains(&keyword);
        self.store_tag(tag, !exists)
    }

    fn store_tag(&mut self, tag: &Tag, create: bool) -> Result<Tag, StoreError> {
        let name = tag.name.trim().to_owned();
        if name.chars().count() > NAME_MAX || name.chars().any(char::is_control) {
            return Err(StoreError::Invalid(tr!(
                "Give the tag a name of 1–40 characters.",
                "Nadaj etykiecie nazwę od 1 do 40 znaków."
            )));
        }
        if !COLORS.contains(&tag.color.as_str()) {
            return Err(StoreError::Invalid(tr!(
                "Pick one of the tag colors.",
                "Wybierz jeden z kolorów etykiet."
            )));
        }
        let tags = self.tags()?;
        let keyword = tag.keyword.trim().to_ascii_lowercase();
        // A default tag's empty name brings back its own; any other tag needs one.
        let stored = if name.is_empty() || default_name(&keyword).as_deref() == Some(&name) {
            if default_name(&keyword).is_none() {
                return Err(StoreError::Invalid(tr!(
                    "Give the tag a name of 1–40 characters.",
                    "Nadaj etykiecie nazwę od 1 do 40 znaków."
                )));
            }
            None
        } else {
            Some(name.clone())
        };
        let shown = stored
            .clone()
            .or_else(|| default_name(&keyword))
            .unwrap_or_default();
        if tags
            .iter()
            .any(|t| t.keyword != keyword && fold(&t.name) == fold(&shown))
        {
            return Err(StoreError::Invalid(tr!(
                "A tag already has that name.",
                "Etykieta o tej nazwie już istnieje."
            )));
        }
        let keyword = if create {
            if tags.len() >= TAGS_MAX {
                return Err(StoreError::Invalid(tr!(
                    "Telex keeps up to 50 tags.",
                    "Telex przechowuje do 50 etykiet."
                )));
            }
            let keyword = if keyword.is_empty() {
                let taken = tags.iter().map(|t| t.keyword.clone()).collect();
                keyword_for(&name, &taken)
            } else {
                keyword
            };
            self.db.execute(
                "INSERT INTO tags (keyword, name, color, position)
                 VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(position), -1) + 1 FROM tags))",
                params![keyword, stored, tag.color],
            )?;
            keyword
        } else {
            let changed = self.db.execute(
                "UPDATE tags SET name = ?2, color = ?3 WHERE keyword = ?1",
                params![keyword, stored, tag.color],
            )?;
            if changed == 0 {
                return Err(tag_gone());
            }
            keyword
        };
        self.tags()?
            .into_iter()
            .find(|t| t.keyword == keyword)
            .ok_or_else(tag_gone)
    }

    /// Delete a tag and take it off every message, here and on the server, in one
    /// transaction. Rules that use it say so when they run. Returns how many messages lost it.
    pub fn delete_tag(&mut self, keyword: &str) -> Result<usize, StoreError> {
        let ids: Vec<MessageId> = {
            let mut stmt = self.db.prepare_cached(
                "SELECT id FROM messages WHERE keywords <> ''
                   AND instr(' ' || keywords || ' ', ' ' || ?1 || ' ') > 0",
            )?;
            let ids = stmt
                .query_map([keyword], |r| r.get::<_, i64>(0))?
                .map(|id| id.map(|id| id as MessageId))
                .collect::<Result<_, _>>()?;
            ids
        };
        let tx = self.db.transaction()?;
        let removed = set_tag_in(&tx, &ids, keyword, false)?;
        tx.execute("DELETE FROM tags WHERE keyword = ?1", [keyword])?;
        tx.commit()?;
        Ok(removed)
    }

    /// Move a tag up (`-1`) or down (`1`) in the order; the first five have keys 1–5.
    pub fn move_tag(&mut self, keyword: &str, delta: i64) -> Result<(), StoreError> {
        let mut order: Vec<String> = self.tags()?.into_iter().map(|t| t.keyword).collect();
        let Some(from) = order.iter().position(|k| k == keyword) else {
            return Ok(());
        };
        let to = (from as i64 + delta).clamp(0, order.len() as i64 - 1) as usize;
        let moved = order.remove(from);
        order.insert(to, moved);
        let tx = self.db.transaction()?;
        for (position, keyword) in order.iter().enumerate() {
            tx.execute(
                "UPDATE tags SET position = ?2 WHERE keyword = ?1",
                params![keyword, position as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Add a tag to messages (`on`) or take it off, in one transaction. Returns how many
    /// changed; the others already were so.
    pub fn set_tag_many(
        &mut self,
        ids: &[MessageId],
        keyword: &str,
        on: bool,
    ) -> Result<usize, StoreError> {
        if !self.tag_keywords()?.contains(keyword) {
            return Err(tag_gone());
        }
        let tx = self.db.transaction()?;
        let done = set_tag_in(&tx, ids, keyword, on)?;
        tx.commit()?;
        Ok(done)
    }

    /// The keywords of a message's tags.
    pub fn keywords_of(&self, id: MessageId) -> Result<Vec<String>, StoreError> {
        let keywords: Option<String> = self
            .db
            .query_row(
                "SELECT keywords FROM messages WHERE id = ?1",
                [id as i64],
                |r| r.get(0),
            )
            .optional()?;
        Ok(split(&keywords.unwrap_or_default()))
    }

    /// The account's server can't keep tags, so they stay on this PC.
    pub fn tags_kept_here(&self, account: &str) -> Result<bool, StoreError> {
        Ok(self
            .db
            .query_row(
                "SELECT local_tags FROM accounts WHERE id = ?1",
                [account],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default())
    }

    /// From now on the account's tags stay on this PC. True when that is news.
    pub(crate) fn keep_tags_here(&mut self, account: &str) -> Result<bool, StoreError> {
        Ok(self.db.execute(
            "UPDATE accounts SET local_tags = 1 WHERE id = ?1 AND local_tags = 0",
            [account],
        )? > 0)
    }

    /// Accounts whose tags stay on this PC, for the window.
    pub fn accounts_keeping_tags_here(&self) -> Result<Vec<String>, StoreError> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT id FROM accounts WHERE local_tags ORDER BY id")?;
        let ids = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|k| (*k).to_owned()).collect()
    }

    #[test]
    fn keywords_are_made_as_thunderbird_makes_them() {
        let none = HashSet::new();
        assert_eq!(keyword_for("Projekt X", &none), "projekt_x");
        assert_eq!(keyword_for("To do (soon)", &none), "to_do__soon_");
        assert_eq!(keyword_for("a/b*c", &none), "a_b_c");
        // Thunderbird's modified UTF-7, lowercased: ż is U+017C.
        assert_eq!(keyword_for("Ważne", &none), "wa&axw-ne");
        assert_eq!(keyword_for("R&D", &none), "r&-d");
        assert_eq!(keyword_for("Projekt X", &set(&["projekt_x"])), "projekt_xa");
        assert_eq!(
            keyword_for("Junk", &none),
            "junka",
            "never another program's mark"
        );
        assert!(is_keyword(&keyword_for("\\Seen", &none)));
        for name in ["Projekt X", "Ważne", "ąęłńóśźż", "日本", "R&D", "{]%*\""] {
            assert!(is_keyword(&keyword_for(name, &none)), "{name}");
        }
    }

    #[test]
    fn other_programs_colors_go_to_the_nearest_tag_color() {
        // Thunderbird's five, then a few of its color picker's.
        for (hex, color) in [
            ("#FF0000", "red"),
            ("#FF9900", "orange"),
            ("#009900", "green"),
            ("#3333FF", "blue"),
            ("#993399", "purple"),
            ("#ffcc00", "yellow"),
            ("#f90", "orange"),
            ("#00CCCC", "teal"),
            ("#CC0066", "red"),
            ("#808080", "gray"),
            ("#000000", "gray"),
            ("#FFFFFF", "gray"),
            ("", "gray"),
            ("blue", "gray"),
        ] {
            assert_eq!(color_from_hex(hex), color, "{hex}");
        }
        assert!(COLORS.contains(&color_from_hex("#123456")));
    }

    #[test]
    fn only_atoms_are_keywords() {
        assert!(is_keyword("$label1") && is_keyword("projekt_x") && is_keyword("wa&axw-ne"));
        for bad in ["", "\\Seen", "a b", "a(b", "a]b", "a*", "zażółć", "a\"b"] {
            assert!(!is_keyword(bad), "{bad}");
        }
    }

    #[test]
    fn keywords_are_kept_sorted_lowercase_and_once() {
        let list = |s: &[&str]| s.iter().map(|k| (*k).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            join(&list(&["$label4", "$Label1", "$label4", "bad word"])),
            "$label1 $label4"
        );
        assert_eq!(split("$label1 $label4"), list(&["$label1", "$label4"]));
        assert_eq!(split(""), Vec::<String>::new());
    }

    #[test]
    fn the_server_s_tags_win_except_where_a_change_is_queued() {
        let tags = set(&["$label1", "$label2", "$label3"]);
        let server = |s: &[&str]| s.iter().map(|k| (*k).to_owned()).collect::<Vec<_>>();
        // Taken off elsewhere, added elsewhere; other keywords left out.
        assert_eq!(
            merge(
                "$label1",
                &server(&["$Label2", "$Forwarded", "NonJunk"]),
                "",
                &tags
            ),
            "$label2"
        );
        // $label1 added here and $label3 taken off here, both still queued.
        assert_eq!(
            merge("$label1", &server(&["$label3"]), "$label1 $label3", &tags),
            "$label1"
        );
        assert_eq!(merge("$label1", &[], "", &tags), "");
    }

    #[test]
    fn the_five_defaults_speak_the_interface_s_language() {
        let store = MailStore::in_memory().unwrap();
        let names = |lang| {
            crate::i18n::with(lang, || {
                store
                    .tags()
                    .unwrap()
                    .into_iter()
                    .map(|t| (t.keyword, t.name, t.color))
                    .collect::<Vec<_>>()
            })
        };
        let en = names(crate::i18n::Lang::En);
        assert_eq!(en.len(), 5);
        assert_eq!(en[0], ("$label1".into(), "Important".into(), "red".into()));
        assert_eq!(en[4], ("$label5".into(), "Later".into(), "purple".into()));
        let pl = names(crate::i18n::Lang::Pl);
        assert_eq!(pl[1].1, "Praca");
        assert_eq!(pl[3].1, "Do zrobienia");
    }

    #[test]
    fn tags_are_made_renamed_reordered_and_checked() {
        let mut store = MailStore::in_memory().unwrap();
        let new = |name: &str, color: &str| Tag {
            keyword: String::new(),
            name: name.into(),
            color: color.into(),
            count: 0,
        };
        let made = store.save_tag(&new("Projekt X", "teal")).unwrap();
        assert_eq!(made.keyword, "projekt_x");
        assert_eq!(store.tags().unwrap().last().unwrap().name, "Projekt X");
        for (bad, color) in [
            ("", "teal"),
            ("projekt x", "teal"),
            ("Nowa", "pink"),
            ("Work", "red"),
        ] {
            assert!(
                matches!(
                    store.save_tag(&new(bad, color)),
                    Err(StoreError::Invalid(_))
                ),
                "{bad} {color}"
            );
        }
        // Renamed and recolored, the keyword stays; an empty name brings the default back.
        let mut work = store.tags().unwrap()[1].clone();
        work.name = "Biuro".into();
        work.color = "gray".into();
        let saved = store.save_tag(&work).unwrap();
        assert_eq!(
            (saved.keyword.as_str(), saved.name.as_str()),
            ("$label2", "Biuro")
        );
        work.name = String::new();
        assert_eq!(store.save_tag(&work).unwrap().name, "Work");
        store.move_tag("projekt_x", -10).unwrap();
        assert_eq!(store.tags().unwrap()[0].keyword, "projekt_x");
        store.move_tag("projekt_x", 1).unwrap();
        assert_eq!(store.tags().unwrap()[1].keyword, "projekt_x");
        let mut gone = made.clone();
        store.delete_tag("projekt_x").unwrap();
        gone.name = "Inna".into();
        assert!(matches!(store.save_tag(&gone), Err(StoreError::Invalid(_))));
    }
}
