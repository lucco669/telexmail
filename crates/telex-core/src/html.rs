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
    let dark = match paper {
        Paper::Light => None,
        Paper::Dark(palette) => Some(palette.checked()),
    };
    let (html, remote) = cleaned(raw, images, dark.as_ref().map(|d| d.1))?;
    let blocked = remote.iter().filter(|u| !images.contains_key(*u)).count();
    Some(Rendered {
        document: wrap(&html, dark.as_ref().map(|d| &d.0)),
        remote,
        blocked,
    })
}

/// A message's HTML body cleaned for white paper, without a document around it, and the
/// remote images it references: what the printed copy shows (`export::print_document`).
pub fn printable(raw: &[u8], images: &HashMap<String, String>) -> Option<(String, Vec<String>)> {
    cleaned(raw, images, None)
}

fn cleaned(
    raw: &[u8],
    images: &HashMap<String, String>,
    dark: Option<Target>,
) -> Option<(String, Vec<String>)> {
    let body = html_body(raw)?;
    let inline = body
        .inline
        .iter()
        .filter(|(_, (mime, _))| mime.starts_with("image/"))
        .map(|(cid, (mime, data))| (cid.clone(), data_uri(mime, data)))
        .collect();
    Some(sanitize(&body.html, inline, images.clone(), dark))
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
        // Where quoted mail starts, so the window can fold it: Gmail's and Thunderbird's
        // classes, Apple's and Thunderbird's cite, and Outlook's reply divider (only that id).
        .add_tag_attributes("blockquote", &["type"])
        .add_tag_attributes("div", &["id"])
        .add_allowed_classes("div", &["gmail_quote", "gmail_attr", "moz-cite-prefix"])
        .add_allowed_classes("blockquote", &["gmail_quote"])
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
        ("div", "id") => (value == "divRplyFwdMsg").then_some(Cow::Borrowed(value)),
        ("blockquote", "type") => value
            .eq_ignore_ascii_case("cite")
            .then_some(Cow::Borrowed("cite")),
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
blockquote{margin:6px 0 6px 2px;padding-left:10px;border-left:3px solid #7f9fd6;color:inherit}\nblockquote blockquote{border-left-color:#c7b07c}blockquote blockquote blockquote{border-left-color:#9fb7a4}";

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
blockquote{{margin:6px 0 6px 2px;padding-left:10px;border-left:3px solid {link};color:inherit}}\nblockquote blockquote{{border-left-color:{muted}}}blockquote blockquote blockquote{{border-left-color:{line}}}"
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

/// Besides those: pictures, and the tables, small print and headings of a quoted message.
const RICH_TAGS: [&str; 14] = [
    "img", "table", "thead", "tbody", "tfoot", "tr", "td", "th", "caption", "sub", "sup", "h4",
    "h5", "h6",
];

/// Text colors the editor offers, as they are sent: dark enough on white paper (AA). Pasted
/// or quoted text keeps its color only when it is one of these.
pub const TEXT_COLORS: [&str; 6] = [
    "#c00000", "#b45309", "#15803d", "#1d4ed8", "#7e22ce", "#6b7280",
];
/// Highlights, light enough under black text.
pub const MARK_COLORS: [&str; 4] = ["#fff59d", "#c8f7c5", "#cfe8ff", "#ffd6e7"];
/// Small and large text; normal text has no size of its own.
pub const FONT_SIZES: [&str; 2] = ["12px", "18px"];
/// The class of the signature's block, so another identity's can take its place.
pub const SIGNATURE_CLASS: &str = "telex-signature";

/// What pictures in one message may take, as data: URIs in the editor (decoded bytes).
#[derive(Debug, Clone, Copy)]
pub struct ImageLimits {
    pub each: usize,
    pub total: usize,
    pub count: usize,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            each: 5 * 1024 * 1024,
            total: 15 * 1024 * 1024,
            count: 40,
        }
    }
}

/// The original's pictures a quote takes along: fewer, so a reply to a newsletter stays light.
const QUOTE_IMAGES: ImageLimits = ImageLimits {
    each: 2 * 1024 * 1024,
    total: 5 * 1024 * 1024,
    count: 20,
};

/// A signature's pictures (a logo): small, since every identity's signature travels with the
/// folder tree to the window.
pub const SIGNATURE_IMAGES: ImageLimits = ImageLimits {
    each: 100 * 1024,
    total: 200 * 1024,
    count: 4,
};

/// An HTML signature, cleaned as sent mail is, within [`SIGNATURE_IMAGES`].
pub fn clean_signature(html: &str) -> String {
    clean_with(html, SIGNATURE_IMAGES)
}

/// Picture types a message written in Telex may carry. No SVG: it is a document, not a picture.
const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// The editor's HTML, cleaned to what Telex sends. Also takes a whole document (a saved
/// draft) and keeps only its body's content.
///
/// Kept: text formatting, links (http, https, mailto), lists, quotes, tables, the signature's
/// block, colors and sizes from the editor's palette ([`TEXT_COLORS`], [`MARK_COLORS`],
/// [`FONT_SIZES`]: no other style), and pictures as `data:` (PNG, JPEG, GIF, WebP in base64,
/// within [`ImageLimits`]) or `cid:` (the parts [`inline_images`] makes of them). Nothing
/// else: no classes, ids, scripts, forms, remote pictures or fonts.
pub fn compose_clean(html: &str) -> String {
    clean_with(html, ImageLimits::default())
}

fn clean_with(html: &str, limits: ImageLimits) -> String {
    let budget = Arc::new(ImageBudget {
        limits,
        bytes: AtomicUsize::new(0),
        count: AtomicUsize::new(0),
    });
    let mut builder = Builder::empty();
    builder
        // empty() empties only the tags: ammonia's own attributes (title, lang, align, an img's
        // height…) would still pass. Only those named here do.
        .tag_attributes(HashMap::new())
        .generic_attributes(HashSet::new())
        .add_tags(COMPOSE_TAGS)
        .add_tags(RICH_TAGS)
        .add_generic_attributes(&["style"])
        .add_tag_attributes("a", &["href"])
        .add_tag_attributes("img", &["src", "alt", "width"])
        .add_tag_attributes("table", &["border", "cellpadding", "cellspacing"])
        .add_tag_attributes("td", &["colspan", "rowspan"])
        .add_tag_attributes("th", &["colspan", "rowspan"])
        .add_allowed_classes("div", &[SIGNATURE_CLASS])
        // data: and cid: must pass ammonia's scheme check to reach the filter, which allows
        // them only on pictures.
        .url_schemes(HashSet::from(["http", "https", "mailto", "data", "cid"]))
        .url_relative(UrlRelative::Deny)
        .link_rel(None)
        .strip_comments(true)
        .clean_content_tags(HashSet::from(["script", "style", "head", "title"]))
        .attribute_filter(move |element, attribute, value| {
            compose_filter(&budget, element, attribute, value)
        });
    tidy(builder.clean(html).to_string().trim())
}

/// How much picture a message has taken so far while it is cleaned.
struct ImageBudget {
    limits: ImageLimits,
    bytes: AtomicUsize,
    count: AtomicUsize,
}

impl ImageBudget {
    /// Room for one more picture of `bytes`: taken, or `false` when it is too large.
    fn take(&self, bytes: usize) -> bool {
        if bytes > self.limits.each
            || self.count.fetch_add(1, Ordering::Relaxed) >= self.limits.count
        {
            return false;
        }
        let before = self.bytes.fetch_add(bytes, Ordering::Relaxed);
        before + bytes <= self.limits.total
    }
}

fn compose_filter<'u>(
    budget: &ImageBudget,
    element: &str,
    attribute: &str,
    value: &'u str,
) -> Option<Cow<'u, str>> {
    let digits = |max: usize| -> Option<Cow<'u, str>> {
        let v = value.trim();
        (!v.is_empty() && v.len() <= max && v.bytes().all(|b| b.is_ascii_digit()))
            .then(|| Cow::Owned(v.to_owned()))
    };
    match (element, attribute) {
        ("a", "href") => {
            let lower = value.trim().to_ascii_lowercase();
            ["http://", "https://", "mailto:"]
                .iter()
                .any(|s| lower.starts_with(s))
                .then_some(Cow::Borrowed(value))
        }
        ("img", "src") => {
            let v = value.trim();
            if let Some(bytes) = data_image_size(v) {
                return budget.take(bytes).then(|| Cow::Owned(v.to_owned()));
            }
            let cid = v.strip_prefix("cid:").or_else(|| v.strip_prefix("CID:"))?;
            let fine = !cid.is_empty()
                && cid.len() <= 200
                && cid
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".@_-$".contains(&b));
            fine.then(|| Cow::Owned(v.to_owned()))
        }
        // The size the writer gave a picture, the one way every mail program keeps it: the width
        // (Outlook honours only that), the height following it.
        ("img", "width") => digits(4).filter(|w| {
            w.parse::<u16>()
                .is_ok_and(|w| (1..=MAX_PICTURE_WIDTH).contains(&w))
        }),
        ("img", "style") => picture_style(value).then_some(Cow::Borrowed(PICTURE_STYLE)),
        ("td" | "th", "colspan" | "rowspan") => digits(2),
        ("table", "border" | "cellpadding" | "cellspacing") => digits(2),
        (_, "style") => compose_style(value).map(Cow::Owned),
        _ => Some(Cow::Borrowed(value)),
    }
}

/// The widest a picture may be written in a message, in pixels.
pub const MAX_PICTURE_WIDTH: u16 = 2000;
/// A picture's one style: never wider than the text, its height in proportion.
pub const PICTURE_STYLE: &str = "max-width:100%;height:auto";

/// A cleaned picture's tag (`img src="…" …`, without its angle brackets) with [`PICTURE_STYLE`]
/// in place of any style it had.
fn with_picture_style(tag: &str) -> String {
    let mut tag = tag.to_owned();
    if let Some(at) = tag.find(" style=\"") {
        if let Some(len) = tag[at + 8..].find('"') {
            tag.replace_range(at..at + 8 + len + 1, "");
        }
    }
    let tag = tag.trim_end_matches('/').trim_end();
    format!("{tag} style=\"{PICTURE_STYLE}\"")
}

/// A picture's style is kept only when it says nothing but [`PICTURE_STYLE`]'s two things.
fn picture_style(style: &str) -> bool {
    let mut any = false;
    for declaration in style.split(';').filter(|d| !d.trim().is_empty()) {
        let Some((property, value)) = declaration.split_once(':') else {
            return false;
        };
        let (property, value) = (
            property.trim().to_ascii_lowercase(),
            value.trim().to_ascii_lowercase(),
        );
        match (property.as_str(), value.as_str()) {
            ("max-width", "100%") | ("height", "auto") => any = true,
            _ => return false,
        }
    }
    any
}

/// A `data:` picture Telex may send: its decoded size, or `None`.
fn data_image_size(src: &str) -> Option<usize> {
    let rest = src
        .get(..5)?
        .eq_ignore_ascii_case("data:")
        .then(|| &src[5..])?;
    let (mime, payload) = rest.split_once(";base64,")?;
    let mime = mime.to_ascii_lowercase();
    if !IMAGE_TYPES.contains(&mime.as_str()) || payload.is_empty() {
        return None;
    }
    let ok = payload
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=');
    ok.then(|| payload.len() / 4 * 3)
}

/// The editor's styles, down to the palette: `color`, `background-color` and `font-size`
/// with the values Telex offers, written the one way the editor's CSS recognizes them.
fn compose_style(style: &str) -> Option<String> {
    let mut kept: Vec<(&str, String)> = Vec::new();
    for declaration in style.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim().to_ascii_lowercase();
        let value = value.trim().trim_end_matches("!important").trim();
        let found = match property.as_str() {
            "color" => colors::opaque_hex(value)
                .filter(|c| TEXT_COLORS.contains(&c.as_str()))
                .map(|c| ("color", c)),
            "background-color" | "background" => colors::opaque_hex(value)
                .filter(|c| MARK_COLORS.contains(&c.as_str()))
                .map(|c| ("background-color", c)),
            // The editor writes the browser's words for its two sizes (execCommand's 2 and 5).
            "font-size" => match value.to_ascii_lowercase().as_str() {
                "small" => Some(FONT_SIZES[0]),
                "x-large" => Some(FONT_SIZES[1]),
                v => FONT_SIZES.iter().copied().find(|s| *s == v),
            }
            .map(|s| ("font-size", s.to_owned())),
            _ => None,
        };
        if let Some((name, value)) = found {
            kept.retain(|(n, _)| *n != name);
            kept.push((name, value));
        }
    }
    (!kept.is_empty()).then(|| {
        kept.iter()
            .map(|(n, v)| format!("{n}:{v}"))
            .collect::<Vec<_>>()
            .join(";")
    })
}

/// What ammonia leaves behind: pictures whose source was refused go whole, not as empty
/// frames, and a class that had none allowed goes.
fn tidy(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = tag_end(after);
        let tag = &after[..end];
        rest = after.get(end + 1..).unwrap_or_default();
        if tag.starts_with("img") && !tag.contains(" src=\"") {
            continue;
        }
        out.push('<');
        out.push_str(&tag.replace(" class=\"\"", ""));
        out.push('>');
    }
    out.push_str(rest);
    out
}

/// A picture of a message written in Telex, as a part of its own (multipart/related).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineImage {
    /// Without angle brackets; the HTML says `cid:` and this.
    pub cid: String,
    pub mime: String,
    pub data: Vec<u8>,
}

/// Cleaned HTML ([`compose_clean`]) ready to send: each `data:` picture becomes a part of
/// its own, referenced as `cid:<n>.<cid_base>` (the same picture twice is one part). A `cid:`
/// that names none of these (pasted from elsewhere) is left out. Each picture goes with
/// [`PICTURE_STYLE`], so its width never makes it wider than the reader's window (the editor
/// leaves the style out: its own stylesheet says the same, so the browser drops it there).
pub fn inline_images(clean: &str, cid_base: &str) -> (String, Vec<InlineImage>) {
    let mut parts: Vec<InlineImage> = Vec::new();
    let mut out = String::with_capacity(clean.len());
    let mut rest = clean;
    while let Some(i) = rest.find("<img") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = tag_end(after);
        let tag = &after[..end];
        rest = after.get(end + 1..).unwrap_or_default();
        let Some(at) = tag.find(" src=\"") else {
            continue;
        };
        let value_start = at + 6;
        let Some(len) = tag[value_start..].find('"') else {
            continue;
        };
        let src = &tag[value_start..value_start + len];
        let Some((mime, payload)) = src
            .get(5..)
            .filter(|_| data_image_size(src).is_some())
            .and_then(|r| r.split_once(";base64,"))
        else {
            continue; // a cid: of someone else's message, or nothing usable
        };
        let Ok(data) = STANDARD.decode(payload) else {
            continue;
        };
        let cid = match parts.iter().find(|p| p.data == data) {
            Some(p) => p.cid.clone(),
            None => {
                let cid = format!("{}.{cid_base}", parts.len() + 1);
                parts.push(InlineImage {
                    cid: cid.clone(),
                    mime: mime.to_ascii_lowercase(),
                    data,
                });
                cid
            }
        };
        let written = format!(
            "{}cid:{cid}{}",
            &tag[..value_start],
            &tag[value_start + len..]
        );
        out.push('<');
        out.push_str(&with_picture_style(&written));
        out.push('>');
    }
    out.push_str(rest);
    (out, parts)
}

/// A stored message's HTML for the editor: its own pictures as `data:`, remote ones left out,
/// cleaned to what Telex sends ([`compose_clean`]). For editing a draft again, or `None` for
/// plain-text mail.
pub fn editable(raw: &[u8]) -> Option<String> {
    editable_with(raw, ImageLimits::default())
}

/// Larger than this, a quote of the original's HTML would weigh down the editor: the reply
/// quotes its text instead.
const MAX_QUOTE_HTML: usize = 2 * 1024 * 1024;

/// The original's HTML for a reply's or a forward's quote: like [`editable`] (its pictures
/// within lighter limits, the rest of its styles gone), or `None` when it has none or is too
/// large to edit comfortably.
pub fn quotable(raw: &[u8]) -> Option<String> {
    let html = editable_with(raw, QUOTE_IMAGES)?;
    (html.len() <= MAX_QUOTE_HTML && !html.trim().is_empty()).then_some(html)
}

fn editable_with(raw: &[u8], limits: ImageLimits) -> Option<String> {
    let body = html_body(raw)?;
    if body.html.len() > 4 * MAX_QUOTE_HTML {
        return None;
    }
    let inline = body
        .inline
        .iter()
        .filter(|(_, (mime, _))| IMAGE_TYPES.contains(&mime.to_ascii_lowercase().as_str()))
        .map(|(cid, (mime, data))| (cid.clone(), data_uri(mime, data)))
        .collect();
    let (html, _) = sanitize(&body.html, inline, HashMap::new(), None);
    Some(clean_with(&html, limits))
}

/// Cleaned HTML with nothing but lines: a signature typed without any formatting.
pub fn is_plain(clean: &str) -> bool {
    let mut rest = clean;
    while let Some(i) = rest.find('<') {
        let after = &rest[i + 1..];
        let end = tag_end(after);
        let name = after[..end]
            .trim_start_matches('/')
            .trim_end_matches('/')
            .trim();
        if !matches!(name, "div" | "br" | "p") {
            return false;
        }
        rest = after.get(end + 1..).unwrap_or_default();
    }
    true
}

/// The text/html part of a message written in Telex: the cleaned HTML in a small document,
/// quotes marked the way mail programs show them.
pub fn compose_document(clean: &str) -> String {
    let body = clean.replace(
        "<blockquote>",
        "<blockquote type=\"cite\" style=\"margin:0 0 0 .8ex;border-left:2px solid #ccc;padding-left:1ex\">",
    );
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <style>img{{max-width:100%;height:auto}}</style></head>\
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

/// Plain text of received HTML mail, for copying it out (`markdown.rs`): like [`to_text`],
/// links with their addresses, and each row of a table on a line of its own.
pub fn received_text(html: &str) -> String {
    let mut builder = Builder::empty();
    builder
        .add_tags(COMPOSE_TAGS)
        .add_tags(&["table", "tr", "td", "th", "h4", "h5", "h6"])
        .add_tag_attributes("a", &["href"])
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .url_relative(UrlRelative::Deny)
        .link_rel(None)
        .strip_comments(true)
        .clean_content_tags(HashSet::from(["script", "style", "head", "title"]));
    to_text(&builder.clean(html).to_string())
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

pub(crate) fn escape(s: &str) -> String {
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
            ("div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "table" | "tr", _) => self.block(),
            ("td" | "th", true) => self.text(" "),
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
                let after_space = match collapsed.chars().last() {
                    Some(last) => last == ' ',
                    None => self.line.ends_with(' '),
                };
                if !at_start && !after_space {
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
    let agent = public_agent(limits.timeout, limits.allow_private);
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

/// An HTTP client for addresses taken from mail (images, one-click unsubscribe): public
/// addresses only (`PublicOnly`), no redirects followed by itself, no cookies (ureq keeps
/// none without its `cookies` feature), no Referer, a generic User-Agent, every status as
/// an answer rather than an error.
pub(crate) fn public_agent(timeout: Duration, allow_private: bool) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent("Mozilla/5.0")
        .build();
    ureq::Agent::with_parts(
        config,
        DefaultConnector::new(),
        PublicOnly { allow_private },
    )
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
pub(crate) fn allowed_url(url: &str) -> bool {
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
    fn where_quoted_mail_starts_stays_marked_and_nothing_else_does() {
        let r = render_html(
            "<p class=\"hero\" id=\"top\">New text</p>\
             <div class=\"gmail_quote big\"><div class=\"gmail_attr\">On Fri, Anna wrote:</div>\
             <blockquote class=\"gmail_quote x\" type=\"CITE\">Old text</blockquote></div>\
             <div id=\"divRplyFwdMsg\">From: Anna</div><div id=\"other\">x</div>",
        );
        let body = body_of(&r);
        assert!(body.contains("<div class=\"gmail_quote\">"), "{body}");
        assert!(body.contains("<div class=\"gmail_attr\">"));
        assert!(body.contains("type=\"cite\""));
        assert!(body.contains("<div id=\"divRplyFwdMsg\">"));
        assert!(!body.contains("hero") && !body.contains("\"top\"") && !body.contains("\"other\""));
        assert!(!body.contains(" x\"") && !body.contains("big"));
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
             <div>oto <b>lista rzeczy</b>:</div><ul><li>pierwsza</li><li>druga<ul><li>pod</li></ul></li></ul>\
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
             oto lista rzeczy:\n\
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

    const PNG: &str = "data:image/png;base64,iVBORw0KGgo=";

    #[test]
    fn colors_and_sizes_keep_only_the_palette() {
        let clean = compose_clean(concat!(
            "<span style=\"color: rgb(192, 0, 0); background-color: #FFF59D; font-size: 18px\">a</span>",
            "<span style=\"color:#123456;font-family:Comic Sans;position:fixed\">b</span>",
            "<span style=\"font-size:72px;background:url(https://t.pl/x.gif) #c8f7c5\">c</span>",
            "<p style=\"color:#1d4ed8 !important;width:expression(alert(1))\">d</p>",
            "<div style=\"behavior:url(x.htc);color:red\">e</div>",
            "<span style=\"font-size: x-large;\">f</span><span style=\"font-size: small\">g</span>",
            "<span style=\"font-size: medium;\">h</span>",
        ));
        assert_eq!(
            clean,
            concat!(
                "<span style=\"color:#c00000;background-color:#fff59d;font-size:18px\">a</span>",
                "<span>b</span><span>c</span><p style=\"color:#1d4ed8\">d</p><div>e</div>",
                "<span style=\"font-size:18px\">f</span><span style=\"font-size:12px\">g</span><span>h</span>",
            )
        );
    }

    #[test]
    fn pictures_go_in_only_as_data_or_cid_and_within_limits() {
        let clean = compose_clean(&format!(
            "<img src=\"{PNG}\" alt=\"logo\" width=\"120\" height=\"40px\" onerror=\"x()\">\
             <img src=\"https://t.pl/open.gif\"><img src=\"data:image/svg+xml;base64,PHN2Zz4=\">\
             <img src=\"data:text/html;base64,PGI+\"><img src=\"cid:1.abc@twojastudio.pl\">\
             <img src=\"cid:a&quot;b\"><img src=\"file:///C:/x.png\"><img>\
             <a href=\"data:text/html;base64,PGI+\">x</a>"
        ));
        assert_eq!(
            clean,
            format!(
                "<img src=\"{PNG}\" alt=\"logo\" width=\"120\"><img src=\"cid:1.abc@twojastudio.pl\"><a>x</a>"
            )
        );
        // Too many, or too large together: the ones past the limit stay out.
        let limits = ImageLimits {
            each: 100,
            total: 15,
            count: 40,
        };
        let two = format!("<img src=\"{PNG}\"><img src=\"{PNG}\">");
        assert_eq!(clean_with(&two, limits).matches("<img").count(), 1);
        let big = format!("<img src=\"data:image/png;base64,{}\">", "A".repeat(400));
        assert_eq!(clean_with(&big, limits), "");
    }

    #[test]
    fn a_pictures_size_goes_out_as_its_width_and_one_style() {
        let img = |attrs: &str| compose_clean(&format!("<img src=\"{PNG}\" {attrs}>"));
        let sized =
            format!("<img src=\"{PNG}\" width=\"480\" style=\"max-width:100%;height:auto\">");
        assert_eq!(
            img("width=\"480\" style=\"max-width: 100%; height: auto;\""),
            sized
        );
        assert_eq!(
            img("width=\"480\" style=\"height:AUTO\""),
            sized,
            "written one way"
        );
        assert_eq!(
            img("width=\" 480 \" height=\"270\" style=\"max-width:100%;height:auto\""),
            sized,
            "no height"
        );
        // Nothing else on a picture: no other style, no width out of 1–2000.
        let bare = format!("<img src=\"{PNG}\">");
        for attrs in [
            "style=\"max-width:100%;height:auto;position:fixed\"",
            "style=\"width:9999px\"",
            "style=\"max-width:100%;background:url(https://t.pl/x.gif)\"",
            "width=\"0\"",
            "width=\"2001\"",
            "width=\"50%\"",
            "width=\"-5\"",
        ] {
            assert_eq!(img(attrs), bare, "{attrs}");
        }
        assert_eq!(
            img("width=\"2000\""),
            format!("<img src=\"{PNG}\" width=\"2000\">")
        );
        // ammonia's own attributes don't come along either.
        assert_eq!(
            compose_clean("<p title=\"x\" lang=\"pl\" align=\"center\"><img src=\"cid:a@b\" align=\"left\" hspace=\"9\">a</p>"),
            "<p><img src=\"cid:a@b\">a</p>"
        );
        // Other elements don't take the picture's style.
        assert_eq!(
            compose_clean("<div style=\"max-width:100%;height:auto\">x</div>"),
            "<div>x</div>"
        );
        // Sent, the size stays on the cid: picture; a draft opened again has it back.
        let (out, _) = inline_images(&sized, "m@x.pl");
        assert_eq!(
            out,
            "<img src=\"cid:1.m@x.pl\" width=\"480\" style=\"max-width:100%;height:auto\">"
        );
    }

    #[test]
    fn the_signature_keeps_its_block_and_nothing_else_keeps_a_class() {
        let clean = compose_clean(
            "<div class=\"telex-signature evil\"><b>Anna</b></div><div class=\"x\" id=\"y\">z</div>",
        );
        assert_eq!(
            clean,
            "<div class=\"telex-signature\"><b>Anna</b></div><div>z</div>"
        );
        assert!(is_plain(
            "<div>Pozdrawiam</div><div><br></div><div>Anna</div>"
        ));
        assert!(!is_plain("<div><b>Anna</b></div>"));
        assert!(!is_plain(&clean));
    }

    #[test]
    fn data_pictures_become_parts_of_their_own() {
        let html = format!(
            "<div>Logo:</div><img src=\"{PNG}\" alt=\"a\"><img src=\"{PNG}\">\
             <img src=\"data:image/jpeg;base64,/9j/\"><img src=\"cid:someone-else@x.pl\">"
        );
        let (out, parts) = inline_images(&compose_clean(&html), "abc@twojastudio.pl");
        assert_eq!(
            out,
            "<div>Logo:</div><img src=\"cid:1.abc@twojastudio.pl\" alt=\"a\" style=\"max-width:100%;height:auto\">\
             <img src=\"cid:1.abc@twojastudio.pl\" style=\"max-width:100%;height:auto\">\
             <img src=\"cid:2.abc@twojastudio.pl\" style=\"max-width:100%;height:auto\">"
        );
        // The editor's picture has no style of its own (its stylesheet says it): sent, it has it.
        let (out, _) = inline_images(&format!("<img src=\"{PNG}\" width=\"300\">"), "m@x.pl");
        assert_eq!(
            out,
            "<img src=\"cid:1.m@x.pl\" width=\"300\" style=\"max-width:100%;height:auto\">"
        );
        assert_eq!(parts.len(), 2, "the same picture twice is one part");
        assert_eq!(parts[0].mime, "image/png");
        assert_eq!(parts[0].data, b"\x89PNG\r\n\x1a\n");
        assert_eq!(parts[1].mime, "image/jpeg");
    }

    #[test]
    fn the_originals_html_is_quoted_with_its_own_pictures_only() {
        let png = "--r\r\n\
Content-Type: image/png\r\n\
Content-ID: <logo@sklep.pl>\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
iVBORw0KGgo=\r\n";
        let raw = html_mail(
            "<style>p{color:red}</style><table bgcolor=\"#000\" width=\"600\"><tr>\
             <td colspan=\"2\" style=\"color:#ffffff;background:#000;font-family:Arial\">\
             <b>Oferta</b> <font color=\"red\">dziś</font></td></tr></table>\
             <img src=\"cid:logo@sklep.pl\"><img src=\"https://track.sklep.pl/o.gif\">\
             <script>alert(1)</script><a href=\"https://sklep.pl\" onclick=\"x\">sklep</a>",
            png,
        );
        let quote = quotable(&raw).unwrap();
        assert_eq!(
            quote,
            "<table><tbody><tr><td colspan=\"2\"><b>Oferta</b> dziś</td></tr></tbody></table>\
             <img src=\"data:image/png;base64,iVBORw0KGgo=\"><a href=\"https://sklep.pl\">sklep</a>"
        );
        assert_eq!(editable(&raw).as_deref(), Some(quote.as_str()));
        assert!(quotable(b"Subject: x\r\n\r\nplain\r\n").is_none());
        // Sent again, the picture goes as a part of the reply.
        let (_, parts) = inline_images(&compose_clean(&quote), "r@x.pl");
        assert_eq!(parts.len(), 1);
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
