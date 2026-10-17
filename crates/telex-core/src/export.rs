//! A whole message out of Telex: saved as an .eml file (the stored message byte for byte, one
//! or several into a folder, under names made from their subjects), its source and all its
//! headers to read, and a document of it to print or save as PDF.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use mail_parser::decoders::charsets::map::charset_decoder;
use mail_parser::parsers::MessageStream;
use mail_parser::HeaderValue;
use serde::Serialize;

use crate::attachments;
use crate::html;
use crate::markdown;
use crate::model::{Message, MessageId};
use crate::store::{MailStore, StoreError};
use crate::tr;

/// How much of a message View source shows: the start of a message with large attachments is
/// what anyone reads, and megabytes of base64 would only slow the window down.
pub const SOURCE_LIMIT: usize = 1024 * 1024;

/// A file name for a message: its subject made safe for any folder, at most 120 characters of
/// it, with `ext` ("eml", "pdf"). A message without a subject is "message.eml".
pub fn file_name(subject: &str, ext: &str) -> String {
    let words = subject.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut base: String = words.chars().take(120).collect();
    // Names Windows can't take (CON) and characters it refuses are dealt with below; an
    // empty subject, or one only of such characters, gets a word instead.
    let usable = attachments::safe_file_name(&base);
    if usable == "attachment" && !base.eq_ignore_ascii_case("attachment") {
        base = tr!("message", "wiadomość");
    }
    let base = base.trim_end_matches(['.', ' ']);
    attachments::safe_file_name(&format!("{base}.{ext}"))
}

/// Write messages into `dir`, one .eml file each, never over a file that is there: two
/// messages called "Faktura" become "Faktura.eml" and "Faktura (2).eml". Returns the paths
/// in the order given.
pub fn write_emls(dir: &Path, messages: &[Eml]) -> io::Result<Vec<PathBuf>> {
    messages
        .iter()
        .map(|m| attachments::write_named(dir, &m.name, &m.raw, false))
        .collect()
}

/// A stored message ready to be a file: its name and its bytes as they came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eml {
    pub name: String,
    pub raw: Vec<u8>,
}

/// What View source shows: the message as text and its header fields decoded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Source {
    /// The raw message as text, lines ending in "\n"; 8-bit text in the charset its part names.
    pub text: String,
    /// Only the first [`SOURCE_LIMIT`] bytes are in `text`.
    pub cut: bool,
    /// The whole message's size in bytes.
    pub bytes: usize,
    /// Every header field of the message, in order, its value unfolded and decoded.
    pub headers: Vec<HeaderLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeaderLine {
    pub name: String,
    pub value: String,
}

/// The raw message as text for reading, never for sending again: lines that are UTF-8 stay as
/// they are, other lines are read in the charset the part they are in names (the last
/// `charset=` above them), else as UTF-8 with replacement marks. At most `limit` bytes, cut
/// after a whole line; the flag says whether it was cut.
pub fn source_text(raw: &[u8], limit: usize) -> (String, bool) {
    let cut = raw.len() > limit;
    let shown = if cut {
        let end = raw[..limit]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(limit, |i| i + 1);
        &raw[..end]
    } else {
        raw
    };
    let mut out = String::with_capacity(shown.len());
    let mut decode = None;
    for line in shown.split_inclusive(|&b| b == b'\n') {
        if let Some(found) = charset_in(line).and_then(charset_decoder) {
            decode = Some(found);
        }
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        match (std::str::from_utf8(line), decode) {
            (Ok(text), _) => out.push_str(text),
            (Err(_), Some(decode)) => out.push_str(&decode(line)),
            (Err(_), None) => out.push_str(&String::from_utf8_lossy(line)),
        }
        out.push('\n');
    }
    (out, cut)
}

/// The value of a `charset=` parameter on this line, if there is one.
fn charset_in(line: &[u8]) -> Option<&[u8]> {
    let at = line
        .windows(8)
        .position(|w| w.eq_ignore_ascii_case(b"charset="))?;
    let rest = &line[at + 8..];
    let rest = rest.strip_prefix(b"\"").unwrap_or(rest);
    let end = rest
        .iter()
        .position(|&b| matches!(b, b'"' | b';' | b' ' | b'\t' | b'\r' | b'\n'))
        .unwrap_or(rest.len());
    Some(&rest[..end]).filter(|c| !c.is_empty())
}

/// The message's header fields in order: folded lines joined, encoded words (=?UTF-8?B?…?=)
/// decoded. Fields of the parts inside are left out.
pub fn headers(raw: &[u8]) -> Vec<HeaderLine> {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 2)
        .into_iter()
        .chain(raw.windows(2).position(|w| w == b"\n\n").map(|i| i + 1))
        .min()
        .unwrap_or(raw.len());
    let mut fields: Vec<(String, Vec<u8>)> = Vec::new();
    for line in raw[..end].split_inclusive(|&b| b == b'\n') {
        if matches!(line.first(), Some(b' ' | b'\t')) {
            if let Some((_, value)) = fields.last_mut() {
                value.extend_from_slice(line);
            }
            continue;
        }
        let Some(colon) = line.iter().position(|&b| b == b':') else {
            continue;
        };
        let name = String::from_utf8_lossy(&line[..colon]).trim().to_owned();
        if !name.is_empty() && !name.contains(' ') {
            fields.push((name, line[colon + 1..].to_vec()));
        }
    }
    fields
        .into_iter()
        .map(|(name, mut value)| {
            if !value.ends_with(b"\n") {
                value.push(b'\n');
            }
            let value = match MessageStream::new(&value).parse_unstructured() {
                HeaderValue::Text(text) => text.trim().to_owned(),
                _ => String::new(),
            };
            HeaderLine { name, value }
        })
        .collect()
}

/// A document of a message to print or save as PDF, and the remote images its HTML refers
/// to (left out unless `images` has them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printable {
    pub document: String,
    pub remote: Vec<String>,
}

/// The message on paper: the subject as a heading, From, To, Cc, Bcc, Date and the
/// attachments, then its text, or its HTML (`html`, when it has some) cleaned as for reading,
/// on white. The styles are scoped to the `.tp` element, so the window can put the body of
/// this document into a shadow root of its own page and print that; the document carries the
/// same policy as the mail frame (nothing from the network).
pub fn print_document(
    m: &Message,
    raw: Option<&[u8]>,
    html: bool,
    images: &HashMap<String, String>,
) -> Printable {
    let esc = html::escape;
    let from = match (m.from_name.trim(), m.from_email.trim()) {
        ("", email) => email.to_owned(),
        (name, "") => name.to_owned(),
        (name, email) if name.eq_ignore_ascii_case(email) => email.to_owned(),
        (name, email) => format!("{name} <{email}>"),
    };
    let date = match crate::i18n::current() {
        crate::i18n::Lang::En => m.date.format("%Y-%m-%d %H:%M"),
        crate::i18n::Lang::Es | crate::i18n::Lang::Fr | crate::i18n::Lang::It => {
            m.date.format("%d/%m/%Y, %H:%M")
        }
        _ => m.date.format("%d.%m.%Y, %H:%M"),
    }
    .to_string();
    let files = m
        .attachments
        .iter()
        .map(|a| format!("{} ({})", a.name, markdown::size(a.size_kb)))
        .collect::<Vec<_>>()
        .join(", ");
    let fields = [
        (tr!("From", "Od"), from),
        (tr!("To", "Do"), m.to.clone()),
        (tr!("Cc", "DW"), m.cc.clone()),
        (tr!("Bcc", "UDW"), m.bcc.clone()),
        (tr!("Date", "Data"), date),
        (tr!("Attachments", "Załączniki"), files),
    ];
    let rows: String = fields
        .iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(label, value)| {
            let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
            format!("<tr><th>{}</th><td>{}</td></tr>", esc(label), esc(&value))
        })
        .collect();
    let cleaned = if html {
        raw.and_then(|raw| html::printable(raw, images))
    } else {
        None
    };
    let (body, remote) = match cleaned {
        Some((html, remote)) => (format!("<div class=\"mail\">{html}</div>"), remote),
        None => (
            format!(
                "<div class=\"text\">{}</div>",
                esc(m.body.replace("\r\n", "\n").trim_end())
            ),
            Vec::new(),
        ),
    };
    let subject = m.subject.split_whitespace().collect::<Vec<_>>().join(" ");
    let document = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\">\
         <title>{title}</title><style>{PRINT_STYLE}</style></head><body><div class=\"tp\">\
         <h1>{title}</h1><table class=\"hd\">{rows}</table>{body}</div></body></html>",
        title = esc(&subject),
    );
    Printable { document, remote }
}

/// Black on white whatever the theme, in points for paper; the mail's own layout kept within
/// the page as in the frame.
const PRINT_STYLE: &str = ".tp{color:#000;background:#fff;font:10.5pt/1.45 'Segoe UI',system-ui,sans-serif;overflow-wrap:anywhere}\
.tp h1{font-size:15pt;line-height:1.25;margin:0 0 8pt}\
.tp .hd{border-collapse:collapse;margin:0 0 12pt;width:100%;border-bottom:.75pt solid #777}\
.tp .hd th{text-align:left;font-weight:600;padding:1pt 12pt 1pt 0;vertical-align:top;white-space:nowrap;color:#333}\
.tp .hd td{padding:1pt 0;width:100%}\
.tp .hd tr:last-child th,.tp .hd tr:last-child td{padding-bottom:8pt}\
.tp .text{white-space:pre-wrap}\
.tp img{max-width:100%;height:auto}.tp table{max-width:100%}.tp a{color:#1d4690}.tp pre{white-space:pre-wrap}\
.tp blockquote{margin:6pt 0 6pt 2pt;padding-left:8pt;border-left:2pt solid #8a9bb8;color:inherit}";

impl MailStore {
    /// A stored message as an .eml file: a name from its subject and the bytes as they came.
    pub fn eml(&self, id: MessageId) -> Result<Eml, StoreError> {
        let m = self.get(id)?;
        let raw = self.raw_of(id)?.ok_or_else(not_stored)?;
        Ok(Eml {
            name: file_name(&m.subject, "eml"),
            raw,
        })
    }

    /// The source and header fields of a stored message, for View source and All headers.
    pub fn source(&self, id: MessageId) -> Result<Source, StoreError> {
        let raw = self.raw_of(id)?.ok_or_else(not_stored)?;
        let (text, cut) = source_text(&raw, SOURCE_LIMIT);
        Ok(Source {
            text,
            cut,
            bytes: raw.len(),
            headers: headers(&raw),
        })
    }
}

fn not_stored() -> StoreError {
    StoreError::Invalid(tr!(
        "This message isn't stored in full on this PC.",
        "Ta wiadomość nie jest zapisana w całości na tym komputerze."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Lang};
    use crate::model::Attachment;
    use chrono::NaiveDate;
    use std::fs;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "telex-export-{tag}-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ))
    }

    #[test]
    fn names_come_from_the_subject_and_are_safe() {
        assert_eq!(file_name("Faktura 10/2026", "eml"), "Faktura 10 2026.eml");
        assert_eq!(
            file_name("Re: Spotkanie?  \r\n jutro", "eml"),
            "Re Spotkanie jutro.eml"
        );
        assert_eq!(
            file_name("Zażółć gęślą jaźń", "pdf"),
            "Zażółć gęślą jaźń.pdf"
        );
        assert_eq!(file_name("CON", "eml"), "_CON.eml");
        assert_eq!(file_name("..\\..\\Startup\\x", "eml"), "Startup x.eml");
        assert_eq!(file_name("Dzięki...", "eml"), "Dzięki.eml");
        assert_eq!(file_name("v1.2", "eml"), "v1.2.eml");
        assert_eq!(file_name("attachment", "eml"), "attachment.eml");
        i18n::with(Lang::En, || {
            assert_eq!(file_name("", "eml"), "message.eml");
            assert_eq!(file_name(" ??? ", "eml"), "message.eml");
        });
        assert_eq!(
            i18n::with(Lang::Pl, || file_name("", "pdf")),
            "wiadomość.pdf"
        );
        let long = file_name(&"ż".repeat(400), "eml");
        assert_eq!(long.chars().count(), 124);
        assert!(long.ends_with(".eml"));
    }

    #[test]
    fn several_messages_get_names_of_their_own_and_nothing_is_overwritten() {
        let dir = temp_dir("many");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Oferta.eml"), b"kept").unwrap();
        let eml = |name: &str, raw: &[u8]| Eml {
            name: name.into(),
            raw: raw.to_vec(),
        };
        let paths = write_emls(
            &dir,
            &[
                eml("Faktura.eml", b"one"),
                eml("Faktura.eml", b"two"),
                eml("faktura.eml", b"three"),
                eml("Oferta.eml", b"four"),
            ],
        )
        .unwrap();
        let names: Vec<String> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names[0], "Faktura.eml");
        assert_eq!(names[1], "Faktura (2).eml");
        // A name that differs only in case is the same file on Windows: never written over.
        assert!(names[2] == "faktura.eml" || names[2] == "faktura (3).eml");
        assert_eq!(names[3], "Oferta (2).eml");
        assert_eq!(fs::read(dir.join("Oferta.eml")).unwrap(), b"kept");
        assert_eq!(fs::read(&paths[1]).unwrap(), b"two");
        assert_eq!(fs::read(&paths[2]).unwrap(), b"three");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stored_message_saves_byte_for_byte() {
        let mut store = crate::sample::sample_store();
        let raw = b"From: Anna <anna@kowalska.pl>\r\nSubject: =?UTF-8?Q?Pozdrowienia_z_=C5=81odzi?=\r\nContent-Type: text/plain; charset=iso-8859-2\r\n\r\nZa\xbf\xf3\xb3\xe6\r\n".to_vec();
        let message = crate::mime::parse_message(&raw, "", None);
        let id = store
            .add_synced(
                "firma/inbox",
                &[crate::store::SyncedMessage {
                    uid: 9876,
                    message,
                    raw: raw.clone(),
                }],
            )
            .unwrap()[0];
        let eml = store.eml(id).unwrap();
        assert_eq!(eml.name, "Pozdrowienia z Łodzi.eml");
        assert_eq!(eml.raw, raw);
        let dir = temp_dir("one");
        let paths = write_emls(&dir, &[eml]).unwrap();
        assert_eq!(fs::read(&paths[0]).unwrap(), raw);
        let _ = fs::remove_dir_all(&dir);
        let source = store.source(id).unwrap();
        assert_eq!(source.bytes, raw.len());
        assert!(source.text.ends_with("\n\nZażółć\n"), "{}", source.text);
        assert_eq!(source.headers[1].value, "Pozdrowienia z Łodzi");
        // The sample mailbox keeps no raw messages: saving one says why.
        let sample = crate::sample::sample_store();
        let other = sample.list("firma/inbox").unwrap()[0].id;
        assert!(sample.eml(other).is_err());
        assert!(sample.source(other).is_err());
    }

    #[test]
    fn the_source_reads_each_part_in_its_charset() {
        let raw = b"Subject: =?ISO-8859-2?Q?Za=BF=F3=B3=E6?=\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n\
            --b\r\nContent-Type: text/plain; charset=\"iso-8859-2\"\r\nContent-Transfer-Encoding: 8bit\r\n\r\nG\xea\xb6l\xb1 ja\xbc\xf1\r\n\
            --b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nZa\xc5\xbc\xc3\xb3\xc5\x82\xc4\x87\r\n--b--\r\n";
        let (text, cut) = source_text(raw, SOURCE_LIMIT);
        assert!(!cut);
        assert!(text.contains("\nGęślą jaźń\n"), "{text}");
        assert!(text.contains("\nZażółć\n"), "{text}");
        assert!(
            text.starts_with("Subject: =?ISO-8859-2?Q?Za=BF=F3=B3=E6?=\n"),
            "headers stay as sent"
        );
        assert!(!text.contains('\r'));
        // Without a charset, bytes that aren't UTF-8 are marked, not guessed.
        let (odd, _) = source_text(b"Subject: x\r\n\r\nb\xb1d\r\n", SOURCE_LIMIT);
        assert_eq!(odd, "Subject: x\n\nb\u{fffd}d\n");
    }

    #[test]
    fn a_long_source_is_cut_after_a_whole_line() {
        let raw = b"Subject: a\r\n\r\nline one\r\nline two\r\nline three\r\n";
        let (text, cut) = source_text(raw, 30);
        assert!(cut);
        assert_eq!(text, "Subject: a\n\nline one\n");
    }

    #[test]
    fn headers_are_unfolded_and_decoded_in_order() {
        let raw = b"Received: from mx.example.pl\r\n\tby mail.firma.pl; Sat, 10 Oct 2026 06:12:00 +0200\r\n\
            From: =?UTF-8?B?QW5uYSBLb3dhbHNrYQ==?= <anna@kowalska.pl>\r\n\
            Subject: =?ISO-8859-2?Q?Za=BF=F3=B3=E6?= i reszta\r\n\
            X-Empty:\r\n\
            Content-Type: multipart/mixed; boundary=b\r\n\r\n\
            --b\r\nContent-Type: text/plain\r\n\r\nNot: a header\r\n--b--\r\n";
        let found: Vec<(String, String)> = headers(raw)
            .into_iter()
            .map(|h| (h.name, h.value))
            .collect();
        assert_eq!(
            found,
            vec![
                (
                    "Received".into(),
                    "from mx.example.pl by mail.firma.pl; Sat, 10 Oct 2026 06:12:00 +0200".into()
                ),
                ("From".into(), "Anna Kowalska <anna@kowalska.pl>".into()),
                ("Subject".into(), "Zażółć i reszta".into()),
                ("X-Empty".into(), String::new()),
                ("Content-Type".into(), "multipart/mixed; boundary=b".into()),
            ]
        );
        // A message of headers only, with bare line ends.
        let bare = headers(b"Subject: tylko\nTo: a@b.pl");
        assert_eq!(bare.len(), 2);
        assert_eq!(bare[1].value, "a@b.pl");
    }

    fn sample() -> Message {
        Message {
            id: 1,
            folder: "firma/inbox".into(),
            from_name: "Anna Kowalska".into(),
            from_email: "anna@kowalska-architekci.pl".into(),
            to: "biuro@twojastudio.pl".into(),
            cc: String::new(),
            bcc: String::new(),
            subject: "Poprawki <na> stronie".into(),
            date: NaiveDate::from_ymd_opt(2026, 10, 10)
                .unwrap()
                .and_hms_opt(6, 12, 0)
                .unwrap(),
            size_kb: 40,
            unread: false,
            flagged: false,
            remote_images: false,
            note: None,
            attachments: vec![Attachment {
                name: "projekt.pdf".into(),
                size_kb: 1229,
                path: None,
            }],
            body: "Dzień dobry,\r\n\r\n<b>nie pogrubione</b>\r\n".into(),
            message_id: None,
            refs: Vec::new(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn plain_text_prints_escaped_under_its_header() {
        let p = i18n::with(Lang::En, || {
            print_document(&sample(), None, true, &HashMap::new())
        });
        let d = &p.document;
        assert!(d.contains("<title>Poprawki &lt;na&gt; stronie</title>"));
        assert!(d.contains("<h1>Poprawki &lt;na&gt; stronie</h1>"));
        assert!(d.contains(
            "<tr><th>From</th><td>Anna Kowalska &lt;anna@kowalska-architekci.pl&gt;</td></tr>"
        ));
        assert!(d.contains("<tr><th>Date</th><td>2026-10-10 06:12</td></tr>"));
        assert!(d.contains("<td>projekt.pdf (1.2 MB)</td>"));
        assert!(!d.contains("<th>Cc</th>"), "empty fields are left out");
        assert!(d.contains(
            "<div class=\"text\">Dzień dobry,\n\n&lt;b&gt;nie pogrubione&lt;/b&gt;</div>"
        ));
        assert!(d.contains("default-src 'none'"));
        assert!(p.remote.is_empty());
        let pl = i18n::with(Lang::Pl, || {
            print_document(&sample(), None, false, &HashMap::new())
        });
        assert!(pl.document.contains("<th>Od</th>"));
        assert!(pl.document.contains("<td>10.10.2026, 06:12</td>"));
        assert!(pl.document.contains("(1,2 MB)"));
    }

    #[test]
    fn html_prints_cleaned_and_remote_images_stay_out_unless_loaded() {
        let raw = b"Subject: Oferta\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
            <p onclick=\"x()\">Zobacz <img src=\"https://sklep.pl/a.png\"></p><script>alert(1)</script>";
        let m = sample();
        let p = print_document(&m, Some(raw), true, &HashMap::new());
        assert!(p.document.contains("<div class=\"mail\"><p>Zobacz"));
        assert!(!p.document.contains("script"));
        assert!(!p.document.contains("onclick"));
        assert!(!p.document.contains("https://sklep.pl/a.png\""));
        assert_eq!(p.remote, vec!["https://sklep.pl/a.png".to_owned()]);
        let loaded = HashMap::from([(
            "https://sklep.pl/a.png".to_owned(),
            "data:image/png;base64,AAAA".to_owned(),
        )]);
        let p = print_document(&m, Some(raw), true, &loaded);
        assert!(p.document.contains("data:image/png;base64,AAAA"));
        // Shown as text, it prints the text.
        let text = print_document(&m, Some(raw), false, &HashMap::new());
        assert!(text.document.contains("<div class=\"text\">"));
    }
}
