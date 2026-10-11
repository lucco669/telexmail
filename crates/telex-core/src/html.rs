//! Showing HTML mail safely.
//!
//! [`render`] turns a message's HTML into a complete, inert document for a sandboxed frame.
//! ammonia removes scripts, forms, event handlers and everything else active. Images the
//! message carries itself (`cid:`) become data: URIs. Remote images and CSS backgrounds are
//! left out, unless their contents are passed in: [`fetch_images`] downloads them when the
//! user allows it, so the window itself never talks to the network. The document also
//! carries its own Content-Security-Policy that forbids any network load. On dark paper
//! the message's own colors are turned around to match (`colors`).
//!
//! [`compose_clean`], [`to_text`] and [`from_text`] serve HTML compose: what the editor
//! may send, and the plain-text version that goes along with it.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use ammonia::{Builder, UrlRelative};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use serde::Deserialize;
use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

use crate::colors::{self, Role, Target};
use crate::mime::html_body;

/// The paper a message is shown on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Paper {
    /// White, as the sender designed it.
    #[default]
    Light,
    /// A dark theme's paper, with the message's colors recolored to match.
    Dark(Palette),
}

/// The colors of the dark theme a message is shown in, as `#rrggbb`: the window sends its
/// theme's tokens. Anything that isn't a plain hex color is replaced by Telex's Dark theme,
/// since these go into the frame's style as they are.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Palette {
    /// The background: what white paper becomes.
    pub paper: String,
    /// The text: what black ink becomes.
    pub ink: String,
    pub link: String,
    /// Quotes: their bar and their text.
    pub line: String,
    pub muted: String,
}

impl Default for Palette {
    /// Telex's Dark theme.
    fn default() -> Self {
        Self {
            paper: "#14171b".into(),
            ink: "#e2e6eb".into(),
            link: "#6f9de6".into(),
            line: "#2e343c".into(),
            muted: "#9aa3ad".into(),
        }
    }
}

impl Palette {
    /// Every color a plain hex color, and dark paper with light ink; else Telex's Dark.
    fn checked(&self) -> (Palette, Target) {
        let fallback = Palette::default();
        let pick = |mine: &String, theirs: String| {
            if colors::is_plain_hex(mine) {
                mine.clone()
            } else {
                theirs
            }
        };
        let p = Palette {
            paper: pick(&self.paper, fallback.paper.clone()),
            ink: pick(&self.ink, fallback.ink.clone()),
            link: pick(&self.link, fallback.link.clone()),
            line: pick(&self.line, fallback.line.clone()),
            muted: pick(&self.muted, fallback.muted.clone()),
        };
        match Target::new(&p.paper, &p.ink) {
            Some(target) => (p, target),
            None => {
                let target = Target::new(&fallback.paper, &fallback.ink).expect("Dark is dark");
                (fallback, target)
            }
        }
    }
}

/// An HTML message ready for the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// A complete document for the frame's `srcdoc`.
    pub document: String,
    /// Every remote image the message references (images and CSS backgrounds), in order.
    pub remote: Vec<String>,
    /// How many of those are left out.
    pub blocked: usize,
}

/// Render a message's HTML body, or `None` for plain-text mail. `images` maps remote
/// image URLs to data: URIs; anything remote not in it is left out.
pub fn render(raw: &[u8], images: &HashMap<String, String>, paper: &Paper) -> Option<Rendered> {
    let body = html_body(raw)?;
    let inline = body
        .inline
        .iter()
        .filter(|(_, (mime, _))| mime.starts_with("image/"))
        .map(|(cid, (mime, data))| (cid.clone(), data_uri(mime, data)))
        .collect();
    let dark = match paper {
        Paper::Light => None,
        Paper::Dark(palette) => Some(palette.checked()),
    };
    let (html, remote) = sanitize(
        &body.html,
        inline,
        images.clone(),
        dark.as_ref().map(|d| d.1),
    );
    let blocked = remote.iter().filter(|u| !images.contains_key(*u)).count();
    Some(Rendered {
        document: wrap(&html, dark.as_ref().map(|d| &d.0)),
        remote,
        blocked,
    })
}

/// Where image references can be resolved: images in the message, and fetched ones. And
/// where colors go on dark paper (`None` on white).
struct Sources {
    inline: HashMap<String, String>,
    images: HashMap<String, String>,
    dark: Option<Target>,
}

/// Clean an HTML fragment. Returns the safe HTML and the remote image URLs it referenced.
fn sanitize(
    html: &str,
    inline: HashMap<String, String>,
    images: HashMap<String, String>,
    dark: Option<Target>,
) -> (String, Vec<String>) {
    let found = Arc::new(Mutex::new(Vec::new()));
    let sources = Sources {
        inline,
        images,
        dark,
    };
    let seen = Arc::clone(&found);
    let mut builder = Builder::default();
    builder
        .add_tags(&["font", "big", "address", "section", "main"])
        .add_generic_attributes(&[
            "style",
            "align",
            "valign",
            "width",
            "height",
            "bgcolor",
            "color",
            "border",
            "dir",
            "background",
        ])
        .add_tag_attributes("td", &["colspan", "rowspan", "nowrap"])
        .add_tag_attributes("th", &["colspan", "rowspan", "nowrap", "scope"])
        .add_tag_attributes("table", &["cellpadding", "cellspacing", "summary"])
        .add_tag_attributes("font", &["face", "size"])
        .add_tag_attributes("img", &["hspace", "vspace"])
        // cid: and data: must pass ammonia's scheme check to reach the filter below,
        // which allows each only where it is safe.
        .url_schemes(HashSet::from(["http", "https", "mailto", "cid", "data"]))
        .url_relative(UrlRelative::Deny)
        .link_rel(Some("noopener noreferrer"))
        .strip_comments(true)
        .attribute_filter(move |element, attribute, value| {
            filter(&sources, &seen, element, attribute, value)
        });
    let clean = builder.clean(html).to_string();
    let mut urls = found.lock().map(|f| f.clone()).unwrap_or_default();
    let mut unique = HashSet::new();
    urls.retain(|u| unique.insert(u.clone()));
    (clean, urls)
}

fn filter<'u>(
    sources: &Sources,
    found: &Mutex<Vec<String>>,
    element: &str,
    attribute: &str,
    value: &'u str,
) -> Option<Cow<'u, str>> {
    match (element, attribute) {
        ("img", "src") | (_, "background") => image_url(sources, found, value).map(Cow::Owned),
        ("a", "href") => {
            let lower = value.trim().to_ascii_lowercase();
            let ok = ["http://", "https://", "mailto:"]
                .iter()
                .any(|s| lower.starts_with(s));
            ok.then_some(Cow::Borrowed(value))
        }
        (_, "style") => {
            let style = clean_style(sources, found, value);
            Some(Cow::Owned(match &sources.dark {
                None => style,
                Some(to) => colors::recolor_style(&style, to),
            }))
        }
        (_, "bgcolor") => Some(match &sources.dark {
            None => Cow::Borrowed(value),
            Some(to) => Cow::Owned(colors::recolor_attribute(value, Role::Background, to)),
        }),
        (_, "color") => Some(match &sources.dark {
            None => Cow::Borrowed(value),
            Some(to) => Cow::Owned(colors::recolor_attribute(value, Role::Text, to)),
        }),
        _ => Some(Cow::Borrowed(value)),
    }
}

/// What an image reference becomes: the message's own image, a fetched one, or nothing.
fn image_url(sources: &Sources, found: &Mutex<Vec<String>>, value: &str) -> Option<String> {
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    if lower.starts_with("cid:") {
        let cid = &v[4..];
        return sources.inline.get(cid).cloned().or_else(|| {
            let wanted = cid.to_ascii_lowercase();
            sources
                .inline
                .iter()
                .find(|(k, _)| k.to_ascii_lowercase() == wanted)
                .map(|(_, data)| data.clone())
        });
    }
    if lower.starts_with("data:image/") {
        return Some(v.to_owned());
    }
    if lower.starts_with("http://") || lower.starts_with("https://") {
        if let Ok(mut f) = found.lock() {
            f.push(v.to_owned());
        }
        return sources.images.get(v).cloned();
    }
    None
}

/// Keep inline CSS, minus anything that loads from outside or runs code (old IE tricks).
fn clean_style(sources: &Sources, found: &Mutex<Vec<String>>, style: &str) -> String {
    let lower = style.to_ascii_lowercase();
    if [
        "expression(",
        "behavior:",
        "-moz-binding",
        "javascript:",
        "@import",
    ]
    .iter()
    .any(|bad| lower.contains(bad))
    {
        return String::new();
    }
    let mut out = String::new();
    let mut rest = style;
    // ASCII lowercasing keeps byte offsets, so positions found in `lower` work in `rest`.
    while let Some(i) = rest.to_ascii_lowercase().find("url(") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 4..];
        let Some(end) = after.find(')') else {
            return out; // an unclosed url( drops the rest
        };
        let inner = after[..end].trim().trim_matches(['"', '\'']);
        match image_url(sources, found, inner) {
            Some(data) => out.push_str(&format!("url(\"{data}\")")),
            None => out.push_str("none"),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The frame's own policy: nothing from the network, whatever slipped through.
const FRAME_CSP: &str =
    "default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:";

/// Mail is designed for white paper: the light frame, in the app's light colors.
const FRAME_STYLE: &str = "html{background:#fff;color-scheme:light}\
body{margin:0;padding:12px 14px 28px;font:14px/1.5 'Segoe UI',system-ui,sans-serif;color:#1a1e23;overflow-wrap:anywhere}\
img{max-width:100%;height:auto}table{max-width:100%}a{color:#2453a6}pre{white-space:pre-wrap}\
blockquote{margin:0 0 0 2px;padding-left:10px;border-left:2px solid #d2d7de;color:#59636e}";

/// The dark frame, in the theme's colors (already checked to be plain hex colors).
fn dark_style(p: &Palette) -> String {
    let Palette {
        paper,
        ink,
        link,
        line,
        muted,
    } = p;
    format!(
        "html{{background:{paper};color-scheme:dark}}\
body{{margin:0;padding:12px 14px 28px;font:14px/1.5 'Segoe UI',system-ui,sans-serif;color:{ink};overflow-wrap:anywhere}}\
img{{max-width:100%;height:auto}}table{{max-width:100%}}a{{color:{link}}}pre{{white-space:pre-wrap}}\
blockquote{{margin:0 0 0 2px;padding-left:10px;border-left:2px solid {line};color:{muted}}}"
    )
}

/// `dark`: the checked palette of dark paper, or `None` for white.
fn wrap(body: &str, dark: Option<&Palette>) -> String {
    let style = match dark {
        None => FRAME_STYLE.to_owned(),
        Some(p) => dark_style(p),
    };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{FRAME_CSP}\">\
         <meta name=\"referrer\" content=\"no-referrer\">\
         <style>{style}</style></head><body>{body}</body></html>"
    )
}

fn data_uri(mime: &str, data: &[u8]) -> String {
    let mime: String = mime
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '+' | '-' | '.'))
        .collect();
    format!("data:{mime};base64,{}", STANDARD.encode(data))
}

// ---- HTML compose ----

/// What a message written in Telex may contain: text formatting, links, lists, quotes.
/// No styles, classes, images or anything else pasted in from a web page.
const COMPOSE_TAGS: [&str; 22] = [
    "a",
    "b",
    "strong",
    "i",
    "em",
    "u",
    "s",
    "strike",
    "br",
    "p",
    "div",
    "span",
    "ul",
    "ol",
    "li",
    "blockquote",
    "pre",
    "code",
    "h1",
    "h2",
    "h3",
    "hr",
];

/// The editor's HTML, cleaned to what Telex sends. Also takes a whole document (a saved
/// draft) and keeps only its body's content.
pub fn compose_clean(html: &str) -> String {
    let mut builder = Builder::empty();
    builder
        .add_tags(COMPOSE_TAGS)
        .add_tag_attributes("a", &["href"])
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .url_relative(UrlRelative::Deny)
        .link_rel(None)
        .strip_comments(true)
        .clean_content_tags(HashSet::from(["script", "style", "head", "title"]));
    builder.clean(html).to_string().trim().to_owned()
}

/// The text/html part of a message written in Telex: the cleaned HTML in a small document,
/// quotes marked the way mail programs show them.
pub fn compose_document(clean: &str) -> String {
    let body = clean.replace(
        "<blockquote>",
        "<blockquote type=\"cite\" style=\"margin:0 0 0 .8ex;border-left:2px solid #ccc;padding-left:1ex\">",
    );
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"></head>\
         <body><div style=\"font-family:Segoe UI,Arial,sans-serif;font-size:14px\">{body}</div></body></html>"
    )
}

/// Plain text for HTML written in Telex: lines for blocks, "- " and "1. " for lists, ">"
/// for quotes, a link's address after its text. Takes cleaned HTML ([`compose_clean`]).
pub fn to_text(html: &str) -> String {
    let mut w = TextWriter::default();
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('<') {
            let end = tag_end(after);
            w.tag(&after[..end]);
            rest = after.get(end + 1..).unwrap_or_default();
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            w.text(&decode_entities(&rest[..end]));
            rest = &rest[end..];
        }
    }
    w.finish()
}

/// HTML for plain text: a line per `<div>`, blank lines kept, `>` quotes as blockquotes,
/// web addresses as links. How a reply or a plain draft opens in the HTML editor.
pub fn from_text(text: &str) -> String {
    let mut out = String::new();
    let mut open = 0;
    for line in text.lines() {
        // "> > text" is quoted twice; a space after each ">" belongs to the marker.
        let (mut depth, mut rest) = (0, line);
        while let Some(r) = rest.strip_prefix('>') {
            depth += 1;
            rest = r.strip_prefix(' ').unwrap_or(r);
        }
        let depth = depth.min(MAX_QUOTE_DEPTH);
        for _ in open..depth {
            out.push_str("<blockquote>");
        }
        for _ in depth..open {
            out.push_str("</blockquote>");
        }
        open = depth;
        if rest.trim().is_empty() {
            out.push_str("<div><br></div>");
        } else {
            out.push_str("<div>");
            out.push_str(&linkify(&escape(rest)));
            out.push_str("</div>");
        }
    }
    for _ in 0..open {
        out.push_str("</blockquote>");
    }
    out
}

/// Quotes deeper than this are shown at this depth, so a crafted reply can't nest without end.
const MAX_QUOTE_DEPTH: usize = 20;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Web addresses in escaped text become links.
fn linkify(escaped: &str) -> String {
    let mut out = String::new();
    let mut rest = escaped;
    while let Some(i) = ["https://", "http://"]
        .iter()
        .filter_map(|p| rest.find(p))
        .min()
    {
        out.push_str(&rest[..i]);
        let url_len = rest[i..]
            .find(|c: char| c.is_whitespace() || c == '<')
            .unwrap_or(rest.len() - i);
        // A sentence's full stop or a closing bracket isn't part of the address.
        let url = rest[i..i + url_len].trim_end_matches(['.', ',', ')', ';', ':', '!', '?']);
        out.push_str(&format!("<a href=\"{url}\">{url}</a>"));
        rest = &rest[i + url.len()..];
    }
    out.push_str(rest);
    out
}

/// Where a tag that starts after `<` ends (the `>`), skipping quoted attribute values.
fn tag_end(after: &str) -> usize {
    let mut quote = None;
    for (i, c) in after.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return i,
            _ => {}
        }
    }
    after.len()
}

fn decode_entities(s: &str) -> Cow<'_, str> {
    if !s.contains('&') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| match &rest[1..e] {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            n => n
                .strip_prefix("#x")
                .or_else(|| n.strip_prefix("#X"))
                .map(|h| u32::from_str_radix(h, 16).ok())
                .unwrap_or_else(|| n.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        });
        match (decoded, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Builds the plain text of [`to_text`] one tag and one run of text at a time.
#[derive(Default)]
struct TextWriter {
    lines: Vec<String>,
    /// The line being written, and whether it has text yet (a list bullet isn't text).
    line: String,
    has_text: bool,
    quote: usize,
    /// One entry per open list: `None` for bullets, the next number for numbered lists.
    lists: Vec<Option<u32>>,
    pre: usize,
    /// A paragraph ended: a blank line comes before the next text.
    blank: bool,
    /// Open links: the address, and where the link's text starts in `line`.
    links: Vec<(String, usize)>,
}

impl TextWriter {
    fn tag(&mut self, tag: &str) {
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        match (name.as_str(), closing) {
            ("br", _) => self.newline(),
            ("p", _) => {
                self.block();
                self.blank = self.blank || closing || !self.lines.is_empty();
            }
            ("div" | "h1" | "h2" | "h3", _) => self.block(),
            ("pre", false) => {
                self.block();
                self.pre += 1;
            }
            ("pre", true) => {
                self.block();
                self.pre = self.pre.saturating_sub(1);
            }
            ("hr", _) => {
                self.block();
                self.line.push_str("----------");
                self.has_text = true;
                self.newline();
            }
            ("blockquote", false) => {
                self.block();
                // A paragraph's blank line goes before the quote, not inside it.
                if self.blank && self.lines.last().is_some_and(|l| !l.is_empty()) {
                    self.emit(String::new());
                }
                self.blank = false;
                self.quote += 1;
            }
            ("blockquote", true) => {
                self.block();
                self.quote = self.quote.saturating_sub(1);
            }
            ("ul", false) => {
                self.block();
                self.lists.push(None);
            }
            ("ol", false) => {
                self.block();
                self.lists.push(Some(1));
            }
            ("ul" | "ol", true) => {
                self.block();
                self.lists.pop();
            }
            ("li", false) => {
                self.block();
                let depth = self.lists.len().max(1);
                let bullet = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        format!("{}. ", *n - 1)
                    }
                    _ => "- ".to_owned(),
                };
                self.line.push_str(&"   ".repeat(depth - 1));
                self.line.push_str(&bullet);
            }
            ("li", true) => self.block(),
            ("a", false) => {
                let href = attribute(tag, "href").unwrap_or_default();
                self.links.push((href, self.line.len()));
            }
            ("a", true) => {
                if let Some((href, start)) = self.links.pop() {
                    let text = self.line.get(start..).unwrap_or_default().trim().to_owned();
                    let bare = href.strip_prefix("mailto:").unwrap_or(&href);
                    if !href.is_empty() && text != href && text != bare {
                        self.text(&format!(" <{bare}>"));
                    }
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        if self.pre > 0 {
            let mut parts = s.split('\n');
            if let Some(first) = parts.next() {
                self.push(first);
            }
            for part in parts {
                self.newline();
                self.push(part);
            }
            return;
        }
        let mut collapsed = String::new();
        for c in s.chars() {
            if c.is_whitespace() {
                let at_start = !self.has_text && collapsed.is_empty();
                if !at_start && !collapsed.ends_with(' ') && !self.line.ends_with(' ') {
                    collapsed.push(' ');
                }
            } else {
                collapsed.push(c);
            }
        }
        if !collapsed.trim().is_empty() || self.has_text {
            self.push(&collapsed);
        }
    }

    fn push(&mut self, s: &str) {
        if self.blank && !s.trim().is_empty() {
            self.blank = false;
            if !self.has_text && self.lines.last().is_some_and(|l| !l.is_empty()) {
                self.emit(String::new());
            }
        }
        if !s.is_empty() {
            self.line.push_str(s);
            self.has_text = self.has_text || !s.trim().is_empty();
        }
    }

    /// End the line here: a `<br>`, or a line of `<pre>`.
    fn newline(&mut self) {
        let line = std::mem::take(&mut self.line);
        self.emit(line);
        self.has_text = false;
    }

    /// A block starts or ends: end the line if it has text.
    fn block(&mut self) {
        if self.has_text {
            self.newline();
        }
    }

    fn emit(&mut self, line: String) {
        let marks = if self.quote > 0 {
            format!("{} ", ">".repeat(self.quote))
        } else {
            String::new()
        };
        self.lines
            .push(format!("{marks}{}", line.trim_end()).trim_end().to_owned());
    }

    fn finish(mut self) -> String {
        self.block();
        while self.lines.last().is_some_and(String::is_empty) {
            self.lines.pop();
        }
        self.lines.join("\n")
    }
}

/// An attribute's value in a tag's source, e.g. `a href="https://x.pl"`.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let at = lower.find(&format!("{name}="))? + name.len() + 1;
    let rest = &tag[at..];
    let value = match rest.chars().next()? {
        q @ ('"' | '\'') => rest[1..].split(q).next()?,
        _ => rest.split(|c: char| c.is_whitespace()).next()?,
    };
    Some(decode_entities(value).into_owned())
}

// ---- Remote images ----

/// Limits for downloading the remote images of one message.
#[derive(Debug, Clone)]
pub struct FetchLimits {
    pub max_images: usize,
    pub max_image_bytes: u64,
    pub max_total_bytes: u64,
    /// For one image.
    pub timeout: Duration,
    /// For all of them: what hasn't come by then is left out.
    pub deadline: Duration,
    /// Allow addresses on this computer and the local network. Only tests do.
    pub allow_private: bool,
}

impl Default for FetchLimits {
    fn default() -> Self {
        Self {
            max_images: 40,
            max_image_bytes: 3 * 1024 * 1024,
            max_total_bytes: 15 * 1024 * 1024,
            timeout: Duration::from_secs(10),
            deadline: Duration::from_secs(20),
            allow_private: false,
        }
    }
}

/// How many images download at once.
const FETCHERS: usize = 4;

/// Download the remote images of a message the user chose to see in full. Returns URL →
/// data: URI for each one that arrived as an image within the limits. No cookies and no
/// Referer are sent, and addresses on the local network are refused, so a message can't
/// use Telex to reach the router or other devices.
pub fn fetch_images(urls: &[String], limits: &FetchLimits) -> HashMap<String, String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(limits.timeout))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent("Mozilla/5.0")
        .build();
    let resolver = PublicOnly {
        allow_private: limits.allow_private,
    };
    let agent = ureq::Agent::with_parts(config, DefaultConnector::new(), resolver);
    let mut wanted: Vec<&String> = Vec::new();
    for url in urls {
        if wanted.len() == limits.max_images {
            break;
        }
        if !wanted.contains(&url) {
            wanted.push(url);
        }
    }
    let deadline = Instant::now() + limits.deadline;
    let next = AtomicUsize::new(0);
    let found = Mutex::new((HashMap::new(), 0u64));
    let full = |found: &(HashMap<String, String>, u64)| found.1 > limits.max_total_bytes;
    let lock = || found.lock().unwrap_or_else(PoisonError::into_inner);
    std::thread::scope(|s| {
        for _ in 0..FETCHERS.min(wanted.len()) {
            s.spawn(|| {
                while let Some(url) = wanted.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                        break;
                    };
                    if full(&lock()) {
                        break;
                    }
                    let Some((mime, data)) = fetch_one(&agent, url, limits.max_image_bytes, left)
                    else {
                        continue;
                    };
                    let uri = data_uri(&mime, &data);
                    let mut found = lock();
                    found.1 += data.len() as u64;
                    if full(&found) {
                        break;
                    }
                    found.0.insert((*url).clone(), uri);
                }
            });
        }
    });
    found.into_inner().unwrap_or_else(PoisonError::into_inner).0
}

/// Looks host names up as ureq does and keeps only public addresses. So the address checked is
/// the one connected to: a name can't give the check a public address and the connection the
/// router's. A proxy set for the system (HTTPS_PROXY) is the user's own and may be nearby.
#[derive(Debug)]
struct PublicOnly {
    allow_private: bool,
}

impl Resolver for PublicOnly {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let found = DefaultResolver::default().resolve(uri, config, timeout)?;
        let proxy = config.proxy().is_some_and(|p| uri.host() == Some(p.host()));
        if self.allow_private || proxy {
            return Ok(found);
        }
        let mut public = self.empty();
        for addr in found.iter().filter(|a| is_public(a.ip())) {
            public.push(*addr);
        }
        if public.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(public)
    }
}

/// One image, following up to three redirects (each checked like the first URL), within
/// `left` of the time for all of them.
fn fetch_one(
    agent: &ureq::Agent,
    url: &str,
    max_bytes: u64,
    left: Duration,
) -> Option<(String, Vec<u8>)> {
    let mut url = url.to_owned();
    let until = Instant::now() + left;
    for _ in 0..4 {
        if !allowed_url(&url) {
            return None;
        }
        let left = until.checked_duration_since(Instant::now())?;
        let timeout = agent
            .config()
            .timeouts()
            .global
            .map_or(left, |t| t.min(left));
        let mut response = agent
            .get(url.as_str())
            .header("Accept", "image/*")
            .config()
            .timeout_global(Some(timeout))
            .build()
            .call()
            .ok()?;
        let status = response.status().as_u16();
        if (300..400).contains(&status) {
            let location = response.headers().get("location")?.to_str().ok()?;
            url = redirect_target(&url, location)?;
            continue;
        }
        if status != 200 {
            return None;
        }
        let declared = response
            .body()
            .mime_type()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let data = response
            .body_mut()
            .with_config()
            .limit(max_bytes)
            .read_to_vec()
            .ok()?;
        let mime = if declared.starts_with("image/") {
            declared
        } else {
            sniff_image(&data)?.to_owned()
        };
        return Some((mime, data));
    }
    None
}

/// An absolute Location, or a path on the same server.
fn redirect_target(current: &str, location: &str) -> Option<String> {
    let lower = location.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(location.to_owned());
    }
    if location.starts_with('/') && !location.starts_with("//") {
        let scheme_end = current.find("://")? + 3;
        let host_end = current[scheme_end..]
            .find('/')
            .map_or(current.len(), |i| scheme_end + i);
        return Some(format!("{}{location}", &current[..host_end]));
    }
    None
}

/// http(s) with a host. Where it may lead is `PublicOnly`'s to say.
fn allowed_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    !host.is_empty() && !host.starts_with(':')
}

/// Not this computer, not the local network, not a reserved range.
fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
                || a == 0
                || (a == 100 && (64..128).contains(&b)) // carrier-grade NAT
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let s = v6.segments();
            // Addresses that carry an IPv4 one inside: NAT64 (64:ff9b::/96) at the end, 6to4
            // (2002::/16) after the prefix. The IPv4 one decides.
            let v4 = |hi: u16, lo: u16| {
                let [a, b] = hi.to_be_bytes();
                let [c, d] = lo.to_be_bytes();
                IpAddr::V4(std::net::Ipv4Addr::new(a, b, c, d))
            };
            if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                return is_public(v4(s[6], s[7]));
            }
            if s[0] == 0x2002 {
                return is_public(v4(s[1], s[2]));
            }
            let first = s[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00 // unique local
                || (first & 0xffc0) == 0xfe80) // link-local
        }
    }
}

/// The image type from the first bytes, for servers that don't say.
fn sniff_image(data: &[u8]) -> Option<&'static str> {
    let starts = |sig: &[u8]| data.starts_with(sig);
    if starts(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        Some("image/gif")
    } else if starts(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if data.len() > 12 && starts(b"RIFF") && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_nest_without_end_but_show_at_most_twenty_deep() {
        let html = from_text(&format!("{} deep", ">".repeat(5_000)));
        assert_eq!(html.matches("<blockquote>").count(), MAX_QUOTE_DEPTH);
        assert_eq!(html.matches("</blockquote>").count(), MAX_QUOTE_DEPTH);
        assert_eq!(
            from_text("Hi\n> one\n> > two\n> one again\nme"),
            "<div>Hi</div><blockquote><div>one</div><blockquote><div>two</div></blockquote><div>one again</div></blockquote><div>me</div>"
        );
    }
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn html_mail(html: &str, extra_parts: &str) -> Vec<u8> {
        format!(
            "From: Sklep <newsletter@sklep.pl>\r\n\
Subject: Promocja\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/related; boundary=\"r\"\r\n\
\r\n\
--r\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
{html}\r\n\
{extra_parts}\
--r--\r\n"
        )
        .into_bytes()
    }

    fn render_html(html: &str) -> Rendered {
        render(&html_mail(html, ""), &HashMap::new(), &Paper::Light).unwrap()
    }

    fn body_of(r: &Rendered) -> &str {
        let start = r.document.find("<body>").unwrap() + 6;
        let end = r.document.rfind("</body>").unwrap();
        &r.document[start..end]
    }

    #[test]
    fn plain_text_mail_has_no_html_view() {
        assert!(render(
            b"Subject: x\r\n\r\nJust text\r\n",
            &HashMap::new(),
            &Paper::Light
        )
        .is_none());
    }

    #[test]
    fn active_content_is_removed() {
        let r = render_html(
            "<p onclick=\"steal()\">Hej <b>Anna</b></p>\
             <script>alert(1)</script><style>p{background:url(https://t.pl/a.gif)}</style>\
             <form action=\"https://phish.pl\"><input name=pass></form>\
             <iframe src=\"https://x.pl\"></iframe>\
             <a href=\"javascript:alert(1)\">bad</a> <a href=\"https://sklep.pl/oferta\">ok</a>\
             <a href=\"mailto:biuro@sklep.pl\">mail</a> <img src=\"images/local.png\">",
        );
        let body = body_of(&r);
        assert!(body.contains("<p>Hej <b>Anna</b></p>"), "{body}");
        for gone in [
            "onclick",
            "script",
            "alert",
            "<style",
            "<form",
            "<input",
            "<iframe",
            "javascript",
            "local.png",
        ] {
            assert!(!body.contains(gone), "{gone} is still there: {body}");
        }
        assert!(body.contains("href=\"https://sklep.pl/oferta\""));
        assert!(body.contains("rel=\"noopener noreferrer\""));
        assert!(body.contains("href=\"mailto:biuro@sklep.pl\""));
        assert!(
            r.remote.is_empty(),
            "the <style> block was dropped, not fetched"
        );
    }

    #[test]
    fn the_document_forbids_network_loads() {
        let r = render_html("<p>x</p>");
        assert!(r
            .document
            .contains("Content-Security-Policy\" content=\"default-src 'none'; img-src data:;"));
        assert!(r.document.contains("no-referrer"));
    }

    #[test]
    fn remote_images_are_left_out_and_counted() {
        let r = render_html(
            "<img src=\"https://track.sklep.pl/open.gif?u=7\" width=1 height=1>\
             <table background=\"http://sklep.pl/bg.jpg\"><tr><td \
             style=\"color:red; background: url('https://sklep.pl/b.png') no-repeat\">x</td></tr></table>\
             <img src=\"https://track.sklep.pl/open.gif?u=7\">",
        );
        let body = body_of(&r);
        assert!(!body.contains("sklep.pl/"), "{body}");
        assert!(
            body.contains("color:red; background: none no-repeat"),
            "{body}"
        );
        assert_eq!(
            r.remote,
            [
                "https://track.sklep.pl/open.gif?u=7",
                "http://sklep.pl/bg.jpg",
                "https://sklep.pl/b.png"
            ],
            "each URL once, in order"
        );
        assert_eq!(r.blocked, 3);
    }

    #[test]
    fn fetched_images_are_put_in_as_data() {
        let html =
            "<img src=\"https://sklep.pl/logo.png\"><img src=\"https://sklep.pl/missing.png\">";
        let images = HashMap::from([(
            "https://sklep.pl/logo.png".to_owned(),
            "data:image/png;base64,AAAA".to_owned(),
        )]);
        let r = render(&html_mail(html, ""), &images, &Paper::Light).unwrap();
        let body = body_of(&r);
        assert!(
            body.contains("src=\"data:image/png;base64,AAAA\""),
            "{body}"
        );
        assert!(!body.contains("missing.png"));
        assert_eq!(r.blocked, 1, "the one that didn't arrive");
    }

    #[test]
    fn images_inside_the_message_show_without_the_network() {
        let png = "--r\r\n\
Content-Type: image/png\r\n\
Content-ID: <logo@sklep.pl>\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
iVBORw0KGgo=\r\n";
        let r = render(
            &html_mail("<img src=\"cid:logo@sklep.pl\" alt=\"Logo\">", png),
            &HashMap::new(),
            &Paper::Light,
        )
        .unwrap();
        let body = body_of(&r);
        assert!(
            body.contains("src=\"data:image/png;base64,iVBORw0KGgo=\""),
            "{body}"
        );
        assert!(r.remote.is_empty() && r.blocked == 0);
    }

    #[test]
    fn css_that_runs_code_is_dropped() {
        let r = render_html("<p style=\"width:expression(alert(1))\">x</p>");
        assert!(!body_of(&r).contains("expression"));
    }

    #[test]
    fn only_public_addresses_are_fetched() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.1.1",
            "172.16.0.9",
            "169.254.1.1",
            "100.64.0.1",
            "0.0.0.0",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "::1",
            "fe80::1",
            "fd00::1",
            "::ffff:192.168.1.1",
            "64:ff9b::c0a8:101", // NAT64 of 192.168.1.1
            "2002:7f00:1::",     // 6to4 of 127.0.0.1
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        assert!(is_public("93.184.216.34".parse().unwrap()));
        assert!(is_public("2606:2800:220:1::".parse().unwrap()));
        assert!(
            is_public("2002:5db8:d822::".parse().unwrap()),
            "6to4 of a public one"
        );
        assert!(!allowed_url("ftp://example.com/x.png"));
        assert!(!allowed_url("http:///x.png"));
        assert!(!allowed_url("http://:80/x.png"));
        assert!(allowed_url("HTTPS://example.com/x.png"));
    }

    #[test]
    fn redirects_resolve_against_the_current_server() {
        assert_eq!(
            redirect_target("https://t.pl/a/b.gif?x=1", "/c.gif").as_deref(),
            Some("https://t.pl/c.gif")
        );
        assert_eq!(
            redirect_target("https://t.pl/a", "https://cdn.pl/i.png").as_deref(),
            Some("https://cdn.pl/i.png")
        );
        assert_eq!(redirect_target("https://t.pl/a", "//evil/x"), None);
    }

    #[test]
    fn dark_paper_turns_the_colors_around() {
        let html = "<table bgcolor=\"#ffffff\"><tr><td style=\"color:#222222;background:url('cid:x') #f4f4f4\">\
                    <font color=\"black\">Faktura</font></td></tr></table><img src=\"cid:x\">";
        let light = render(&html_mail(html, ""), &HashMap::new(), &Paper::Light).unwrap();
        assert!(body_of(&light).contains("bgcolor=\"#ffffff\""));
        assert!(light
            .document
            .contains("html{background:#fff;color-scheme:light}"));

        // Fjord: white becomes its blue-slate paper, links take its frost-cyan.
        let fjord = Palette {
            paper: "#151e28".into(),
            ink: "#e3ebf3".into(),
            link: "#6cc4dc".into(),
            line: "#2d3b4b".into(),
            muted: "#a0b1c3".into(),
        };
        let dark = render(&html_mail(html, ""), &HashMap::new(), &Paper::Dark(fjord)).unwrap();
        let body = body_of(&dark);
        assert!(body.contains("bgcolor=\"#151e28\""), "{body}");
        for gone in ["#ffffff", "#222222", "#f4f4f4", "\"black\""] {
            assert!(!body.contains(gone), "{gone} is still there: {body}");
        }
        assert!(dark
            .document
            .contains("html{background:#151e28;color-scheme:dark}"));
        assert!(dark.document.contains("a{color:#6cc4dc}"));
        assert_eq!(dark.remote, light.remote, "the same images either way");
    }

    #[test]
    fn a_palette_cant_inject_css() {
        let evil = Palette {
            paper: "#000;}*{background:url(https://x.pl/t.gif)".into(),
            link: "red".into(),
            ..Palette::default()
        };
        let r = render(
            &html_mail("<p>x</p>", ""),
            &HashMap::new(),
            &Paper::Dark(evil),
        )
        .unwrap();
        assert!(!r.document.contains("x.pl") && !r.document.contains("red"));
        assert!(r
            .document
            .contains("html{background:#14171b;color-scheme:dark}"));
    }

    #[test]
    fn compose_html_keeps_formatting_and_drops_the_rest() {
        let clean = compose_clean(
            "<html><head><title>x</title><style>p{color:red}</style></head><body>\
             <div style=\"color:red\" class=\"x\"><b>Dzień</b> <i>dobry</i>, \
             <a href=\"https://firma.pl\" onclick=\"x()\">firma</a> \
             <a href=\"javascript:alert(1)\">zła</a><img src=\"https://t.pl/a.gif\"></div>\
             <script>alert(1)</script><ul><li>raz</li></ul></body></html>",
        );
        assert_eq!(
            clean,
            "<div><b>Dzień</b> <i>dobry</i>, <a href=\"https://firma.pl\">firma</a> \
             <a>zła</a></div><ul><li>raz</li></ul>"
        );
        let doc = compose_document("<blockquote><div>x</div></blockquote>");
        assert!(doc.contains("<blockquote type=\"cite\" style="), "{doc}");
        assert!(doc.starts_with("<!doctype html>"));
    }

    #[test]
    fn html_becomes_readable_plain_text() {
        let text = to_text(&compose_clean(
            "<div>Dzień dobry,</div><div><br></div>\
             <div>oto <b>lista</b>:</div><ul><li>pierwsza</li><li>druga<ul><li>pod</li></ul></li></ul>\
             <ol><li>raz</li><li>dwa</li></ol>\
             <div>Strona: <a href=\"https://firma.pl/oferta\">oferta</a>, \
             <a href=\"https://firma.pl\">https://firma.pl</a>, \
             <a href=\"mailto:anna@firma.pl\">anna@firma.pl</a></div>\
             <p>Akapit&nbsp;pierwszy</p><p>drugi &amp; ostatni</p>\
             <blockquote><div>cytat</div><blockquote><div>starszy</div></blockquote></blockquote>\
             <div>Pozdrawiam<br>Anna</div>",
        ));
        assert_eq!(
            text,
            "Dzień dobry,\n\
             \n\
             oto lista:\n\
             - pierwsza\n\
             - druga\n   - pod\n\
             1. raz\n\
             2. dwa\n\
             Strona: oferta <https://firma.pl/oferta>, https://firma.pl, anna@firma.pl\n\
             \n\
             Akapit pierwszy\n\
             \n\
             drugi & ostatni\n\
             \n\
             > cytat\n\
             >> starszy\n\
             Pozdrawiam\n\
             Anna"
        );
    }

    #[test]
    fn plain_text_opens_as_html_and_comes_back() {
        let text = "Dzień dobry,\n\nzobacz https://firma.pl/a?b=1&c=2.\n> cytat <tag>\n>> starszy\nPozdrawiam";
        let html = from_text(text);
        assert_eq!(
            html,
            "<div>Dzień dobry,</div><div><br></div>\
             <div>zobacz <a href=\"https://firma.pl/a?b=1&amp;c=2\">https://firma.pl/a?b=1&amp;c=2</a>.</div>\
             <blockquote><div>cytat &lt;tag&gt;</div><blockquote><div>starszy</div></blockquote></blockquote>\
             <div>Pozdrawiam</div>"
        );
        assert_eq!(to_text(&compose_clean(&html)), text);
    }

    /// A tiny HTTP server: /logo.png, /page (HTML), /big (too large), /moved (to /logo.png).
    fn start_http() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                if reader.read_line(&mut request).is_err() {
                    continue;
                }
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) <= 2 {
                        break;
                    }
                }
                let path = request.split(' ').nth(1).unwrap_or("/").to_owned();
                let png: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-png";
                let (status, kind, body): (&str, &str, Vec<u8>) = match path.as_str() {
                    "/logo.png" => ("200 OK", "image/png", png.to_vec()),
                    "/unlabeled" => ("200 OK", "application/octet-stream", png.to_vec()),
                    "/page" => ("200 OK", "text/html", b"<html>no</html>".to_vec()),
                    "/big" => ("200 OK", "image/png", vec![0; 4096]),
                    "/moved" => ("302 Found", "text/plain", Vec::new()),
                    _ => ("404 Not Found", "text/plain", Vec::new()),
                };
                let mut out = stream;
                let location = if path == "/moved" {
                    "Location: /logo.png\r\n"
                } else {
                    ""
                };
                let _ = write!(
                    out,
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{location}Connection: close\r\n\r\n",
                    body.len()
                );
                let _ = out.write_all(&body);
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn remote_images_are_fetched_within_the_limits() {
        let base = start_http();
        let url = |p: &str| format!("{base}{p}");
        let limits = FetchLimits {
            max_image_bytes: 1024,
            allow_private: true, // the test server is on this computer
            ..FetchLimits::default()
        };
        let urls: Vec<String> = [
            "/logo.png",
            "/unlabeled",
            "/page",
            "/big",
            "/moved",
            "/nothing",
        ]
        .iter()
        .map(|p| url(p))
        .collect();
        let images = fetch_images(&urls, &limits);
        let mut got: Vec<_> = images
            .keys()
            .map(|k| k.trim_start_matches(&base).to_owned())
            .collect();
        got.sort();
        assert_eq!(got, ["/logo.png", "/moved", "/unlabeled"]);
        assert!(images[&url("/logo.png")].starts_with("data:image/png;base64,"));
        assert!(
            images[&url("/unlabeled")].starts_with("data:image/png;base64,"),
            "sniffed"
        );

        // The same server is refused when private addresses aren't allowed, also by a name
        // that leads there: what is connected to is what was checked.
        assert!(fetch_images(&urls, &FetchLimits::default()).is_empty());
        let named = base.replace("127.0.0.1", "localhost") + "/logo.png";
        assert!(fetch_images(&[named], &FetchLimits::default()).is_empty());
    }

    #[test]
    fn slow_servers_get_one_deadline_for_all_images() {
        // A server that takes connections and never answers.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let urls: Vec<String> = (0..12).map(|i| format!("{base}/{i}.png")).collect();
        let limits = FetchLimits {
            deadline: Duration::from_millis(300),
            allow_private: true,
            ..FetchLimits::default()
        };
        let started = Instant::now();
        assert!(fetch_images(&urls, &limits).is_empty());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        drop(listener);
    }
}
