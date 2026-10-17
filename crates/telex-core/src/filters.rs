//! Rules, Telex's answer to The Bat!'s Sorting Office: conditions on a message, and what to
//! do when they match.
//!
//! Rules run on mail that arrives in an Inbox (never on an account's first download, so old
//! mail isn't touched) and on demand on a folder. Every message a rule touches gets a note
//! that says which rule did what. Moves and flags go to the server like the user's own
//! changes. Auto-replies only answer new mail, and follow RFC 3834 so they never answer
//! mailing lists, bulk mail, other robots or the same sender twice in a few days.
//!
//! A rule can also run on a schedule (every hour, day or week) over folders the user picks,
//! besides new mail or instead of it: "move read mail older than 30 days from the Inbox to
//! Archive". Scheduled runs happen while Telex runs (the window's timer asks [`run_scheduled`]
//! every minute), catch up once when overdue, and like Run now never answer or forward mail.

use std::cell::OnceCell;
use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDateTime, TimeDelta, Timelike};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::mime::auto_reply_blocker;
use crate::model::{
    account_of, Account, FolderKind, Message, MessageId, MessageSummary, OutgoingDraft,
};
use crate::smtp::{self, Originals};
use crate::store::{MailStore, StoreError, LOCAL_ID};
use crate::tr;

/// How long a rule waits before auto-replying to the same sender again.
pub const AUTO_REPLY_EVERY: TimeDelta = TimeDelta::days(4);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// An account id, or `None` for every account.
    #[serde(default)]
    pub account: Option<String>,
    /// Every condition must match (`true`), or any one (`false`).
    #[serde(default = "yes")]
    pub match_all: bool,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    /// Later rules don't run on a message this one matched.
    #[serde(default)]
    pub stop: bool,
    /// Runs on mail as it arrives in an Inbox (every rule from before schedules does).
    #[serde(default = "yes")]
    pub on_arrival: bool,
    /// Runs on a schedule too, or instead, over these folders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
    /// When the schedule last ran it, local time; none before its first run. The store keeps
    /// it: saving a rule leaves it as it was.
    #[serde(default)]
    pub last_run: Option<NaiveDateTime>,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            enabled: true,
            account: None,
            match_all: true,
            conditions: Vec::new(),
            actions: Vec::new(),
            stop: false,
            on_arrival: true,
            schedule: None,
            last_run: None,
        }
    }
}

fn yes() -> bool {
    true
}

/// When a rule runs by itself besides new mail, and on which folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub every: Every,
    /// Folder keys ("account/folder"): the rule's account's, or for a rule on every account,
    /// any account's and the folders on this PC.
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Every {
    Hour,
    Day,
    Week,
}

impl Every {
    pub fn period(self) -> TimeDelta {
        match self {
            Every::Hour => TimeDelta::hours(1),
            Every::Day => TimeDelta::days(1),
            Every::Week => TimeDelta::weeks(1),
        }
    }
}

/// At most this many folders on one rule's schedule.
pub const SCHEDULE_FOLDERS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub field: Field,
    pub op: Op,
    pub value: String,
    /// For Header conditions only; old rules omit this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    /// The sender's address or name.
    From,
    /// Any recipient in To or Cc, by address or name.
    To,
    Subject,
    Body,
    /// Any of the above.
    Anywhere,
    Header,
    AttachmentName,
    AttachmentType,
    /// When the message arrived (the server's INTERNALDATE, else its Date): `OlderThan` or
    /// `NewerThan` a number of days.
    Received,
    /// `LargerThan` or `SmallerThan` a size: 500k, 5m (KB without a unit), as in search.
    Size,
    /// `Is` read, unread, flagged or unflagged.
    State,
}

impl Field {
    /// The fields whose conditions compare text.
    fn is_text(self) -> bool {
        !matches!(self, Field::Received | Field::Size | Field::State)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Contains,
    NotContains,
    Is,
    StartsWith,
    EndsWith,
    Exists,
    NotExists,
    /// A regular expression, found anywhere in the text unless anchored with ^ or $.
    Matches,
    OlderThan,
    NewerThan,
    LargerThan,
    SmallerThan,
}

impl Op {
    /// Whether a condition on `field` can compare this way.
    fn fits(self, field: Field) -> bool {
        match field {
            Field::Received => matches!(self, Op::OlderThan | Op::NewerThan),
            Field::Size => matches!(self, Op::LargerThan | Op::SmallerThan),
            Field::State => self == Op::Is,
            _ => !matches!(
                self,
                Op::OlderThan | Op::NewerThan | Op::LargerThan | Op::SmallerThan
            ),
        }
    }
}

/// The longest pattern a condition takes, in characters, and how big its compiled form may
/// grow (bytes; the regex crate's own limit is 10 MB). The regex crate's engines take linear
/// time on any text, never backtracking, and these keep a pattern's memory and start-up small,
/// so one rule can't stall a check.
pub const PATTERN_LENGTH: usize = 1_000;
const PATTERN_SIZE: usize = 1 << 20;
/// How deeply groups and repetitions may nest.
const PATTERN_NESTING: u32 = 64;
/// Most days an age condition takes: a hundred years.
const MAX_DAYS: i64 = 36_500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Move to this folder ("account/folder") of the rule's account.
    Move {
        folder: String,
    },
    Copy {
        folder: String,
    },
    /// New arrivals only, once per original message and recipient list.
    Forward {
        to: String,
    },
    MarkRead,
    Flag,
    /// Move to Trash.
    Delete,
    /// Answer the sender with this text. New mail only.
    AutoReply {
        body: String,
    },
}

impl Rule {
    /// Catch mistakes before the rule is saved, in words for the rule editor.
    pub fn check(&self, accounts: &[Account]) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err(tr!("Give the rule a name.", "Nadaj regule nazwę."));
        }
        if self.conditions.is_empty() {
            return Err(tr!(
                "Add at least one condition.",
                "Dodaj co najmniej jeden warunek."
            ));
        }
        if self.conditions.iter().any(|c| {
            c.field.is_text()
                && c.value.trim().is_empty()
                && !matches!(c.op, Op::Exists | Op::NotExists)
        }) {
            return Err(tr!(
                "Every condition needs some text to look for.",
                "Każdy warunek potrzebuje szukanego tekstu."
            ));
        }
        if self.conditions.len() > 64
            || self.actions.len() > 32
            || self.conditions.iter().any(|c| c.value.len() > 4096)
        {
            return Err(tr!(
                "Keep a rule within 64 conditions, 32 actions and 4 KB per condition.",
                "Reguła może mieć do 64 warunków, 32 działań i 4 KB tekstu na warunek."
            ));
        }
        for condition in &self.conditions {
            if condition.field == Field::Header
                && !condition.header.as_deref().is_some_and(|h| {
                    !h.is_empty()
                        && h.len() <= 78
                        && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err(tr!(
                    "Enter a header name, such as List-Id or X-Priority.",
                    "Wpisz nazwę nagłówka, np. List-Id lub X-Priority."
                ));
            }
            condition.check()?;
        }
        if self.actions.is_empty() {
            return Err(tr!(
                "Add at least one action.",
                "Dodaj co najmniej jedno działanie."
            ));
        }
        let account =
            match &self.account {
                Some(id) => Some(accounts.iter().find(|a| &a.id == id).ok_or_else(|| {
                    tr!("That account no longer exists.", "Tego konta już nie ma.")
                })?),
                None => None,
            };
        let destinations = self
            .actions
            .iter()
            .filter(|a| matches!(a, Action::Move { .. } | Action::Delete))
            .count();
        if destinations > 1 {
            return Err(tr!(
                "A rule can move a message to one place only.",
                "Reguła może przenieść wiadomość tylko w jedno miejsce."
            ));
        }
        for action in &self.actions {
            match action {
                // A folder on this PC takes mail from any account; a server folder only from
                // its own.
                Action::Move { folder } | Action::Copy { folder } if is_local(folder) => {
                    let here = accounts.iter().find(|a| a.id == LOCAL_ID);
                    if !here
                        .is_some_and(|a| a.folders.iter().any(|f| &a.folder_key(&f.id) == folder))
                    {
                        return Err(tr!(
                            "Pick the folder to move to.",
                            "Wybierz folder, do którego przenieść."
                        ));
                    }
                }
                Action::Move { folder } | Action::Copy { folder } => {
                    let account = account.ok_or_else(|| {
                        tr!(
                            "To move mail, pick the rule's account.",
                            "Żeby przenosić pocztę, wybierz konto reguły."
                        )
                    })?;
                    if !account
                        .folders
                        .iter()
                        .any(|f| &account.folder_key(&f.id) == folder)
                    {
                        return Err(tr!(
                            "Pick the folder to move to.",
                            "Wybierz folder, do którego przenieść."
                        ));
                    }
                }
                Action::Forward { to } => {
                    let account = account.ok_or_else(|| {
                        tr!(
                            "To forward mail, pick the sending account.",
                            "Żeby przekazywać pocztę, wybierz konto nadawcy."
                        )
                    })?;
                    let targets = smtp::recipient_emails(to)?;
                    if targets.is_empty()
                        || targets.len() > 20
                        || targets.iter().any(|target| {
                            target.eq_ignore_ascii_case(&account.email)
                                || account
                                    .identities
                                    .iter()
                                    .any(|i| target.eq_ignore_ascii_case(&i.email))
                        })
                    {
                        return Err(tr!(
                            "Forward to 1–20 addresses other than this account’s own addresses.",
                            "Przekazuj na 1–20 adresów innych niż adresy tego konta."
                        ));
                    }
                    if account.smtp.is_none() {
                        return Err(tr!(
                            "Set up SMTP for the forwarding account first.",
                            "Najpierw ustaw SMTP dla konta przekazującego."
                        ));
                    }
                }
                Action::AutoReply { body } => {
                    if account.is_none() {
                        return Err(tr!(
                            "To answer mail, pick the account that answers.",
                            "Żeby odpowiadać na pocztę, wybierz konto, które odpowiada."
                        ));
                    }
                    if body.trim().is_empty() {
                        return Err(tr!(
                            "Write the text of the automatic reply.",
                            "Napisz treść automatycznej odpowiedzi."
                        ));
                    }
                }
                Action::MarkRead | Action::Flag | Action::Delete => {}
            }
        }
        self.check_schedule(accounts, account)
    }

    /// When the rule runs: on new mail, on a schedule over folders that exist, or both.
    fn check_schedule(
        &self,
        accounts: &[Account],
        account: Option<&Account>,
    ) -> Result<(), String> {
        let Some(schedule) = &self.schedule else {
            if self.on_arrival {
                return Ok(());
            }
            return Err(tr!(
                "Pick when the rule runs: on new mail, on a schedule, or both.",
                "Wybierz, kiedy reguła działa: na nowej poczcie, według harmonogramu albo obie."
            ));
        };
        if !self.on_arrival
            && self
                .actions
                .iter()
                .any(|a| matches!(a, Action::AutoReply { .. } | Action::Forward { .. }))
        {
            return Err(tr!(
                "Automatic replies and forwards answer new mail only: let the rule run on new mail too.",
                "Automatyczne odpowiedzi i przekazania działają tylko na nowej poczcie: niech reguła działa też na nowej poczcie."
            ));
        }
        if schedule.folders.is_empty() || schedule.folders.len() > SCHEDULE_FOLDERS {
            return Err(tr!(
                "Pick 1–20 folders for the schedule.",
                "Wybierz od 1 do 20 folderów dla harmonogramu."
            ));
        }
        for key in &schedule.folders {
            // A rule of one account runs on that account's mail only, as on new mail.
            let owner = account_of(key);
            let fits = account.is_none_or(|a| a.id == owner);
            let exists = accounts
                .iter()
                .find(|a| a.id == owner)
                .and_then(|a| a.folders.iter().find(|f| &a.folder_key(&f.id) == key))
                .is_some_and(|f| f.kind != FolderKind::Outbox);
            if !fits || !exists {
                return Err(tr!(
                    "A folder of the schedule no longer exists: pick the folders again.",
                    "Jednego z folderów harmonogramu już nie ma: wybierz foldery ponownie."
                ));
            }
        }
        Ok(())
    }

    pub fn matches(&self, m: &Message) -> bool {
        self.matches_raw(m, None)
    }
    /// With the original message, for header and attachment-type conditions. A pattern that
    /// doesn't compile matches nothing (`check` refuses it before a rule is saved).
    pub fn matches_raw(&self, m: &Message, raw: Option<&[u8]>) -> bool {
        Ready::new(self).is_ok_and(|ready| ready.matches(&Folded::with_raw(m, raw)))
    }
}

/// A rule ready to run: its patterns compiled once for every message of a run.
struct Ready<'r> {
    rule: &'r Rule,
    patterns: Vec<Option<Regex>>,
}

impl<'r> Ready<'r> {
    fn new(rule: &'r Rule) -> Result<Self, String> {
        let patterns = rule
            .conditions
            .iter()
            .map(|c| (c.op == Op::Matches).then(|| pattern(&c.value)).transpose())
            .collect::<Result<_, _>>()?;
        Ok(Self { rule, patterns })
    }

    fn matches(&self, m: &Folded) -> bool {
        let mut results = (0..self.rule.conditions.len()).map(|i| self.condition(i, m));
        if self.rule.match_all {
            results.all(|hit| hit)
        } else {
            results.any(|hit| hit)
        }
    }

    fn condition(&self, i: usize, m: &Folded) -> bool {
        self.rule.conditions[i].matches_folded(m, self.patterns[i].as_ref())
    }

    /// Whether the message's summary alone (its date, size, read and flag) rules it out: a
    /// scheduled run then never reads its text.
    fn rules_out(&self, m: &MessageSummary, now: NaiveDateTime) -> bool {
        let mut known = self.rule.conditions.iter().map(|c| c.on_summary(m, now));
        if self.rule.match_all {
            known.any(|hit| hit == Some(false))
        } else {
            known.all(|hit| hit == Some(false))
        }
    }
}

/// Compile a condition's pattern, or say what is wrong with it. Case is ignored, and the
/// pattern's letters with diacritics are folded like the text (ł to l, Ą to a): a pattern sees
/// the same lowercase text without diacritics as the other ways of comparing.
pub fn pattern(value: &str) -> Result<Regex, String> {
    let value = value.trim();
    if value.chars().count() > PATTERN_LENGTH {
        return Err(tr!(
            "Keep a pattern within 1,000 characters.",
            "Wzorzec może mieć do 1000 znaków."
        ));
    }
    // The parser on its own first: its mistakes say where they are.
    if let Err(e) = regex_syntax::ast::parse::ParserBuilder::new()
        .nest_limit(PATTERN_NESTING)
        .build()
        .parse(value)
    {
        let at = e.span().start.column;
        let what = pattern_mistake(e.kind());
        return Err(tr!(
            "The pattern “{value}” has a mistake at character {at}: {what}.",
            "Wzorzec „{value}” ma błąd przy znaku {at}: {what}."
        ));
    }
    let folded: String = value
        .chars()
        .map(|c| {
            if c.is_ascii() {
                c.to_string()
            } else {
                fold(&c.to_string())
            }
        })
        .collect();
    RegexBuilder::new(&folded)
        .case_insensitive(true)
        .size_limit(PATTERN_SIZE)
        .dfa_size_limit(PATTERN_SIZE)
        .nest_limit(PATTERN_NESTING)
        .build()
        .map_err(|e| match e {
            regex::Error::CompiledTooBig(_) => tr!(
                "The pattern “{value}” is too big to check quickly: make it simpler.",
                "Wzorzec „{value}” jest zbyt duży, by sprawdzać go szybko: uprość go."
            ),
            // Rare after the parser: a Unicode class this build doesn't know, say.
            e => {
                let detail = e.to_string();
                let detail = detail.lines().last().unwrap_or_default().trim();
                tr!(
                    "The pattern “{value}” can't be used: {detail}",
                    "Wzorca „{value}” nie da się użyć: {detail}"
                )
            }
        })
}

/// A pattern's mistake in a few words.
fn pattern_mistake(kind: &regex_syntax::ast::ErrorKind) -> String {
    use regex_syntax::ast::ErrorKind as K;
    match kind {
        K::GroupUnclosed => tr!("a ( isn't closed", "nawias ( nie jest zamknięty"),
        K::GroupUnopened => tr!("a ) wasn't opened", "nawias ) nie był otwarty"),
        K::ClassUnclosed => tr!("a [ isn't closed", "nawias [ nie jest zamknięty"),
        K::ClassRangeInvalid | K::ClassRangeLiteral => {
            tr!(
                "the range in [ ] is backwards or unclear",
                "zakres w [ ] jest odwrócony lub niejasny"
            )
        }
        K::RepetitionMissing => tr!(
            "*, + or ? has nothing before it to repeat (write \\* for the character itself)",
            "*, + lub ? nie ma przed sobą nic do powtórzenia (sam znak to \\*)"
        ),
        K::RepetitionCountInvalid | K::RepetitionCountUnclosed | K::RepetitionCountDecimalEmpty => {
            tr!("a count in {{ }} isn't right", "liczba w {{ }} jest błędna")
        }
        K::EscapeUnrecognized | K::EscapeUnexpectedEof | K::ClassEscapeInvalid => tr!(
            "this \\ escape isn't known (write \\\\ for a backslash)",
            "nieznana sekwencja z \\ (sam ukośnik to \\\\)"
        ),
        K::NestLimitExceeded(_) => tr!("it nests too deeply", "jest zbyt głęboko zagnieżdżony"),
        K::UnsupportedLookAround | K::UnsupportedBackreference => tr!(
            "look-around and back-references aren't supported",
            "odwołania wstecz i warunki (?=…) nie są obsługiwane"
        ),
        other => other.to_string(),
    }
}

impl Condition {
    /// Case and diacritics don't matter: "faktura" finds "Fakturą".
    pub fn matches(&self, m: &Message) -> bool {
        match self.op {
            // A pattern that doesn't compile matches nothing.
            Op::Matches => {
                pattern(&self.value).is_ok_and(|p| self.matches_folded(&Folded::new(m), Some(&p)))
            }
            _ => self.matches_folded(&Folded::new(m), None),
        }
    }

    /// Catch a condition whose value doesn't fit what it compares.
    fn check(&self) -> Result<(), String> {
        if !self.op.fits(self.field) {
            return Err(tr!(
                "One condition compares in a way its field doesn't take: pick how again.",
                "Jeden warunek porównuje w sposób, którego jego pole nie przyjmuje: wybierz ponownie."
            ));
        }
        match self.field {
            Field::Received if self.days().is_none() => Err(tr!(
                "Enter a number of days, such as 30.",
                "Wpisz liczbę dni, np. 30."
            )),
            Field::Size if crate::list::parse_kb(&self.value).is_none() => Err(tr!(
                "Enter a size, such as 500k or 5m.",
                "Wpisz rozmiar, np. 500k lub 5m."
            )),
            Field::State if self.state().is_none() => Err(tr!(
                "Pick read, unread, flagged or not flagged.",
                "Wybierz: przeczytana, nieprzeczytana, z flagą lub bez flagi."
            )),
            _ if self.op == Op::Matches => pattern(&self.value).map(drop),
            _ => Ok(()),
        }
    }

    fn days(&self) -> Option<i64> {
        self.value
            .trim()
            .parse()
            .ok()
            .filter(|d| (1..=MAX_DAYS).contains(d))
    }

    /// (unread, flagged): which of the two a state condition asks about, and the answer.
    fn state(&self) -> Option<(bool, bool)> {
        match self.value.trim() {
            "unread" => Some((true, true)),
            "read" => Some((true, false)),
            "flagged" => Some((false, true)),
            "unflagged" => Some((false, false)),
            _ => None,
        }
    }

    /// The answer for the conditions a summary can answer (date, size, read, flag).
    fn on_summary(&self, m: &MessageSummary, now: NaiveDateTime) -> Option<bool> {
        self.on_facts(m.date, m.size_kb, m.unread, m.flagged, now)
    }

    fn on_facts(
        &self,
        date: NaiveDateTime,
        size_kb: u32,
        unread: bool,
        flagged: bool,
        now: NaiveDateTime,
    ) -> Option<bool> {
        match self.field {
            Field::Received => {
                let since = now - TimeDelta::days(self.days()?);
                Some(match self.op {
                    Op::OlderThan => date < since,
                    _ => date >= since,
                })
            }
            Field::Size => {
                let kb = crate::list::parse_kb(&self.value)?;
                Some(match self.op {
                    Op::LargerThan => u64::from(size_kb) >= kb,
                    _ => u64::from(size_kb) < kb,
                })
            }
            Field::State => {
                let (asks_unread, yes) = self.state()?;
                Some(if asks_unread {
                    unread == yes
                } else {
                    flagged == yes
                })
            }
            _ => None,
        }
    }

    fn matches_folded(&self, m: &Folded, pattern: Option<&Regex>) -> bool {
        if !self.field.is_text() {
            let msg = m.m;
            return self
                .on_facts(msg.date, msg.size_kb, msg.unread, msg.flagged, m.now)
                .unwrap_or(false);
        }
        let needle = fold(self.value.trim());
        let values = m.texts(self.field, self.header.as_deref());
        let present = if matches!(
            self.field,
            Field::Header | Field::AttachmentName | Field::AttachmentType
        ) {
            !values.is_empty()
        } else {
            values.iter().any(|c| !c.is_empty())
        };
        if self.op == Op::Exists {
            return present;
        }
        if self.op == Op::NotExists {
            return !present;
        }
        let mut hits = values.into_iter().map(|c| match self.op {
            Op::Contains | Op::NotContains => c.contains(&needle),
            Op::Is => c == needle,
            Op::StartsWith => c.starts_with(&needle),
            Op::EndsWith => c.ends_with(&needle),
            Op::Matches => pattern.is_some_and(|p| p.is_match(c)),
            // `check` keeps these to the fields above.
            Op::OlderThan | Op::NewerThan | Op::LargerThan | Op::SmallerThan => false,
            Op::Exists | Op::NotExists => unreachable!(),
        });
        if self.op == Op::NotContains {
            !hits.any(|hit| hit)
        } else {
            hits.any(|hit| hit)
        }
    }
}

/// A message's texts as conditions compare them, folded (`fold`) the first time a condition
/// asks: a long body is folded once for all of a message's rules, not once per condition.
struct Folded<'a> {
    m: &'a Message,
    /// The clock age conditions compare with.
    now: NaiveDateTime,
    from: OnceCell<[String; 2]>,
    to: OnceCell<Vec<String>>,
    subject: OnceCell<String>,
    body: OnceCell<String>,
    raw: Option<&'a [u8]>,
    headers: OnceCell<HashMap<String, Vec<String>>>,
    attachments: OnceCell<Vec<String>>,
    attachment_types: OnceCell<Vec<String>>,
}

impl<'a> Folded<'a> {
    fn new(m: &'a Message) -> Self {
        Self::with_raw(m, None)
    }
    fn with_raw(m: &'a Message, raw: Option<&'a [u8]>) -> Self {
        Self::at(m, raw, Local::now().naive_local())
    }
    fn at(m: &'a Message, raw: Option<&'a [u8]>, now: NaiveDateTime) -> Self {
        Self {
            m,
            now,
            from: OnceCell::new(),
            to: OnceCell::new(),
            subject: OnceCell::new(),
            body: OnceCell::new(),
            raw,
            headers: OnceCell::new(),
            attachments: OnceCell::new(),
            attachment_types: OnceCell::new(),
        }
    }

    /// The texts a condition on `field` looks at. Addresses count by address and by name,
    /// so "From is anna@x.pl" works as well as "From contains Kowalska".
    fn texts(&self, field: Field, header: Option<&str>) -> Vec<&str> {
        let m = self.m;
        let from = || {
            self.from
                .get_or_init(|| [fold(&m.from_email), fold(&m.from_name)])
                .iter()
        };
        let to = || {
            self.to
                .get_or_init(|| {
                    smtp::split_addresses(&format!("{}, {}", m.to, m.cc))
                        .into_iter()
                        .flat_map(|a| match a.split_once('<') {
                            Some((name, rest)) => vec![
                                fold(rest.trim_end_matches('>').trim()),
                                fold(name.trim().trim_matches('"')),
                            ],
                            None => vec![fold(&a)],
                        })
                        .collect()
                })
                .iter()
        };
        let subject = || std::iter::once(self.subject.get_or_init(|| fold(&m.subject)));
        let body = || std::iter::once(self.body.get_or_init(|| fold(&m.body)));
        let texts: Box<dyn Iterator<Item = &String>> = match field {
            Field::From => Box::new(from()),
            Field::To => Box::new(to()),
            Field::Subject => Box::new(subject()),
            Field::Body => Box::new(body()),
            Field::Anywhere => Box::new(from().chain(to()).chain(subject()).chain(body())),
            Field::Header => {
                let headers = self.headers.get_or_init(|| {
                    let mut values: HashMap<String, Vec<String>> = HashMap::new();
                    if let Some(raw) = self.raw {
                        if let Some(parsed) =
                            mail_parser::MessageParser::default().parse_headers(raw)
                        {
                            for h in parsed.headers() {
                                let text =
                                    h.value.as_text().map(str::to_owned).unwrap_or_else(|| {
                                        String::from_utf8_lossy(
                                            raw.get(h.offset_start as usize..h.offset_end as usize)
                                                .unwrap_or_default(),
                                        )
                                        .replace("\r\n", " ")
                                        .replace('\n', " ")
                                    });
                                values
                                    .entry(h.name.as_str().to_ascii_lowercase())
                                    .or_default()
                                    .push(fold(text.trim()));
                            }
                        }
                    }
                    values
                });
                Box::new(
                    headers
                        .get(&header.unwrap_or_default().to_ascii_lowercase())
                        .into_iter()
                        .flatten(),
                )
            }
            Field::AttachmentName => Box::new(
                self.attachments
                    .get_or_init(|| m.attachments.iter().map(|a| fold(&a.name)).collect())
                    .iter(),
            ),
            Field::AttachmentType => Box::new(
                self.attachment_types
                    .get_or_init(|| {
                        self.raw
                            .and_then(|raw| mail_parser::MessageParser::default().parse(raw))
                            .map(|p| {
                                p.attachments()
                                    .map(|a| {
                                        use mail_parser::MimeHeaders;
                                        a.content_type()
                                            .map(|c| {
                                                format!(
                                                    "{}/{}",
                                                    c.ctype(),
                                                    c.subtype().unwrap_or("octet-stream")
                                                )
                                            })
                                            .unwrap_or_else(|| "application/octet-stream".into())
                                            .to_ascii_lowercase()
                                    })
                                    .collect()
                            })
                            .unwrap_or_else(|| {
                                m.attachments
                                    .iter()
                                    .map(|a| crate::mime::content_type_for(&a.name).to_owned())
                                    .collect()
                            })
                    })
                    .iter(),
            ),
            Field::Received | Field::Size | Field::State => Box::new(std::iter::empty()),
        };
        texts.map(String::as_str).collect()
    }
}

/// Lowercase without diacritics, for matching: "Fakturą Łódź" → "faktura lodz".
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ą' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
            'ć' | 'ç' | 'č' => 'c',
            'ď' => 'd',
            'ę' | 'è' | 'é' | 'ê' | 'ë' | 'ě' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ł' => 'l',
            'ń' | 'ñ' | 'ň' => 'n',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
            'ř' => 'r',
            'ś' | 'š' | 'ß' => 's',
            'ť' => 't',
            'ù' | 'ú' | 'û' | 'ü' | 'ů' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            other => other,
        })
        .collect()
}

/// Why rules are running: new mail (auto-replies allowed), or the user's "run now" on a
/// folder of existing mail (no auto-replies: old mail shouldn't be answered).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Arrival,
    OnDemand,
    /// A rule's schedule: like Run now, no auto-replies or forwards.
    Scheduled,
}

/// What a run did, for the status bar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RunReport {
    /// Messages a rule changed.
    pub matched: usize,
    pub moved: usize,
    pub replied: usize,
    pub copied: usize,
    pub forwarded: usize,
    pub problems: Vec<String>,
}

/// Where a message goes once its rules have run.
enum Destination {
    Folder(String),
    Trash,
}

/// Run the enabled rules, in order, on these messages. On new mail only the rules that run on
/// arrival; on demand every one.
pub fn run(
    store: &mut MailStore,
    ids: &[MessageId],
    trigger: Trigger,
    now: DateTime<Local>,
) -> Result<RunReport, StoreError> {
    let rules: Vec<Rule> = store
        .rules()?
        .into_iter()
        .filter(|r| r.enabled && (trigger != Trigger::Arrival || r.on_arrival))
        .collect();
    let mut report = RunReport::default();
    let ready: Vec<Ready> = rules
        .iter()
        .filter_map(|rule| match Ready::new(rule) {
            Ok(ready) => Some(ready),
            Err(e) => {
                report.problems.push(skipped_rule(&rule.name, &e));
                None
            }
        })
        .collect();
    run_ready(store, &ready, ids, trigger, now, &mut report)?;
    Ok(report)
}

/// A rule left out of a run because its pattern no longer compiles (edited by hand, or from a
/// Telex with other limits).
fn skipped_rule(name: &str, why: &str) -> String {
    tr!(
        "rule \"{name}\" was left out: {why}",
        "reguła „{name}” została pominięta: {why}"
    )
}

fn run_ready(
    store: &mut MailStore,
    rules: &[Ready],
    ids: &[MessageId],
    trigger: Trigger,
    now: DateTime<Local>,
    report: &mut RunReport,
) -> Result<(), StoreError> {
    if rules.is_empty() {
        return Ok(());
    }
    let needs_raw = rules.iter().any(|r| {
        r.rule
            .conditions
            .iter()
            .any(|c| matches!(c.field, Field::Header | Field::AttachmentType))
    });
    for &id in ids {
        let Ok(message) = store.get(id) else {
            continue; // gone meanwhile
        };
        let account = account_of(&message.folder).to_owned();
        let raw = if needs_raw { store.raw_of(id)? } else { None };
        let folded = Folded::at(&message, raw.as_deref(), now.naive_local());
        let mut notes = Vec::new();
        let mut destination = None;
        for ready in rules
            .iter()
            .filter(|r| r.rule.account.as_deref().is_none_or(|a| a == account))
        {
            if !ready.matches(&folded) {
                continue;
            }
            let rule = ready.rule;
            let name = &rule.name;
            // What the rule did, said of the message: "flagged it", "oznaczyła ją flagą".
            let mut did = Vec::new();
            for action in &rule.actions {
                match action {
                    Action::MarkRead => {
                        if message.unread {
                            store.set_unread(id, false)?;
                            did.push(tr!("marked it read", "oznaczyła ją jako przeczytaną"));
                        }
                    }
                    Action::Flag => {
                        if !message.flagged {
                            store.set_flagged(id, true)?;
                            did.push(tr!("flagged it", "oznaczyła ją flagą"));
                        }
                    }
                    Action::Move { folder } if folder != &message.folder => {
                        match store.folder_name(folder) {
                            Some(to) if account_of(folder) == account || is_local(folder) => {
                                // The Polish interface names special folders by their kind.
                                let pl_to = store
                                    .kind_of(folder)
                                    .and_then(FolderKind::pl_name)
                                    .unwrap_or(to.as_str());
                                did.push(if is_local(folder) {
                                    tr!(
                                        "moved it to {to} on this PC",
                                        "przeniosła ją do folderu {pl_to} na tym komputerze"
                                    )
                                } else {
                                    tr!("moved it to {to}", "przeniosła ją do folderu {pl_to}")
                                });
                                destination = Some(Destination::Folder(folder.clone()));
                            }
                            _ => report.problems.push(tr!(
                                "rule \"{name}\" moves mail to a folder that no longer exists",
                                "reguła „{name}” przenosi pocztę do folderu, którego już nie ma"
                            )),
                        }
                    }
                    Action::Move { .. } => {}
                    Action::Copy { folder } => match store.rule_copy(rule.id, id, folder) {
                        Ok(true) => {
                            report.copied += 1;
                            let to = store.folder_name(folder).unwrap_or_else(|| folder.clone());
                            did.push(tr!("copied it to {to}", "skopiowała ją do folderu {to}"));
                        }
                        Ok(false) => {}
                        Err(e) => report.problems.push(e.to_string()),
                    },
                    Action::Forward { to } if trigger == Trigger::Arrival => {
                        match forward(store, rule, &message, to, now) {
                            Ok(true) => {
                                report.forwarded += 1;
                                report.replied += 1;
                                did.push(tr!(
                                    "queued a forward to {to}",
                                    "dodała przekazanie do {to} do kolejki"
                                ));
                            }
                            Ok(false) => {}
                            Err(e) => report.problems.push(e),
                        }
                    }
                    Action::Forward { .. } => {}
                    Action::Delete => {
                        if store.kind_of(&message.folder) != Some(FolderKind::Trash) {
                            did.push(tr!("moved it to Trash", "przeniosła ją do Kosza"));
                            destination = Some(Destination::Trash);
                        }
                    }
                    Action::AutoReply { body } if trigger == Trigger::Arrival => {
                        let replier = Replier::Rule(rule.id);
                        match auto_reply(store, replier, &message, "", body, AUTO_REPLY_EVERY, now)
                        {
                            Ok(true) => {
                                did.push(tr!("answered it", "odpowiedziała na nią"));
                                report.replied += 1;
                            }
                            Ok(false) => {}
                            Err(e) => report.problems.push(tr!(
                                "rule \"{name}\" couldn't answer: {e}",
                                "reguła „{name}” nie mogła odpowiedzieć: {e}"
                            )),
                        }
                    }
                    Action::AutoReply { .. } => {}
                }
            }
            // Only what actually changed: running the rules again adds nothing.
            if !did.is_empty() {
                notes.push(rule_note(name, &did));
            }
            if rule.stop {
                break;
            }
        }
        if notes.is_empty() {
            continue;
        }
        report.matched += 1;
        store.add_note(id, &notes.join(" "))?;
        match destination {
            Some(Destination::Folder(key)) => {
                store.move_to(id, &key)?;
                report.moved += 1;
            }
            Some(Destination::Trash) => {
                store.delete(id)?;
                report.moved += 1;
            }
            None => {}
        }
    }
    Ok(())
}

/// What one scheduled run of a rule did, for the status bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScheduledRun {
    pub rule: i64,
    pub name: String,
    #[serde(flatten)]
    pub report: RunReport,
}

/// Whether a rule's schedule is due at `now`: it never ran, its period has passed since it
/// last did, or the clock was put back past that.
pub fn due(rule: &Rule, now: NaiveDateTime) -> bool {
    let Some(schedule) = rule.schedule.as_ref().filter(|_| rule.enabled) else {
        return false;
    };
    rule.last_run
        .is_none_or(|last| last > now || now - last >= schedule.every.period())
}

/// Run every rule whose schedule is due on the folders it names, one rule at a time, and
/// remember when. A rule overdue by many periods (Telex was closed for a week) runs once.
/// Like Run now it answers and forwards nothing; its moves and flags are queued for the
/// server like the user's own, and each message it changes gets a note.
pub fn run_scheduled(
    store: &mut MailStore,
    now: DateTime<Local>,
) -> Result<Vec<ScheduledRun>, StoreError> {
    let clock = now.naive_local();
    let clock = clock.with_nanosecond(0).unwrap_or(clock);
    let mut runs = Vec::new();
    for rule in store.rules()?.into_iter().filter(|r| due(r, clock)) {
        let mut report = RunReport::default();
        match Ready::new(&rule) {
            Err(e) => report.problems.push(skipped_rule(&rule.name, &e)),
            Ok(ready) => {
                let mut ids = Vec::new();
                for folder in rule.schedule.iter().flat_map(|s| &s.folders) {
                    if store.folder_name(folder).is_none() {
                        let name = &rule.name;
                        report.problems.push(tr!(
                            "rule \"{name}\" runs on a folder that no longer exists",
                            "reguła „{name}” działa na folderze, którego już nie ma"
                        ));
                        continue;
                    }
                    // What the list's summaries rule out (too new, unread) is never read.
                    ids.extend(
                        store
                            .list(folder)?
                            .iter()
                            .filter(|m| !ready.rules_out(m, clock))
                            .map(|m| m.id),
                    );
                }
                let ready = [ready];
                run_ready(store, &ready, &ids, Trigger::Scheduled, now, &mut report)?;
            }
        }
        store.set_rule_last_run(rule.id, clock)?;
        runs.push(ScheduledRun {
            rule: rule.id,
            name: rule.name,
            report,
        });
    }
    Ok(runs)
}

/// Who answers automatically, and so who remembers whom it answered: a rule, or an account's
/// away reply.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Replier<'a> {
    Rule(i64),
    Away(&'a str),
}

fn forward(
    store: &mut MailStore,
    rule: &Rule,
    message: &Message,
    to: &str,
    now: DateTime<Local>,
) -> Result<bool, String> {
    let account = store
        .account(rule.account.as_deref().unwrap_or_default())
        .map_err(|e| e.to_string())?
        .clone();
    if store.kind_of(&message.folder) == Some(FolderKind::Spam)
        || message.from_email.eq_ignore_ascii_case(&account.email)
        || account
            .identities
            .iter()
            .any(|i| message.from_email.eq_ignore_ascii_case(&i.email))
    {
        return Ok(false);
    }
    let raw = store
        .raw_of(message.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            tr!(
                "The original message is needed to forward it.",
                "Przekazanie wymaga oryginalnej wiadomości."
            )
        })?;
    let parsed = mail_parser::MessageParser::default().parse_headers(&raw);
    if parsed.as_ref().is_some_and(|p| {
        p.headers().iter().any(|h| {
            h.name.as_str().eq_ignore_ascii_case("auto-submitted")
                && h.value
                    .as_text()
                    .is_none_or(|v| !v.trim().eq_ignore_ascii_case("no"))
        })
    }) {
        return Ok(false);
    }
    let mut targets = smtp::recipient_emails(to)?;
    targets.iter_mut().for_each(|a| a.make_ascii_lowercase());
    targets.sort();
    targets.dedup();
    let action = format!("forward:{}", targets.join(","));
    if store
        .rule_action_done(rule.id, message.id, &action)
        .map_err(|e| e.to_string())?
    {
        return Ok(false);
    }
    let start = crate::drafts::start(
        store,
        crate::drafts::DraftKind::Forward,
        Some(message.id),
        &account.id,
        true,
    )
    .map_err(|e| e.to_string())?;
    let draft = OutgoingDraft {
        account: account.id.clone(),
        to: to.to_owned(),
        subject: start.subject,
        body: start.body,
        forward_of: Some(message.id),
        identity: Some(start.identity),
        ..OutgoingDraft::default()
    };
    let composed = smtp::compose_auto_forward(
        &account,
        &draft,
        &Originals {
            forwarded: Some(raw),
            ..Originals::default()
        },
        now,
    )?;
    store
        .queue_rule_forward(rule.id, message.id, &action, &account.id, &composed)
        .map_err(|e| e.to_string())
}

/// How many of a list's messages a rule's preview reads, from the top of the list. The
/// preview holds the mail base while it reads, so a check or a send waits for it; all mail
/// would take seconds.
pub const PREVIEW_LIMIT: usize = 2_000;

#[derive(Debug, Serialize)]
pub struct RulePreview {
    /// The messages read that the rule could act on (of its account).
    pub checked: usize,
    pub matched: usize,
    /// Messages in the list, and how many of them were read.
    pub listed: usize,
    pub read: usize,
    /// The list is longer than `PREVIEW_LIMIT`: only its first ones were read.
    pub limited: bool,
    pub samples: Vec<PreviewMessage>,
    pub actions: Vec<Action>,
    pub new_mail_only: bool,
}
#[derive(Debug, Serialize)]
pub struct PreviewMessage {
    pub id: MessageId,
    pub subject: String,
    pub from: String,
    pub conditions: Vec<bool>,
}

/// Preview an unsaved rule against current search/folder results. No writes, sends or
/// remote image loads. Reads the first `PREVIEW_LIMIT` messages of the list (`limited` says
/// when there were more); a limited sample accompanies the count of matches among them.
pub fn preview(
    store: &MailStore,
    rule: &Rule,
    view: &crate::ListView,
) -> Result<RulePreview, StoreError> {
    rule.check(store.accounts()).map_err(StoreError::Invalid)?;
    let ready = Ready::new(rule).map_err(StoreError::Invalid)?;
    let now = Local::now().naive_local();
    let mut view = view.clone();
    view.grouped = false;
    view.expanded.clear();
    view.keep.clear();
    let mut report = RulePreview {
        checked: 0,
        matched: 0,
        listed: 0,
        read: 0,
        limited: false,
        samples: Vec::new(),
        actions: rule.actions.clone(),
        new_mail_only: rule
            .actions
            .iter()
            .any(|a| matches!(a, Action::Forward { .. } | Action::AutoReply { .. })),
    };
    // The list in its order, once (a filtered list the window shows is worked out already),
    // then its first messages one by one.
    let ids = store.ids_between(&view, None, None)?;
    report.listed = ids.len();
    report.read = ids.len().min(PREVIEW_LIMIT);
    report.limited = report.listed > report.read;
    for &id in &ids[..report.read] {
        let m = store.get(id)?;
        if rule
            .account
            .as_deref()
            .is_some_and(|a| a != account_of(&m.folder))
        {
            continue;
        }
        report.checked += 1;
        let raw = if rule
            .conditions
            .iter()
            .any(|c| matches!(c.field, Field::Header | Field::AttachmentType))
        {
            store.raw_of(m.id)?
        } else {
            None
        };
        let folded = Folded::at(&m, raw.as_deref(), now);
        if ready.matches(&folded) {
            report.matched += 1;
            if report.samples.len() < 50 {
                report.samples.push(PreviewMessage {
                    id: m.id,
                    subject: m.subject.clone(),
                    from: m.from_email.clone(),
                    conditions: (0..rule.conditions.len())
                        .map(|i| ready.condition(i, &folded))
                        .collect(),
                });
            }
        }
    }
    Ok(report)
}

/// Queue an automatic answer, unless RFC 3834 or common sense says not to: not to lists, bulk
/// mail, robots, the account's own address or Spam, and to the same sender at most once in
/// `every`. `Ok(false)` means it was rightly skipped. An empty `subject` is "Re: " and the
/// original's.
pub(crate) fn auto_reply(
    store: &mut MailStore,
    replier: Replier<'_>,
    m: &Message,
    subject: &str,
    body: &str,
    every: TimeDelta,
    now: DateTime<Local>,
) -> Result<bool, String> {
    let account = store
        .account(account_of(&m.folder))
        .map_err(|e| e.to_string())?
        .clone();
    let sender = m.from_email.trim();
    if sender.is_empty()
        || sender.eq_ignore_ascii_case(&account.email)
        || looks_automated(sender)
        || store.kind_of(&m.folder) == Some(FolderKind::Spam)
    {
        return Ok(false);
    }
    let raw = store
        .raw_of(m.id)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    if auto_reply_blocker(&raw).is_some() {
        return Ok(false);
    }
    let last = match replier {
        Replier::Rule(id) => store.last_auto_reply(id, sender),
        Replier::Away(account) => store.last_away_reply(account, sender),
    }
    .map_err(|e| e.to_string())?;
    if last.is_some_and(|at| now.naive_local() - at < every) {
        return Ok(false);
    }
    let to = if m.from_name.trim().is_empty() || m.from_name == m.from_email {
        sender.to_owned()
    } else {
        format!("\"{}\" <{sender}>", m.from_name.replace('"', "'"))
    };
    let theirs = m.subject.trim();
    let draft = OutgoingDraft {
        account: account.id.clone(),
        to,
        subject: if !subject.trim().is_empty() {
            subject.trim().to_owned()
        } else if theirs.to_ascii_lowercase().starts_with("re:") {
            theirs.to_owned()
        } else {
            format!("Re: {theirs}")
        },
        body: body.to_owned(),
        reply_to: Some(m.id),
        ..OutgoingDraft::default()
    };
    let originals = Originals {
        replied: Some(raw),
        ..Originals::default()
    };
    let composed = smtp::compose_auto_reply(&account, &draft, &originals, now)?;
    store
        .queue_outgoing(
            &account.id,
            &composed.message,
            &composed.raw,
            &composed.env_from,
            &composed.env_to,
            None,
            None,
        )
        .map_err(|e| e.to_string())?;
    match replier {
        Replier::Rule(id) => store.record_auto_reply(id, sender, now.naive_local()),
        Replier::Away(account) => store.record_away_reply(account, sender, now.naive_local()),
    }
    .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Addresses that are robots, mailing lists or bounces by name.
fn looks_automated(address: &str) -> bool {
    let local = address
        .split('@')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let local = local.split('+').next().unwrap_or_default();
    [
        "mailer-daemon",
        "postmaster",
        "noreply",
        "no-reply",
        "no_reply",
        "donotreply",
        "do-not-reply",
        "do_not_reply",
        "bounce",
        "bounces",
        "listserv",
        "majordomo",
    ]
    .contains(&local)
        || local.starts_with("owner-")
        || local.ends_with("-request")
        || local.starts_with("bounce")
}

/// `Rule "Faktury" moved it to Faktury and flagged it.`, from what the rule did.
fn rule_note(name: &str, did: &[String]) -> String {
    let did = join_and(did);
    tr!("Rule \"{name}\" {did}.", "Reguła „{name}” {did}.")
}

/// "a" · "a and b" · "a, b and c"; in Polish "a, b i c". Neither puts a comma before the
/// last one: the English notes never had one, and Polish never does before "i".
fn join_and(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => {
            let rest = rest.join(", ");
            tr!("{rest} and {last}", "{rest} i {last}")
        }
    }
}

/// A folder on this PC, which takes mail from every account.
fn is_local(folder_key: &str) -> bool {
    account_of(folder_key) == LOCAL_ID
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSetup, Folder, SmtpSecurity};
    use crate::sample::sample_store;
    use crate::store::SyncedMessage;
    use chrono::TimeZone;

    fn when(day: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, day, 9, 0, 0).unwrap()
    }

    fn cond(field: Field, op: Op, value: &str) -> Condition {
        Condition {
            header: None,
            field,
            op,
            value: value.into(),
        }
    }

    fn rule(
        name: &str,
        account: Option<&str>,
        conditions: Vec<Condition>,
        actions: Vec<Action>,
    ) -> Rule {
        Rule {
            id: 0,
            name: name.into(),
            enabled: true,
            account: account.map(Into::into),
            match_all: true,
            conditions,
            actions,
            stop: false,
            ..Rule::default()
        }
    }

    fn message(from: (&str, &str), to: &str, subject: &str, body: &str) -> Message {
        Message {
            id: 0,
            folder: "firma/inbox".into(),
            from_name: from.0.into(),
            from_email: from.1.into(),
            to: to.into(),
            cc: String::new(),
            subject: subject.into(),
            date: when(10).naive_local(),
            size_kb: 1,
            unread: true,
            flagged: false,
            remote_images: false,
            note: None,
            attachments: vec![],
            body: body.into(),
            message_id: None,
            bcc: String::new(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn conditions_ignore_case_and_diacritics() {
        let m = message(
            ("Paweł Mazur", "pawel@mazur-ksiegowosc.pl"),
            "\"Studio, biuro\" <biuro@twojastudio.pl>, anna@x.pl",
            "Faktura za październik",
            "W załączniku fakturą.",
        );
        let hit = |c: Condition| c.matches(&m);
        assert!(hit(cond(
            Field::Subject,
            Op::Contains,
            "FAKTURA ZA PAZDZIERNIK"
        )));
        assert!(hit(cond(Field::From, Op::Is, "pawel@mazur-ksiegowosc.pl")));
        assert!(hit(cond(Field::From, Op::Contains, "mazur")));
        assert!(hit(cond(Field::From, Op::EndsWith, "@mazur-ksiegowosc.pl")));
        assert!(hit(cond(Field::To, Op::Is, "anna@x.pl")));
        assert!(
            hit(cond(Field::To, Op::Is, "Studio, biuro")),
            "names count too"
        );
        assert!(hit(cond(Field::Body, Op::Contains, "faktura")));
        assert!(!hit(cond(Field::Anywhere, Op::Contains, "lodz")));
        assert!(hit(cond(Field::Subject, Op::NotContains, "oferta")));
        assert!(!hit(cond(Field::Subject, Op::NotContains, "faktura")));
        assert!(hit(cond(Field::Subject, Op::StartsWith, "faktura")));
    }

    #[test]
    fn all_or_any() {
        let m = message(("A", "a@x.pl"), "b@y.pl", "Oferta", "");
        let mut r = rule(
            "r",
            None,
            vec![
                cond(Field::From, Op::Is, "a@x.pl"),
                cond(Field::Subject, Op::Is, "nope"),
            ],
            vec![Action::Flag],
        );
        assert!(!r.matches(&m));
        r.match_all = false;
        assert!(r.matches(&m));
    }

    fn accounts() -> Vec<Account> {
        sample_store().accounts().to_vec()
    }

    #[test]
    fn rules_are_checked_before_saving() {
        let ok = rule(
            "Faktury",
            Some("firma"),
            vec![cond(Field::Subject, Op::Contains, "faktura")],
            vec![
                Action::Move {
                    folder: "firma/faktury".into(),
                },
                Action::Flag,
            ],
        );
        assert_eq!(ok.check(&accounts()), Ok(()));
        let mut bad = ok.clone();
        bad.account = None;
        assert!(bad
            .check(&accounts())
            .unwrap_err()
            .contains("pick the rule's account"));
        let mut bad = ok.clone();
        bad.actions = vec![Action::Move {
            folder: "gmail/inbox".into(),
        }];
        assert!(bad.check(&accounts()).unwrap_err().contains("folder"));
        let mut bad = ok.clone();
        bad.actions.push(Action::Delete);
        assert!(bad.check(&accounts()).unwrap_err().contains("one place"));
        let mut bad = ok;
        bad.conditions[0].value = "  ".into();
        assert!(bad.check(&accounts()).is_err());
    }

    #[test]
    fn matching_mail_is_sorted_and_says_why() {
        let mut store = sample_store();
        let faktury = store
            .save_rule(&rule(
                "Faktury",
                Some("firma"),
                vec![cond(Field::Subject, Op::Contains, "dokumenty")],
                vec![
                    Action::Move {
                        folder: "firma/faktury".into(),
                    },
                    Action::Flag,
                    Action::MarkRead,
                ],
            ))
            .unwrap();
        assert!(faktury.id > 0);
        let target = store
            .list("firma/inbox")
            .unwrap()
            .into_iter()
            .find(|m| m.subject.contains("Dokumenty"))
            .unwrap();
        let others = store.list("firma/inbox").unwrap().len() - 1;
        let ids: Vec<_> = store
            .list("firma/inbox")
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();

        let report = run(&mut store, &ids, Trigger::OnDemand, when(10)).unwrap();
        assert_eq!((report.matched, report.moved), (1, 1));
        let m = store.get(target.id).unwrap();
        assert_eq!(m.folder, "firma/faktury");
        assert!(m.flagged && !m.unread);
        assert_eq!(
            m.note.as_deref(),
            Some("Rule \"Faktury\" moved it to Faktury and flagged it.")
        );
        assert_eq!(store.list("firma/inbox").unwrap().len(), others);

        // Running again changes nothing and doesn't repeat the note.
        let again = run(&mut store, &[target.id], Trigger::OnDemand, when(10)).unwrap();
        assert_eq!(again.moved, 0);
        assert_eq!(
            store.get(target.id).unwrap().note.as_deref(),
            Some("Rule \"Faktury\" moved it to Faktury and flagged it.")
        );
    }

    #[test]
    fn notes_and_checks_speak_polish_when_asked() {
        use crate::i18n::{self, Lang};
        let mut store = sample_store();
        let m = store.list("firma/inbox").unwrap()[0].clone();
        store.set_unread(m.id, true).unwrap();
        store.set_flagged(m.id, false).unwrap();
        let archive = rule(
            "Porządki",
            Some("firma"),
            vec![cond(Field::Subject, Op::Is, &m.subject)],
            vec![
                Action::Move {
                    folder: "firma/archive".into(),
                },
                Action::Flag,
                Action::MarkRead,
            ],
        );
        store.save_rule(&archive).unwrap();
        let report = i18n::with(Lang::Pl, || {
            run(&mut store, &[m.id], Trigger::OnDemand, when(10)).unwrap()
        });
        assert_eq!(report.moved, 1);
        assert_eq!(
            store.get(m.id).unwrap().note.as_deref(),
            Some(
                "Reguła „Porządki” przeniosła ją do folderu Archiwum, oznaczyła ją flagą \
                 i oznaczyła ją jako przeczytaną."
            ),
            "the Archive folder by its Polish name, no comma before \"i\""
        );

        let parts = ["a", "b", "c"].map(String::from);
        assert_eq!(join_and(&parts), "a, b and c");
        assert_eq!(i18n::with(Lang::Pl, || join_and(&parts[..2])), "a i b");

        let mut unnamed = archive;
        unnamed.account = None;
        assert_eq!(
            i18n::with(Lang::Pl, || unnamed.check(&accounts()).unwrap_err()),
            "Żeby przenosić pocztę, wybierz konto reguły."
        );
    }

    #[test]
    fn rules_run_in_order_and_stop_stops() {
        let mut store = sample_store();
        let mut first = rule(
            "Anna",
            None,
            vec![cond(Field::From, Op::Contains, "kowalska")],
            vec![Action::MarkRead],
        );
        first.stop = true;
        store.save_rule(&first).unwrap();
        store
            .save_rule(&rule(
                "Wszystko",
                Some("firma"),
                vec![cond(Field::Anywhere, Op::Contains, "a")],
                vec![Action::Move {
                    folder: "firma/archive".into(),
                }],
            ))
            .unwrap();
        let anna = store.list("firma/inbox").unwrap()[0].clone();
        assert_eq!(anna.from_name, "Anna Kowalska");
        run(&mut store, &[anna.id], Trigger::OnDemand, when(10)).unwrap();
        let m = store.get(anna.id).unwrap();
        assert!(!m.unread);
        assert_eq!(m.folder, "firma/inbox", "the second rule didn't run");
        assert_eq!(m.note.as_deref(), Some("Rule \"Anna\" marked it read."));

        // Moving the second rule up changes the order.
        let rules = store.rules().unwrap();
        store.move_rule(rules[1].id, -1).unwrap();
        assert_eq!(store.rules().unwrap()[0].name, "Wszystko");
    }

    #[test]
    fn rules_for_one_account_leave_the_others_alone() {
        let mut store = sample_store();
        store
            .save_rule(&rule(
                "Gmail only",
                Some("gmail"),
                vec![cond(Field::Anywhere, Op::Contains, "e")],
                vec![Action::Flag],
            ))
            .unwrap();
        let firma: Vec<_> = store
            .list("firma/inbox")
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(
            run(&mut store, &firma, Trigger::OnDemand, when(10))
                .unwrap()
                .matched,
            0
        );
    }

    #[test]
    fn delete_moves_to_trash_but_never_deletes_for_good() {
        let mut store = sample_store();
        store
            .save_rule(&rule(
                "Spam words",
                Some("firma"),
                vec![cond(Field::Subject, Op::Contains, "google")],
                vec![Action::Delete],
            ))
            .unwrap();
        let spam: Vec<_> = store
            .list("firma/spam")
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        let target = spam[0];
        store.move_to(target, "firma/trash").unwrap();
        run(&mut store, &[target], Trigger::OnDemand, when(10)).unwrap();
        assert!(store.get(target).is_ok(), "already in Trash: left alone");
    }

    // ---- Auto-reply ----

    /// A server account with Inbox, Sent and Trash, one auto-reply rule, and a way to
    /// deliver a message into the Inbox.
    fn holiday_store() -> (MailStore, Account, Rule) {
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
                    folder("sent", FolderKind::Sent),
                    folder("spam", FolderKind::Spam),
                ],
            )
            .unwrap();
        let account = store.account(&account.id).unwrap().clone();
        let rule = store
            .save_rule(&rule(
                "Urlop",
                Some(&account.id),
                vec![cond(Field::To, Op::Contains, "biuro@")],
                vec![Action::AutoReply {
                    body: "Dzień dobry, wracam 20 października.".into(),
                }],
            ))
            .unwrap();
        (store, account, rule)
    }

    fn deliver(store: &mut MailStore, account: &Account, uid: u32, headers: &str) -> MessageId {
        let raw = format!(
            "Message-ID: <m{uid}@x.pl>\r\n{headers}To: biuro@twojastudio.pl\r\nSubject: Pytanie\r\n\r\nHej\r\n"
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

    fn outbox(store: &MailStore, account: &Account) -> usize {
        store.list(&account.folder_key("outbox")).unwrap().len()
    }

    #[test]
    fn headers_attachments_and_presence_use_original_mime() {
        let raw = b"From: a@x.pl\r\nX-Project: first\r\nX-Project: second\r\nX-Empty:\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nHi\r\n--x\r\nContent-Type: application/pdf\r\nContent-Disposition: attachment; filename=invoice.PDF\r\n\r\n%PDF\r\n--x--\r\n";
        let m = crate::mime::parse_message(raw, "firma/inbox", None);
        let mut c = cond(Field::Header, Op::Contains, "second");
        c.header = Some("x-project".into());
        let mut r = rule("headers", Some("firma"), vec![c], vec![Action::Flag]);
        assert!(r.matches_raw(&m, Some(raw)));
        assert!(
            !r.matches(&m),
            "Headers cannot match without the original message"
        );
        r.conditions[0].op = Op::Exists;
        r.conditions[0].header = Some("X-Empty".into());
        assert!(r.matches_raw(&m, Some(raw)), "An empty header still exists");
        r.conditions = vec![
            cond(Field::AttachmentName, Op::EndsWith, ".pdf"),
            cond(Field::AttachmentType, Op::Is, "application/pdf"),
        ];
        assert!(r.matches_raw(&m, Some(raw)));
        r.conditions = vec![cond(Field::AttachmentName, Op::NotExists, "")];
        assert!(!r.matches(&m));
        r.conditions[0].op = Op::Exists;
        assert!(r.matches(&m));
        r.conditions[0].field = Field::Header;
        r.conditions[0].header = Some("X-Evil\r\nBcc".into());
        assert!(r.check(&accounts()).is_err());
    }

    #[test]
    fn preview_reads_a_limited_part_of_long_lists_and_says_so() {
        let (mut store, account, _) = holiday_store();
        let inbox = account.folder_key("inbox");
        let fill = |store: &mut MailStore, from: u32, to: u32| {
            let batch: Vec<SyncedMessage> = (from..to)
                .map(|uid| {
                    let sender = if uid % 2 == 0 { "anna@x.pl" } else { "jan@x.pl" };
                    let raw = format!(
                        "Message-ID: <m{uid}@x.pl>\r\nFrom: {sender}\r\nTo: biuro@twojastudio.pl\r\nSubject: {uid}\r\n\r\nHej\r\n"
                    );
                    let mut message = crate::mime::parse_message(raw.as_bytes(), &inbox, None);
                    message.folder = inbox.clone();
                    SyncedMessage {
                        uid,
                        message,
                        raw: raw.into_bytes(),
                    }
                })
                .collect();
            store.add_synced(&inbox, &batch).unwrap();
        };
        let r = rule(
            "Anna",
            None,
            vec![cond(Field::From, Op::Is, "anna@x.pl")],
            vec![Action::Flag],
        );
        let view = crate::ListView {
            folder: Some(inbox.clone()),
            ..Default::default()
        };
        // A short list is read whole, each message once.
        fill(&mut store, 1, 251);
        let report = preview(&store, &r, &view).unwrap();
        assert_eq!(
            (
                report.checked,
                report.matched,
                report.listed,
                report.limited
            ),
            (250, 125, 250, false)
        );
        // A long one: the first PREVIEW_LIMIT, and the report says there were more.
        fill(&mut store, 251, PREVIEW_LIMIT as u32 + 301);
        let report = preview(&store, &r, &view).unwrap();
        assert_eq!(
            (
                report.checked,
                report.matched,
                report.listed,
                report.limited
            ),
            (PREVIEW_LIMIT, PREVIEW_LIMIT / 2, PREVIEW_LIMIT + 300, true)
        );
        assert_eq!(report.samples.len(), 50);
    }

    #[test]
    fn preview_leaves_flags_folders_notes_rules_and_outbox_untouched() {
        let (mut store, account, r) = holiday_store();
        let id = deliver(&mut store, &account, 1, "From: Anna <anna@x.pl>\r\n");
        let before = store.get(id).unwrap();
        let rules_before = store.rules().unwrap();
        let mut r = r;
        r.actions = vec![
            Action::Flag,
            Action::Copy {
                folder: "local/archive".into(),
            },
            Action::Forward {
                to: "other@x.pl".into(),
            },
        ];
        let report = preview(
            &store,
            &r,
            &crate::ListView {
                folder: Some(account.folder_key("inbox")),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.matched, 1);
        assert_eq!(report.samples[0].conditions, [true]);
        assert!(report.new_mail_only);
        assert_eq!(store.get(id).unwrap(), before);
        assert_eq!(store.rules().unwrap(), rules_before);
        assert_eq!(outbox(&store, &account), 0);
        assert!(store.list("local/archive").unwrap().is_empty());
    }

    #[test]
    fn copies_preserve_original_and_queue_a_server_upload_only_once() {
        let (mut store, account, old) = holiday_store();
        store.delete_rule(old.id).unwrap();
        let id = deliver(&mut store, &account, 1, "From: Anna <anna@x.pl>\r\n");
        let r = rule(
            "Copy",
            Some(&account.id),
            vec![cond(Field::To, Op::Contains, "biuro@")],
            vec![
                Action::Copy {
                    folder: "local/archive".into(),
                },
                Action::Copy {
                    folder: account.folder_key("sent"),
                },
            ],
        );
        store.save_rule(&r).unwrap();
        let report = run(&mut store, &[id], Trigger::OnDemand, when(10)).unwrap();
        assert_eq!(report.copied, 2);
        assert_eq!(store.get(id).unwrap().folder, account.folder_key("inbox"));
        let copies = store.list("local/archive").unwrap();
        assert_eq!(copies.len(), 1);
        assert_eq!(
            store.raw_of(copies[0].id).unwrap(),
            store.raw_of(id).unwrap()
        );
        assert!(store
            .pending_changes(&account.id)
            .unwrap()
            .iter()
            .any(|c| matches!(c.kind, crate::store::ChangeKind::Append { .. })));
        assert_eq!(
            run(&mut store, &[id], Trigger::OnDemand, when(10))
                .unwrap()
                .copied,
            0
        );
        assert_eq!(store.list("local/archive").unwrap().len(), 1);
    }

    #[test]
    fn forwards_are_once_only_marked_automatic_and_never_replay_old_mail() {
        let (mut store, account, old) = holiday_store();
        store.delete_rule(old.id).unwrap();
        let id = deliver(&mut store, &account, 1, "From: Anna <anna@x.pl>\r\n");
        let mut r = rule(
            "Forward",
            Some(&account.id),
            vec![cond(Field::To, Op::Contains, "biuro@")],
            vec![Action::Forward {
                to: "archive@x.pl".into(),
            }],
        );
        store.save_rule(&r).unwrap();
        assert_eq!(
            run(&mut store, &[id], Trigger::OnDemand, when(10))
                .unwrap()
                .forwarded,
            0
        );
        assert_eq!(
            run(&mut store, &[id], Trigger::Arrival, when(10))
                .unwrap()
                .forwarded,
            1
        );
        assert_eq!(outbox(&store, &account), 1);
        let sent = store.list(&account.folder_key("outbox")).unwrap();
        let raw = String::from_utf8(store.raw_of(sent[0].id).unwrap().unwrap()).unwrap();
        assert!(raw.contains("Auto-Submitted: auto-generated"));
        assert_eq!(
            run(&mut store, &[id], Trigger::Arrival, when(10))
                .unwrap()
                .forwarded,
            0
        );
        let robot = deliver(
            &mut store,
            &account,
            2,
            "From: bot@x.pl\r\nAuto-Submitted: auto-generated\r\n",
        );
        assert_eq!(
            run(&mut store, &[robot], Trigger::Arrival, when(10))
                .unwrap()
                .forwarded,
            0
        );
        r.actions = vec![Action::Forward {
            to: account.email.clone(),
        }];
        assert!(r.check(store.accounts()).is_err());
        r.actions = vec![Action::Forward {
            to: "victim@x.pl\r\nBcc: spy@x.pl".into(),
        }];
        assert!(r.check(store.accounts()).is_err());
    }

    #[test]
    fn auto_replies_answer_people_once_in_a_while() {
        let (mut store, account, _) = holiday_store();
        let id = deliver(
            &mut store,
            &account,
            1,
            "From: Anna Kowalska <anna@x.pl>\r\n",
        );
        let report = run(&mut store, &[id], Trigger::Arrival, when(10)).unwrap();
        assert_eq!(report.replied, 1, "{report:?}");
        assert_eq!(outbox(&store, &account), 1);
        let reply = store.list(&account.folder_key("outbox")).unwrap().remove(0);
        assert_eq!(reply.subject, "Re: Pytanie");
        assert!(reply.to.contains("anna@x.pl"));
        let raw = String::from_utf8(store.raw_of(reply.id).unwrap().unwrap()).unwrap();
        assert!(raw.contains("Auto-Submitted: auto-replied"), "{raw}");
        assert!(raw.contains("In-Reply-To: <m1@x.pl>"));
        assert_eq!(
            store.get(id).unwrap().note.as_deref(),
            Some("Rule \"Urlop\" answered it.")
        );

        // The same sender, two days later: no second reply. Six days later: yes.
        let again = deliver(&mut store, &account, 2, "From: anna@x.pl\r\n");
        run(&mut store, &[again], Trigger::Arrival, when(12)).unwrap();
        assert_eq!(outbox(&store, &account), 1);
        let later = deliver(&mut store, &account, 3, "From: anna@x.pl\r\n");
        run(&mut store, &[later], Trigger::Arrival, when(16)).unwrap();
        assert_eq!(outbox(&store, &account), 2);
    }

    #[test]
    fn auto_replies_skip_robots_lists_and_old_mail() {
        let (mut store, account, _) = holiday_store();
        let cases = [
            "From: newsletter@sklep.pl\r\nList-Unsubscribe: <mailto:u@sklep.pl>\r\n",
            "From: news@sklep.pl\r\nPrecedence: bulk\r\n",
            "From: anna@x.pl\r\nAuto-Submitted: auto-replied\r\n",
            "From: noreply@bank.pl\r\n",
            "From: MAILER-DAEMON@mail.x.pl\r\n",
            "From: biuro@twojastudio.pl\r\n",
        ];
        for (i, headers) in cases.iter().enumerate() {
            let id = deliver(&mut store, &account, 10 + i as u32, headers);
            let report = run(&mut store, &[id], Trigger::Arrival, when(10)).unwrap();
            assert_eq!(report.replied, 0, "answered: {headers}");
        }
        // A real person, but the user ran the rules on old mail: no answer either.
        let id = deliver(&mut store, &account, 30, "From: piotr@y.pl\r\n");
        run(&mut store, &[id], Trigger::OnDemand, when(10)).unwrap();
        assert_eq!(outbox(&store, &account), 0);
        // In Spam: no answer.
        let id = deliver(&mut store, &account, 31, "From: piotr@y.pl\r\n");
        store.move_to(id, &account.folder_key("spam")).unwrap();
        run(&mut store, &[id], Trigger::Arrival, when(10)).unwrap();
        assert_eq!(outbox(&store, &account), 0);
    }

    #[test]
    fn rules_file_mail_from_any_account_on_this_pc() {
        let (mut store, account, holiday) = holiday_store();
        store.delete_rule(holiday.id).unwrap();
        let archive = rule(
            "Newslettery",
            None,
            vec![cond(Field::Subject, Op::Contains, "pytanie")],
            vec![Action::Move {
                folder: "local/archive".into(),
            }],
        );
        assert_eq!(
            archive.check(store.accounts()),
            Ok(()),
            "no account needed for this PC"
        );
        let mut gone = archive.clone();
        gone.actions = vec![Action::Move {
            folder: "local/nowhere".into(),
        }];
        assert!(gone.check(store.accounts()).is_err());
        store.save_rule(&archive).unwrap();
        let id = deliver(&mut store, &account, 1, "From: Anna <anna@x.pl>\r\n");
        let report = run(&mut store, &[id], Trigger::Arrival, when(10)).unwrap();
        assert_eq!((report.matched, report.moved), (1, 1));
        let m = store.get(id).unwrap();
        assert_eq!(m.folder, "local/archive");
        assert_eq!(
            m.note.as_deref(),
            Some("Rule \"Newslettery\" moved it to Archive on this PC.")
        );
        assert_eq!(
            store.pending_changes(&account.id).unwrap()[0].kind,
            crate::store::ChangeKind::Expunge,
            "it leaves the server in the same sync"
        );
    }

    // ---- Patterns, age and state, schedules ----

    #[test]
    fn patterns_see_the_folded_text_and_ignore_case() {
        let m = message(
            ("Paweł Mazur", "pawel@mazur-ksiegowosc.pl"),
            "biuro@twojastudio.pl",
            "Re: Faktura FV/2026/0137 za październik",
            "Kwota: 1 230,00 zł",
        );
        let hit = |field, p: &str| cond(field, Op::Matches, p).matches(&m);
        assert!(hit(Field::Subject, r"FV/\d{4}/\d+"));
        assert!(hit(Field::Subject, "^re:"), "case ignored, anchored");
        assert!(!hit(Field::Subject, "^faktura"), "anchors hold");
        assert!(
            hit(Field::Subject, "październik"),
            "the pattern is folded too"
        );
        assert!(hit(Field::Subject, "pazdziernik"));
        assert!(hit(Field::Subject, "PAŹDZIERNIK|listopad"));
        assert!(hit(Field::From, r"^pawe[lł]\b"), "names count");
        assert!(hit(Field::From, r"@mazur-[a-z]+\.pl$"), "and addresses");
        assert!(hit(Field::Body, r"\d+,\d{2} zl"));
        assert!(hit(Field::Anywhere, "twojastudio"));
        assert!(!hit(Field::Body, "euro"));
    }

    #[test]
    fn bad_and_huge_patterns_are_refused_with_where_and_why() {
        let with = |p: &str| {
            rule(
                "p",
                None,
                vec![cond(Field::Subject, Op::Matches, p)],
                vec![Action::Flag],
            )
            .check(&accounts())
        };
        assert_eq!(with(r"faktur(a|y)"), Ok(()));
        assert_eq!(
            with("(faktura").unwrap_err(),
            "The pattern “(faktura” has a mistake at character 1: a ( isn't closed."
        );
        assert!(with("faktura)").unwrap_err().contains("a ) wasn't opened"));
        assert!(with("[a-").unwrap_err().contains("a [ isn't closed"));
        assert!(with("*faktura").unwrap_err().contains("nothing before it"));
        assert!(with(r"(?=x)").unwrap_err().contains("look-around"));
        assert!(with(&"a".repeat(PATTERN_LENGTH + 1))
            .unwrap_err()
            .contains("1,000 characters"));
        assert!(with(r"(\w{100}){100}").unwrap_err().contains("too big"));
        assert!(with(&format!("{}x{}", "(".repeat(80), ")".repeat(80)))
            .unwrap_err()
            .contains("nests too deeply"));
        assert_eq!(
            crate::i18n::with(crate::i18n::Lang::Pl, || with("(faktura").unwrap_err()),
            "Wzorzec „(faktura” ma błąd przy znaku 1: nawias ( nie jest zamknięty."
        );
        // A pattern stored wrong (by hand) leaves its rule out of a run, saying so.
        let mut store = sample_store();
        let mut broken = rule(
            "Zepsuta",
            None,
            vec![cond(Field::Subject, Op::Contains, "x")],
            vec![Action::Flag],
        );
        broken = store.save_rule(&broken).unwrap();
        broken.conditions[0].op = Op::Matches;
        broken.conditions[0].value = "(x".into();
        assert!(store.save_rule(&broken).is_err());
        store.force_rule_conditions(broken.id, &broken.conditions);
        let ids: Vec<_> = store
            .list("firma/inbox")
            .unwrap()
            .iter()
            .map(|m| m.id)
            .collect();
        let report = run(&mut store, &ids, Trigger::OnDemand, when(10)).unwrap();
        assert_eq!(report.matched, 0);
        assert!(report.problems[0].contains("\"Zepsuta\" was left out"));
    }

    #[test]
    fn age_size_and_state_conditions_use_the_clock_given() {
        let mut m = message(("A", "a@x.pl"), "b@y.pl", "Oferta", "");
        m.date = when(1).naive_local();
        m.size_kb = 600;
        m.unread = false;
        let at = |c: Condition, day: u32| {
            Ready::new(&rule("r", None, vec![c], vec![Action::Flag]))
                .unwrap()
                .matches(&Folded::at(&m, None, when(day).naive_local()))
        };
        let older = cond(Field::Received, Op::OlderThan, "7");
        assert!(!at(older.clone(), 8), "exactly 7 days is not older");
        assert!(at(older.clone(), 9));
        assert!(at(cond(Field::Received, Op::NewerThan, "7"), 8));
        assert!(!at(cond(Field::Received, Op::NewerThan, "7"), 9));
        assert!(at(cond(Field::Size, Op::LargerThan, "500k"), 1));
        assert!(!at(cond(Field::Size, Op::LargerThan, "1m"), 1));
        assert!(at(cond(Field::Size, Op::SmallerThan, "1m"), 1));
        assert!(at(cond(Field::State, Op::Is, "read"), 1));
        assert!(!at(cond(Field::State, Op::Is, "unread"), 1));
        assert!(at(cond(Field::State, Op::Is, "unflagged"), 1));
        assert!(!at(cond(Field::State, Op::Is, "flagged"), 1));

        let check = |c: Condition| rule("r", None, vec![c], vec![Action::Flag]).check(&accounts());
        assert_eq!(check(older), Ok(()));
        assert!(check(cond(Field::Received, Op::OlderThan, "dużo"))
            .unwrap_err()
            .contains("number of days"));
        assert!(check(cond(Field::Received, Op::OlderThan, "0")).is_err());
        assert!(check(cond(Field::Size, Op::LargerThan, "huge"))
            .unwrap_err()
            .contains("500k"));
        assert!(check(cond(Field::State, Op::Is, "maybe"))
            .unwrap_err()
            .contains("not flagged"));
        assert!(check(cond(Field::Received, Op::Contains, "7"))
            .unwrap_err()
            .contains("doesn't take"));
        assert!(check(cond(Field::Subject, Op::OlderThan, "7")).is_err());
    }

    #[test]
    fn schedules_are_due_once_their_period_has_passed() {
        let at = |day: u32, hour: u32| {
            Local
                .with_ymd_and_hms(2026, 10, day, hour, 0, 0)
                .unwrap()
                .naive_local()
        };
        let mut r = rule(
            "r",
            None,
            vec![cond(Field::Subject, Op::Contains, "x")],
            vec![Action::Flag],
        );
        assert!(!due(&r, at(10, 9)), "no schedule");
        r.schedule = Some(Schedule {
            every: Every::Hour,
            folders: vec!["firma/inbox".into()],
        });
        assert!(due(&r, at(10, 9)), "never ran");
        r.last_run = Some(at(10, 9));
        assert!(!due(&r, at(10, 9) + TimeDelta::minutes(59)));
        assert!(due(&r, at(10, 10)));
        r.schedule.as_mut().unwrap().every = Every::Day;
        assert!(!due(&r, at(10, 23)));
        assert!(due(&r, at(11, 9)));
        assert!(due(&r, at(30, 9)), "overdue for weeks: due (once)");
        r.schedule.as_mut().unwrap().every = Every::Week;
        assert!(!due(&r, at(16, 9)));
        assert!(due(&r, at(17, 9)));
        assert!(due(&r, at(9, 9)), "the clock went back");
        r.enabled = false;
        assert!(!due(&r, at(17, 9)));
    }

    #[test]
    fn schedules_are_checked_before_saving() {
        let (store, account, _) = holiday_store();
        let mut r = rule(
            "Stare",
            Some(&account.id),
            vec![cond(Field::Received, Op::OlderThan, "30")],
            vec![Action::Flag],
        );
        r.on_arrival = false;
        assert!(r
            .check(store.accounts())
            .unwrap_err()
            .contains("Pick when the rule runs"));
        r.schedule = Some(Schedule {
            every: Every::Day,
            folders: vec![],
        });
        assert!(r.check(store.accounts()).unwrap_err().contains("1–20"));
        r.schedule.as_mut().unwrap().folders = vec!["firma/inbox".into()];
        assert!(
            r.check(store.accounts())
                .unwrap_err()
                .contains("no longer exists"),
            "another account's folder"
        );
        r.schedule.as_mut().unwrap().folders = vec![account.folder_key("outbox")];
        assert!(r.check(store.accounts()).is_err(), "never the Outbox");
        r.schedule.as_mut().unwrap().folders = vec![account.folder_key("inbox")];
        assert_eq!(r.check(store.accounts()), Ok(()));
        r.actions.push(Action::AutoReply { body: "Hej".into() });
        assert!(r
            .check(store.accounts())
            .unwrap_err()
            .contains("new mail only"));
        r.on_arrival = true;
        assert_eq!(
            r.check(store.accounts()),
            Ok(()),
            "both: replies on arrival"
        );
    }

    /// The example from the roadmap: "move read mail older than 30 days from Inbox to Archive",
    /// every day, never on new mail.
    #[test]
    fn a_daily_rule_files_old_read_mail_and_remembers_when() {
        let (mut store, account, holiday) = holiday_store();
        store.delete_rule(holiday.id).unwrap();
        let mut folders = store.account(&account.id).unwrap().folders.clone();
        folders.push(Folder {
            id: "archive".into(),
            name: "Archiwum".into(),
            kind: FolderKind::Archive,
            remote: Some("Archiwum".into()),
        });
        store.set_folders(&account.id, &folders).unwrap();
        let date = |day: u32| format!("From: anna@x.pl\r\nDate: {day} Sep 2026 09:00:00 +0200\r\n");
        let old_read = deliver(&mut store, &account, 1, &date(1));
        let old_unread = deliver(&mut store, &account, 2, &date(2));
        let new_read = deliver(&mut store, &account, 3, &date(28));
        store.set_unread(old_read, false).unwrap();
        store.set_unread(new_read, false).unwrap();
        let mut r = rule(
            "Archive old",
            Some(&account.id),
            vec![
                cond(Field::State, Op::Is, "read"),
                cond(Field::Received, Op::OlderThan, "30"),
            ],
            vec![
                Action::Move {
                    folder: account.folder_key("archive"),
                },
                // Allowed while it runs on new mail too, but never on a schedule.
                Action::AutoReply { body: "Hej".into() },
            ],
        );
        r.on_arrival = false;
        r.schedule = Some(Schedule {
            every: Every::Day,
            folders: vec![account.folder_key("inbox")],
        });
        assert!(store.save_rule(&r).is_err(), "auto-reply needs new mail");
        r.on_arrival = true;
        let r = store.save_rule(&r).unwrap();
        assert_eq!(r.last_run, None);

        // On new mail, the age condition keeps it from anything just arrived.
        assert_eq!(
            run(&mut store, &[new_read], Trigger::Arrival, when(10))
                .unwrap()
                .matched,
            0
        );
        let runs = run_scheduled(&mut store, when(10)).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(
            (runs[0].name.as_str(), runs[0].report.moved),
            ("Archive old", 1)
        );
        assert_eq!(runs[0].report.replied, 0, "a schedule never answers");
        assert_eq!(outbox(&store, &account), 0);
        let m = store.get(old_read).unwrap();
        assert_eq!(m.folder, account.folder_key("archive"));
        assert_eq!(
            m.note.as_deref(),
            Some("Rule \"Archive old\" moved it to Archiwum.")
        );
        assert_eq!(
            store.get(old_unread).unwrap().folder,
            account.folder_key("inbox")
        );
        assert_eq!(
            store.get(new_read).unwrap().folder,
            account.folder_key("inbox")
        );
        assert!(store
            .pending_changes(&account.id)
            .unwrap()
            .iter()
            .any(|c| c.kind == crate::store::ChangeKind::Move(account.folder_key("archive"))));
        let rules = store.rules().unwrap();
        assert_eq!(rules[0].last_run, Some(when(10).naive_local()));

        // Saving the rule again keeps its last run; it isn't due again until tomorrow.
        store.save_rule(&rules[0]).unwrap();
        assert_eq!(
            store.rules().unwrap()[0].last_run,
            Some(when(10).naive_local())
        );
        assert!(run_scheduled(&mut store, when(10) + TimeDelta::hours(5))
            .unwrap()
            .is_empty());
        store.set_unread(old_unread, false).unwrap();
        let runs = run_scheduled(&mut store, when(11)).unwrap();
        assert_eq!(runs[0].report.moved, 1);
        assert_eq!(
            store.get(old_unread).unwrap().folder,
            account.folder_key("archive")
        );
    }

    #[test]
    fn rules_off_new_mail_wait_for_their_schedule() {
        let (mut store, account, holiday) = holiday_store();
        store.delete_rule(holiday.id).unwrap();
        let mut r = rule(
            "Later",
            Some(&account.id),
            vec![cond(Field::Subject, Op::Matches, "^pyta")],
            vec![Action::Flag],
        );
        r.on_arrival = false;
        r.schedule = Some(Schedule {
            every: Every::Hour,
            folders: vec![account.folder_key("inbox")],
        });
        store.save_rule(&r).unwrap();
        let id = deliver(&mut store, &account, 1, "From: anna@x.pl\r\n");
        assert_eq!(
            run(&mut store, &[id], Trigger::Arrival, when(10))
                .unwrap()
                .matched,
            0
        );
        assert_eq!(
            run(&mut store, &[id], Trigger::OnDemand, when(10))
                .unwrap()
                .matched,
            1,
            "Run now runs it"
        );
        let report = preview(
            &store,
            &r,
            &crate::ListView {
                folder: Some(account.folder_key("inbox")),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.matched, 1, "previews take patterns");
    }
}
