//! Unsubscribing from a mailing list with one click (List-Unsubscribe, RFC 2369 and 8058).
//!
//! A message from a list says how to leave it: web addresses and mail addresses in angle
//! brackets. Telex picks the best of them ([`choose`]): an https address the list marks for
//! one click (`List-Unsubscribe-Post: List-Unsubscribe=One-Click`), which Telex POSTs itself
//! ([`post`]: public addresses only, no cookies, no redirects, a generic User-Agent); else a
//! mail address, written to through the account's SMTP; else a page, opened in the browser.
//! Never for mail in Spam: answering a spammer confirms the address is read. Lists left
//! are remembered (`unsubscribed`, by List-Id or sender), so every message of the list says so.

use std::time::Duration;

use chrono::NaiveDateTime;
use mail_parser::MessageParser;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::model::{FolderKind, Message, MessageId, OutgoingDraft};
use crate::store::{MailStore, StoreError, LOCAL_ID};
use crate::tr;

/// How long the list's server has to answer a one-click request.
const TIMEOUT: Duration = Duration::from_secs(10);

/// How to leave the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// RFC 8058: an https address that takes a POST and needs nothing more.
    OneClick(String),
    /// A mail to this address, with this subject and text.
    Mail {
        to: String,
        subject: String,
        body: String,
    },
    /// A web page where the user unsubscribes, opened in the browser.
    Link(String),
}

impl Method {
    pub fn code(&self) -> &'static str {
        match self {
            Self::OneClick(_) => "one_click",
            Self::Mail { .. } => "mail",
            Self::Link(_) => "link",
        }
    }

    /// What the user is shown before it happens: the web server's name, or the address.
    pub fn target(&self) -> String {
        match self {
            Self::OneClick(url) | Self::Link(url) => host(url),
            Self::Mail { to, .. } => to.clone(),
        }
    }
}

/// What the message pane shows: the Unsubscribe button and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Offer {
    /// one_click, mail or link.
    pub method: &'static str,
    /// The web server's name or the mail address, for the confirmation.
    pub target: String,
    /// When the user left this list (from any of its messages), if they did.
    pub done: Option<NaiveDateTime>,
    /// In Spam: shown, but never offered.
    pub spam: bool,
}

/// What unsubscribing from a message needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub method: Method,
    /// The list's List-Id, else the sender's address, lowercase.
    pub list: String,
    pub message: MessageId,
}

/// The addresses of a List-Unsubscribe header, in order: what is inside each `<…>`, without
/// the spaces a folded line leaves; comments `(…)` between them are skipped. A header that
/// forgot the angle brackets is read as comma-separated addresses.
pub fn urls(header: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut depth, mut inside, mut current) = (0usize, false, String::new());
    for ch in header.chars() {
        if inside {
            if ch == '>' {
                inside = false;
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            } else if !ch.is_whitespace() {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '<' if depth == 0 => {
                inside = true;
                current.clear();
            }
            _ => {}
        }
    }
    if out.is_empty() && !header.contains('<') {
        out = header
            .split(',')
            .map(|s| s.split_whitespace().collect::<String>())
            .filter(|s| scheme(s).is_some())
            .collect();
    }
    out
}

/// True when List-Unsubscribe-Post asks for one click (RFC 8058).
pub fn one_click(post: Option<&str>) -> bool {
    post.is_some_and(|p| {
        p.split_whitespace()
            .collect::<String>()
            .eq_ignore_ascii_case("List-Unsubscribe=One-Click")
    })
}

/// The best way out of the list: one click over https when the list offers it, else a mail,
/// else a web page (https before http). `None` when the header has nothing Telex can use.
pub fn choose(header: &str, post: Option<&str>) -> Option<Method> {
    let found = urls(header);
    let of = |want: &str| found.iter().find(|u| scheme(u) == Some(want)).cloned();
    if one_click(post) {
        if let Some(url) = of("https").filter(|u| crate::html::allowed_url(u)) {
            return Some(Method::OneClick(url));
        }
    }
    if let Some(mail) = found.iter().find_map(|u| mailto(u)) {
        return Some(mail);
    }
    of("https")
        .or_else(|| of("http"))
        .filter(|u| crate::html::allowed_url(u))
        .map(Method::Link)
}

/// The way out of the list a message offers, from its header.
pub fn method_of(raw: &[u8]) -> Option<Method> {
    let p = MessageParser::default().parse_headers(raw)?;
    let header = p.header_raw("List-Unsubscribe")?;
    choose(&unfold(header), p.header_raw("List-Unsubscribe-Post"))
}

/// The list a message came from: its List-Id (`Name <id>` → id), else the sender's address.
pub fn list_key(raw: Option<&[u8]>, from_email: &str) -> String {
    let id = raw
        .and_then(|raw| MessageParser::default().parse_headers(raw))
        .and_then(|p| {
            let header = unfold(p.header_raw("List-Id")?);
            let id = match (header.rfind('<'), header.rfind('>')) {
                (Some(start), Some(end)) if start < end => header[start + 1..end].to_owned(),
                _ => header,
            };
            Some(id.trim().to_lowercase()).filter(|id| !id.is_empty())
        });
    id.unwrap_or_else(|| from_email.trim().to_lowercase())
}

/// Ask the list's server to unsubscribe (RFC 8058): a POST of `List-Unsubscribe=One-Click`,
/// nothing else. Only https, and only to public addresses (tests may allow this computer).
/// A redirect isn't followed: the RFC forbids them, and it would not be one click.
pub fn post(url: &str, allow_private: bool) -> Result<(), String> {
    let server = host(url);
    let https = scheme(url) == Some("https");
    if !crate::html::allowed_url(url) || !(https || allow_private) {
        return Err(tr!(
            "Telex unsubscribes with one click only over https.",
            "Telex wypisuje jednym kliknięciem tylko przez https."
        ));
    }
    let agent = crate::html::public_agent(TIMEOUT, allow_private);
    let response = agent
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send("List-Unsubscribe=One-Click")
        .map_err(|e| match e {
            ureq::Error::HostNotFound => tr!(
                "{server} can't be found, or it points into your own network",
                "nie można znaleźć {server} albo adres prowadzi do Twojej sieci lokalnej"
            ),
            ureq::Error::Timeout(_) => tr!(
                "{server} didn't answer in time",
                "{server} nie odpowiedział na czas"
            ),
            e => tr!(
                "couldn't reach {server}: {e}",
                "brak połączenia z {server}: {e}"
            ),
        })?;
    let status = response.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(tr!(
            "{server} answered {status} instead of taking the request",
            "{server} odpowiedział kodem {status} zamiast przyjąć prośbę"
        ))
    }
}

/// The mail that asks a list to unsubscribe, from the account and identity the list writes to.
pub fn draft(account: &str, identity: Option<i64>, method: &Method) -> Option<OutgoingDraft> {
    let Method::Mail { to, subject, body } = method else {
        return None;
    };
    Some(OutgoingDraft {
        account: account.to_owned(),
        identity,
        to: to.clone(),
        subject: subject.clone(),
        body: body.clone(),
        ..OutgoingDraft::default()
    })
}

/// `mailto:a@x.pl?subject=unsubscribe&body=…` (RFC 6068). Without a subject or text, both
/// say "unsubscribe", which list servers understand.
fn mailto(url: &str) -> Option<Method> {
    if scheme(url) != Some("mailto") {
        return None;
    }
    let rest = &url["mailto:".len()..];
    let (to, query) = rest.split_once('?').unwrap_or((rest, ""));
    let to = decoded(to);
    let (mut subject, mut body) = (String::new(), String::new());
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        match name.to_ascii_lowercase().as_str() {
            "subject" => subject = decoded(value),
            "body" => body = decoded(value),
            _ => {}
        }
    }
    let one_line = |s: &str| !s.contains(['\r', '\n']);
    let address = to.split(',').next().unwrap_or_default().trim().to_owned();
    if !address.contains('@') || !one_line(&address) || !one_line(&subject) {
        return None;
    }
    let fill = |s: String| {
        if s.trim().is_empty() {
            "unsubscribe".to_owned()
        } else {
            s
        }
    };
    Some(Method::Mail {
        to: address,
        subject: fill(subject),
        body: fill(body),
    })
}

/// The lowercase scheme of an address: https, http, mailto.
fn scheme(url: &str) -> Option<&'static str> {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    ["https:", "http:", "mailto:"]
        .into_iter()
        .find(|s| lower.starts_with(s))
        .map(|s| s.trim_end_matches(':'))
}

/// The server's name in a web address.
fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    // Without the port: "x.pl:8443", "[::1]:8443"; an IPv6 address alone keeps its colons.
    let host = match host.rsplit_once(':') {
        Some((name, port))
            if port.chars().all(|c| c.is_ascii_digit())
                && (!name.contains(':') || name.ends_with(']')) =>
        {
            name
        }
        _ => host,
    };
    host.to_lowercase()
}

/// %-escapes decoded as UTF-8 (a broken one stays as it is).
fn decoded(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unfold(header: &str) -> String {
    header.replace(['\r', '\n'], "")
}

impl MailStore {
    /// The Unsubscribe button for a message (`raw`: its header, when Telex has it), or `None`
    /// when it isn't from a list, or is the user's own mail.
    pub fn unsubscribe_offer(
        &self,
        message: &Message,
        raw: Option<&[u8]>,
    ) -> Result<Option<Offer>, StoreError> {
        let kind = self.kind_of(&message.folder);
        if kind.is_some_and(FolderKind::is_outgoing) {
            return Ok(None);
        }
        let Some(method) = raw.and_then(method_of) else {
            return Ok(None);
        };
        let done = self.unsubscribed_at(&list_key(raw, &message.from_email))?;
        Ok(Some(Offer {
            method: method.code(),
            target: method.target(),
            done,
            spam: kind == Some(FolderKind::Spam),
        }))
    }

    /// How to leave the list of a message, or why Telex won't: it is in Spam, it is the
    /// user's own, or it says no way out.
    pub fn unsubscribe_plan(&self, id: MessageId) -> Result<Plan, StoreError> {
        let message = self.get(id)?;
        match self.kind_of(&message.folder) {
            Some(FolderKind::Spam) => {
                return Err(StoreError::Invalid(tr!(
                    "Telex doesn't unsubscribe from mail in Spam: that would tell the sender your address is read. Delete it instead.",
                    "Telex nie wypisuje z poczty w Spamie: nadawca dowiedziałby się, że ktoś czyta ten adres. Lepiej ją usuń."
                )))
            }
            Some(kind) if kind.is_outgoing() => {
                return Err(StoreError::Invalid(tr!(
                    "This is your own message.",
                    "To Twoja własna wiadomość."
                )))
            }
            _ => {}
        }
        let raw = self.raw_of(id)?;
        let method = raw.as_deref().and_then(method_of).ok_or_else(|| {
            StoreError::Invalid(tr!(
                "This message doesn't say how to unsubscribe.",
                "Ta wiadomość nie podaje, jak się wypisać."
            ))
        })?;
        Ok(Plan {
            list: list_key(raw.as_deref(), &message.from_email),
            method,
            message: id,
        })
    }

    /// The account and identity a mail to the list goes from: the account the message is in,
    /// or for mail on this PC the one it was sent to.
    pub fn unsubscribe_sender(&self, id: MessageId) -> Result<(String, i64), StoreError> {
        let message = self.get(id)?;
        let account = crate::drafts::sender(self, Some(&message), LOCAL_ID)?;
        let identity = crate::drafts::identity_for(account, &message);
        Ok((account.id.clone(), identity.id))
    }

    /// The user left the list: every message of it says so from now on, and this one gets a
    /// note saying how and when.
    pub fn record_unsubscribe(&mut self, plan: &Plan, at: NaiveDateTime) -> Result<(), StoreError> {
        let when = at.format("%d.%m.%Y");
        let note = match &plan.method {
            Method::OneClick(_) => tr!(
                "Unsubscribed with one click on {when}.",
                "Wypisano jednym kliknięciem {when}."
            ),
            Method::Mail { to, .. } => tr!(
                "Unsubscribed by a mail to {to} on {when}.",
                "Wypisano mailem do {to} {when}."
            ),
            Method::Link(url) => {
                let server = host(url);
                tr!(
                    "The unsubscribe page on {server} was opened on {when}.",
                    "Otwarto stronę wypisania na {server} {when}."
                )
            }
        };
        self.db.execute(
            "INSERT INTO unsubscribed (list, at, method, message) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (list) DO UPDATE SET at = ?2, method = ?3, message = ?4",
            params![plan.list, at, plan.method.code(), plan.message as i64],
        )?;
        self.add_note(plan.message, &note)
    }

    fn unsubscribed_at(&self, list: &str) -> Result<Option<NaiveDateTime>, StoreError> {
        Ok(self
            .db
            .prepare_cached("SELECT at FROM unsubscribed WHERE list = ?1")?
            .query_row([list], |r| r.get(0))
            .optional()?)
    }
}

#[cfg(test)]
mod tests;
