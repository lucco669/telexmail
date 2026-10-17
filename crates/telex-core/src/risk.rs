//! Bounded, offline checks for impersonation and misleading links in received mail.
//!
//! A score is the strength of warning signs, NOT a calibrated probability. No result
//! verifies identity or certifies safety. Never trust attacker-supplied Authentication-
//! Results/X-Spam headers, fetch a URL, resolve DNS, or move mail from this module.

use std::cell::RefCell;
use std::collections::HashSet;

use html5ever::tokenizer::{BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer};
use mail_parser::{Address, MessageParser, PartType};
use serde::Serialize;
use url::Url;

use crate::model::Message;

pub(crate) const MAX_RAW: usize = 2 * 1024 * 1024;
const MAX_TEXT: usize = 128 * 1024;
const MAX_HTML: usize = 256 * 1024;
const MAX_LINKS: usize = 512;
const MAX_URL: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InspectedMessage {
    #[serde(flatten)]
    pub message: Message,
    pub risk: Report,
    /// In an Outbox: when it goes (sent later, or waiting out its moment to undo).
    pub goes: Option<chrono::NaiveDateTime>,
    /// From a mailing list: how to leave it (`unsubscribe.rs`).
    pub unsubscribe: Option<crate::unsubscribe::Offer>,
    /// A calendar invitation in it, for the card above the text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite: Option<crate::calendar::invites::Invite>,
    /// The addresses of From, To and Cc that are in the address book, with the saved names.
    pub contacts: Vec<crate::contacts::ContactName>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Heuristic points, 0..99. Deliberately not labelled as a percentage.
    pub score: u8,
    pub signals: Vec<Signal>,
    /// Some input was unavailable or exceeded the work limits.
    pub limited: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    SenderBrandMismatch,
    SenderLookalike,
    LinkLookalike,
    MisleadingUserinfo,
    LinkTextMismatch,
    HiddenCharacters,
    OffsiteAccountAction,
    ReplyToMismatch,
    SensitiveRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Signal {
    pub kind: Kind,
    pub points: u8,
    /// The brand, visible domain or other claim being contradicted.
    pub claimed: String,
    /// The actual domain (never a private token or URL query).
    pub actual: String,
}

impl Kind {
    fn group(self) -> u8 {
        match self {
            Self::SenderBrandMismatch | Self::SenderLookalike => 0,
            Self::LinkLookalike | Self::MisleadingUserinfo | Self::LinkTextMismatch => 1,
            Self::HiddenCharacters => 2,
            Self::OffsiteAccountAction => 3,
            Self::ReplyToMismatch => 4,
            Self::SensitiveRequest => 5,
        }
    }
}

impl Report {
    fn add(&mut self, kind: Kind, points: u8, claimed: &str, actual: &str) {
        // Repetition and correlated signs within one category never inflate the score.
        let signal = Signal {
            kind,
            points,
            claimed: claimed.into(),
            actual: actual.into(),
        };
        if let Some(old) = self
            .signals
            .iter_mut()
            .find(|s| s.kind.group() == kind.group())
        {
            if old.points < points {
                *old = signal;
            }
        } else {
            self.signals.push(signal);
        }
    }

    fn finish(&mut self) {
        // A deceptive link already explains why an offsite action is suspicious.
        if self.signals.iter().any(|s| s.kind.group() == 1) {
            self.signals
                .retain(|s| s.kind != Kind::OffsiteAccountAction);
        }
        self.score = self
            .signals
            .iter()
            .map(|s| u16::from(s.points))
            .sum::<u16>()
            .min(99) as u8;
    }
}

struct Brand {
    name: &'static str,
    words: &'static [&'static str],
    domains: &'static [&'static str],
}

// A small, reviewable starting catalogue, not an exhaustive list of genuine senders.
// References and limitations are in docs/mail-risk.md. Domain suffixes need a dot boundary.
const BRANDS: &[Brand] = &[
    Brand {
        name: "Spotify",
        words: &["spotify"],
        domains: &["spotify.com", "spotify.link"],
    },
    Brand {
        name: "Netflix",
        words: &["netflix"],
        domains: &["netflix.com", "netflix.net", "netflix.shop"],
    },
    Brand {
        name: "mBank",
        words: &["mbank"],
        domains: &["mbank.pl"],
    },
    Brand {
        name: "PKO Bank Polski",
        words: &["pko", "ipko"],
        domains: &["pkobp.pl", "ipko.pl", "inteligo.pl", "ipko.net.pl"],
    },
];

fn belongs(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn official(brand: &Brand, host: &str) -> bool {
    brand.domains.iter().any(|domain| belongs(host, domain))
}

fn site(host: &str) -> &str {
    psl::domain_str(host).unwrap_or(host)
}

fn web_url(value: &str) -> Option<Url> {
    if value.len() > MAX_URL {
        return None;
    }
    let url = Url::parse(value.trim()).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(url)
}

fn email_host(email: &str) -> Option<String> {
    if email.len() > 512 {
        return None;
    }
    let (_, host) = email.trim().rsplit_once('@')?;
    let url = Url::parse(&format!("https://{host}")).ok()?;
    (url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.username().is_empty())
    .then(|| url.host_str().map(|h| h.trim_end_matches('.').to_owned()))
    .flatten()
}

fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{00ad}' | '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
}

// Common Latin-looking Cyrillic/Greek characters and Polish accents. This intentionally
// makes no claim to implement the complete Unicode confusable algorithm.
fn skeleton(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'а' | 'α' | 'ą' => 'a',
            'с' => 'c',
            'е' | 'ε' | 'ę' => 'e',
            'і' | 'ι' | 'ı' => 'i',
            'о' | 'ο' | 'ó' => 'o',
            'р' | 'ρ' => 'p',
            'ѕ' | 'ś' => 's',
            'т' | 'τ' => 't',
            'х' | 'χ' => 'x',
            'у' => 'y',
            'к' | 'κ' => 'k',
            'м' => 'm',
            'ν' => 'v',
            'ł' => 'l',
            'ń' => 'n',
            'ż' | 'ź' => 'z',
            _ => c,
        })
        .collect()
}

fn resembles(word: &str, known: &str) -> bool {
    if word == known {
        return true;
    }
    // Short bank acronyms are exact matches only. Bound all fuzzy comparisons.
    if known.len() < 6 || word.len().abs_diff(known.len()) > 1 || word.len() > 24 {
        return false;
    }
    let a: Vec<_> = word.chars().collect();
    let b: Vec<_> = known.chars().collect();
    if a.len() == b.len() {
        let different: Vec<_> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
        return different.len() == 1
            || (different.len() == 2
                && different[1] == different[0] + 1
                && a[different[0]] == b[different[1]]
                && a[different[1]] == b[different[0]]);
    }
    let (short, long) = if a.len() < b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    let mut i = 0;
    while i < short.len() && short[i] == long[i] {
        i += 1;
    }
    short[i..] == long[i + 1..]
}

fn claimed_brand(name: &str) -> Option<&'static Brand> {
    let normalized = skeleton(name);
    BRANDS.iter().find(|brand| {
        normalized
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| brand.words.iter().any(|known| resembles(word, known)))
    })
}

fn lookalike_brand(host: &str) -> Option<&'static Brand> {
    let unicode = idna::domain_to_unicode(host).0;
    let normalized = skeleton(&unicode);
    BRANDS.iter().find(|brand| {
        !official(brand, host)
            && normalized
                .split(['.', '-'])
                .any(|word| brand.words.iter().any(|known| resembles(word, known)))
    })
}

fn sensitive(value: &str) -> bool {
    let value = skeleton(value);
    [
        "update payment",
        "payment details",
        "billing details",
        "verify your account",
        "verify account",
        "confirm your account",
        "confirm account",
        "confirm your password",
        "enter your password",
        "sign in",
        "log in",
        "login",
        "account suspended",
        "account is suspended",
        "subscription is on hold",
        "update your card",
        "card details",
        "potwierdz haslo",
        "zweryfikuj konto",
        "potwierdz konto",
        "zaktualizuj dane",
        "zaloguj sie",
        "dane karty",
        "konto zostanie zablokowane",
        "zaktualizuj karte",
        "zaktualizuj metode platnosci",
        "potwierdz dane",
        "potwierdz tozsamosc",
        "weryfikacja konta",
        "zaktualizuj platnosc",
        "dane platnosci",
    ]
    .iter()
    .any(|phrase| value.contains(phrase))
}

fn account_action_points(brand: &Brand, label: &str) -> u8 {
    let label = skeleton(label);
    // Direct credential requests on another site are stronger than ordinary payment
    // provider links. Netflix also explicitly says it never asks for third-party payment.
    if brand.name == "Netflix"
        || [
            "login",
            "log in",
            "sign in",
            "password",
            "confirm account",
            "verify account",
            "verify your account",
            "haslo",
            "zaloguj",
            "potwierdz konto",
            "zweryfikuj konto",
        ]
        .iter()
        .any(|word| label.contains(word))
    {
        50
    } else {
        30
    }
}

fn prefix(value: &str, max: usize) -> (&str, bool) {
    if value.len() <= max {
        return (value, false);
    }
    let mut end = max;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (&value[..end], true)
}

#[derive(Default)]
struct Link {
    href: String,
    label: String,
}

#[derive(Default)]
struct HtmlLinks {
    links: Vec<Link>,
    active: Option<Link>,
    quote: usize,
    ignored: usize,
    limited: bool,
}

struct LinkSink(RefCell<HtmlLinks>);

impl HtmlLinks {
    fn close(&mut self) {
        if let Some(link) = self.active.take() {
            if self.links.len() < MAX_LINKS {
                self.links.push(link);
            } else {
                self.limited = true;
            }
        }
    }
}

impl TokenSink for LinkSink {
    type Handle = ();
    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        let mut found = self.0.borrow_mut();
        match token {
            Token::TagToken(tag) => {
                let name: &str = tag.name.as_ref();
                let start = tag.kind == TagKind::StartTag;
                if matches!(name, "script" | "style" | "textarea" | "title") {
                    found.close();
                    if start {
                        found.ignored += 1;
                        let kind = if name == "script" {
                            html5ever::tokenizer::states::RawKind::ScriptData
                        } else if matches!(name, "textarea" | "title") {
                            html5ever::tokenizer::states::RawKind::Rcdata
                        } else {
                            html5ever::tokenizer::states::RawKind::Rawtext
                        };
                        return TokenSinkResult::RawData(kind);
                    }
                    found.ignored = found.ignored.saturating_sub(1);
                }
                if name == "blockquote" {
                    found.close();
                    if start {
                        found.quote += 1;
                    } else {
                        found.quote = found.quote.saturating_sub(1);
                    }
                }
                if name == "a" && found.quote == 0 && found.ignored == 0 {
                    found.close();
                    if start {
                        if let Some(href) = tag.attrs.iter().find(|a| &*a.name.local == "href") {
                            if href.value.len() <= MAX_URL {
                                found.active = Some(Link {
                                    href: href.value.to_string(),
                                    label: String::new(),
                                });
                            } else {
                                found.limited = true;
                            }
                        }
                    }
                }
                if name == "img" && start {
                    let mut cut = false;
                    if let Some(link) = &mut found.active {
                        if let Some(alt) = tag.attrs.iter().find(|a| &*a.name.local == "alt") {
                            let (text, truncated) =
                                prefix(&alt.value, 1024usize.saturating_sub(link.label.len()));
                            link.label.push_str(text);
                            cut = truncated;
                        }
                    }
                    found.limited |= cut;
                }
            }
            Token::CharacterTokens(text) => {
                let mut cut = false;
                if let Some(link) = &mut found.active {
                    let (text, truncated) =
                        prefix(&text, 1024usize.saturating_sub(link.label.len()));
                    link.label.push_str(text);
                    cut = truncated;
                }
                found.limited |= cut;
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

fn html_links(html: &str) -> (Vec<Link>, bool) {
    let (html, cut) = prefix(html, MAX_HTML);
    let input = BufferQueue::default();
    input.push_back(html.into());
    let tokenizer = Tokenizer::new(
        LinkSink(RefCell::new(HtmlLinks::default())),
        Default::default(),
    );
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let mut found = tokenizer.sink.0.into_inner();
    found.close();
    (found.links, cut || found.limited)
}

// Recognize only bounded, documented redirect formats. Inspect the embedded URL;
// neither the wrapper nor the embedded destination is visited or considered trustworthy.
fn destination(url: Url) -> Url {
    let host = url.host_str().unwrap_or("");
    let key = if belongs(host, "safelinks.protection.outlook.com") {
        "url"
    } else if belongs(host, "google.com") && url.path() == "/url" {
        "q"
    } else {
        return url;
    };
    url.query_pairs()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| web_url(&value))
        .unwrap_or(url)
}

fn inspect_link(report: &mut Report, link: &Link, claim: Option<&Brand>) {
    let Some(url) = web_url(&link.href) else {
        return;
    };
    let url = destination(url);
    let host = url.host_str().unwrap_or("").trim_end_matches('.');
    if link.href.chars().any(hidden) {
        report.add(Kind::HiddenCharacters, 40, "", host);
    }
    if !url.username().is_empty() {
        let apparent = url.username().to_ascii_lowercase();
        let domain_like = psl::suffix(apparent.as_bytes()).is_some_and(|s| s.is_known());
        if domain_like && site(&apparent) != site(host) {
            report.add(Kind::MisleadingUserinfo, 65, &apparent, host);
        }
    }
    if let Some(brand) = lookalike_brand(host) {
        report.add(Kind::LinkLookalike, 50, brand.name, host);
    }
    let label = link.label.trim();
    let displayed = web_url(label).or_else(|| {
        if !label.contains(char::is_whitespace) && label.contains('.') {
            web_url(&format!("https://{label}"))
        } else {
            None
        }
    });
    if let Some(displayed) = displayed {
        let named = displayed.host_str().unwrap_or("").trim_end_matches('.');
        if site(named) != site(host) {
            report.add(Kind::LinkTextMismatch, 25, named, host);
        }
    }
    if let Some(brand) = claim {
        if !official(brand, host) && sensitive(label) {
            report.add(
                Kind::OffsiteAccountAction,
                account_action_points(brand, label),
                brand.name,
                host,
            );
        }
    }
}

/// Inspect one opened message. Bulk listing/searching does not call this function.
pub fn analyse(message: &Message, raw: Option<&[u8]>) -> Report {
    let mut report = Report {
        limited: raw.is_none(),
        ..Default::default()
    };
    let (name, name_cut) = prefix(&message.from_name, 512);
    let claim = claimed_brand(name);
    let (email, email_cut) = prefix(&message.from_email, 512);
    let sender = if email_cut {
        String::new()
    } else {
        email_host(email).unwrap_or_default()
    };
    report.limited |= name_cut || email_cut;
    if let Some(brand) = claim {
        if !sender.is_empty() && !official(brand, &sender) {
            report.add(Kind::SenderBrandMismatch, 25, brand.name, &sender);
        }
    }
    if let Some(brand) = lookalike_brand(&sender) {
        report.add(Kind::SenderLookalike, 50, brand.name, &sender);
    }
    if name.chars().any(hidden) || email.chars().any(hidden) {
        report.add(Kind::HiddenCharacters, 40, "", &sender);
    }
    let (subject, subject_cut) = prefix(&message.subject, 1024);
    let (body, body_cut) = prefix(&message.body, MAX_TEXT);
    report.limited |= subject_cut || body_cut;
    let unquoted = body
        .lines()
        .filter(|line| !line.trim_start().starts_with('>'))
        .collect::<Vec<_>>()
        .join("\n");
    // A URL's /login path is not itself a request to log in.
    let prose = unquoted
        .split_whitespace()
        .filter(|word| {
            let word = word.trim_start_matches(['<', '>', '"', '\'', '(']);
            !word.starts_with("https://") && !word.starts_with("http://")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let solicits = sensitive(subject) || sensitive(&prose);
    let mut links = Vec::new();
    let mut reply = None;
    let mut scan_stored_text = true;
    if let Some(raw) = raw {
        if raw.len() > MAX_RAW {
            report.limited = true;
        } else if let Some(parsed) = MessageParser::default().parse(raw) {
            reply = parsed
                .reply_to()
                .and_then(Address::first)
                .and_then(|a| a.address())
                .and_then(email_host);
            for part in parsed.html_bodies() {
                if let PartType::Html(html) = &part.body {
                    scan_stored_text = false; // generated HTML-to-text must not reintroduce quoted links
                    let (found, limited) = html_links(html);
                    report.limited |= limited;
                    links.extend(found);
                    if links.len() >= MAX_LINKS {
                        report.limited = true;
                        break;
                    }
                }
            }
            if !scan_stored_text {
                for part in parsed.text_bodies() {
                    if let PartType::Text(text) = &part.body {
                        add_plain_links(text, &mut links, &mut report.limited);
                    }
                }
            }
        } else {
            report.limited = true;
        }
    }
    if scan_stored_text {
        add_plain_links(&unquoted, &mut links, &mut report.limited);
    }
    let mut seen = HashSet::new();
    for link in links.iter().take(MAX_LINKS) {
        if seen.insert((&link.href, &link.label)) {
            inspect_link(&mut report, link, claim);
        }
    }
    let solicits = solicits
        || links
            .iter()
            .take(MAX_LINKS)
            .any(|link| web_url(&link.label).is_none() && sensitive(&link.label));
    if !report.signals.is_empty() && solicits {
        if let Some(brand) = claim.filter(|brand| !official(brand, &sender)) {
            if let Some(target) = links
                .iter()
                .take(MAX_LINKS)
                .filter_map(|link| web_url(&link.href))
                .map(destination)
                .find(|url| !official(brand, url.host_str().unwrap_or("")))
            {
                report.add(
                    Kind::OffsiteAccountAction,
                    30,
                    brand.name,
                    target.host_str().unwrap_or(""),
                );
            }
        }
        report.add(
            Kind::SensitiveRequest,
            if claim.is_some() { 20 } else { 15 },
            "",
            "",
        );
        if let Some(reply) =
            reply.filter(|reply| !sender.is_empty() && site(reply) != site(&sender))
        {
            report.add(Kind::ReplyToMismatch, 15, &sender, &reply);
        }
    }
    report.finish();
    report
}

fn add_plain_links(text: &str, links: &mut Vec<Link>, limited: &mut bool) {
    let (text, cut) = prefix(text, MAX_TEXT);
    *limited |= cut;
    for word in text
        .lines()
        .filter(|line| !line.trim_start().starts_with('>'))
        .flat_map(str::split_whitespace)
    {
        let href = word.trim_matches(['<', '>', '"', '\'', '(', ')', ',', ';', '.']);
        if href.to_ascii_lowercase().starts_with("https://")
            || href.to_ascii_lowercase().starts_with("http://")
        {
            if links.len() >= MAX_LINKS {
                *limited = true;
                break;
            }
            if href.len() > MAX_URL {
                *limited = true;
                continue;
            }
            links.push(Link {
                href: href.into(),
                label: String::new(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::parse_message;

    fn inspect(from: &str, subject: &str, body: &str, html: bool, extra: &str) -> Report {
        let content = if html { "text/html" } else { "text/plain" };
        let raw = format!("From: {from}\r\nTo: user@example.com\r\nSubject: {subject}\r\n{extra}MIME-Version: 1.0\r\nContent-Type: {content}; charset=utf-8\r\n\r\n{body}");
        let message = parse_message(raw.as_bytes(), "account/inbox", None);
        analyse(&message, Some(raw.as_bytes()))
    }

    #[test]
    fn spotify_typo_and_disguised_payment_button_are_high_risk() {
        let report = inspect("Sporify Billing <billing@music-account.example>", "Update payment details",
            "<a href='https://spotify.com@billing.example/renew?private=token'>Update payment details</a>", true, "");
        assert!(report.score >= 70);
        assert!(report
            .signals
            .iter()
            .any(|s| s.kind == Kind::MisleadingUserinfo && s.actual == "billing.example"));
        assert!(!report.limited);
        assert!(!serde_json::to_string(&report)
            .unwrap()
            .contains("private=token"));
    }

    #[test]
    fn netflix_plain_text_and_polish_bank_impersonations_are_high_risk() {
        for (from, subject, body) in [
            (
                "Netflix <billing@verify-account.example>",
                "Confirm your account",
                "Confirm your password at https://verify-account.example/login",
            ),
            (
                "mBank <support@secure-mbank.example>",
                "Potwierdź konto",
                "Zaktualizuj dane karty: https://secure-mbank.example/login",
            ),
            (
                "PKO Bank Polski <admin@account-check.example>",
                "Zaloguj się",
                "Potwierdź konto: https://account-check.example/verify",
            ),
        ] {
            let report = inspect(from, subject, body, false, "");
            assert!(report.score >= 70, "{from}: {report:?}");
        }
    }

    #[test]
    fn official_senders_and_subdomains_have_no_warning_signs() {
        for (from, href) in [
            (
                "Spotify <support@spotify.com>",
                "https://accounts.spotify.com/login",
            ),
            (
                "Netflix <info@account.netflix.com>",
                "https://www.netflix.com/YourAccount",
            ),
            ("mBank <noreply@mbank.pl>", "https://online.mbank.pl/login"),
            (
                "Netflix Shop <orders@netflix.shop>",
                "https://netflix.shop/account",
            ),
            (
                "Netflix <partners@netflix.net>",
                "https://www.netflix.net/account",
            ),
            (
                "PKO Bank Polski <noreply@pkobp.pl>",
                "https://www.ipko.pl/login",
            ),
        ] {
            let html = format!("<a href='{href}'>Update payment details</a>");
            assert_eq!(
                inspect(from, "Your account", &html, true, "").score,
                0,
                "{from}"
            );
        }
    }

    #[test]
    fn forged_official_from_does_not_hide_an_offsite_account_request() {
        for (from, label) in [
            ("Netflix <info@netflix.com>", "Update payment details"),
            ("PKO Bank Polski <info@pkobp.pl>", "Zaloguj się"),
        ] {
            let html = format!("<a href='https://account-check.example'>{label}</a>");
            assert!(inspect(from, label, &html, true, "").score >= 70);
        }
    }

    #[test]
    fn image_button_alt_text_is_checked_without_loading_the_image() {
        let html = "<a href='https://account-check.example'><img src='https://tracker.example/pixel' alt='Update payment details'></a>";
        let report = inspect("Netflix <info@netflix.com>", "Notice", html, true, "");
        assert!(report.score >= 70);
        assert!(report
            .signals
            .iter()
            .any(|s| s.kind == Kind::OffsiteAccountAction && s.actual == "account-check.example"));
        assert!(!report.signals.iter().any(|s| s.actual == "tracker.example"));
    }

    #[test]
    fn punycode_and_suffix_tricks_cannot_hide_a_lookalike() {
        for href in [
            "https://spоtify.com/pay",
            "https://xn--sptify-xqf.com/pay",
            "https://spotify.com.evil.example/pay",
        ] {
            let html = format!("<a href='{href}'>Update payment details</a>");
            let report = inspect(
                "Spotify <billing@music-account.example>",
                "Update payment details",
                &html,
                true,
                "",
            );
            assert!(report.score >= 70, "{href}: {report:?}");
        }
        assert_eq!(
            inspect(
                "Żółw <hello@żółw.pl>",
                "News",
                "https://żółw.pl/news",
                false,
                ""
            )
            .score,
            0
        );
    }

    #[test]
    fn safe_links_are_unwrapped_locally_without_blanket_trust() {
        let benign = "<a href='https://eur01.safelinks.protection.outlook.com/?url=https%3A%2F%2Fwww.netflix.com%2Faccount'>https://www.netflix.com/account</a>";
        assert_eq!(
            inspect("Netflix <info@netflix.com>", "Account", benign, true, "").score,
            0
        );
        let malicious = benign.replace(
            "www.netflix.com%2Faccount",
            "spotify.com%40billing.example%2Faccount",
        );
        let report = inspect(
            "Sporify <info@other.example>",
            "Confirm your account",
            &malicious,
            true,
            "",
        );
        assert!(report.score >= 70);
        assert!(report.signals.iter().any(|s| s.actual == "billing.example"));
    }

    #[test]
    fn marketing_tracking_and_payment_language_alone_do_not_get_high_scores() {
        assert_eq!(inspect("Frontend Weekly <news@weekly.example>", "Netflix and Spotify news",
            "<a href='https://stripe.com/billing'>Update payment details</a><a href='https://news.example/article'>Read our Netflix review</a>", true, "Reply-To: editor@other.example\r\n").score, 0);
        let score = inspect(
            "Spotify <info@spotify.com>",
            "Weekly news",
            "<a href='https://click.mailchimp.com/opaque'>https://spotify.com/news</a>",
            true,
            "",
        )
        .score;
        assert!(score > 0 && score < 40);
    }

    #[test]
    fn domain_comparisons_handle_multi_part_suffixes_and_hosting_tenants() {
        let same = "<a href='https://click.shop.co.uk/track'>https://www.shop.co.uk/account</a>";
        assert_eq!(
            inspect("Shop <hello@shop.co.uk>", "News", same, true, "").score,
            0
        );
        let other = "<a href='https://evil.co.uk/track'>https://shop.co.uk/account</a>";
        assert_eq!(
            inspect("Shop <hello@shop.co.uk>", "News", other, true, "").score,
            25
        );
        let tenants = "<a href='https://attacker.github.io'>https://company.github.io</a>";
        assert_eq!(
            inspect("Company <hello@company.example>", "News", tenants, true, "").score,
            25
        );
    }

    #[test]
    fn spoofed_authentication_results_cannot_lower_a_score() {
        let body = "<a href='https://netflix.com@billing.example/pay'>Update payment details</a>";
        let baseline = inspect(
            "Netflix <billing@verify.example>",
            "Update payment details",
            body,
            true,
            "",
        );
        let forged = inspect("Netflix <billing@verify.example>", "Update payment details", body, true,
            "Authentication-Results: mx.example.com; dmarc=pass; spf=pass; dkim=pass\r\nX-Spam-Status: No\r\n");
        assert_eq!(baseline, forged);
    }

    #[test]
    fn reply_to_mismatch_needs_other_signs_and_a_sensitive_request() {
        assert_eq!(
            inspect(
                "Anna <anna@studio.example>",
                "Invoice",
                "Please reply to our accounts team.",
                false,
                "Reply-To: accounts@accountant.example\r\n"
            )
            .score,
            0
        );
        let report = inspect(
            "Netflix <billing@netflix.com>",
            "Confirm your account",
            "<a href='https://verify.example'>Confirm account</a>",
            true,
            "Reply-To: support@elsewhere.example\r\n",
        );
        assert!(report
            .signals
            .iter()
            .any(|s| s.kind == Kind::ReplyToMismatch));
    }

    #[test]
    fn repetition_does_not_inflate_scores_and_html_entities_are_decoded() {
        let link = "<a href='https://spotify.com&#64;billing.example/pay?a=1&amp;b=2'>Update payment details</a>";
        let one = inspect("Spotify <billing@other.example>", "Account", link, true, "");
        let repeated = inspect(
            "Spotify <billing@other.example>",
            "Account",
            &link.repeat(100),
            true,
            "",
        );
        assert_eq!(one.score, repeated.score);
        assert!(one.signals.iter().any(|s| s.actual == "billing.example"));
    }

    #[test]
    fn quoted_reports_and_script_text_do_not_become_link_evidence() {
        let quoted = "<p>A scam I reported:</p><blockquote><a href='https://spotify.com@evil.example'>Update payment details</a></blockquote>";
        assert_eq!(
            inspect(
                "Anna <anna@studio.example>",
                "Reported scam",
                quoted,
                true,
                ""
            )
            .score,
            0
        );
        let scripts = "<script>const example = \"<a href='https://spotify.com@evil.example'>login</a>\";</script><p>Hello</p>";
        assert_eq!(
            inspect("Anna <anna@studio.example>", "News", scripts, true, "").score,
            0
        );
        assert_eq!(
            inspect(
                "Anna <anna@studio.example>",
                "Reported scam",
                "> https://spotify.com@evil.example",
                false,
                ""
            )
            .score,
            0
        );
    }

    #[test]
    fn missing_large_and_excessive_inputs_are_explicitly_incomplete() {
        let raw = b"From: Anna <anna@example.com>\r\nSubject: Hello\r\n\r\nHello";
        let mut message = parse_message(raw, "account/inbox", None);
        assert!(analyse(&message, None).limited);
        assert!(analyse(&message, Some(&vec![b'a'; MAX_RAW + 1])).limited);
        message.body = "ą".repeat(MAX_TEXT);
        assert!(analyse(&message, Some(raw)).limited);
        let many = "<a href='https://news.example/article'>Read</a>".repeat(MAX_LINKS + 1);
        assert!(inspect("News <hello@news.example>", "News", &many, true, "").limited);
    }

    #[test]
    fn hidden_characters_are_visible_signals_but_not_a_spam_verdict() {
        let report = inspect(
            "Anna <anna@example.com>",
            "News",
            "https://news.example/\u{202e}login",
            false,
            "",
        );
        assert_eq!(report.score, 40);
        assert_eq!(report.signals[0].kind, Kind::HiddenCharacters);
    }

    #[test]
    #[ignore = "synthetic timing measurement; not a correctness or latency threshold"]
    fn timing() {
        let raw = b"From: Spotify <billing@music-account.example>\r\nSubject: Update payment details\r\nContent-Type: text/html\r\n\r\n<a href='https://spotify.com@billing.example/pay'>Update payment details</a>";
        let message = parse_message(raw, "account/inbox", None);
        let mut micros = Vec::new();
        for _ in 0..2000 {
            let at = std::time::Instant::now();
            std::hint::black_box(analyse(&message, Some(raw)));
            micros.push(at.elapsed().as_micros());
        }
        micros.sort_unstable();
        println!(
            "Offline synthetic HTML check: median {} µs, p95 {} µs (2000 runs, current build)",
            micros[1000], micros[1900]
        );
    }
}
