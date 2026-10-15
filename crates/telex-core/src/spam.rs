//! Telex's own spam filter: learned from the user's mail, run on this PC, nothing sent anywhere.
//!
//! A statistical filter in the manner of SpamBayes: every message is cut into tokens (words
//! of the subject and the text, the sender's address and domain, the hosts links point to,
//! attachment types, a few header facts), each token gets Gary Robinson's estimate of how
//! spammy it is from how often it was seen in spam and in good mail, and the strongest of
//! them are combined with Fisher's chi-squared method into one score between 0 and 1.
//!
//! What it learns from: mail in an account's Spam folder is spam; read mail in the Inbox,
//! Sent, Archive and the user's own folders is good mail ([`learn_pending`], a batch at each
//! check). Moving a message into Spam or out of it (the menu's Spam and Not spam, or any move)
//! learns it again, undoing what it was learned as before. Mail the filter moves itself is
//! not learned from, so it can't talk itself into its own mistakes.
//!
//! What it does: new Inbox mail scoring above the chosen [`Level`] goes to the account's Spam
//! folder with a note saying why ([`sort_arrivals`], before the rules). It never touches mail
//! from people the user has written to, from trusted senders, or answering the user's own
//! mail, and does nothing until it has learned from [`MIN_LEARNED`] messages of each kind.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, Ordering};

use mail_parser::{Address, MessageParser, MimeHeaders};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::filters::fold;
use crate::model::{account_of, FolderKind, Message, MessageId};
use crate::store::{MailStore, StoreError, LOCAL_ID};
use crate::tr;

/// How sure the filter must be before it moves new mail to Spam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// It learns, but moves nothing.
    Off,
    /// Only mail it is all but certain of (the default: a good message in Spam costs more
    /// than spam in the Inbox).
    #[default]
    Careful,
    Normal,
    /// More spam caught, and now and then a good message with it.
    Strict,
}

impl Level {
    /// The score from which new mail goes to Spam; `None`: never.
    pub fn cutoff(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Careful => Some(0.99),
            Self::Normal => Some(0.95),
            Self::Strict => Some(0.90),
        }
    }
}

/// The level the settings chose, for every thread (like the language, see `i18n`).
static LEVEL: AtomicU8 = AtomicU8::new(Level::Careful as u8);

/// Use this level from now on (the settings call it when they are read or changed).
pub fn set_level(level: Level) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

/// The level checks use now.
pub fn level() -> Level {
    match LEVEL.load(Ordering::Relaxed) {
        0 => Level::Off,
        2 => Level::Normal,
        3 => Level::Strict,
        _ => Level::Careful,
    }
}

/// The filter acts once it has learned from this many spam and this many good messages.
pub const MIN_LEARNED: u32 = 50;
/// Messages of each kind learned from folders by themselves; moves are learned beyond it.
const AUTO_LEARN_MAX: u32 = 5000;
/// Messages learned from folders at one check, so a first check with a large mail base
/// doesn't hold the store for long; the rest come at the next checks.
pub const LEARN_BATCH: usize = 400;
/// Robinson's strength of the prior (s) and the score of a token seen nowhere yet (x).
const PRIOR_STRENGTH: f64 = 0.45;
const UNKNOWN: f64 = 0.5;
/// Tokens closer to 0.5 than this say nothing, and at most this many of the strongest count.
const MIN_STRENGTH: f64 = 0.1;
const MAX_CLUES: usize = 150;
/// Of a message, the filter reads this much: the start of the raw message for its header,
/// and of its text.
const HEAD_BYTES: i64 = 16 * 1024;
const BODY_CHARS: usize = 64 * 1024;
const MAX_LINKS: usize = 100;

/// What the filter knows about a message (`spam_trained.class`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Spam = 1,
    Ham = 0,
    /// Moved to Spam by the filter: not learned from.
    Judged = -1,
}

impl Class {
    fn from_db(v: i64) -> Self {
        match v {
            1 => Self::Spam,
            0 => Self::Ham,
            _ => Self::Judged,
        }
    }
}

/// Which side of the filter a folder is on: Spam, good mail, or neither (Drafts, the
/// Outbox, Trash: what is there says nothing).
fn side(kind: Option<FolderKind>) -> Option<Class> {
    match kind? {
        FolderKind::Spam => Some(Class::Spam),
        FolderKind::Inbox | FolderKind::Sent | FolderKind::Archive | FolderKind::Custom => {
            Some(Class::Ham)
        }
        FolderKind::Drafts | FolderKind::Outbox | FolderKind::Trash => None,
    }
}

// ---- Tokens ----

/// Words of a text, lowercase without diacritics: 3 to 20 letters or digits, not only digits.
/// Longer runs (encoded junk, hashes) count by their length alone.
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && !w.chars().all(|c| c.is_ascii_digit()))
        .map(|w| {
            let n = w.chars().count();
            if n > 20 {
                format!("long:{}", n / 10 * 10)
            } else {
                fold(w)
            }
        })
}

/// The last two labels of a host name: "mail.shop.example.com" → "example.com".
fn base_domain(host: &str) -> &str {
    let mut dots = host.rmatch_indices('.');
    match (dots.next(), dots.next()) {
        (Some(_), Some((i, _))) => &host[i + 1..],
        _ => host,
    }
}

/// The hosts of links written out in a text (http, https, www.).
fn link_hosts(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut hosts = Vec::new();
    let mut rest = lower.as_str();
    while hosts.len() < MAX_LINKS {
        let Some(at) = ["http://", "https://", "www."]
            .iter()
            .filter_map(|p| rest.find(p).map(|i| (i, p.len())))
            .min()
        else {
            break;
        };
        let (start, skip) = (
            at.0,
            if rest[at.0..].starts_with("www.") {
                0
            } else {
                at.1
            },
        );
        let after = &rest[start + skip..];
        let end = after
            .find(|c: char| c.is_whitespace() || "/?#<>\"'()[]".contains(c))
            .unwrap_or(after.len());
        let authority = &after[..end];
        // "https://bank.com@evil.example": the host is after the last "@".
        let host = authority.rsplit('@').next().unwrap_or_default();
        let host = host
            .split(':')
            .next()
            .unwrap_or_default()
            .trim_end_matches('.');
        if host.contains('.') {
            hosts.push(host.to_owned());
        }
        rest = &after[end..];
    }
    hosts
}

/// The tokens of a message: what the filter learns from and judges by. `head` is the start of
/// the raw message (its header); `own` the user's own addresses, whose sender tokens are left
/// out (the user's Sent mail would otherwise vouch for any spam that forges their address).
pub fn tokens(m: &Message, head: Option<&[u8]>, own: &HashSet<String>) -> HashSet<String> {
    let mut t = HashSet::new();

    let subject = m.subject.trim();
    if subject.is_empty() {
        t.insert("subject:none".to_owned());
    }
    t.extend(words(subject).map(|w| format!("subject:{w}")));
    if subject.matches('!').count() >= 2 {
        t.insert("subject:!!".to_owned());
    }
    let letters: Vec<char> = subject.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 8
        && letters.iter().filter(|c| c.is_uppercase()).count() * 10 >= letters.len() * 7
    {
        t.insert("subject:shouting".to_owned());
    }

    let from = m.from_email.trim().to_lowercase();
    if !own.contains(&from) {
        match from.rsplit_once('@') {
            Some((local, domain)) => {
                t.insert(format!("from:{from}"));
                t.insert(format!("from:@{domain}"));
                t.insert(format!("from:@@{}", base_domain(domain)));
                if local.chars().filter(char::is_ascii_digit).count() >= 4 {
                    t.insert("from:digits".to_owned());
                }
            }
            None => {
                t.insert("from:none".to_owned());
            }
        }
        t.extend(words(&m.from_name).map(|w| format!("name:{w}")));
    }

    let recipients = crate::smtp::split_addresses(&format!("{}, {}", m.to, m.cc)).len();
    match recipients {
        0 => t.insert("to:none".to_owned()),
        1 => t.insert("to:one".to_owned()),
        2..=9 => t.insert("to:few".to_owned()),
        _ => t.insert("to:many".to_owned()),
    };

    let body: String = m.body.chars().take(BODY_CHARS).collect();
    t.extend(words(&body));
    for host in link_hosts(&body) {
        if host.parse::<std::net::Ipv4Addr>().is_ok() {
            t.insert("url:ip".to_owned());
            continue;
        }
        t.insert(format!("url:@@{}", base_domain(&host)));
        t.insert(format!("url:{host}"));
    }
    if body.trim().is_empty() {
        t.insert("body:none".to_owned());
    }
    for (sign, token) in [("$", "body:$"), ("€", "body:€"), ("!!!", "body:!!!")] {
        if body.contains(sign) {
            t.insert(token.to_owned());
        }
    }

    for a in &m.attachments {
        let ext = a
            .name
            .rsplit_once('.')
            .map_or("none", |(_, e)| e)
            .to_lowercase();
        if ext.len() <= 8 {
            t.insert(format!("file:{ext}"));
        }
    }
    if m.remote_images {
        t.insert("html:remote-images".to_owned());
    }
    t.insert(format!("size:{}", (m.size_kb.max(1)).ilog2()));

    if let Some(head) = head {
        header_tokens(head, &from, &mut t);
    }
    t
}

/// Facts from the header: what wrote the message, how it is built, whether it came from a
/// mailing list, what the user's server said of the sender (the first Authentication-Results,
/// the one the last server added), where Reply-To goes and where the Message-ID was made.
/// The filter learns how much each of these says; none of them is trusted on its own.
fn header_tokens(head: &[u8], from: &str, t: &mut HashSet<String>) {
    let Some(p) = MessageParser::default().parse_headers(head) else {
        return;
    };
    if let Some(ct) = p.content_type() {
        let kind = match ct.subtype() {
            Some(sub) => format!("{}/{sub}", ct.ctype()),
            None => ct.ctype().to_owned(),
        };
        t.insert(format!("type:{}", kind.to_lowercase()));
        if let Some(charset) = ct.attribute("charset") {
            t.insert(format!("charset:{}", charset.to_lowercase()));
        }
    }
    let mut seen = HashSet::new();
    let mut received = 0;
    for h in p.headers() {
        let name = h.name.as_str().to_ascii_lowercase();
        if name == "received" {
            received += 1;
        }
        // The first of each: the one the last server put on top.
        if !seen.insert(name.clone()) {
            continue;
        }
        let value = h.value.as_text().unwrap_or_default().to_lowercase();
        match name.as_str() {
            "x-mailer" | "user-agent" => {
                t.extend(words(&value).take(4).map(|w| format!("mailer:{w}")));
            }
            "list-unsubscribe" | "list-id" => {
                t.insert(format!("header:{name}"));
            }
            "precedence" => {
                t.insert(format!("precedence:{}", value.trim()));
            }
            "authentication-results" => {
                for method in ["spf", "dkim", "dmarc"] {
                    let key = format!("{method}=");
                    if let Some(i) = value.find(&key) {
                        let result: String = value[i + key.len()..]
                            .chars()
                            .take_while(char::is_ascii_alphabetic)
                            .collect();
                        t.insert(format!("auth:{method}={result}"));
                    }
                }
            }
            "x-spam-flag" => {
                t.insert(format!("x-spam-flag:{}", value.trim()));
            }
            _ => {}
        }
    }
    t.insert(format!("received:{}", received.min(8)));
    let from_domain = from.rsplit_once('@').map(|(_, d)| d);
    if let Some(reply) = p
        .reply_to()
        .and_then(Address::first)
        .and_then(|a| a.address())
        .map(str::to_lowercase)
    {
        let reply_domain = reply.rsplit_once('@').map(|(_, d)| d.to_owned());
        t.insert(
            if reply_domain.as_deref().map(base_domain) == from_domain.map(base_domain) {
                "reply-to:same".to_owned()
            } else {
                "reply-to:elsewhere".to_owned()
            },
        );
    }
    match p.message_id().and_then(|id| id.rsplit_once('@')) {
        Some((_, domain)) => {
            t.insert(format!("msgid:@@{}", base_domain(&domain.to_lowercase())));
        }
        None => {
            t.insert("msgid:none".to_owned());
        }
    }
}

// ---- Scoring ----

/// What the filter thinks of a message.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Verdict {
    /// 0: good mail, 1: spam, about 0.5: it can't tell.
    pub score: f64,
    /// The tokens that decided, strongest first, with their own score.
    pub clues: Vec<(String, f64)>,
}

/// How many spam and good messages were learned from.
fn totals(db: &Connection) -> Result<(u32, u32), StoreError> {
    Ok(db
        .query_row(
            "SELECT spam, ham FROM spam_tokens WHERE token = ''",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or((0, 0)))
}

/// Robinson's estimate for a token seen in `spam` of `nspam` spam messages and `ham` of
/// `nham` good ones; `None` for a token seen in neither.
fn token_score(spam: u32, ham: u32, nspam: u32, nham: u32) -> Option<f64> {
    let n = f64::from(spam) + f64::from(ham);
    if n == 0.0 {
        return None;
    }
    let spam_ratio = f64::from(spam) / f64::from(nspam.max(1));
    let ham_ratio = f64::from(ham) / f64::from(nham.max(1));
    let p = spam_ratio / (spam_ratio + ham_ratio);
    Some((PRIOR_STRENGTH * UNKNOWN + n * p) / (PRIOR_STRENGTH + n))
}

/// The chance that a chi-squared value as large as `x2` comes by with `v` (even) degrees of
/// freedom.
fn chi2q(x2: f64, v: usize) -> f64 {
    let m = x2 / 2.0;
    let mut term = (-m).exp();
    let mut sum = term;
    for i in 1..v / 2 {
        term *= m / i as f64;
        sum += term;
    }
    sum.min(1.0)
}

/// Fisher's combination of token scores, both ways: how sure it is spam, how sure it is
/// good, and the score half way between.
fn combine(scores: &[f64]) -> f64 {
    if scores.is_empty() {
        return UNKNOWN;
    }
    let (spam_ln, ham_ln) = scores
        .iter()
        .fold((0.0, 0.0), |(s, h), p| (s + (1.0 - p).ln(), h + p.ln()));
    let v = 2 * scores.len();
    let spam = 1.0 - chi2q(-2.0 * spam_ln, v);
    let ham = 1.0 - chi2q(-2.0 * ham_ln, v);
    (spam - ham + 1.0) / 2.0
}

/// Score a message's tokens against what was learned.
fn judge(db: &Connection, tokens: &HashSet<String>) -> Result<Verdict, StoreError> {
    let (nspam, nham) = totals(db)?;
    let mut stmt = db.prepare_cached("SELECT spam, ham FROM spam_tokens WHERE token = ?1")?;
    let mut clues = Vec::new();
    for token in tokens {
        let counts: Option<(u32, u32)> = stmt
            .query_row([token], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        if let Some(p) = counts.and_then(|(s, h)| token_score(s, h, nspam, nham)) {
            if (p - UNKNOWN).abs() >= MIN_STRENGTH {
                clues.push((token.clone(), p));
            }
        }
    }
    clues.sort_by(|a, b| {
        (b.1 - UNKNOWN)
            .abs()
            .total_cmp(&(a.1 - UNKNOWN).abs())
            .then_with(|| a.0.cmp(&b.0))
    });
    clues.truncate(MAX_CLUES);
    let scores: Vec<f64> = clues.iter().map(|c| c.1).collect();
    Ok(Verdict {
        score: combine(&scores),
        clues,
    })
}

// ---- Learning ----

/// A message to learn: its tokens, what it was learned as before, and what it is now.
pub(crate) struct Lesson {
    id: MessageId,
    tokens: HashSet<String>,
    before: Option<Class>,
    now: Class,
}

/// The user's own addresses, lowercase.
fn own_addresses(store: &MailStore) -> HashSet<String> {
    store
        .accounts()
        .iter()
        .flat_map(|a| {
            std::iter::once(a.email.to_lowercase())
                .chain(a.identities.iter().map(|i| i.email.to_lowercase()))
        })
        .collect()
}

/// The start of a stored message's raw form, for its header.
fn head_of(db: &Connection, id: MessageId) -> Result<Option<Vec<u8>>, StoreError> {
    Ok(db
        .prepare_cached("SELECT substr(raw, 1, ?2) FROM message_content WHERE id = ?1")?
        .query_row(params![id as i64, HEAD_BYTES], |r| r.get(0))
        .optional()?
        .flatten())
}

fn message_tokens(
    store: &MailStore,
    id: MessageId,
    own: &HashSet<String>,
) -> Result<HashSet<String>, StoreError> {
    let m = store.get(id)?;
    Ok(tokens(&m, head_of(&store.db, id)?.as_deref(), own))
}

fn class_of(db: &Connection, id: MessageId) -> Result<Option<Class>, StoreError> {
    Ok(db
        .prepare_cached("SELECT class FROM spam_trained WHERE message = ?1")?
        .query_row([id as i64], |r| r.get(0))
        .optional()?
        .map(Class::from_db))
}

/// What moving these messages (each with the folder it is in) to `to` teaches: into Spam,
/// spam; out of Spam into the Inbox or a folder of the user's, good mail. Other moves
/// (to Trash, between the user's folders) teach nothing.
pub(crate) fn lessons_for_move(
    store: &MailStore,
    places: &[(MessageId, &str)],
    to: &str,
) -> Result<Vec<Lesson>, StoreError> {
    let target = side(store.kind_of(to));
    let mut lessons = Vec::new();
    let mut own = None;
    for &(id, from) in places {
        let source = side(store.kind_of(from));
        let now = match (source, target) {
            (Some(Class::Spam), Some(Class::Spam)) => continue,
            (_, Some(Class::Spam)) => Class::Spam,
            (Some(Class::Spam), Some(Class::Ham)) => Class::Ham,
            _ => continue,
        };
        let before = class_of(&store.db, id)?;
        if before == Some(now) {
            continue;
        }
        let own = own.get_or_insert_with(|| own_addresses(store));
        lessons.push(Lesson {
            id,
            tokens: message_tokens(store, id, own)?,
            before,
            now,
        });
    }
    Ok(lessons)
}

/// Learn these lessons: forget what each message was learned as before, learn what it is now.
pub(crate) fn learn(db: &Connection, lessons: &[Lesson]) -> Result<(), StoreError> {
    if lessons.is_empty() {
        return Ok(());
    }
    // Per token (and "" for the message counts): spam and good mail to add or take away.
    let mut delta: HashMap<&str, (i64, i64)> = HashMap::new();
    for l in lessons {
        let changes = l.before.map(|b| (b, -1)).into_iter().chain([(l.now, 1)]);
        for (class, by) in changes {
            let (spam, ham) = match class {
                Class::Spam => (by, 0),
                Class::Ham => (0, by),
                Class::Judged => continue,
            };
            for token in std::iter::once("").chain(l.tokens.iter().map(String::as_str)) {
                let d = delta.entry(token).or_default();
                d.0 += spam;
                d.1 += ham;
            }
        }
    }
    let mut upsert = db.prepare_cached(
        "INSERT INTO spam_tokens (token, spam, ham) VALUES (?1, max(?2, 0), max(?3, 0))
         ON CONFLICT (token) DO UPDATE SET spam = max(spam + ?2, 0), ham = max(ham + ?3, 0)",
    )?;
    let mut prune = db.prepare_cached(
        "DELETE FROM spam_tokens WHERE token = ?1 AND token <> '' AND spam = 0 AND ham = 0",
    )?;
    for (token, (spam, ham)) in delta {
        if spam == 0 && ham == 0 {
            continue;
        }
        upsert.execute(params![token, spam, ham])?;
        if spam < 0 || ham < 0 {
            prune.execute([token])?;
        }
    }
    let mut mark =
        db.prepare_cached("INSERT OR REPLACE INTO spam_trained (message, class) VALUES (?1, ?2)")?;
    for l in lessons {
        mark.execute(params![l.id as i64, l.now as i64])?;
    }
    Ok(())
}

/// The folder keys of every account (and the folders on this PC) on each side.
fn folders_by_side(store: &MailStore) -> (Vec<String>, Vec<String>) {
    let (mut spam, mut ham) = (Vec::new(), Vec::new());
    for a in store.accounts() {
        for f in &a.folders {
            match side(Some(f.kind)) {
                Some(Class::Spam) => spam.push(a.folder_key(&f.id)),
                Some(Class::Ham) => ham.push(a.folder_key(&f.id)),
                _ => {}
            }
        }
    }
    (spam, ham)
}

/// Up to `limit` messages of these folders the filter knows nothing of, newest first.
fn unlearned(
    db: &Connection,
    folders: &[String],
    read_only: bool,
    limit: usize,
) -> Result<Vec<MessageId>, StoreError> {
    if folders.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let marks = vec!["?"; folders.len()].join(", ");
    let read = if read_only { "AND m.unread = 0" } else { "" };
    let mut stmt = db.prepare(&format!(
        "SELECT m.id FROM messages m WHERE m.folder IN ({marks}) {read}
           AND NOT EXISTS (SELECT 1 FROM spam_trained t WHERE t.message = m.id)
         ORDER BY m.id DESC LIMIT {limit}"
    ))?;
    let ids = stmt
        .query_map(rusqlite::params_from_iter(folders), |r| {
            r.get::<_, i64>(0).map(|id| id as MessageId)
        })?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Learn from the mail already sorted: up to `limit` messages of each kind not learned yet,
/// newest first. Spam: what is in Spam folders. Good mail: what was read in the Inbox, Sent,
/// Archive and the user's folders (unread mail may still turn out to be spam). Returns how
/// many it learned.
pub fn learn_pending(store: &mut MailStore, limit: usize) -> Result<usize, StoreError> {
    let (nspam, nham) = totals(&store.db)?;
    let (spam_folders, ham_folders) = folders_by_side(store);
    let room = |n: u32| limit.min(AUTO_LEARN_MAX.saturating_sub(n) as usize);
    let spam_ids = unlearned(&store.db, &spam_folders, false, room(nspam))?;
    let ham_ids = unlearned(&store.db, &ham_folders, true, room(nham))?;
    if spam_ids.is_empty() && ham_ids.is_empty() {
        return Ok(0);
    }
    let own = own_addresses(store);
    let mut lessons = Vec::with_capacity(spam_ids.len() + ham_ids.len());
    for (ids, now) in [(spam_ids, Class::Spam), (ham_ids, Class::Ham)] {
        for id in ids {
            lessons.push(Lesson {
                id,
                tokens: message_tokens(store, id, &own)?,
                before: None,
                now,
            });
        }
    }
    let tx = store.db.unchecked_transaction()?;
    learn(&tx, &lessons)?;
    tx.commit()?;
    Ok(lessons.len())
}

// ---- Sorting new mail ----

/// The words of the strongest spam clues, as a person would recognize them.
fn telling_words(clues: &[(String, f64)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (token, p) in clues {
        if *p <= UNKNOWN {
            continue;
        }
        let word = match token.split_once(':') {
            None => token.as_str(),
            Some(("subject", w)) => w,
            Some(("url" | "from", w)) if w.starts_with("@@") => &w[2..],
            Some(("url", w)) if w != "ip" => w,
            _ => continue,
        };
        if !out.iter().any(|o| o == word) {
            out.push(word.to_owned());
        }
        if out.len() == 3 {
            break;
        }
    }
    out
}

/// The note on a message the filter moved to Spam.
fn note(verdict: &Verdict) -> String {
    let score = ((verdict.score * 100.0).floor() as u32).min(99);
    let words = telling_words(&verdict.clues).join(", ");
    if words.is_empty() {
        tr!(
            "The spam filter moved it to Spam (spam score {score} of 100). If it isn't spam, choose Not spam in its menu.",
            "Filtr spamu przeniósł ją do Spamu (ocena {score} na 100). Jeśli to nie spam, wybierz „To nie spam” z jej menu."
        )
    } else {
        tr!(
            "The spam filter moved it to Spam (spam score {score} of 100; telling words: {words}). If it isn't spam, choose Not spam in its menu.",
            "Filtr spamu przeniósł ją do Spamu (ocena {score} na 100; słowa, które przesądziły: {words}). Jeśli to nie spam, wybierz „To nie spam” z jej menu."
        )
    }
}

/// Judge new Inbox mail and move what scores above `level`'s cutoff to its account's Spam
/// folder, with a note. Leaves alone mail from people the user wrote to, trusted senders,
/// mail answering the user's own, and everything while too little has been learned. Returns
/// the messages it moved.
pub fn sort_arrivals(
    store: &mut MailStore,
    ids: &[MessageId],
    level: Level,
) -> Result<Vec<MessageId>, StoreError> {
    let Some(cutoff) = level.cutoff() else {
        return Ok(Vec::new());
    };
    let (nspam, nham) = totals(&store.db)?;
    if ids.is_empty() || nspam < MIN_LEARNED || nham < MIN_LEARNED {
        return Ok(Vec::new());
    }
    let known = store.correspondence()?;
    let trusted: HashSet<String> = store
        .trusted_senders()?
        .into_iter()
        .map(|e| e.to_lowercase())
        .collect();
    let own = own_addresses(store);
    let mut moved = Vec::new();
    for &id in ids {
        let Ok(m) = store.get(id) else {
            continue; // gone meanwhile
        };
        if store.kind_of(&m.folder) != Some(FolderKind::Inbox) {
            continue;
        }
        let from = m.from_email.trim().to_lowercase();
        if known.addresses.contains(&from)
            || trusted.contains(&from)
            || m.refs.iter().any(|r| known.message_ids.contains(r))
        {
            continue;
        }
        let account = account_of(&m.folder);
        let Some(spam) = store.account(account).ok().and_then(|a| {
            a.folders
                .iter()
                .find(|f| f.kind == FolderKind::Spam)
                .map(|f| a.folder_key(&f.id))
        }) else {
            continue;
        };
        let verdict = judge(&store.db, &message_tokens(store, id, &own)?)?;
        if verdict.score < cutoff {
            continue;
        }
        store.add_note(id, &note(&verdict))?;
        store.move_unlearned(&[id], &spam)?;
        store.db.execute(
            "INSERT OR REPLACE INTO spam_trained (message, class) VALUES (?1, ?2)",
            params![id as i64, Class::Judged as i64],
        )?;
        moved.push(id);
    }
    Ok(moved)
}

// ---- The store's side ----

/// What the filter has learned, for Settings › Mail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Status {
    /// Messages learned from: spam and good mail.
    pub spam: u32,
    pub ham: u32,
    /// How many of each it needs before it acts.
    pub needed: u32,
}

impl MailStore {
    /// How much the spam filter has learned.
    pub fn spam_status(&self) -> Result<Status, StoreError> {
        let (spam, ham) = totals(&self.db)?;
        Ok(Status {
            spam,
            ham,
            needed: MIN_LEARNED,
        })
    }

    /// What the spam filter thinks of a stored message.
    pub fn spam_verdict(&self, id: MessageId) -> Result<Verdict, StoreError> {
        let own = own_addresses(self);
        judge(&self.db, &message_tokens(self, id, &own)?)
    }

    /// The menu's Spam (`spam` true: to each account's Spam folder) and Not spam (to its
    /// Inbox), learned from either way. All or none: mail on this PC, or an account without
    /// such a folder, stops it. Returns where each message was, for undo.
    pub fn mark_spam(
        &mut self,
        ids: &[MessageId],
        spam: bool,
    ) -> Result<Vec<(MessageId, String)>, StoreError> {
        let kind = if spam {
            FolderKind::Spam
        } else {
            FolderKind::Inbox
        };
        let mut groups: Vec<(String, Vec<MessageId>)> = Vec::new();
        for &id in ids {
            let folder = self.folder_of(id)?.ok_or(StoreError::UnknownMessage(id))?;
            let account = account_of(&folder);
            let target = (account != LOCAL_ID)
                .then(|| self.account(account).ok())
                .flatten()
                .and_then(|a| {
                    a.folders
                        .iter()
                        .find(|f| f.kind == kind)
                        .map(|f| a.folder_key(&f.id))
                })
                .ok_or_else(|| {
                    StoreError::Invalid(if spam {
                        tr!(
                            "This mail has no Spam folder to go to.",
                            "Ta poczta nie ma folderu Spam, do którego mogłaby trafić."
                        )
                    } else {
                        tr!(
                            "This mail has no Inbox to go back to.",
                            "Ta poczta nie ma Odebranych, do których mogłaby wrócić."
                        )
                    })
                })?;
            match groups.iter_mut().find(|(t, _)| *t == target) {
                Some((_, group)) => group.push(id),
                None => groups.push((target, vec![id])),
            }
        }
        let mut places = Vec::with_capacity(ids.len());
        for (target, group) in groups {
            places.extend(self.move_many(&group, &target)?);
        }
        Ok(places)
    }
}

#[cfg(test)]
mod tests;
