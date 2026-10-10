//! Showing HTML mail safely.
//!
//! [`render`] turns a message's HTML into a complete, inert document for a sandboxed frame.
//! ammonia removes scripts, forms, event handlers and everything else active. Images the
//! message carries itself (`cid:`) become data: URIs. Remote images and CSS backgrounds are
//! left out, unless their contents are passed in: [`fetch_images`] downloads them when the
//! user allows it, so the window itself never talks to the network. The document also
//! carries its own Content-Security-Policy that forbids any network load.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ammonia::{Builder, UrlRelative};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::mime::html_body;

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
pub fn render(raw: &[u8], images: &HashMap<String, String>) -> Option<Rendered> {
    let body = html_body(raw)?;
    let inline = body
        .inline
        .iter()
        .filter(|(_, (mime, _))| mime.starts_with("image/"))
        .map(|(cid, (mime, data))| (cid.clone(), data_uri(mime, data)))
        .collect();
    let (html, remote) = sanitize(&body.html, inline, images.clone());
    let blocked = remote.iter().filter(|u| !images.contains_key(*u)).count();
    Some(Rendered {
        document: wrap(&html),
        remote,
        blocked,
    })
}

/// Where image references can be resolved: images in the message, and fetched ones.
struct Sources {
    inline: HashMap<String, String>,
    images: HashMap<String, String>,
}

/// Clean an HTML fragment. Returns the safe HTML and the remote image URLs it referenced.
fn sanitize(
    html: &str,
    inline: HashMap<String, String>,
    images: HashMap<String, String>,
) -> (String, Vec<String>) {
    let found = Arc::new(Mutex::new(Vec::new()));
    let sources = Sources { inline, images };
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
        (_, "style") => Some(Cow::Owned(clean_style(sources, found, value))),
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

/// Mail is designed for white paper, so the frame stays light in the dark theme too.
const FRAME_STYLE: &str = "html{background:#fff;color-scheme:light}\
body{margin:0;padding:12px 14px 28px;font:14px/1.5 'Segoe UI',system-ui,sans-serif;color:#1a1e23;overflow-wrap:anywhere}\
img{max-width:100%;height:auto}table{max-width:100%}a{color:#2453a6}pre{white-space:pre-wrap}\
blockquote{margin:0 0 0 2px;padding-left:10px;border-left:2px solid #d2d7de;color:#59636e}";

fn wrap(body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{FRAME_CSP}\">\
         <meta name=\"referrer\" content=\"no-referrer\">\
         <style>{FRAME_STYLE}</style></head><body>{body}</body></html>"
    )
}

fn data_uri(mime: &str, data: &[u8]) -> String {
    let mime: String = mime
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '+' | '-' | '.'))
        .collect();
    format!("data:{mime};base64,{}", STANDARD.encode(data))
}

// ---- Remote images ----

/// Limits for downloading the remote images of one message.
#[derive(Debug, Clone)]
pub struct FetchLimits {
    pub max_images: usize,
    pub max_image_bytes: u64,
    pub max_total_bytes: u64,
    pub timeout: Duration,
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
            allow_private: false,
        }
    }
}

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
    let agent = ureq::Agent::new_with_config(config);
    let mut images = HashMap::new();
    let mut total = 0;
    for url in urls.iter().take(limits.max_images) {
        if images.contains_key(url) {
            continue;
        }
        if let Some((mime, data)) = fetch_one(&agent, url, limits) {
            total += data.len() as u64;
            if total > limits.max_total_bytes {
                break;
            }
            images.insert(url.clone(), data_uri(&mime, &data));
        }
    }
    images
}

/// One image, following up to three redirects (each checked like the first URL).
fn fetch_one(agent: &ureq::Agent, url: &str, limits: &FetchLimits) -> Option<(String, Vec<u8>)> {
    let mut url = url.to_owned();
    for _ in 0..4 {
        if !allowed_url(&url, limits.allow_private) {
            return None;
        }
        let mut response = agent
            .get(url.as_str())
            .header("Accept", "image/*")
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
            .limit(limits.max_image_bytes)
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

/// http(s) only, and every address the host name resolves to must be public.
fn allowed_url(url: &str, allow_private: bool) -> bool {
    let lower = url.to_ascii_lowercase();
    let (rest, default_port) = if let Some(r) = lower.strip_prefix("https://") {
        (r, 443)
    } else if let Some(r) = lower.strip_prefix("http://") {
        (r, 80)
    } else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let (host, port) = match host_port.strip_prefix('[') {
        Some(v6) => match v6.split_once(']') {
            Some((h, p)) => (h, p.strip_prefix(':').and_then(|p| p.parse().ok())),
            None => return false,
        },
        None => match host_port.rsplit_once(':') {
            Some((h, p)) => (h, p.parse().ok()),
            None => (host_port, None),
        },
    };
    if host.is_empty() {
        return false;
    }
    if allow_private {
        return true;
    }
    match (host, port.unwrap_or(default_port)).to_socket_addrs() {
        Ok(addrs) => {
            let addrs: Vec<_> = addrs.collect();
            !addrs.is_empty() && addrs.iter().all(|a| is_public(a.ip()))
        }
        Err(_) => false,
    }
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
            let first = v6.segments()[0];
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
        render(&html_mail(html, ""), &HashMap::new()).unwrap()
    }

    fn body_of(r: &Rendered) -> &str {
        let start = r.document.find("<body>").unwrap() + 6;
        let end = r.document.rfind("</body>").unwrap();
        &r.document[start..end]
    }

    #[test]
    fn plain_text_mail_has_no_html_view() {
        assert!(render(b"Subject: x\r\n\r\nJust text\r\n", &HashMap::new()).is_none());
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
        let r = render(&html_mail(html, ""), &images).unwrap();
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
        for ip in ["::1", "fe80::1", "fd00::1", "::ffff:192.168.1.1"] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        assert!(is_public("93.184.216.34".parse().unwrap()));
        assert!(is_public("2606:2800:220:1::".parse().unwrap()));
        assert!(!allowed_url("http://127.0.0.1:8080/x.png", false));
        assert!(!allowed_url("http://localhost/x.png", false));
        assert!(!allowed_url("ftp://example.com/x.png", false));
        assert!(allowed_url("http://127.0.0.1:8080/x.png", true));
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

        // The same server is refused when private addresses aren't allowed.
        assert!(fetch_images(&urls, &FetchLimits::default()).is_empty());
    }
}
