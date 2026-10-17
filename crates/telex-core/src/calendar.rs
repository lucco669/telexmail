//! Calendar invitations in mail: iCalendar (RFC 5545) read into an event, shown in local
//! time, and answered with an iTIP reply (RFC 5546, sent as RFC 6047 says).
//!
//! The parser is Telex's own and expects hostile input: a calendar is read only up to
//! [`MAX_BYTES`], a line up to [`MAX_LINE`], at most [`MAX_COMPONENTS`] components nested at
//! most [`MAX_DEPTH`] deep, and long texts and attendee lists are cut. Folded lines are joined
//! before the text is decoded, so a fold inside a UTF-8 character (Google folds by bytes)
//! does no harm; old charsets come in through the part's `charset`.
//!
//! Times: a `TZID` is looked up as an IANA name (`Europe/Warsaw`), a Windows name (`Central
//! European Standard Time`), or else in the calendar's own `VTIMEZONE` rules ([`zones`]). A
//! wall time that a change to summer time skips is moved past the gap, and one that comes
//! twice in autumn is the first of the two, as RFC 5545 says.
//!
//! The store's side (which mail holds an invitation, answers, outdated ones) is in
//! `calendar/invites.rs`.

use std::fmt::Write as _;

use chrono::{
    DateTime, Datelike, Duration as TimeDelta, NaiveDate, NaiveDateTime, Offset, TimeZone, Utc,
    Weekday,
};
use serde::{Deserialize, Serialize};

use crate::i18n::{self, pl_form, Lang};
use crate::tr;

pub mod invites;
pub mod zones;

#[cfg(test)]
mod tests;

/// A calendar longer than this is read only this far.
pub const MAX_BYTES: usize = 1 << 20;
/// A longer (unfolded) line is cut here.
pub const MAX_LINE: usize = 32 * 1024;
/// Lines after this many are left out.
const MAX_LINES: usize = 50_000;
/// Components (events, time zones, alarms…) after this many are left out.
pub const MAX_COMPONENTS: usize = 500;
/// VCALENDAR › VEVENT › VALARM is 3; deeper nesting is skipped.
pub const MAX_DEPTH: usize = 6;
/// Properties of one component after this many are left out.
const MAX_PROPS: usize = 2_000;
/// Attendees shown; the rest are counted.
pub const MAX_ATTENDEES: usize = 200;
/// Characters of a description or comment kept.
const MAX_TEXT: usize = 20_000;
/// Meeting links kept.
const MAX_LINKS: usize = 4;

// ---- Content lines ----

/// One property: `ATTENDEE;CN="Kowalska, Anna";PARTSTAT=ACCEPTED:mailto:anna@x.pl`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prop {
    /// Upper case: "ATTENDEE".
    pub name: String,
    /// Names upper case, values without their quotes (RFC 6868 carets decoded).
    pub params: Vec<(String, String)>,
    /// As written, still escaped.
    pub value: String,
}

impl Prop {
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// A BEGIN…END block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Component {
    /// Upper case: "VEVENT".
    pub name: String,
    pub props: Vec<Prop>,
    pub children: Vec<Component>,
}

impl Component {
    pub fn prop(&self, name: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.name == name)
    }

    pub fn props<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Prop> + 'a {
        self.props.iter().filter(move |p| p.name == name)
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.prop(name).map(|p| p.value.as_str())
    }
}

/// Join folded lines (a line break followed by a space or a tab) in raw bytes, before they
/// are decoded.
fn unfold_bytes(bytes: &[u8]) -> Vec<u8> {
    let bytes = &bytes[..bytes.len().min(MAX_BYTES)];
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let fold = match b {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => 2,
            b'\n' => 1,
            _ => 0,
        };
        if fold > 0 && matches!(bytes.get(i + fold), Some(b' ' | b'\t')) {
            i += fold + 1;
            continue;
        }
        out.push(b);
        i += 1;
    }
    out
}

/// Calendar bytes as text: UTF-8 when they are, else the part's charset, else Windows-1250
/// (the old Polish default), else UTF-8 with replacement characters.
pub fn decode(bytes: &[u8], charset: Option<&str>) -> String {
    let bytes = unfold_bytes(bytes);
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => {
            let bytes = e.into_bytes();
            let decoder = charset
                .filter(|c| !c.eq_ignore_ascii_case("utf-8") && !c.eq_ignore_ascii_case("utf8"))
                .and_then(|c| mail_parser::decoders::charsets::map::charset_decoder(c.as_bytes()))
                .or_else(|| mail_parser::decoders::charsets::map::charset_decoder(b"windows-1250"));
            match decoder {
                Some(decode) => decode(&bytes),
                None => String::from_utf8_lossy(&bytes).into_owned(),
            }
        }
    }
}

/// Logical lines, unfolded (again, for text that came in already decoded), within the limits.
fn lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(rest) = raw.strip_prefix([' ', '\t']) {
            if let Some(last) = lines.last_mut() {
                push_capped(last, rest);
            }
            continue;
        }
        if raw.trim().is_empty() {
            continue;
        }
        if lines.len() >= MAX_LINES {
            break;
        }
        let mut line = String::new();
        push_capped(&mut line, raw);
        lines.push(line);
    }
    lines
}

/// Add `more` to `s` as far as [`MAX_LINE`] allows, never splitting a character.
fn push_capped(s: &mut String, more: &str) {
    let room = MAX_LINE.saturating_sub(s.len());
    if more.len() <= room {
        s.push_str(more);
        return;
    }
    let mut cut = room;
    while cut > 0 && !more.is_char_boundary(cut) {
        cut -= 1;
    }
    s.push_str(&more[..cut]);
}

/// One content line, or `None` when it isn't one.
pub fn content_line(line: &str) -> Option<Prop> {
    let name_end = line.find([';', ':'])?;
    let name = line[..name_end].trim().to_ascii_uppercase();
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    let mut params = Vec::new();
    let mut rest = &line[name_end..];
    while let Some(after) = rest.strip_prefix(';') {
        let eq = after.find(['=', ':', ';'])?;
        if after.as_bytes()[eq] != b'=' {
            // A parameter without a value: skipped.
            rest = &after[eq..];
            continue;
        }
        let pname = after[..eq].trim().to_ascii_uppercase();
        let mut v = &after[eq + 1..];
        let mut value = String::new();
        loop {
            if let Some(quoted) = v.strip_prefix('"') {
                let close = quoted.find('"')?;
                value.push_str(&quoted[..close]);
                v = &quoted[close + 1..];
            } else {
                let end = v.find([',', ';', ':']).unwrap_or(v.len());
                value.push_str(&v[..end]);
                v = &v[end..];
            }
            match v.strip_prefix(',') {
                Some(more) => {
                    value.push(',');
                    v = more;
                }
                None => break,
            }
        }
        if params.len() < 50 {
            params.push((pname, carets(&value)));
        }
        rest = v;
    }
    let value = rest.strip_prefix(':')?;
    Some(Prop {
        name,
        params,
        value: value.to_owned(),
    })
}

/// RFC 6868: ^n is a line break, ^' a double quote, ^^ a caret.
fn carets(value: &str) -> String {
    if !value.contains('^') {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '^' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('\'') => out.push('"'),
            Some('^') => out.push('^'),
            Some(other) => {
                out.push('^');
                out.push(other);
            }
            None => out.push('^'),
        }
    }
    out
}

/// The VCALENDAR in `text`, as components, or `None` when there is none.
pub fn components(text: &str) -> Option<Component> {
    let mut stack: Vec<Component> = Vec::new();
    let mut root: Option<Component> = None;
    let mut count = 0;
    // Components below MAX_DEPTH are skipped whole: how deep inside one we are.
    let mut skipped = 0usize;
    for line in lines(text) {
        let Some(prop) = content_line(&line) else {
            continue;
        };
        match prop.name.as_str() {
            "BEGIN" => {
                if skipped > 0 || stack.len() >= MAX_DEPTH {
                    skipped += 1;
                    continue;
                }
                count += 1;
                if count > MAX_COMPONENTS {
                    break;
                }
                stack.push(Component {
                    name: prop.value.trim().to_ascii_uppercase(),
                    ..Component::default()
                });
            }
            "END" => {
                if skipped > 0 {
                    skipped -= 1;
                    continue;
                }
                let Some(done) = stack.pop() else { continue };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(done),
                    None if done.name == "VCALENDAR" => {
                        root = Some(done);
                        break;
                    }
                    None => {}
                }
            }
            _ if skipped > 0 => {}
            _ => {
                if let Some(c) = stack.last_mut() {
                    if c.props.len() < MAX_PROPS {
                        c.props.push(prop);
                    }
                }
            }
        }
    }
    // What wasn't closed (cut off, or past the limits) closes here.
    while let Some(done) = stack.pop() {
        match stack.last_mut() {
            Some(parent) => parent.children.push(done),
            None if root.is_none() => root = Some(done),
            None => {}
        }
    }
    root.filter(|r| r.name == "VCALENDAR")
}

/// A TEXT value unescaped: `\n` is a line break, `\,` `\;` `\\` the character.
pub fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// A TEXT value escaped for writing.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Text cut to `max` characters, trimmed.
fn capped(s: String, max: usize) -> String {
    let s = s.trim();
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", s[..at].trim_end()),
        None => s.to_owned(),
    }
}

// ---- Values ----

/// A DATE or DATE-TIME as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moment {
    /// All day: `VALUE=DATE:20261020`.
    Date(NaiveDate),
    /// `20261020T120000Z`
    Utc(NaiveDateTime),
    /// `TZID=Europe/Warsaw:20261020T140000`: a wall time in that zone.
    Zoned(NaiveDateTime, String),
    /// `20261020T140000`: the same wall time wherever one is.
    Floating(NaiveDateTime),
}

impl Moment {
    pub fn read(prop: &Prop) -> Option<Self> {
        let v = prop.value.trim();
        if !v.is_ascii() {
            return None;
        }
        // A list (RDATE, EXDATE) gives its first value here.
        let v = v.split(',').next().unwrap_or_default();
        let date_only = prop
            .param("VALUE")
            .is_some_and(|x| x.eq_ignore_ascii_case("DATE"))
            || v.len() == 8;
        if date_only {
            return NaiveDate::parse_from_str(v.get(..8)?, "%Y%m%d")
                .ok()
                .map(Self::Date);
        }
        let (v, utc) = match v.strip_suffix(['Z', 'z']) {
            Some(v) => (v, true),
            None => (v, false),
        };
        let at = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S")
            .or_else(|_| NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M"))
            .ok()?;
        Some(if utc {
            Self::Utc(at)
        } else if let Some(tzid) = prop.param("TZID").map(str::trim).filter(|t| !t.is_empty()) {
            Self::Zoned(at, tzid.to_owned())
        } else {
            Self::Floating(at)
        })
    }

    fn date(&self) -> NaiveDate {
        match self {
            Self::Date(d) => *d,
            Self::Utc(at) | Self::Zoned(at, _) | Self::Floating(at) => at.date(),
        }
    }
}

/// `PT1H30M`, `P1D`, `-PT15M`, `P1W`.
pub fn duration(v: &str) -> Option<TimeDelta> {
    let v = v.trim();
    let (negative, v) = match v.as_bytes().first()? {
        b'-' => (true, &v[1..]),
        b'+' => (false, &v[1..]),
        _ => (false, v),
    };
    let v = v.strip_prefix(['P', 'p'])?;
    let mut total: i64 = 0;
    let mut number: Option<i64> = None;
    let mut timed = false;
    for c in v.chars() {
        match c.to_ascii_uppercase() {
            '0'..='9' => {
                let digit = i64::from(c.to_digit(10)?);
                number = Some(number.unwrap_or(0).checked_mul(10)?.checked_add(digit)?);
            }
            'T' => timed = true,
            unit => {
                let n = number.take()?;
                let seconds = match (unit, timed) {
                    ('W', false) => 604_800,
                    ('D', false) => 86_400,
                    ('H', true) => 3_600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
                total = total.checked_add(n.checked_mul(seconds)?)?;
            }
        }
    }
    if number.is_some() || total > 10 * 366 * 86_400 {
        return None;
    }
    Some(TimeDelta::seconds(if negative { -total } else { total }))
}

/// `mailto:Anna@X.pl` → "anna@x.pl"; other URIs (a phone, a room's id) → None.
fn mailto(value: &str) -> Option<String> {
    let v = value.trim();
    let address = if v.len() > 7 && v[..7].eq_ignore_ascii_case("mailto:") {
        &v[7..]
    } else if v.contains('@') && !v.contains(':') {
        v
    } else {
        return None;
    };
    let address = address.trim();
    (address.contains('@') && address.len() <= 320 && !address.contains(['<', '>', ' ', '\n']))
        .then(|| address.to_ascii_lowercase())
}

// ---- Events ----

/// What a calendar in mail asks for (RFC 5546).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// An invitation, or an update of one: it can be answered.
    Request,
    /// The meeting (or one of its dates) is off.
    Cancel,
    /// Someone's answer to the user's invitation.
    Reply,
    /// An event to look at or add to a calendar, nothing to answer. Also a calendar file
    /// without a METHOD.
    Publish,
    /// Someone proposes another time.
    Counter,
    Other,
}

impl Method {
    fn read(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_uppercase()).as_deref() {
            Some("REQUEST") | Some("ADD") => Self::Request,
            Some("CANCEL") => Self::Cancel,
            Some("REPLY") => Self::Reply,
            Some("PUBLISH") | None | Some("") => Self::Publish,
            Some("COUNTER") => Self::Counter,
            Some(_) => Self::Other,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Request => "REQUEST",
            Self::Cancel => "CANCEL",
            Self::Reply => "REPLY",
            Self::Publish => "PUBLISH",
            Self::Counter => "COUNTER",
            Self::Other => "OTHER",
        }
    }

    pub fn from_code(code: &str) -> Self {
        Self::read(Some(code))
    }
}

/// An attendee's answer (PARTSTAT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    NeedsAction,
    Accepted,
    Tentative,
    Declined,
    Delegated,
}

impl Answer {
    pub fn read(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_uppercase()).as_deref() {
            Some("ACCEPTED") => Self::Accepted,
            Some("TENTATIVE") => Self::Tentative,
            Some("DECLINED") => Self::Declined,
            Some("DELEGATED") => Self::Delegated,
            _ => Self::NeedsAction,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::NeedsAction => "NEEDS-ACTION",
            Self::Accepted => "ACCEPTED",
            Self::Tentative => "TENTATIVE",
            Self::Declined => "DECLINED",
            Self::Delegated => "DELEGATED",
        }
    }

    /// "You accepted on 17.10.2026, 14:02."
    pub fn note(self, at: NaiveDateTime) -> String {
        let at = when(at);
        match self {
            Self::Accepted => tr!("You accepted on {at}.", "Zaakceptowano {at}."),
            Self::Tentative => tr!("You answered maybe on {at}.", "Odpowiedziano „może” {at}."),
            Self::Declined => tr!("You declined on {at}.", "Odrzucono {at}."),
            Self::NeedsAction | Self::Delegated => {
                tr!("You answered on {at}.", "Odpowiedziano {at}.")
            }
        }
    }
}

/// The date and time in a note: "17 Oct 2026, 14:02" / "17.10.2026, 14:02".
fn when(at: NaiveDateTime) -> String {
    match i18n::current() {
        Lang::Pl => at.format("%d.%m.%Y, %H:%M").to_string(),
        Lang::En => at.format("%-d %b %Y, %H:%M").to_string(),
    }
}

/// A day in a sentence: "31 Dec 2026" / "31.12.2026".
fn day(d: NaiveDate) -> String {
    match i18n::current() {
        Lang::Pl => d.format("%d.%m.%Y").to_string(),
        Lang::En => d.format("%-d %b %Y").to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Chair,
    Required,
    Optional,
    /// Told about it, not taking part.
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Person {
    /// The CN, or "" when there is none.
    pub name: String,
    /// Lower case; "" when the calendar gives no address.
    pub email: String,
}

impl Person {
    fn read(prop: &Prop) -> Self {
        Self {
            name: capped(
                prop.param("CN")
                    .unwrap_or_default()
                    .trim()
                    .trim_matches('\'')
                    .to_owned(),
                200,
            ),
            email: mailto(&prop.value).unwrap_or_default(),
        }
    }

    /// "Anna Kowalska <anna@x.pl>", for a To line.
    pub fn mailbox(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case(&self.email) {
            self.email.clone()
        } else {
            format!("\"{}\" <{}>", name.replace(['"', '\\'], "'"), self.email)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attendee {
    #[serde(flatten)]
    pub person: Person,
    pub answer: Answer,
    pub role: Role,
    /// A meeting room or another resource, not a person.
    pub room: bool,
    /// One of the user's own addresses (set by the store).
    #[serde(default)]
    pub me: bool,
}

impl Attendee {
    fn read(prop: &Prop) -> Self {
        Self {
            person: Person::read(prop),
            answer: Answer::read(prop.param("PARTSTAT")),
            role: match prop.param("ROLE").map(str::to_ascii_uppercase).as_deref() {
                Some("CHAIR") => Role::Chair,
                Some("OPT-PARTICIPANT") => Role::Optional,
                Some("NON-PARTICIPANT") => Role::Info,
                _ => Role::Required,
            },
            room: prop.param("CUTYPE").is_some_and(|c| {
                c.eq_ignore_ascii_case("ROOM") || c.eq_ignore_ascii_case("RESOURCE")
            }),
            me: false,
        }
    }
}

/// An online meeting's address: Teams, Meet, Zoom…
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    /// "Microsoft Teams"
    pub label: String,
    /// http or https only.
    pub url: String,
}

/// One VEVENT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub uid: String,
    pub sequence: i64,
    /// The one date of a repeating event this is about (RECURRENCE-ID), as written.
    pub recurrence: Option<Prop>,
    /// DTSTAMP.
    pub stamp: Option<Moment>,
    pub summary: String,
    pub description: String,
    pub location: String,
    pub comment: String,
    pub start: Option<Moment>,
    pub end: Option<Moment>,
    pub duration: Option<TimeDelta>,
    /// The RRULE as written.
    pub rule: Option<String>,
    /// STATUS:CANCELLED
    pub cancelled: bool,
    pub organizer: Option<Person>,
    /// The ORGANIZER line as written, for a reply.
    organizer_prop: Option<Prop>,
    pub attendees: Vec<Attendee>,
    /// How many there are, including those past [`MAX_ATTENDEES`].
    pub attendee_count: usize,
    pub links: Vec<Link>,
}

impl Event {
    fn read(c: &Component) -> Self {
        let mut attendees = Vec::new();
        let mut attendee_count = 0;
        for p in c.props("ATTENDEE") {
            attendee_count += 1;
            if attendees.len() < MAX_ATTENDEES {
                attendees.push(Attendee::read(p));
            }
        }
        let field = |name: &str, max| capped(text(c.value(name).unwrap_or_default()), max);
        let mut event = Self {
            uid: c
                .value("UID")
                .unwrap_or_default()
                .trim()
                .chars()
                .take(1000)
                .collect(),
            sequence: c
                .value("SEQUENCE")
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0),
            recurrence: c.prop("RECURRENCE-ID").cloned(),
            stamp: c.prop("DTSTAMP").and_then(Moment::read),
            summary: field("SUMMARY", 1000),
            description: field("DESCRIPTION", MAX_TEXT),
            location: field("LOCATION", 1000),
            comment: field("COMMENT", MAX_TEXT),
            start: c.prop("DTSTART").and_then(Moment::read),
            end: c
                .prop("DTEND")
                .or_else(|| c.prop("DUE"))
                .and_then(Moment::read),
            duration: c.value("DURATION").and_then(duration),
            rule: c
                .value("RRULE")
                .map(|r| r.trim().chars().take(500).collect()),
            cancelled: c
                .value("STATUS")
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("CANCELLED")),
            organizer: c.prop("ORGANIZER").map(Person::read),
            organizer_prop: c.prop("ORGANIZER").cloned(),
            attendees,
            attendee_count,
            links: Vec::new(),
        };
        event.links = meeting_links(c, &event);
        event
    }
}

/// A calendar from a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calendar {
    pub method: Method,
    pub events: Vec<Event>,
    /// The VTIMEZONE components, for TZIDs that aren't known names.
    pub zones: Vec<Component>,
}

impl Calendar {
    /// The event a card shows: the series (no RECURRENCE-ID) before one changed date.
    pub fn main(&self) -> Option<&Event> {
        self.events
            .iter()
            .find(|e| e.recurrence.is_none())
            .or_else(|| self.events.first())
    }

    /// When a moment is, in UTC, with the zone it was written in; `None` for all-day dates
    /// and floating times, which are the same wall time everywhere.
    pub fn instant(&self, m: &Moment) -> Option<(NaiveDateTime, Option<zones::Zone>)> {
        match m {
            Moment::Utc(at) => Some((*at, None)),
            Moment::Zoned(at, tzid) => {
                let zone = zones::find(tzid, &self.zones)?;
                Some((zone.to_utc(*at), Some(zone)))
            }
            Moment::Date(_) | Moment::Floating(_) => None,
        }
    }
}

/// Read a calendar. `None` when there is no VCALENDAR with a VEVENT in it.
pub fn parse(text: &str) -> Option<Calendar> {
    let root = components(text)?;
    let events: Vec<Event> = root
        .children
        .iter()
        .filter(|c| c.name == "VEVENT")
        .map(Event::read)
        .collect();
    if events.is_empty() {
        return None;
    }
    Some(Calendar {
        method: Method::read(root.value("METHOD")),
        events,
        zones: root
            .children
            .iter()
            .filter(|c| c.name == "VTIMEZONE")
            .cloned()
            .collect(),
    })
}

/// Online meeting links: from the properties meeting services write (RFC 7986 CONFERENCE,
/// Google's and Microsoft's own), then any address of a known service in the location or
/// the description.
fn meeting_links(c: &Component, e: &Event) -> Vec<Link> {
    let mut found: Vec<String> = Vec::new();
    for name in [
        "CONFERENCE",
        "X-GOOGLE-CONFERENCE",
        "X-MICROSOFT-SKYPETEAMSMEETINGURL",
        "X-MICROSOFT-ONLINEMEETINGEXTERNALLINK",
        "URL",
    ] {
        for p in c.props(name) {
            found.extend(urls_in(&text(&p.value)));
        }
    }
    found.extend(urls_in(&e.location));
    found.extend(urls_in(&e.description));
    let mut links: Vec<Link> = Vec::new();
    for url in found {
        let Some(label) = service(&url) else { continue };
        if links.len() < MAX_LINKS && !links.iter().any(|l| l.url == url) {
            links.push(Link {
                label: label.to_owned(),
                url,
            });
        }
    }
    links
}

/// http(s) addresses in a text, up to the first space, quote or bracket.
fn urls_in(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find("http") {
        let start = from + found;
        let rest = &text[start..];
        let end = rest
            .find(|c: char| c.is_whitespace() || "<>\"'()[]{}|\\^`".contains(c))
            .unwrap_or(rest.len());
        let candidate = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        let lower_candidate = candidate.to_ascii_lowercase();
        if (lower_candidate.starts_with("https://") || lower_candidate.starts_with("http://"))
            && candidate.len() <= 2048
            && urls.len() < 50
        {
            urls.push(candidate.to_owned());
        }
        from = start + end.max(4);
        if from >= text.len() {
            break;
        }
    }
    urls
}

/// Which meeting service a link is for, by its host.
fn service(url: &str) -> Option<&'static str> {
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    let is = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    if is("teams.microsoft.com") || is("teams.live.com") {
        Some("Microsoft Teams")
    } else if host == "meet.google.com" {
        Some("Google Meet")
    } else if is("zoom.us") || is("zoomgov.com") {
        Some("Zoom")
    } else if is("webex.com") {
        Some("Webex")
    } else if host == "meet.jit.si" {
        Some("Jitsi Meet")
    } else if is("whereby.com") {
        Some("Whereby")
    } else if host == "facetime.apple.com" {
        Some("FaceTime")
    } else {
        None
    }
}

// ---- Repeating events ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freq {
    Secondly,
    Minutely,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// An RRULE, as far as Telex reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub freq: Freq,
    pub interval: u32,
    /// (nth, day): 0 is every one; 2 the second, -1 the last.
    pub by_day: Vec<(i8, Weekday)>,
    pub by_month_day: Vec<i8>,
    pub by_month: Vec<u8>,
    pub by_set_pos: Vec<i16>,
    pub count: Option<u32>,
    pub until: Option<Moment>,
    /// Parts a sentence can't say (BYHOUR, BYWEEKNO…).
    pub more: bool,
}

fn weekday(code: &str) -> Option<Weekday> {
    Some(match code {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

impl Rule {
    pub fn read(value: &str) -> Option<Self> {
        if !value.is_ascii() {
            return None;
        }
        let mut rule = Self {
            freq: Freq::Daily,
            interval: 1,
            by_day: Vec::new(),
            by_month_day: Vec::new(),
            by_month: Vec::new(),
            by_set_pos: Vec::new(),
            count: None,
            until: None,
            more: false,
        };
        let mut freq = None;
        for part in value.trim().split(';').take(20) {
            let Some((key, v)) = part.split_once('=') else {
                continue;
            };
            let v = v.trim().to_ascii_uppercase();
            let list = || v.split(',').take(64).map(str::trim);
            match key.trim().to_ascii_uppercase().as_str() {
                "FREQ" => {
                    freq = Some(match v.as_str() {
                        "SECONDLY" => Freq::Secondly,
                        "MINUTELY" => Freq::Minutely,
                        "HOURLY" => Freq::Hourly,
                        "DAILY" => Freq::Daily,
                        "WEEKLY" => Freq::Weekly,
                        "MONTHLY" => Freq::Monthly,
                        "YEARLY" => Freq::Yearly,
                        _ => return None,
                    })
                }
                "INTERVAL" => rule.interval = v.parse().ok().filter(|n| (1..=1000).contains(n))?,
                "COUNT" => rule.count = v.parse().ok(),
                "UNTIL" => {
                    rule.until = Moment::read(&Prop {
                        value: v.clone(),
                        ..Prop::default()
                    })
                }
                "BYDAY" => {
                    for d in list() {
                        let split = d.len().checked_sub(2)?;
                        let (n, code) = d.split_at(split);
                        let n = if n.is_empty() {
                            0
                        } else {
                            n.trim_start_matches('+').parse().ok()?
                        };
                        rule.by_day.push((n, weekday(code)?));
                    }
                }
                "BYMONTHDAY" => {
                    for d in list() {
                        rule.by_month_day.push(d.parse().ok()?);
                    }
                }
                "BYMONTH" => {
                    for d in list() {
                        rule.by_month
                            .push(d.parse().ok().filter(|m| (1..=12).contains(m))?);
                    }
                }
                "BYSETPOS" => {
                    for d in list() {
                        rule.by_set_pos.push(d.parse().ok()?);
                    }
                }
                "WKST" => {}
                _ => rule.more = true,
            }
        }
        rule.freq = freq?;
        Some(rule)
    }
}

const WEEK: [Weekday; 7] = [
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
    Weekday::Sat,
    Weekday::Sun,
];

/// "Monday" / "poniedziałek" (in Polish as after "w": środę).
fn day_name(d: Weekday) -> &'static str {
    match i18n::current() {
        Lang::En => match d {
            Weekday::Mon => "Monday",
            Weekday::Tue => "Tuesday",
            Weekday::Wed => "Wednesday",
            Weekday::Thu => "Thursday",
            Weekday::Fri => "Friday",
            Weekday::Sat => "Saturday",
            Weekday::Sun => "Sunday",
        },
        Lang::Pl => match d {
            Weekday::Mon => "poniedziałek",
            Weekday::Tue => "wtorek",
            Weekday::Wed => "środę",
            Weekday::Thu => "czwartek",
            Weekday::Fri => "piątek",
            Weekday::Sat => "sobotę",
            Weekday::Sun => "niedzielę",
        },
    }
}

/// "Monday, Wednesday and Friday" / "poniedziałek, środę i piątek".
fn and_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => {
            let and = tr!("and", "i");
            format!("{} {and} {last}", rest.join(", "))
        }
    }
}

/// "on Monday" / "w poniedziałek", "we wtorek".
fn on_days(days: &[Weekday]) -> String {
    let names: Vec<&str> = days.iter().map(|d| day_name(*d)).collect();
    let list = and_list(&names);
    match i18n::current() {
        Lang::En => format!("on {list}"),
        Lang::Pl if days.first() == Some(&Weekday::Tue) => format!("we {list}"),
        Lang::Pl => format!("w {list}"),
    }
}

/// "the second Tuesday" / "drugi wtorek", "ostatnią środę"; None past what a sentence says.
fn nth_day(n: i16, d: Weekday) -> Option<String> {
    let name = day_name(d);
    Some(match i18n::current() {
        Lang::En => {
            let nth = match n {
                1 => "first",
                2 => "second",
                3 => "third",
                4 => "fourth",
                5 => "fifth",
                -1 => "last",
                -2 => "second to last",
                _ => return None,
            };
            format!("the {nth} {name}")
        }
        Lang::Pl => {
            let feminine = matches!(d, Weekday::Wed | Weekday::Sat | Weekday::Sun);
            let nth = match (n, feminine) {
                (1, false) => "pierwszy",
                (1, true) => "pierwszą",
                (2, false) => "drugi",
                (2, true) => "drugą",
                (3, false) => "trzeci",
                (3, true) => "trzecią",
                (4, false) => "czwarty",
                (4, true) => "czwartą",
                (5, false) => "piąty",
                (5, true) => "piątą",
                (-1, false) => "ostatni",
                (-1, true) => "ostatnią",
                (-2, false) => "przedostatni",
                (-2, true) => "przedostatnią",
                _ => return None,
            };
            format!("{nth} {name}")
        }
    })
}

/// "Every 2 weeks" / "Co 2 tygodnie", with Polish plurals.
fn every(n: u32, en_one: &str, en_many: &str, pl_one: &str, pl_many: [&str; 3]) -> String {
    match (i18n::current(), n) {
        (Lang::En, 1) => format!("Every {en_one}"),
        (Lang::En, n) => format!("Every {n} {en_many}"),
        (Lang::Pl, 1) => pl_one.to_owned(),
        (Lang::Pl, n) => format!("Co {n} {}", pl_many[pl_form(u64::from(n))]),
    }
}

/// A repeating event's rule in words: "Every week on Monday", "Every 2 weeks on Monday and
/// Thursday, until 31 Dec 2026", "Every month on the second Tuesday", "Every year". A rule a
/// sentence can't say well is "Repeats". `start` is the first date, which a rule without
/// BYDAY or BYMONTHDAY repeats; `until` turns an UNTIL into a local date.
pub fn describe(
    rule: &str,
    start: Option<NaiveDate>,
    until: impl Fn(&Moment) -> NaiveDate,
) -> String {
    let repeats = || tr!("Repeats", "Powtarza się");
    let Some(r) = Rule::read(rule) else {
        return repeats();
    };
    if r.more
        || !r
            .by_set_pos
            .iter()
            .all(|p| r.by_set_pos.len() == 1 && *p != 0)
    {
        return repeats();
    }
    let plain_days: Option<Vec<Weekday>> = r
        .by_day
        .iter()
        .map(|(n, d)| (*n == 0).then_some(*d))
        .collect();
    let base = match r.freq {
        Freq::Daily
            if r.by_month_day.is_empty() && r.by_month.is_empty() && r.by_set_pos.is_empty() =>
        {
            let days = plain_days.unwrap_or_default();
            if days.is_empty() {
                every(
                    r.interval,
                    "day",
                    "days",
                    "Codziennie",
                    ["dzień", "dni", "dni"],
                )
            } else if r.interval == 1
                && days.len() == 5
                && WEEK[..5].iter().all(|d| days.contains(d))
            {
                tr!("Every weekday", "W dni robocze")
            } else {
                return repeats();
            }
        }
        Freq::Weekly
            if r.by_month_day.is_empty() && r.by_month.is_empty() && r.by_set_pos.is_empty() =>
        {
            let Some(mut days) = plain_days else {
                return repeats();
            };
            if days.is_empty() {
                match start {
                    Some(s) => days.push(s.weekday()),
                    None => return repeats(),
                }
            }
            days.sort_by_key(|d| d.num_days_from_monday());
            days.dedup();
            let every = every(
                r.interval,
                "week",
                "weeks",
                "Co tydzień",
                ["tydzień", "tygodnie", "tygodni"],
            );
            if days.len() == 7 {
                every
            } else if r.interval == 1
                && days.len() == 5
                && WEEK[..5].iter().all(|d| days.contains(d))
            {
                tr!("Every weekday", "W dni robocze")
            } else {
                format!("{every} {}", on_days(&days))
            }
        }
        Freq::Monthly if r.by_month.is_empty() => {
            let every = every(
                r.interval,
                "month",
                "months",
                "Co miesiąc",
                ["miesiąc", "miesiące", "miesięcy"],
            );
            let nth = match (r.by_day.as_slice(), r.by_set_pos.as_slice()) {
                ([(n, d)], []) if *n != 0 => Some((i16::from(*n), *d)),
                ([(0, d)], [pos]) => Some((*pos, *d)),
                _ => None,
            };
            if let Some((n, d)) = nth {
                if !r.by_month_day.is_empty() {
                    return repeats();
                }
                let Some(words) = nth_day(n, d) else {
                    return repeats();
                };
                match i18n::current() {
                    Lang::En => format!("{every} on {words}"),
                    Lang::Pl => format!("{every} w {words}"),
                }
            } else if r.by_day.is_empty() {
                let day = match r.by_month_day.as_slice() {
                    [] => match start {
                        Some(s) => s.day() as i8,
                        None => return repeats(),
                    },
                    [d] => *d,
                    _ => return repeats(),
                };
                match day {
                    -1 => tr!("{every} on the last day", "{every}, ostatniego dnia"),
                    1..=31 => tr!("{every} on day {day}", "{every}, {day}. dnia"),
                    _ => return repeats(),
                }
            } else {
                return repeats();
            }
        }
        Freq::Yearly
            if r.by_day.is_empty()
                && r.by_set_pos.is_empty()
                && r.by_month.len() <= 1
                && r.by_month_day.len() <= 1 =>
        {
            every(
                r.interval,
                "year",
                "years",
                "Co roku",
                ["rok", "lata", "lat"],
            )
        }
        _ => return repeats(),
    };
    if let Some(n) = r.count {
        return match i18n::current() {
            Lang::En if n == 1 => format!("{base}, once"),
            Lang::En => format!("{base}, {n} times"),
            Lang::Pl if n == 1 => format!("{base}, 1 raz"),
            Lang::Pl => format!("{base}, {n} razy"),
        };
    }
    if let Some(u) = &r.until {
        let date = day(until(u));
        return tr!("{base}, until {date}", "{base}, do {date}");
    }
    base
}

// ---- What the card shows ----

/// An event as the card shows it: times in the user's local zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventView {
    pub uid: String,
    pub sequence: i64,
    pub summary: String,
    pub location: String,
    pub description: String,
    /// What the attendee wrote along with a reply.
    pub comment: String,
    /// Local time; for an all-day event midnight of its first day.
    pub start: Option<NaiveDateTime>,
    /// Local time; for an all-day event midnight of its last day (inclusive).
    pub end: Option<NaiveDateTime>,
    pub all_day: bool,
    /// The zone the event was written in, when its clock differs from this PC's.
    pub zone: Option<ZoneView>,
    /// "Every week on Monday"
    pub repeats: Option<String>,
    /// One date of a repeating event, not the whole series.
    pub one_date: bool,
    pub organizer: Option<Person>,
    pub attendees: Vec<Attendee>,
    /// Attendees past those listed.
    pub more_attendees: usize,
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneView {
    /// "Pacific Standard Time", "America/New_York"
    pub name: String,
    /// Minutes east of UTC at the start.
    pub offset: i32,
    /// Wall times there.
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
}

/// The local wall time of a UTC instant in `local`, and that zone's offset then (seconds).
fn local_of<L: TimeZone>(local: &L, utc: NaiveDateTime) -> (NaiveDateTime, i32) {
    let at = local.from_utc_datetime(&utc);
    (at.naive_local(), at.offset().fix().local_minus_utc())
}

/// The event `e` of `cal` in the time zone `local` (chrono's `Local` in the app).
pub fn view<L: TimeZone>(cal: &Calendar, e: &Event, local: &L) -> EventView {
    let all_day = matches!(e.start, Some(Moment::Date(_)));
    let mut start = None;
    let mut end = None;
    let mut zone = None;
    match &e.start {
        Some(Moment::Date(d)) => {
            let last = match &e.end {
                Some(Moment::Date(end)) if *end > *d => *end - TimeDelta::days(1),
                _ => match e.duration {
                    Some(len) if len.num_days() > 1 => *d + TimeDelta::days(len.num_days() - 1),
                    _ => *d,
                },
            };
            start = d.and_hms_opt(0, 0, 0);
            end = last.and_hms_opt(0, 0, 0);
        }
        Some(m) => match cal.instant(m) {
            Some((utc, written_in)) => {
                let end_utc = match &e.end {
                    Some(end @ (Moment::Utc(_) | Moment::Zoned(..))) => {
                        cal.instant(end).map(|(u, _)| u)
                    }
                    _ => None,
                }
                .or_else(|| e.duration.map(|len| utc + len));
                let (here, offset) = local_of(local, utc);
                start = Some(here);
                end = end_utc.map(|u| local_of(local, u).0);
                // Only a zone the event was written in is worth naming; UTC is nobody's.
                if let Some(z) = written_in {
                    let theirs = z.offset_at(utc);
                    if theirs != offset {
                        zone = Some(ZoneView {
                            name: z.name().to_owned(),
                            offset: theirs / 60,
                            start: utc + TimeDelta::seconds(i64::from(theirs)),
                            end: end_utc.map(|u| u + TimeDelta::seconds(i64::from(z.offset_at(u)))),
                        });
                    }
                }
            }
            // Floating, or a zone no one knows: the wall time as written.
            None => {
                if let Moment::Floating(w) | Moment::Zoned(w, _) = m {
                    start = Some(*w);
                    end = match &e.end {
                        Some(Moment::Floating(end) | Moment::Zoned(end, _)) => Some(*end),
                        _ => e.duration.map(|len| *w + len),
                    };
                }
            }
        },
        None => {}
    }
    let repeats = e.rule.as_deref().map(|r| {
        describe(r, e.start.as_ref().map(Moment::date), |u| match u {
            Moment::Utc(at) => local_of(local, *at).0.date(),
            other => other.date(),
        })
    });
    EventView {
        uid: e.uid.clone(),
        sequence: e.sequence,
        summary: e.summary.clone(),
        location: e.location.clone(),
        description: e.description.clone(),
        comment: e.comment.clone(),
        start,
        end: end.filter(|end| start.is_none_or(|s| *end >= s)),
        all_day,
        zone,
        repeats,
        one_date: e.recurrence.is_some(),
        organizer: e.organizer.clone(),
        attendees: e.attendees.clone(),
        more_attendees: e.attendee_count.saturating_sub(e.attendees.len()),
        links: e.links.clone(),
    }
}

// ---- Answering ----

/// Fold a content line at 75 octets (RFC 5545 3.1), never inside a character.
fn fold_line(out: &mut String, line: &str) {
    let mut width = 0;
    for c in line.chars() {
        let len = c.len_utf8();
        if width + len > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += len;
    }
    out.push_str("\r\n");
}

/// A parameter value, quoted when it must be.
fn param_value(v: &str) -> String {
    let v: String = v.chars().filter(|c| !c.is_control() && *c != '"').collect();
    if v.contains([':', ';', ',']) || v.contains(' ') {
        format!("\"{v}\"")
    } else {
        v
    }
}

/// A property line as it was read, written back.
fn prop_line(p: &Prop) -> String {
    let mut line = p.name.clone();
    for (name, value) in &p.params {
        let _ = write!(line, ";{name}={}", param_value(value));
    }
    line.push(':');
    line.push_str(&p.value.replace(['\r', '\n'], ""));
    line
}

/// DTSTART / DTEND / RECURRENCE-ID for a reply: in UTC when the zone is known, so the reply
/// needs no VTIMEZONE of its own.
fn moment_line(name: &str, cal: &Calendar, m: &Moment) -> String {
    match m {
        Moment::Date(d) => format!("{name};VALUE=DATE:{}", d.format("%Y%m%d")),
        Moment::Floating(at) => format!("{name}:{}", at.format("%Y%m%dT%H%M%S")),
        _ => match cal.instant(m) {
            Some((utc, _)) => format!("{name}:{}", utc.format("%Y%m%dT%H%M%SZ")),
            None => match m {
                Moment::Zoned(at, _) => format!("{name}:{}", at.format("%Y%m%dT%H%M%S")),
                _ => String::new(),
            },
        },
    }
}

/// The iTIP REPLY (RFC 5546 3.2.3) to an invitation: the same UID, SEQUENCE and
/// RECURRENCE-ID, the organizer as written, and only the user's own ATTENDEE line with their
/// answer, and their comment.
pub fn reply_ics(
    cal: &Calendar,
    e: &Event,
    me: &Person,
    answer: Answer,
    comment: &str,
    now: DateTime<Utc>,
) -> String {
    let mut out = String::new();
    let mut line = |l: &str| fold_line(&mut out, l);
    line("BEGIN:VCALENDAR");
    line("PRODID:-//Telex//Telex mail//EN");
    line("VERSION:2.0");
    line("METHOD:REPLY");
    line("BEGIN:VEVENT");
    line(&format!("UID:{}", e.uid.replace(['\r', '\n'], "")));
    if let Some(r) = &e.recurrence {
        match Moment::read(r) {
            Some(m) => line(&moment_line("RECURRENCE-ID", cal, &m)),
            None => line(&prop_line(r)),
        }
    }
    line(&format!("SEQUENCE:{}", e.sequence));
    line(&format!("DTSTAMP:{}", now.format("%Y%m%dT%H%M%SZ")));
    if let Some(start) = &e.start {
        let l = moment_line("DTSTART", cal, start);
        if !l.is_empty() {
            line(&l);
        }
    }
    if let Some(end) = &e.end {
        let l = moment_line("DTEND", cal, end);
        if !l.is_empty() {
            line(&l);
        }
    }
    if !e.summary.is_empty() {
        line(&format!("SUMMARY:{}", escape(&e.summary)));
    }
    if let Some(org) = &e.organizer_prop {
        line(&prop_line(org));
    }
    let mut attendee = format!("ATTENDEE;PARTSTAT={}", answer.code());
    if !me.name.trim().is_empty() {
        let _ = write!(attendee, ";CN={}", param_value(me.name.trim()));
    }
    let _ = write!(attendee, ":mailto:{}", me.email);
    line(&attendee);
    let comment = comment.trim();
    if !comment.is_empty() {
        line(&format!(
            "COMMENT:{}",
            escape(&capped(comment.to_owned(), MAX_TEXT))
        ));
    }
    line("END:VEVENT");
    line("END:VCALENDAR");
    out
}

/// The subject of a reply: "Accepted: Planning" / "Zaakceptowano: Planning".
pub fn reply_subject(answer: Answer, summary: &str) -> String {
    let summary = if summary.trim().is_empty() {
        tr!("Invitation", "Zaproszenie")
    } else {
        summary.trim().to_owned()
    };
    match answer {
        Answer::Accepted => tr!("Accepted: {summary}", "Zaakceptowano: {summary}"),
        Answer::Tentative => tr!("Tentative: {summary}", "Wstępnie zaakceptowano: {summary}"),
        Answer::Declined => tr!("Declined: {summary}", "Odrzucono: {summary}"),
        Answer::NeedsAction | Answer::Delegated => summary,
    }
}

/// The text of a reply, for mail programs that show no calendar.
pub fn reply_text(answer: Answer, who: &str, summary: &str, comment: &str) -> String {
    let summary = summary.trim();
    let mut text = match answer {
        Answer::Accepted => tr!(
            "{who} accepted the invitation: {summary}",
            "{who} akceptuje zaproszenie: {summary}"
        ),
        Answer::Tentative => tr!(
            "{who} may attend: {summary}",
            "{who} może wziąć udział: {summary}"
        ),
        Answer::Declined => tr!(
            "{who} declined the invitation: {summary}",
            "{who} odrzuca zaproszenie: {summary}"
        ),
        Answer::NeedsAction | Answer::Delegated => summary.to_owned(),
    };
    let comment = comment.trim();
    if !comment.is_empty() {
        text.push_str("\n\n");
        text.push_str(comment);
    }
    text.push('\n');
    text
}
